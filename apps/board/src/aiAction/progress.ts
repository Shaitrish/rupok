// Working cards: while the AI builds something, a card sits on the board
// where the result will land, showing the agent, its current step, its plan
// and (for Smart Objects) a live preview. When the result arrives the card
// moves onto it and resolves, and the result animates in.
//
// Cards come from three places:
// - the in-app chat: a prompt starts a card; the agent's ACP plan and tool
//   calls stream into it (chatHooks in chat.ts);
// - Solve this: "Thinking…" below the problem (solve/index.ts);
// - any agent, explicitly: the `show_progress` op (rupok_live::progress,
//   docs/ai-agents.md § "AI in action").
// Agent ops that land (live.ts op listeners) resolve them.

import { produce } from "solid-js/store";
import type { Controller } from "../controller";
import { chatBusy, chatStatus, sendPrompt, stopChat } from "../chat";
import { items, pageIndex } from "../state";
import { aiPrefs, cards, setCards, setSparkle, type Card, type CardKind, type Rect } from "./state";

let c: Controller | null = null;
let seq = 0;
const now = () => Date.now();
const SLIDE = { w: 1920, h: 1080 };

export function initProgress(controller: Controller) {
  c = controller;
  setInterval(sweep, 4000);
}

function frame(): { w: number; h: number } {
  try {
    const p = JSON.parse(c!.board.pages_json())[c!.board.page_index()];
    return p?.frame ?? SLIDE;
  } catch { return SLIDE; }
}
const currentPage = () => (c ? c.board.page_index() + 1 : pageIndex() + 1);

/** Where a card goes when nothing says: centred on the page, sized for its kind. */
export function defaultRect(kind: CardKind): Rect {
  const f = frame();
  const [w, h] = kind === "object" ? [Math.min(1280, f.w * 0.7), Math.min(720, f.h * 0.67)] : [Math.min(1040, f.w * 0.56), Math.min(420, f.h * 0.4)];
  return { x: (f.w - w) / 2, y: (f.h - h) / 2, w, h };
}

/** A rough guess at what a request will make, for the card's icon and size. */
export function guessKind(text: string): CardKind {
  if (/simulat|smart object|interactive|3d|model|molecule|animation|graph|chart|quiz|game|diagram|visuali[sz]e|show me/i.test(text)) return "object";
  if (/solve|step by step|working|handwrit/i.test(text)) return "handwriting";
  if (/shape|flowchart|venn|table|draw/i.test(text)) return "shape";
  return "other";
}

const byId = (id: string) => cards.list.find((x) => x.id === id);
export const activeCard = (sources: Card["source"][] = ["chat", "solve", "student"]) =>
  [...cards.list].reverse().find((x) => sources.includes(x.source) && x.state === "working");

export function startCard(o: Partial<Card> & { source: Card["source"]; agent?: string }): string | null {
  if (!aiPrefs.read().progress) return null;
  const kind = o.kind ?? "other";
  const id = o.id ?? `p${++seq}`;
  const card: Card = {
    id, source: o.source, agent: o.agent ?? chatStatus().label ?? "AI", title: o.title ?? "", step: o.step ?? "Planning…",
    kind, plan: o.plan ?? [], tools: [], percent: o.percent, page: o.page ?? currentPage(), rect: o.rect ?? defaultRect(kind),
    state: "working", preview: o.preview, fixes: 0, started: now(), updated: now(),
  };
  // One card per source at a time: a new request replaces the old card.
  setCards("list", (l) => [...l.filter((x) => !(x.source === o.source && x.source !== "agent" && x.state === "working")), card]);
  return id;
}

export function patchCard(id: string, fn: (c: Card) => void) {
  setCards("list", produce((l) => {
    const it = l.find((x) => x.id === id);
    if (it) { fn(it); it.updated = now(); }
  }));
}

export function removeCard(id: string) {
  setCards("list", (l) => l.filter((x) => x.id !== id));
}

// ------------------------------------------------------------ landing

let audio: AudioContext | null = null;
/** A soft two-note chime (Settings: off by default). */
function chime() {
  if (!aiPrefs.read().sound) return;
  try {
    audio ??= new AudioContext();
    const t = audio.currentTime;
    for (const [f, d] of [[880, 0], [1320, 0.09]] as const) {
      const o = audio.createOscillator(), g = audio.createGain();
      o.type = "sine";
      o.frequency.value = f;
      g.gain.setValueAtTime(0.0001, t + d);
      g.gain.exponentialRampToValueAtTime(0.06, t + d + 0.02);
      g.gain.exponentialRampToValueAtTime(0.0001, t + d + 0.35);
      o.connect(g).connect(audio.destination);
      o.start(t + d);
      o.stop(t + d + 0.4);
    }
  } catch { /* no audio */ }
}

/** Animate a placed item in (a Smart Object fades and scales into its spot). */
export function animateItem(itemId?: string) {
  if (!itemId) return;
  let tries = 0;
  const go = () => {
    const el = document.querySelector<HTMLElement>(`[data-item="${CSS.escape(itemId)}"]`);
    if (!el) { if (++tries < 20) requestAnimationFrame(go); return; }
    el.classList.remove("ai-land");
    void el.offsetWidth;
    el.classList.add("ai-land");
    el.addEventListener("animationend", () => el.classList.remove("ai-land"), { once: true });
  };
  requestAnimationFrame(go);
}

/** Something landed at `rect` on `page`: a small celebration there. */
export function celebrateAt(rect: Rect, page: number) {
  if (aiPrefs.read().celebrate) setSparkle({ x: rect.x + rect.w / 2, y: rect.y + rect.h / 2, page, n: ++seq });
  chime();
}

/** The card's result arrived: move onto it, resolve, then go. */
export function landCard(id: string, rect?: Rect, page?: number) {
  const card = byId(id);
  if (!card) return;
  patchCard(id, (x) => {
    x.state = "landing";
    if (rect) x.rect = rect;
    if (page) x.page = page;
    x.step = "Done";
    x.percent = 100;
  });
  celebrateAt(rect ?? card.rect, page ?? card.page);
  setTimeout(() => removeCard(id), 900);
}

export function failCard(id: string, error: string) {
  patchCard(id, (x) => { x.state = "failed"; x.error = error; x.step = "Something went wrong"; });
}

/** Retry a failed card: ask the agent again, with what went wrong. */
export function retryCard(id: string) {
  const card = byId(id);
  if (!card) return;
  removeCard(id);
  const what = card.kind === "object" ? "the Smart Object" : "it";
  void sendPrompt(`That did not work${card.error ? ` (${card.error})` : ""}. Please fix ${what} and try again.${card.title ? ` It was: ${card.title}` : ""}`);
}

/** The teacher pressed the card's ✕: stop the AI's turn (chat) and drop it. */
export function cancelCard(id: string) {
  const card = byId(id);
  removeCard(id);
  if (card && card.source !== "agent" && chatBusy()) stopChat();
}

function sweep() {
  const t = now();
  for (const x of cards.list) {
    const idle = t - x.updated;
    if (x.state === "working" && x.source === "agent" && idle > 120_000) removeCard(x.id);
    else if (x.state === "working" && x.source !== "agent" && idle > 90_000 && !chatBusy()) removeCard(x.id);
    else if (x.state === "failed" && idle > 90_000) removeCard(x.id);
    else if (x.state === "done" && idle > 1500) removeCard(x.id);
  }
}

// ------------------------------------------------------- the chat's stream

/** Plain words for what a tool call is doing. */
export function stepFor(title: string): { step: string; kind?: CardKind } | null {
  const t = title.toLowerCase().replace(/[^a-z_]/g, "_");
  const has = (n: string) => t.includes(n);
  if (has("show_progress")) return null;
  if (has("validate_smart_object")) return { step: "Testing the Smart Object…", kind: "object" };
  if (has("read_object_spec")) return { step: "Reading how Smart Objects work…", kind: "object" };
  if (has("update_smart_object")) return { step: "Improving the Smart Object…", kind: "object" };
  if (has("add_smart_object")) return { step: "Building the Smart Object…", kind: "object" };
  if (has("add_3d_model") || has("add_model")) return { step: "Placing the 3D model…", kind: "object" };
  if (has("write_handwriting")) return { step: "Writing the solution…", kind: "handwriting" };
  if (has("add_text") || has("replace_")) return { step: "Writing text…", kind: "text" };
  if (has("add_shape") || has("add_table")) return { step: "Drawing…", kind: "shape" };
  if (has("snapshot") || has("get_current_page") || has("get_selection") || has("outline")) return { step: "Looking at the board…" };
  if (has("add_page")) return { step: "Adding a page…" };
  if (has("set_page_design") || has("branding")) return { step: "Designing the page…" };
  if (has("point_at")) return { step: "Pointing at the board…" };
  const short = title.replace(/^mcp__\w+?__/, "").replace(/_/g, " ").slice(0, 48);
  return { step: `Working: ${short}…` };
}

function onPrompt(text: string, source: "chat" | "solve" | "student") {
  if (source === "solve") return; // Solve this starts its own card, below the problem
  const kind = guessKind(text);
  const title = text.replace(/\s+/g, " ").trim().slice(0, 80);
  startCard({ source, kind, title, step: "Planning…" });
}

function onUpdate(u: any) {
  const card = activeCard();
  if (!card) return;
  switch (u.sessionUpdate) {
    case "plan": {
      const plan = (u.entries ?? []).slice(0, 12).map((e: any) => ({ content: String(e.content ?? ""), status: String(e.status ?? "pending") }));
      const doing = plan.find((e: any) => e.status === "in_progress");
      const done = plan.filter((e: any) => e.status === "completed").length;
      patchCard(card.id, (x) => {
        x.plan = plan;
        if (doing) x.step = `${doing.content.replace(/[.…]+$/, "")}…`;
        if (plan.length) x.percent = Math.round((done / plan.length) * 100);
      });
      break;
    }
    case "agent_thought_chunk":
      if (card.step === "Planning…") patchCard(card.id, (x) => { x.step = "Thinking…"; });
      break;
    case "tool_call": {
      const s = stepFor(String(u.title ?? ""));
      if (!s) break;
      const inp = u.rawInput ?? {};
      patchCard(card.id, (x) => {
        x.step = s.step;
        if (s.kind && (x.kind === "other" || s.kind === "object")) x.kind = s.kind;
        x.tools = [...x.tools, s.step].slice(-3);
        // The agent says where the result goes: move the card there now.
        const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
        const [rx, ry, rw, rh] = [num(inp.x), num(inp.y), num(inp.width ?? inp.w), num(inp.height ?? inp.h)];
        if (rx !== undefined && ry !== undefined && rw && rh) x.rect = { x: rx, y: ry, w: rw, h: rh };
        else if (s.kind === "object" && x.kind === "object" && rw && rh) x.rect = { ...defaultRect("object"), w: rw, h: rh };
        if (typeof inp.page === "number") x.page = inp.page;
      });
      break;
    }
    case "tool_call_update":
      if (u.status === "failed" && !card.fixes) patchCard(card.id, (x) => { x.step = "That step failed; trying again…"; });
      break;
  }
}

function onDone(f: { stop_reason?: string; error?: string }) {
  const card = activeCard();
  if (!card) return;
  if (f.stop_reason === "cancelled") return removeCard(card.id);
  if (f.stop_reason === "error" || f.stop_reason === "refusal") return failCard(card.id, f.error ?? "The assistant stopped.");
  if (card.fixes > 0 && card.kind === "object") return failCard(card.id, "The Smart Object did not pass Rupok's checks");
  // Nothing landed (an answer in chat only): say done and fade.
  patchCard(card.id, (x) => { x.state = "done"; x.step = "Done"; x.percent = 100; });
}

// ------------------------------------------------------- agent ops landing

const PLACING = new Set(["add_smart_object", "update_smart_object", "add_text", "write_handwriting", "add_shape", "add_table", "add_media", "add_model", "replace_items_with_text"]);

function rectOf(op: any, r: any): Rect | undefined {
  const g = r?.geometry ?? r?.bounds;
  if (g && [g.x, g.y, g.w, g.h].every((v: unknown) => typeof v === "number")) return { x: g.x, y: g.y, w: g.w, h: g.h };
  const id = r?.item_id;
  const it = id ? items.find((i) => i.id === id) : undefined;
  if (it) return { x: it.x, y: it.y, w: it.w, h: it.h };
  if (op?.op === "add_shape" && [op.x, op.y, op.w, op.h].every((v: unknown) => typeof v === "number")) {
    return { x: Math.min(op.x, op.x + op.w), y: Math.min(op.y, op.y + op.h), w: Math.abs(op.w) || 4, h: Math.abs(op.h) || 4 };
  }
  return undefined;
}

/** An agent op ran on the board: resolve the card it belongs to. */
export function onOpLanded(agent: string, op: any, result: any, page: number) {
  if (op?.op === "add_page_after_current" || op?.op === "add_page") {
    // The work moves to the new page: the card follows it.
    const card = activeCard(["chat", "student"]);
    if (card && typeof result?.page === "number") patchCard(card.id, (x) => { x.page = result.page; x.step = "Building the new page…"; });
    return;
  }
  if (!PLACING.has(op?.op) || result?.status === "preview") return;
  const rp: number = typeof result?.page === "number" ? result.page : page;
  const rect = rectOf(op, result);
  const card =
    [...cards.list].reverse().find((x) => x.source === "agent" && x.state === "working" && x.agent === agent && x.page === rp) ??
    activeCard();
  animateItem(result?.item_id);
  if (card) landCard(card.id, rect ?? card.rect, rp);
  else if (rect) celebrateAt(rect, rp);
}

/** An agent op failed: a Smart Object that did not pass the checks keeps its
 * card, which says so (agents fix and retry); the turn's end offers Retry. */
export function onOpFailed(agent: string, op: any, error: string, data: any) {
  if (op?.op !== "add_smart_object" && op?.op !== "update_smart_object") return;
  const card = [...cards.list].reverse().find((x) => x.state === "working" && (x.source !== "agent" || x.agent === agent));
  if (!card) return;
  const first = (data?.diagnostics ?? []).find((d: any) => d.severity === "error");
  patchCard(card.id, (x) => {
    x.fixes += 1;
    x.kind = "object";
    x.step = x.fixes > 1 ? `Fixing the Smart Object (try ${x.fixes + 1})…` : "It did not pass the checks yet; fixing it…";
    x.error = first?.message ? String(first.message).slice(0, 140) : error.slice(0, 140);
  });
}

// ------------------------------------------------------------ show_progress

const STATES = ["working", "done", "failed", "clear"];
const KINDS = ["object", "handwriting", "text", "shape", "other"];

/** Run an agent's `show_progress` op. */
export function runShowProgress(op: any, agent: string): any {
  const bad = (m: string) => { throw new Error(m); };
  const str = (v: unknown, max: number, name: string) => {
    if (v == null) return undefined;
    if (typeof v !== "string") return bad(`${name} must be text`);
    if (v.length > max) bad(`${name} is longer than ${max} characters`);
    return v;
  };
  const num = (v: unknown, name: string) => {
    if (v == null) return undefined;
    if (typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 1e6) return bad(`${name} must be a finite number`);
    return v;
  };
  const state = str(op.state, 16, "state") ?? "working";
  if (!STATES.includes(state)) bad(`state must be one of ${STATES.join(", ")}`);
  const kind = str(op.kind, 16, "kind");
  if (kind && !KINDS.includes(kind)) bad(`kind must be one of ${KINDS.join(", ")}`);
  const title = str(op.title, 120, "title"), step = str(op.step, 200, "step"), error = str(op.error, 200, "error");
  const preview = str(op.preview_html, 512 * 1024, "preview_html");
  const percent = num(op.percent, "percent");
  const [x, y, w, h] = [num(op.x, "x"), num(op.y, "y"), num(op.w, "w"), num(op.h, "h")];
  const page = num(op.page, "page");
  if (!Array.isArray(op.plan ?? []) || (op.plan ?? []).length > 12) bad("plan is a list of at most 12 entries");
  const plan = op.plan?.map((e: any) => ({ content: String(e?.content ?? e?.text ?? "").slice(0, 200), status: String(e?.status ?? "pending") }));
  const id: string | undefined = str(op.id, 64, "id");
  const existing = id ? byId(id) : undefined;

  if (state === "clear") {
    if (existing) removeCard(existing.id);
    return { id, cleared: !!existing };
  }
  const fallback = existing?.rect ?? defaultRect((kind as CardKind) ?? "other");
  const rect: Rect = { x: x ?? fallback.x, y: y ?? fallback.y, w: w && w > 0 ? w : fallback.w, h: h && h > 0 ? h : fallback.h };
  let cardId = existing?.id;
  // The in-app chat already shows a card for this request: the agent's own
  // card takes it over (one card per request, not two).
  const adopt = !existing ? activeCard() : undefined;
  if (adopt) {
    cardId = adopt.id;
    patchCard(adopt.id, (cd) => {
      cd.agent = agent;
      if (title) cd.title = title;
      if (step) cd.step = step;
      if (kind) cd.kind = kind as CardKind;
      if (plan) cd.plan = plan;
      if (percent != null) cd.percent = percent;
      if (preview != null) cd.preview = preview;
      if (x != null || y != null || w != null || h != null) cd.rect = rect;
      if (page != null) cd.page = page;
    });
  } else if (!cardId) {
    cardId = startCard({ id, source: "agent", agent, kind: (kind as CardKind) ?? "other", title: title ?? "", step: step ?? "Working…", rect, page: page ?? currentPage(), plan, percent, preview }) ?? undefined;
    if (!cardId) return { shown: false, note: "the teacher has turned off progress cards; carry on" };
  } else {
    patchCard(cardId, (cd) => {
      if (title != null) cd.title = title;
      if (step != null) cd.step = step;
      if (kind) cd.kind = kind as CardKind;
      if (plan) cd.plan = plan;
      if (percent != null) cd.percent = percent;
      if (preview != null) cd.preview = preview;
      if (x != null || y != null || w != null || h != null) cd.rect = rect;
      if (page != null) cd.page = page;
      if (cd.state === "failed" && state === "working") { cd.state = "working"; cd.error = undefined; }
    });
  }
  if (state === "done") landCard(cardId, rect, page ?? undefined);
  if (state === "failed") failCard(cardId, error ?? "It did not work");
  const card = byId(cardId);
  return {
    id: cardId, page: card?.page ?? page ?? currentPage(), state, shown: card?.page === currentPage(),
    note: state === "working" ? "Place your result in this box (same page, x, y, w, h): the card turns into it. Update with the same id." : undefined,
  };
}

/** Wire the chat's stream and the board's ops into the cards. */
export function attachProgress(hooks: { prompt: Set<any>; update: Set<any>; done: Set<any> }) {
  hooks.prompt.add(onPrompt);
  hooks.update.add(onUpdate);
  hooks.done.add(onDone);
}
