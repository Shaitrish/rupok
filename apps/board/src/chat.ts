// In-app AI chat: talk to the teacher's own agent (Claude Code, Codex,
// OpenCode, Gemini CLI) inside Rupok. The chat travels on the live
// channel's `chat` channel to `rupok agent` (or the desktop app), which
// speaks ACP to the agent and gives it the Rupok tools; what the agent
// builds arrives on the board through the live channel (live.ts).
// Frames: docs/ai-agents.md § "`chat`". Attachments, @-mentions and
// /commands: chatContext.ts. Providers (Rupok AI later): chatProviders.ts.

import { createSignal } from "solid-js";
import { createStore, produce, unwrap } from "solid-js/store";
import type { Controller } from "./controller";
import { live, liveHasChat, livePhase, type OpDone } from "./live";
import { renderSnapshot } from "./snapshot";
import { isView } from "./state";
import { jsonStore } from "./settings/bind";
import { RUPOK_COMMANDS, runRupokCommand, solveApi, type Attachment, type CtxEntry, type SlashCommand } from "./chatContext";

/** Chat preferences (Settings → AI & agents → Chat, settings/defs/chat.ts). */
export const chatPrefs = jsonStore("rupok.chat", {
  /** Agent to use: "auto" (the first installed) or an agent id. */
  agent: "auto",
  /** Attach the current page to messages by default. */
  attach: true,
  /** "ask": confirm each Rupok board edit; "session": auto-approve them in each chat. */
  permissions: "ask" as "ask" | "session",
  /** Side the panel docks to. */
  position: "right" as "right" | "left",
});

export interface AgentChoice {
  id: string;
  label: string;
  available: boolean;
  install: string[];
  login: string;
  logged_in?: boolean;
}

export interface ModeInfo { id: string; name: string; description?: string }
export interface ConfigOption {
  id: string;
  name: string;
  description?: string;
  category?: string;
  type?: string;
  currentValue?: unknown;
  options?: any[];
}

export type ChatState = "idle" | "starting" | "ready" | "busy" | "offline" | "error";
export interface ChatStatus {
  state: ChatState;
  agent?: string;
  label?: string;
  session?: string;
  code?: "not_installed" | "login" | "crashed" | string;
  error?: string;
  install?: string[];
  login?: string;
  images?: boolean;
  warning?: string;
  caps?: { images?: boolean; embedded?: boolean; audio?: boolean; load?: boolean; list?: boolean; http_mcp?: boolean };
  modes?: { currentModeId?: string; availableModes?: ModeInfo[] } | null;
  models?: { currentModelId?: string; availableModels?: { modelId: string; name: string; description?: string }[] } | null;
  config?: ConfigOption[] | null;
}

/** A change the agent made on the board, shown on its tool card. */
export interface BoardChange { label: string; page?: number; itemId?: string }

export type ChatItem =
  | { kind: "user"; id: string; text: string; context: string[]; queued?: boolean }
  | { kind: "agent"; id: string; text: string; done: boolean }
  | { kind: "thought"; id: string; text: string }
  | { kind: "tool"; id: string; title: string; status: string; toolKind?: string; changes: BoardChange[]; detail?: string; input?: string }
  | { kind: "permission"; id: string; reqId: number; title: string; options: { optionId: string; name: string; kind: string }[]; answered?: string; preview?: { page: number; png: string } | null; detail?: string }
  | { kind: "plan"; id: string; entries: { content: string; status: string; priority?: string }[] }
  | { kind: "notice"; id: string; text: string; tone: "info" | "error" };

export const [chatOpen, setChatOpen] = createSignal(false);
export const [chatStatus, setChatStatus] = createSignal<ChatStatus>({ state: "idle" });
export const [chatAgents, setChatAgents] = createSignal<AgentChoice[]>([]);
export const [chatAgent, setChatAgent] = createSignal<string | null>(null);
export const [chat, setChat] = createStore<{ items: ChatItem[] }>({ items: [] });
/** Attach the current page (or the selection) to the next message. */
export const [attachPage, setAttachPage] = createSignal(chatPrefs.read().attach);
/** Width of the docked panel (px); persisted per browser. */
export const [chatWidth, setChatWidthSignal] = createSignal(loadWidth());
/** The agent's own slash commands (ACP `available_commands_update`). */
export const [agentCommands, setAgentCommands] = createSignal<SlashCommand[]>([]);
/** Context window use and cost, when the agent reports it. */
export const [chatUsage, setChatUsage] = createSignal<{ used: number; size: number; cost?: { amount: number; currency: string } } | null>(null);
/** Auto-approve Rupok board edits in this chat (Settings sets the default). */
export const [autoApprove, setAutoApprove] = createSignal(chatPrefs.read().permissions === "session");
/** Earlier chats the agent itself lists (ACP `session/list`). */
export const [agentSessions, setAgentSessions] = createSignal<{ sessionId: string; title?: string; updatedAt?: string }[] | null>(null);
/** Text the composer should show (edit a sent message, dictation, recall). */
export const [composerDraft, setComposerDraft] = createSignal<{ text: string; n: number } | null>(null);
/** The agent is replaying an earlier chat (`session/load`). */
export const [replaying, setReplaying] = createSignal(false);

const WIDTH_KEY = "rupok.chat.width";
function loadWidth(): number {
  try {
    const w = Number(localStorage.getItem(WIDTH_KEY));
    return w >= 300 && w <= 720 ? w : 380;
  } catch { return 380; }
}
export function setChatWidth(w: number) {
  const v = Math.round(Math.max(300, Math.min(720, w)));
  setChatWidthSignal(v);
  try { localStorage.setItem(WIDTH_KEY, String(v)); } catch { /* storage off */ }
}

export const chatBusy = () => chatStatus().state === "busy" || chatStatus().state === "starting";
export const pendingPermissions = () => chat.items.filter((x) => x.kind === "permission" && !x.answered) as Extract<ChatItem, { kind: "permission" }>[];

/** Open the AI panel: the chat when the paired relay offers it, else the
 * connect flow. Used by the header's AI button and the ⋯ menu. */
export function openAiPanel() {
  if (isView()) return;
  setChatOpen(true);
  // Not connected yet: look for `rupok agent` / the desktop relay.
  const p = livePhase();
  if (p === "idle" || p === "unreachable") startRelay();
}

export function closeAiPanel() {
  setChatOpen(false);
}

/** Desktop: start the app's built-in relay; browser: probe for `rupok agent`. */
export async function startRelay() {
  const tauri = (window as any).__TAURI_INTERNALS__;
  if (tauri?.invoke) {
    try {
      const r = await tauri.invoke("agent_start");
      if (r?.port && r?.code) return live.connectTo(r.port, r.code);
    } catch (e) {
      console.warn("rupok: could not start the built-in agent relay", e);
    }
  }
  live.connect();
}

// ------------------------------------------------------- models and modes

/** Friendly names for the modes agents commonly use (Claude Code's first). */
const MODE_NAMES: Record<string, { name: string; description: string }> = {
  default: { name: "Ask first", description: "Asks before each change" },
  acceptEdits: { name: "Accept edits", description: "Makes changes without asking" },
  plan: { name: "Plan only", description: "Plans, changes nothing" },
  bypassPermissions: { name: "Never ask", description: "No permission prompts at all" },
  auto: { name: "Auto", description: "Decides when to ask" },
  "read-only": { name: "Read only", description: "Looks, changes nothing" },
  "full-access": { name: "Full access", description: "No permission prompts at all" },
};

/** Modes that drop every permission prompt. Hidden unless the teacher opts in
 * (Settings → AI & agents → "Allow no-prompt modes"): in a live class the agent
 * must not act on the computer without the teacher's OK. */
const DANGEROUS = new Set(["bypassPermissions", "full-access"]);
const allowDangerous = () => { try { return localStorage.getItem("rupok.chat.allowNoPrompt") === "1"; } catch { return false; } };

export function modes(): ModeInfo[] {
  return (chatStatus().modes?.availableModes ?? []).filter((m) => allowDangerous() || !DANGEROUS.has(m.id)).map((m) => ({
    id: m.id,
    name: MODE_NAMES[m.id]?.name ?? m.name,
    description: m.description || MODE_NAMES[m.id]?.description,
  }));
}
export const currentMode = () => chatStatus().modes?.currentModeId ?? null;
/** Modes in which the agent already edits without asking. */
const NO_ASK = new Set(["acceptEdits", "bypassPermissions", "full-access", "auto"]);
export const modeAutoApproves = () => { const m = currentMode(); return !!m && NO_ASK.has(m); };
/** Board edits are approved without a prompt: by this chat's toggle or the agent's mode. */
export const editsAutoApproved = () => autoApprove() || modeAutoApproves();

/** The model choices: an ACP config option with `category: "model"`, else the
 * agent's (unstable) model list. Grouped options are flattened. */
export function models(): { id: string; name: string; description?: string; group?: string }[] {
  const opt = (chatStatus().config ?? []).find((o) => o.category === "model");
  if (opt?.options) {
    const out: { id: string; name: string; description?: string; group?: string }[] = [];
    for (const o of opt.options) {
      if (Array.isArray(o.options)) for (const x of o.options) out.push({ id: String(x.value), name: x.name, description: x.description, group: o.name ?? o.group });
      else out.push({ id: String(o.value), name: o.name, description: o.description });
    }
    return out;
  }
  return (chatStatus().models?.availableModels ?? []).map((m) => ({ id: m.modelId, name: m.name, description: m.description }));
}
export function currentModel(): string | null {
  const opt = (chatStatus().config ?? []).find((o) => o.category === "model");
  if (opt) return opt.currentValue == null ? null : String(opt.currentValue);
  return chatStatus().models?.currentModelId ?? null;
}
/** Other config options (thinking level…), shown in the picker. */
export const otherConfig = () => (chatStatus().config ?? []).filter((o) => o.category !== "model" && o.category !== "mode");

// ---------------------------------------------------------------- frames

let seq = 0;
const nid = () => `c${++seq}`;
let c: Controller | null = null;
/** The agent text item streaming in this turn. */
let streaming: string | null = null;
/** Tool cards by ACP toolCallId. */
const toolIds = new Map<string, string>();
/** Messages waiting for the current turn to end. */
const queue: { itemId: string; text: string; ctx: CtxEntry[] }[] = [];
/** The last prompt sent, for Retry. */
let lastSent: { text: string; ctx: CtxEntry[]; labels: string[] } | null = null;
/** Sent prompts, newest last, for ↑ recall. */
const sentTexts: string[] = [];

function push(item: ChatItem) {
  setChat("items", (l) => [...l, item]);
}
function patch(id: string, fn: (it: any) => void) {
  setChat("items", produce((l) => {
    const it = l.find((x) => x.id === id);
    if (it) fn(it);
  }));
}
function note(text: string, tone: "info" | "error" = "info") {
  push({ kind: "notice", id: nid(), text, tone });
}

function textOf(content: any): string {
  if (!content) return "";
  if (Array.isArray(content)) return content.map(textOf).join("");
  if (content.type === "text") return content.text ?? "";
  if (content.type === "content") return textOf(content.content);
  if (content.type === "diff") return `${content.path ?? "file"}: changed`;
  return "";
}

/** Does an ACP tool title ("mcp__rupok__add_text", "rupok: add_text") name this Rupok tool? */
const titleNames = (title: string, op: string) => title.toLowerCase().replace(/[^a-z_]/g, "_").includes(op);
const isBoardTool = (tc: any) => /rupok|board/i.test(`${tc?.title ?? ""} ${JSON.stringify(tc?.rawInput ?? "")}`);

function onFrame(f: any) {
  switch (f.t) {
    case "hello":
      setChatAgents(Array.isArray(f.agents) ? f.agents : []);
      if (!chatAgent()) {
        const pref = chatPrefs.read().agent;
        const ok = (f.agents ?? []).some((a: AgentChoice) => a.id === pref && a.available);
        setChatAgent(f.current ?? (ok ? pref : f.default) ?? null);
      }
      // Start the assistant right away, so its models, modes and commands
      // are in the panel before the first message.
      if (!f.current && chatStatus().state === "idle" && chatAgent() && (f.agents ?? []).some((a: AgentChoice) => a.id === chatAgent() && a.available)) {
        live.sendChat({ t: "start", agent: chatAgent() });
      }
      break;
    case "status": {
      const prev = chatStatus();
      setChatStatus(f as ChatStatus);
      if (f.agent) setChatAgent(f.agent);
      if (Array.isArray(f.commands)) setAgentCommands(f.commands.map((x: any) => ({ name: x.name, description: x.description ?? "", hint: x.input?.hint, source: "agent" as const })));
      if (f.state !== "busy") streaming = null;
      if (f.state === "ready" && prev.state === "busy") flushQueue();
      if (f.session && f.session !== prev.session && f.state === "ready") setAgentSessions(null);
      break;
    }
    case "update": onUpdate(f.update ?? {}); break;
    case "permission": void onPermission(f); break;
    case "done":
      streaming = null;
      if (f.stop_reason === "cancelled") note("Stopped.");
      else if (f.stop_reason === "error" && f.error) note(f.error, "error");
      else if (f.stop_reason === "refusal") note("The assistant declined this request.");
      else if (f.stop_reason === "max_tokens" || f.stop_reason === "max_turn_requests") note("The assistant stopped early (it hit its limit). Ask it to continue.");
      setChat("items", produce((l) => { for (const it of l) if (it.kind === "agent") it.done = true; }));
      saveHistory();
      break;
    case "notice":
      if (f.error) note(f.error, "error");
      break;
    case "cleared":
      setChat("items", []);
      toolIds.clear();
      streaming = null;
      setChatUsage(null);
      break;
    case "loading":
      setChat("items", []);
      toolIds.clear();
      streaming = null;
      setReplaying(true);
      break;
    case "loaded":
      setReplaying(false);
      streaming = null;
      setChat("items", produce((l) => { for (const it of l) if (it.kind === "agent") it.done = true; }));
      note("Reopened. Carry on where you left off.");
      saveHistory();
      break;
    case "sessions":
      setAgentSessions(Array.isArray(f.sessions) ? f.sessions : []);
      break;
  }
}

function onUpdate(u: any) {
  switch (u.sessionUpdate) {
    case "user_message_chunk": {
      // Replayed history (session/load).
      const t = textOf(u.content);
      const last = chat.items[chat.items.length - 1];
      if (last?.kind === "user" && replaying()) patch(last.id, (it) => { it.text += t; });
      else push({ kind: "user", id: nid(), text: t, context: [] });
      streaming = null;
      break;
    }
    case "agent_message_chunk": {
      const t = textOf(u.content);
      if (!t) return;
      if (streaming) patch(streaming, (it) => { it.text += t; });
      else {
        streaming = nid();
        push({ kind: "agent", id: streaming, text: t, done: false });
      }
      break;
    }
    case "agent_thought_chunk": {
      const t = textOf(u.content);
      const last = chat.items[chat.items.length - 1];
      if (last?.kind === "thought") patch(last.id, (it) => { it.text += t; });
      else push({ kind: "thought", id: nid(), text: t });
      break;
    }
    case "tool_call": {
      streaming = null; // text after a tool call starts a new bubble
      const id = nid();
      toolIds.set(u.toolCallId, id);
      push({
        kind: "tool", id, title: u.title ?? "Tool", status: u.status ?? "pending", toolKind: u.kind, changes: [],
        input: u.rawInput ? JSON.stringify(u.rawInput, null, 2).slice(0, 4000) : undefined,
        detail: textOf(u.content) || undefined,
      });
      break;
    }
    case "tool_call_update": {
      const id = toolIds.get(u.toolCallId);
      if (!id) return;
      patch(id, (it) => {
        if (u.status) it.status = u.status;
        if (u.title) it.title = u.title;
        if (u.rawInput) it.input = JSON.stringify(u.rawInput, null, 2).slice(0, 4000);
        const d = textOf(u.content);
        if (d) it.detail = d.slice(0, 4000);
      });
      break;
    }
    case "plan": {
      const last = [...chat.items].reverse().find((x) => x.kind === "plan");
      const entries = (u.entries ?? []).map((e: any) => ({ content: e.content, status: e.status, priority: e.priority }));
      if (last) patch(last.id, (it) => { it.entries = entries; });
      else push({ kind: "plan", id: nid(), entries });
      break;
    }
    case "available_commands_update":
      setAgentCommands((u.availableCommands ?? []).map((x: any) => ({ name: x.name, description: x.description ?? "", hint: x.input?.hint, source: "agent" as const })));
      break;
    case "current_mode_update":
      setChatStatus((s) => (s.modes ? { ...s, modes: { ...s.modes, currentModeId: u.currentModeId } } : s));
      break;
    case "config_option_update":
      setChatStatus((s) => ({ ...s, config: u.configOptions ?? s.config }));
      break;
    case "usage_update":
      setChatUsage({ used: u.used, size: u.size, cost: u.cost ?? undefined });
      break;
    case "session_info_update":
      if (u.title) saveHistory(u.title);
      break;
  }
}

/** A permission request: auto-answer board edits when approved for this
 * chat, else show it with a preview of the page it will change. */
async function onPermission(f: any) {
  const tc = f.tool_call ?? {};
  const opts: { optionId: string; name: string; kind: string }[] = f.options ?? [];
  const allow = opts.find((o) => o.kind === "allow_once") ?? opts.find((o) => o.kind === "allow_always");
  if (autoApprove() && allow && isBoardTool(tc)) {
    live.sendChat({ t: "permission_reply", id: f.id, option: allow.optionId });
    push({ kind: "permission", id: nid(), reqId: f.id, title: tc.title ?? "Board edit", options: [], answered: "Approved automatically (auto-approve board edits is on)" });
    return;
  }
  const id = nid();
  const input = tc.rawInput ?? {};
  push({
    kind: "permission", id, reqId: f.id, title: tc.title ?? "The assistant wants to use a tool",
    options: opts.map((o) => ({ optionId: o.optionId, name: o.name, kind: o.kind })),
    detail: Object.keys(input).length ? JSON.stringify(input, null, 2).slice(0, 2000) : undefined,
  });
  // Preview: the page the change lands on (current page unless it says).
  if (c && isBoardTool(tc)) {
    const page = typeof input.page === "number" ? input.page : c.board.page_index() + 1;
    const pages = (JSON.parse(c.board.pages_json()) as unknown[]).length;
    if (page >= 1 && page <= pages) {
      try {
        const s = await renderSnapshot(c, { page, width: 480 }, "Teacher");
        patch(id, (it) => { it.preview = { page, png: s.png }; });
      } catch { /* no preview */ }
    }
  }
}

/** Describe a board change for a card: "Added Smart Object 'Ohm's law' on page 3". */
export function describeChange(e: OpDone): BoardChange {
  const op = e.op ?? {}, r = e.result ?? {};
  const page: number | undefined = r.page ?? op.page ?? e.page;
  const on = page ? ` on page ${page}` : "";
  switch (op.op) {
    case "add_smart_object": return { label: `Added Smart Object '${op.title ?? "Untitled"}'${on}`, page, itemId: r.item_id };
    case "update_smart_object": return { label: `Updated Smart Object${on}`, page, itemId: r.item_id };
    case "add_media": return { label: `Added ${r.kind === "youtube" ? "a YouTube video" : r.kind === "web" ? "a web embed" : r.kind === "audio" ? "audio" : "a video"}${on}`, page, itemId: r.item_id };
    case "add_model": return { label: `Added a 3D model${on}`, page, itemId: r.item_id };
    case "add_text": {
      const t = String(op.text ?? "").split("\n")[0];
      return { label: `Added text '${t.length > 40 ? t.slice(0, 39) + "…" : t}'${on}`, page, itemId: r.item_id };
    }
    case "add_page": case "add_page_after_current": return { label: `Added page ${r.page ?? ""}${op.title ? ` '${op.title}'` : ""}`.trim(), page: r.page };
    case "go_to_page": return { label: `Showed page ${op.page}`, page: op.page };
    case "delete_item": return { label: `Deleted an item${on}`, page };
    case "replace_items_with_text": return { label: `Turned ink into text${on}`, page, itemId: r.item_id };
    case "set_page_background": return { label: `Changed the background${on}`, page };
    case "set_board_theme": return { label: "Changed the board theme", page };
    case "set_page_design": return { label: r.pages_changed > 1 ? `Changed the design of ${r.pages_changed} pages` : `Changed the page design${on}`, page };
    case "set_branding": return { label: "Changed the branding", page };
    case "step_object": return { label: `Stepped a Smart Object to step ${r.step ?? ""}`.trim(), page };
    case "write_handwriting": return r.status === "preview"
      ? { label: `Solution ready to check (${r.lines ?? ""} lines)`, page }
      : { label: `Wrote ${(op.lines ?? []).length} lines in handwriting${on}`, page, itemId: r.first_item_id };
    case "point_at": return { label: op.label ? `Pointed at '${op.label}'` : "Pointed at the board", page };
    default: return { label: op.op ?? "Changed the board", page };
  }
}

/** Attach a board change to the tool card running it, else its own card. */
function onBoardChange(e: OpDone) {
  if (!chatOpen() && chat.items.length === 0) return;
  const ch = describeChange(e);
  const name = e.op?.op as string;
  const cards = chat.items.filter((x): x is Extract<ChatItem, { kind: "tool" }> => x.kind === "tool");
  const live = [...cards].reverse();
  const card =
    live.find((t) => (t.status === "pending" || t.status === "in_progress") && !!name && titleNames(t.title, name)) ??
    live.find((t) => t.status === "pending" || t.status === "in_progress") ??
    (cards.length && chatBusy() ? cards[cards.length - 1] : undefined);
  if (card) patch(card.id, (it) => { it.changes.push(ch); });
  else if (chatBusy()) push({ kind: "tool", id: nid(), title: name ?? "Board", status: "completed", changes: [ch] });
}

// ---------------------------------------------------------------- history

/** Chats are kept per lesson (by the lesson's doc key), newest first. */
export interface HistoryEntry { session: string; agent: string; label: string; title: string; updated: number; items: ChatItem[] }
const HISTORY_MAX = 20;
const historyKey = () => {
  try { return `rupok.chat.history.${JSON.parse(c?.board.live_doc_json() ?? "{}").key || "untitled"}`; } catch { return "rupok.chat.history.untitled"; }
};
export const [historyVersion, setHistoryVersion] = createSignal(0);

export function chatHistory(): HistoryEntry[] {
  historyVersion();
  try { return JSON.parse(localStorage.getItem(historyKey()) || "[]"); } catch { return []; }
}

function saveHistory(title?: string) {
  const st = chatStatus();
  if (!st.session || !c) return;
  const items = unwrap(chat.items).filter((x) => x.kind !== "permission" || x.answered).slice(-200);
  if (!items.some((x) => x.kind === "user")) return;
  const list = chatHistory().filter((h) => h.session !== st.session);
  const prev = chatHistory().find((h) => h.session === st.session);
  const first = items.find((x) => x.kind === "user") as Extract<ChatItem, { kind: "user" }> | undefined;
  list.unshift({
    session: st.session, agent: st.agent ?? chatAgent() ?? "", label: st.label ?? "",
    title: title ?? prev?.title ?? (first?.text.slice(0, 60) || "Chat"), updated: Date.now(),
    items: JSON.parse(JSON.stringify(items)),
  });
  try { localStorage.setItem(historyKey(), JSON.stringify(list.slice(0, HISTORY_MAX))); } catch { /* full */ }
  setHistoryVersion((v) => v + 1);
}

/** Reopen an earlier chat: the agent resumes it (ACP `session/load`) when it
 * can; otherwise the transcript is shown and the next message starts fresh. */
export function resumeChat(h: HistoryEntry) {
  const st = chatStatus();
  const canLoad = !!st.caps?.load && (st.agent ?? chatAgent()) === h.agent;
  if (canLoad) {
    live.sendChat({ t: "load_session", session: h.session, agent: h.agent });
    return;
  }
  live.sendChat({ t: "new" });
  queueMicrotask(() => {
    setChat("items", JSON.parse(JSON.stringify(h.items)));
    note(`${h.label || "This assistant"} cannot continue an earlier chat. Your next message starts a new one.`);
  });
}

export function resumeAgentSession(sessionId: string) {
  live.sendChat({ t: "load_session", session: sessionId });
}

export function forgetChat(session: string) {
  const list = chatHistory().filter((h) => h.session !== session);
  try { localStorage.setItem(historyKey(), JSON.stringify(list)); } catch { /* storage off */ }
  setHistoryVersion((v) => v + 1);
}

export function listAgentSessions() {
  setAgentSessions(null);
  live.sendChat({ t: "list_sessions" });
}

// ---------------------------------------------------------------- actions

export function initChat(controller: Controller) {
  c = controller;
  live.chatHandler = onFrame;
  live.onOpDone(onBoardChange);
}

/** Ask the relay for the agent list again (after installing one). */
export function refreshAgents() {
  live.sendChat({ t: "agents" });
}

export function pickAgent(id: string) {
  setChatAgent(id);
  setAgentCommands([]);
  setChatUsage(null);
  live.sendChat({ t: "start", agent: id });
}

export function retryAgent() {
  live.sendChat({ t: "start", agent: chatAgent() ?? undefined });
}

export function newChat() {
  saveHistory();
  live.sendChat({ t: "new" });
  setChat("items", []);
  toolIds.clear();
  streaming = null;
  queue.length = 0;
}

export function clearView() {
  setChat("items", []);
  toolIds.clear();
  streaming = null;
}

export function stopChat() {
  queue.length = 0;
  setChat("items", produce((l) => { for (const it of l) if (it.kind === "user" && it.queued) it.queued = false; }));
  live.sendChat({ t: "cancel" });
}

/** Refuse every waiting permission request and stop the turn. */
export function denyAllAndStop() {
  setChat("items", produce((l) => { for (const it of l) if (it.kind === "permission" && !it.answered) it.answered = "Denied (stopped)"; }));
  stopChat();
}

export function setMode(id: string) {
  live.sendChat({ t: "set_mode", mode: id });
}

export function setModel(id: string) {
  live.sendChat({ t: "set_model", model: id });
}

export function setConfig(id: string, value: unknown) {
  live.sendChat({ t: "set_config", id, value });
}

export function answerPermission(itemId: string, reqId: number, optionId: string | null, label: string) {
  live.sendChat({ t: "permission_reply", id: reqId, option: optionId });
  patch(itemId, (it) => { it.answered = label; });
}

export function goToPage(page?: number, itemId?: string) {
  if (!c || !page) return;
  c.board.goto_page(page - 1);
  if (itemId) {
    try { c.board.select_json(JSON.stringify([itemId])); } catch { /* not selectable */ }
  }
  c.sync(true);
}

/** The page (or selection) as context, when "Attach this page" is on and no
 * explicit @page/@selection was given. */
async function autoContext(images: boolean, skip: boolean): Promise<{ ctx: CtxEntry[]; labels: string[] }> {
  if (!c || !attachPage() || skip) return { ctx: [], labels: [] };
  const { pageAttachment, selectionAttachment } = await import("./chatContext");
  let sel: any = null;
  try { sel = JSON.parse(c.board.live_apply(JSON.stringify({ op: "get_selection" }), "Teacher")); } catch { /* none */ }
  const n = sel?.items?.length ?? 0;
  const a = n ? selectionAttachment(n) : pageAttachment(c.board.page_index() + 1);
  try { return { ctx: await a.resolve(c, images), labels: [a.label] }; } catch { return { ctx: [], labels: [] }; }
}

function sendNow(text: string, ctx: CtxEntry[]) {
  live.sendChat({ t: "prompt", text, context: ctx, agent: chatAgent() ?? undefined });
}

function flushQueue() {
  const next = queue.shift();
  if (!next) return;
  patch(next.itemId, (it) => { it.queued = false; });
  streaming = null;
  sendNow(next.text, next.ctx);
}

/** Drop a queued message before it is sent. */
export function unqueue(itemId: string) {
  const i = queue.findIndex((q) => q.itemId === itemId);
  if (i >= 0) queue.splice(i, 1);
  setChat("items", (l) => l.filter((x) => x.id !== itemId));
}

/** Send a message with its attachments; /commands run here first. While the
 * agent works, the message waits in a queue and goes when the turn ends. */
export async function sendPrompt(text: string, attachments: Attachment[] = []): Promise<void> {
  let t = text.trim();
  if (!t || !c) return;
  if (!liveHasChat()) return;
  let forcePage = false;
  const slash = /^\/([a-z][\w-]*)\s*([\s\S]*)$/i.exec(t);
  if (slash) {
    const [, name, args] = slash;
    const isAgent = agentCommands().some((x) => x.name === name);
    if (!isAgent && RUPOK_COMMANDS.some((x) => x.name === name)) {
      const action = runRupokCommand(name, args);
      if ("local" in action) { action.local === "clear" ? clearView() : newChat(); return; }
      if ("error" in action) { note(action.error, "error"); return; }
      if ("solve" in action) {
        const api = await solveApi();
        if (api?.solve) { await api.solve({ prompt: action.solve || undefined, source: "chat" }); note("Opened Solve this."); return; }
        t = `Solve the problem on this page step by step${action.solve ? ` (${action.solve})` : ""}. Show the working on a new page after it, one step per build step.`;
        forcePage = true;
      } else {
        t = action.prompt;
        forcePage = !!action.attachPage;
      }
    }
  }
  sentTexts.push(text.trim());
  const images = !!(chatStatus().caps?.images ?? chatStatus().images);
  const explicit: CtxEntry[] = [];
  const labels: string[] = [];
  for (const a of attachments) {
    try { explicit.push(...(await a.resolve(c, images))); labels.push(a.label); } catch (e) { note(`Could not attach ${a.label}: ${String((e as Error)?.message ?? e)}`, "error"); }
  }
  const hasBoard = attachments.some((a) => a.kind === "page" || a.kind === "selection");
  const auto = await (forcePage && !attachPage()
    ? (async () => { const prev = attachPage(); setAttachPage(true); const r = await autoContext(images, hasBoard); setAttachPage(prev); return r; })()
    : autoContext(images, hasBoard));
  const ctx = [...explicit, ...auto.ctx];
  const allLabels = [...labels, ...auto.labels];
  lastSent = { text: t, ctx, labels: allLabels };
  const itemId = nid();
  if (chatBusy() || queue.length) {
    push({ kind: "user", id: itemId, text: text.trim(), context: allLabels, queued: true });
    queue.push({ itemId, text: t, ctx });
    return;
  }
  push({ kind: "user", id: itemId, text: text.trim(), context: allLabels });
  streaming = null;
  sendNow(t, ctx);
}

/** Send the last message again. */
export function retryLast() {
  if (!lastSent || chatBusy()) return;
  push({ kind: "user", id: nid(), text: lastSent.text, context: lastSent.labels });
  streaming = null;
  sendNow(lastSent.text, lastSent.ctx);
}

/** The n-th most recent message sent (↑ in an empty composer). */
export const recallSent = (n: number) => sentTexts[sentTexts.length - 1 - n] ?? null;

/** Put a sent message back in the composer to edit and resend. */
export function editMessage(text: string) {
  setComposerDraft({ text, n: Date.now() });
}

export function formatUsage(u: { used: number; size: number; cost?: { amount: number; currency: string } }): string {
  const k = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k` : String(n));
  const cost = u.cost && u.cost.amount > 0 ? ` · ${u.cost.currency === "USD" ? "$" : ""}${u.cost.amount.toFixed(u.cost.amount < 0.1 ? 3 : 2)}${u.cost.currency !== "USD" ? ` ${u.cost.currency}` : ""}` : "";
  return `${k(u.used)} / ${k(u.size)} tokens${cost}`;
}

export const SUGGESTIONS = [
  "Make a class on Ohm's law: 4 pages with a circuit simulation",
  "/quiz 5",
  "Turn my sketch into a clean labelled diagram",
  "Make 3 harder practice problems like this one on the next page",
  "/translate bn",
];
