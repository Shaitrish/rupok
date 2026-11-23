// Live agent bridge, board side: connect this board to the teacher's own AI
// agent (`rupok mcp --live`, started by Claude Code, Claude Desktop, Codex,
// OpenCode…) over a loopback WebSocket, so the agent can see and edit the
// open lesson during class.
//
// Protocol v1 (docs/ai-agents.md § "Live protocol"): JSON frames tagged with
// a channel `ch` (session | doc | tools; chat is reserved) and a type `t`.
// - session: hello → welcome {code} (pairing) or paired {token}; state.
// - doc: Loro sync. We stream every change to the agent's mirror; after a
//   reconnect the mirror sends back what we lack, and the CRDT merges.
// - tools: the agent's ops. Document ops run in wasm (`Board.live_apply`) as
//   local changes tagged with the agent's name: one undo step each.
//   `snapshot`, `point_at` and the UI part of `set_board_theme` run here.
//
// Rupok never sees the teacher's AI account: the agent runs on this computer
// and only this board's lesson crosses the socket.

import { createSignal } from "solid-js";
import type { Controller } from "./controller";
import { color, isView, setColor, setPrefs } from "./state";
import { renderSnapshot } from "./snapshot";

export const LIVE_PORTS = [47373, 47374, 47375, 47376];
const PROTOCOL = 1;
const CHANNELS = ["session", "doc", "tools"];
const CLIENT = `rupok-board/0.1 (${(window as any).__TAURI_INTERNALS__ ? "desktop" : "web"})`;
const TOKEN_KEY = "rupok.live.token";
const ALLOW_KEY = "rupok.live.allowEdit";
const SYNC_MS = 80;

export type LivePhase = "idle" | "searching" | "pairing" | "connected" | "reconnecting" | "unreachable";
/** A server waiting to pair. `pair: "agent"`: show `code`, the teacher gives
 * it to the agent. `pair: "board"` (`rupok agent`, the desktop app): the
 * teacher types the code that server shows. `chat`: it relays the in-app chat. */
export interface Waiting { port: number; agent: string; code: string; expiresAt: number; pair: "agent" | "board"; chat: boolean }
/** An agent op the board ran successfully (for the chat's tool cards). */
export interface OpDone { agent: string; op: any; result: any; page: number }
export interface Activity { id: number; agent: string; label: string; at: number; ok: boolean; error?: string }
export interface LivePointer { x: number; y: number; w?: number; h?: number; label?: string; until: number; page: number }
/** Chrome's Local Network Access permission for this site, if the browser has one. */
export type LnaState = "granted" | "prompt" | "denied" | "unsupported";

export const [livePhase, setLivePhase] = createSignal<LivePhase>("idle");
export const [liveWaiting, setLiveWaiting] = createSignal<Waiting[]>([]);
export const [liveAgent, setLiveAgent] = createSignal<string | null>(null);
/** Set while agent ops arrive ("Claude is editing…"). */
export const [liveEditing, setLiveEditing] = createSignal<{ agent: string; label: string } | null>(null);
export const [liveActivity, setLiveActivity] = createSignal<Activity[]>([]);
export const [livePointer, setLivePointer] = createSignal<LivePointer | null>(null);
/** The last agent edit, while it is still the newest change (offers Undo). */
export const [liveLastEdit, setLiveLastEdit] = createSignal<{ agent: string; label: string; at: number } | null>(null);
export const [liveLna, setLiveLna] = createSignal<LnaState>("unsupported");
/** Channels the paired server speaks; "chat" means the in-app chat is available. */
export const [liveChannels, setLiveChannels] = createSignal<string[]>([]);
export const liveHasChat = () => livePhase() === "connected" && liveChannels().includes("chat");
/** The last pairing problem to show (a wrong code…), cleared on success. */
export const [livePairError, setLivePairError] = createSignal<string | null>(null);
export const [allowEdit, setAllowEditSignal] = createSignal(load(ALLOW_KEY, "1") !== "0");

function load(key: string, fallback: string): string {
  try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; }
}

// ------------------------------------------------------------- encoding

export function b64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export function unb64(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function same(a: Uint8Array | null, b: Uint8Array): boolean {
  if (!a || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

/** How an op touches the board (mirrors `rupok_live::op::Access`). */
function access(op: any): "read" | "control" | "write" {
  switch (op?.op) {
    case "get_page": case "get_outline": case "get_selection": case "snapshot": return "read";
    case "go_to_page": case "point_at": return "control";
    case "set_board_theme": return op.page_color || op.pattern ? "write" : "control";
    default: return "write";
  }
}

const LABELS: Record<string, string> = {
  get_page: "is reading the board", get_outline: "is reading the lesson", get_selection: "is reading your selection",
  snapshot: "is looking at the board", go_to_page: "changed the page", point_at: "pointed at the board",
  set_board_theme: "changed the board theme", add_page: "added a page", add_page_after_current: "added a page",
  add_text: "added text", add_smart_object: "added a Smart Object", update_smart_object: "updated a Smart Object",
  delete_item: "deleted an item", set_page_background: "changed the background",
  replace_items_with_text: "turned ink into text", step_object: "stepped a Smart Object",
  write_handwriting: "is writing on the board", add_media: "added media", add_model: "added a 3D model",
};
export const opLabel = (op: any) => LABELS[op?.op] ?? "is working";

class OpError extends Error {
  constructor(message: string, public data?: any) { super(message); }
}

// --------------------------------------------------------------- client

class LiveClient {
  private c!: Controller;
  private sock: WebSocket | null = null;
  private port = 0;
  private probes = new Set<WebSocket>();
  private sentVV: Uint8Array | null = null;
  private docKey = "";
  private lastRev = -1;
  private lastState = "";
  private timer: number | undefined;
  private editTimer: number | undefined;
  private retry = 0;
  private wanted = false; // the teacher asked to be connected
  private editVV: Uint8Array | null = null;
  private activityId = 0;
  private probeByPort = new Map<number, WebSocket>();
  /** Codes to send as soon as that port's server welcomes us (`#rupok-agent=`, desktop). */
  private autoCode = new Map<number, string>();
  /** Receives `chat` frames from the paired server (chat.ts). */
  chatHandler: ((f: any) => void) | null = null;
  private opListeners = new Set<(e: OpDone) => void>();

  init(c: Controller) {
    this.c = c;
    void this.checkLna();
    // A reload during class reconnects silently with this tab's token.
    const saved = this.savedToken();
    // `rupok agent --open` adds #rupok-agent=PORT:CODE: pair at once.
    const m = /(?:^|[#&])rupok-agent=(\d+):(\d{6})/.exec(location.hash);
    if (m && !isView()) {
      history.replaceState(null, "", location.pathname + location.search);
      this.connectTo(Number(m[1]), m[2]);
    } else if (saved && !isView()) {
      this.wanted = true;
      this.open(saved.port, saved.token, true);
    }
  }

  /** Pair with the server on `port` using the code it shows (the
   * `rupok agent` terminal, or the desktop app's own relay). */
  connectTo(port: number, code: string) {
    if (isView()) return;
    this.wanted = true;
    this.retry = 0;
    this.closeProbes();
    setLiveWaiting([]);
    setLivePairError(null);
    setLivePhase("searching");
    this.autoCode.set(port, code);
    this.open(port, null, false, () => {
      if (livePhase() === "searching") setLivePhase("unreachable");
    });
  }

  /** Send the code the teacher typed to a server that asked for it. */
  pairWith(port: number, code: string) {
    const ws = this.probeByPort.get(port);
    setLivePairError(null);
    if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ ch: "session", t: "pair", code: code.replace(/\D/g, "") }));
  }

  sendChat(f: Record<string, unknown>) {
    this.send({ ch: "chat", ...f });
  }

  onOpDone(fn: (e: OpDone) => void): () => void {
    this.opListeners.add(fn);
    return () => this.opListeners.delete(fn);
  }

  // -------------------------------------------------------- connecting

  /** "Connect AI": find agents on this computer and show their codes. */
  connect() {
    if (isView()) return;
    this.wanted = true;
    this.retry = 0;
    this.closeProbes();
    setLiveWaiting([]);
    setLivePhase("searching");
    const saved = this.savedToken();
    const params = new URLSearchParams(location.search);
    const forced = Number(params.get("live-port"));
    const ports = forced > 0 ? [forced] : LIVE_PORTS;
    // One port after another: servers take the first free port in order, so
    // the first refused port ends the search (and the browser logs one
    // failed connection, not four).
    const probe = (i: number) => {
      if (i >= ports.length || !this.wanted) return;
      const port = ports[i];
      this.open(port, saved?.port === port ? saved.token : null, false, () => {
        if (livePhase() === "searching") {
          setLivePhase("unreachable");
          void this.checkLna();
        }
      }, () => probe(i + 1));
    }
    probe(0);
  }

  /** Forget the agent: it can pair again with a new code. */
  disconnect() {
    this.wanted = false;
    this.send({ ch: "session", t: "bye" });
    this.clearToken();
    this.closeProbes();
    this.drop();
    setLivePhase("idle");
    setLiveAgent(null);
    setLiveWaiting([]);
    setLiveLastEdit(null);
  }

  /** Cancel a search or a pairing in progress. */
  cancel() {
    this.wanted = false;
    this.closeProbes();
    if (!this.sock) setLivePhase("idle");
    setLiveWaiting([]);
  }

  private open(port: number, token: string | null, quiet: boolean, failed?: () => void, found?: () => void) {
    let ws: WebSocket;
    try {
      ws = new WebSocket(`ws://127.0.0.1:${port}`);
    } catch {
      failed?.();
      return;
    }
    this.probes.add(ws);
    this.probeByPort.set(port, ws);
    let opened = false;
    ws.onopen = () => {
      opened = true;
      found?.();
      const doc = this.docInfo();
      ws.send(JSON.stringify({ ch: "session", t: "hello", v: PROTOCOL, client: CLIENT, channels: CHANNELS, token, doc }));
    };
    ws.onmessage = (e) => this.onFrame(ws, port, e.data);
    ws.onerror = () => { /* onclose follows */ };
    ws.onclose = () => {
      this.probes.delete(ws);
      if (this.probeByPort.get(port) === ws) this.probeByPort.delete(port);
      setLiveWaiting((w) => w.filter((x) => x.port !== port));
      if (ws === this.sock) this.lost();
      else if (!opened) {
        if (quiet && livePhase() !== "connected") this.lost();
        failed?.();
      } else if (livePhase() === "pairing" && !liveWaiting().length && !this.sock) {
        setLivePhase(this.wanted ? "unreachable" : "idle");
      }
    };
  }

  private onFrame(ws: WebSocket, port: number, data: unknown) {
    let f: any;
    try { f = JSON.parse(String(data)); } catch { return; }
    if (!f || typeof f.ch !== "string") return;
    if (f.ch === "session") {
      if (f.t === "welcome") {
        const pair = f.pair === "board" ? "board" : "agent";
        const auto = this.autoCode.get(port);
        if (auto && pair === "board") {
          this.autoCode.delete(port);
          ws.send(JSON.stringify({ ch: "session", t: "pair", code: auto }));
          return;
        }
        setLiveWaiting((w) => [...w.filter((x) => x.port !== port), { port, agent: f.agent, code: f.code, expiresAt: Date.now() + f.expires_in * 1000, pair, chat: !!f.chat }]);
        if (livePhase() !== "connected") setLivePhase("pairing");
      } else if (f.t === "paired") {
        setLivePairError(null);
        setLiveChannels(Array.isArray(f.channels) ? f.channels : []);
        this.adopt(ws, port, f.token, f.agent);
      } else if (f.t === "agent") {
        if (ws === this.sock) setLiveAgent(f.agent);
        setLiveWaiting((w) => w.map((x) => (x.port === port ? { ...x, agent: f.agent } : x)));
      } else if (f.t === "error") {
        if (f.code === "replaced" || f.code === "unpaired") this.wanted = false;
        if (f.code === "bad_code") setLivePairError(f.error);
        console.info("rupok live:", f.error);
      }
      return;
    }
    if (ws !== this.sock) return; // only the paired socket syncs and runs ops
    if (f.ch === "doc") this.onDoc(f);
    else if (f.ch === "tools" && f.t === "op") void this.onOp(f.id, f.agent, f.op);
    else if (f.ch === "chat") this.chatHandler?.(f);
    // Other channels (from a newer server) are ignored.
  }

  private adopt(ws: WebSocket, port: number, token: string, agent: string) {
    if (this.sock && this.sock !== ws) this.sock.close();
    this.sock = ws;
    this.port = port;
    this.retry = 0;
    this.probes.delete(ws);
    this.closeProbes();
    this.saveToken(port, token);
    setLiveAgent(agent);
    setLiveWaiting([]);
    setLivePhase("connected");
    this.lastRev = -1;
    this.lastState = "";
    clearInterval(this.timer);
    this.timer = window.setInterval(() => this.tick(), SYNC_MS);
  }

  private drop() {
    clearInterval(this.timer);
    setLiveChannels([]);
    const s = this.sock;
    this.sock = null;
    if (s && s.readyState <= WebSocket.OPEN) s.close();
  }

  /** The paired socket closed: reconnect with the token while wanted. */
  private lost() {
    this.drop();
    setLiveEditing(null);
    if (!this.wanted) {
      setLivePhase("idle");
      setLiveAgent(null);
      return;
    }
    const saved = this.savedToken();
    if (!saved || this.retry >= 6) {
      setLivePhase("unreachable");
      return;
    }
    setLivePhase("reconnecting");
    const delay = Math.min(8000, 500 * 2 ** this.retry++);
    setTimeout(() => {
      if (this.wanted && !this.sock) this.open(saved.port, saved.token, true);
    }, delay);
  }

  private closeProbes() {
    for (const p of this.probes) if (p !== this.sock) p.close();
    this.probes.clear();
  }

  private send(f: unknown) {
    if (this.sock?.readyState === WebSocket.OPEN) this.sock.send(JSON.stringify(f));
  }

  private savedToken(): { port: number; token: string } | null {
    try {
      const v = JSON.parse(sessionStorage.getItem(TOKEN_KEY) || "null");
      return v && typeof v.token === "string" && typeof v.port === "number" ? v : null;
    } catch { return null; }
  }
  private saveToken(port: number, token: string) {
    try { sessionStorage.setItem(TOKEN_KEY, JSON.stringify({ port, token })); } catch { /* storage off */ }
  }
  private clearToken() {
    try { sessionStorage.removeItem(TOKEN_KEY); } catch { /* storage off */ }
  }

  setAllowEdit(on: boolean) {
    setAllowEditSignal(on);
    try { localStorage.setItem(ALLOW_KEY, on ? "1" : "0"); } catch { /* storage off */ }
    this.sendState(true);
  }

  // -------------------------------------------------------------- doc sync

  private docInfo(): { key: string; title: string } {
    try { return JSON.parse(this.c.board.live_doc_json()); } catch { return { key: "", title: "" }; }
  }

  private onDoc(f: any) {
    const b = this.c.board;
    if (f.t === "sync") {
      const doc = this.docInfo();
      this.docKey = doc.key;
      if (f.key && f.vv && f.key === doc.key) {
        // Same lesson: send what the mirror lacks, then ask for what we lack.
        this.send({ ch: "doc", t: "update", data: b64(b.live_export_updates(unb64(f.vv))) });
        this.sentVV = b.live_version();
        this.send({ ch: "doc", t: "sync", key: doc.key, vv: b64(this.sentVV) });
      } else {
        this.sendSnapshot();
      }
      this.sendState(true);
    } else if (f.t === "update" && typeof f.data === "string") {
      try {
        b.live_import_updates(unb64(f.data));
      } catch (e) {
        console.warn("rupok live: could not merge the agent's update", e);
      }
    }
  }

  private sendSnapshot() {
    const doc = this.docInfo();
    this.docKey = doc.key;
    try {
      this.send({ ch: "doc", t: "snapshot", data: b64(this.c.board.save()), doc });
      this.sentVV = this.c.board.live_version();
    } catch (e) {
      console.warn("rupok live: snapshot failed", e);
    }
  }

  /** Stream local changes to the agent's mirror. */
  private pushUpdates() {
    const b = this.c.board;
    const v = b.live_version();
    if (same(this.sentVV, v)) return;
    const from = this.sentVV ?? new Uint8Array();
    this.send({ ch: "doc", t: "update", data: b64(b.live_export_updates(from)) });
    this.sentVV = v;
  }

  private tick() {
    if (!this.sock || this.sock.readyState !== WebSocket.OPEN) return;
    const rev = this.c.board.revision();
    if (rev !== this.lastRev) {
      this.lastRev = rev;
      const key = this.docInfo().key;
      if (key !== this.docKey) this.sendSnapshot(); // another lesson was opened
      else this.pushUpdates();
      const last = liveLastEdit();
      if (last && !same(this.editVV, this.c.board.live_version())) setLiveLastEdit(null);
      this.sendState(false); // page changes bump the revision too
    }
  }

  private sendState(force: boolean) {
    if (!this.c || !this.sock) return;
    const b = this.c.board;
    const st = {
      page: b.page_index() + 1,
      page_count: (JSON.parse(b.pages_json()) as unknown[]).length,
      allow_edit: allowEdit(),
      read_only: isView(),
      title: b.title(),
    };
    const key = JSON.stringify(st);
    if (!force && key === this.lastState) return;
    this.lastState = key;
    this.send({ ch: "session", t: "state", ...st });
  }

  // ------------------------------------------------------------------ ops

  private async onOp(id: number, agent: string, op: any) {
    const kind = access(op);
    setLiveAgent(agent);
    setLiveEditing({ agent, label: opLabel(op) });
    clearTimeout(this.editTimer);
    this.editTimer = window.setTimeout(() => setLiveEditing(null), 1800);
    try {
      if (kind !== "read" && !allowEdit()) {
        throw new OpError("The teacher has turned off \"Allow AI to edit live\" on the board. You can still read the board; ask the teacher if you may edit.");
      }
      if (kind === "write" && isView()) {
        throw new OpError("The board is in Student view (read-only); ask the teacher to switch to Prepare or Teach.");
      }
      let result: any;
      if (op.op === "snapshot") result = await renderSnapshot(this.c, op, agent);
      else if (op.op === "point_at") result = this.pointAt(op);
      else if (op.op === "write_handwriting") result = (await import("./solve")).onWriteOp(op, agent); // preview or write (solve/)
      else {
        try {
          result = JSON.parse(this.c.board.live_apply(JSON.stringify(op), agent));
        } catch (e) {
          let body: any;
          try { body = JSON.parse(String(e)); } catch { body = { error: String((e as any)?.message ?? e) }; }
          throw new OpError(body.error ?? "the board refused the operation", body);
        }
        if (op.op === "set_board_theme") this.applyTheme(op);
      }
      this.c.sync(true);
      if (kind === "write") {
        this.pushUpdates(); // before the result, so the agent's mirror is current
        this.editVV = this.c.board.live_version();
        setLiveLastEdit({ agent, label: opLabel(op), at: Date.now() });
      }
      if (kind !== "read") {
        this.log(agent, opLabel(op), true);
        const page = this.c.board.page_index() + 1;
        for (const fn of this.opListeners) { try { fn({ agent, op, result, page }); } catch { /* a listener failed */ } }
        this.sendState(false); // page or page count may have changed: report before answering
      }
      this.send({ ch: "tools", t: "result", id, ok: true, result });
    } catch (e: any) {
      const msg = e instanceof OpError ? e.message : String(e?.message ?? e);
      if (kind !== "read") this.log(agent, opLabel(op), false, msg);
      this.send({ ch: "tools", t: "result", id, ok: false, error: msg, data: e instanceof OpError ? e.data : undefined });
    }
  }

  private log(agent: string, label: string, ok: boolean, error?: string) {
    const a: Activity = { id: ++this.activityId, agent, label, at: Date.now(), ok, error };
    setLiveActivity((l) => [a, ...l].slice(0, 30));
  }

  private pointAt(op: any) {
    const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
    const x = num(op.x), y = num(op.y);
    if (x === undefined || y === undefined) throw new OpError("x and y must be numbers (slide pixels)");
    const seconds = Math.max(1, Math.min(15, num(op.seconds) ?? 4));
    const w = num(op.w), h = num(op.h);
    const page = this.c.board.page_index() + 1;
    const label = typeof op.label === "string" ? op.label.slice(0, 80) : undefined;
    setLivePointer({ x, y, w: w && w > 0 ? w : undefined, h: h && h > 0 ? h : undefined, label, page, until: Date.now() + seconds * 1000 });
    return { shown: true, page, seconds };
  }

  /** The UI half of set_board_theme, and the pen colour flip on dark pages
   * (as when the teacher picks a board colour). */
  private applyTheme(op: any) {
    if (op.ui === "light" || op.ui === "dark" || op.ui === "system") setPrefs({ theme: op.ui });
    const page = JSON.parse(this.c.board.pages_json())[this.c.board.page_index()];
    if (!page) return;
    const dark = (page.bg_color >>> 8) < 0x808080;
    const c = color();
    if (dark && c === 0x1c2024ff) { setColor(0xffffffff); this.c.board.set_color(0xffffffff); }
    if (!dark && c === 0xffffffff) { setColor(0x1c2024ff); this.c.board.set_color(0x1c2024ff); }
  }

  /** Undo the agent's last edit, if it is still the newest change. */
  undoLast() {
    if (!liveLastEdit() || !same(this.editVV, this.c.board.live_version())) {
      setLiveLastEdit(null);
      return;
    }
    this.c.board.undo();
    setLiveLastEdit(null);
  }

  // ------------------------------------------------------------ LNA

  /** Chrome asks before a public site (https://rupok.app) talks to this
   * computer ("Local Network Access"). Read the permission to explain it. */
  async checkLna() {
    const perms = (navigator as any).permissions;
    if (!perms?.query) return setLiveLna("unsupported");
    for (const name of ["local-network-access", "loopback-network"]) {
      try {
        const s = await perms.query({ name });
        setLiveLna(s.state as LnaState);
        s.onchange = () => setLiveLna(s.state as LnaState);
        return;
      } catch { /* not this name */ }
    }
    setLiveLna("unsupported");
  }

  /** Item ids an agent made on the current page → agent name. */
  attribution(): Record<string, string> {
    try { return JSON.parse(this.c.board.live_attribution_json(this.c.board.page_index())); } catch { return {}; }
  }
}

export const live = new LiveClient();
