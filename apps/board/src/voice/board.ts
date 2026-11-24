// Voice on the board ("Voice text", the dock's mic-with-a-T tool).
//
// The teacher places a voice box on the slide and talks; the words appear in
// it while they speak (interim results lighter, final words in the ink
// colour), wrapped inside the box and the page. Finishing (tap the box, Esc,
// releasing a push-to-talk) turns the box into one normal text item: one
// undo step per box. In "Ask AI" mode the words become a prompt to the
// in-app chat instead, with the page or the selection attached.
//
// Speech comes from the shared dictation session (dictation.ts: offline
// Whisper when downloaded, else Web Speech, with the privacy line), spoken
// punctuation from punctuation.ts. A pause of about 2 s starts a new line.
// The UI is VoiceBoard.tsx; agents drive it with the live ops
// `start_voice_text` / `stop_voice_text` (onVoiceOp, called by live.ts).

import { batch, createMemo, createSignal } from "solid-js";
import { openAiPanel, sendPrompt, setComposerDraft } from "../chat";
import { pageAttachment, selectionAttachment } from "../chatContext";
import { controller } from "../controller";
import { liveHasChat } from "../live";
import { jsonStore } from "../settings/bind";
import { color, hex, isView, mode, pageIndex, pages, showToast, tool } from "../state";
import { inkForBoard } from "../dock";
import {
  type CustomTarget, activeLang, cancelDictation, isDictating, setDictationLang, startDictation, stopDictation,
  voiceChoice, voicePrefs, voiceStatus,
} from "./dictation";
import { joinDictation } from "./punctuation";
import type { VoiceLang } from "./types";
import { type LiveLine, WRAP_SLACK, boardMeasure, loadBoardFont, splitLines, tidyText, wrapLines } from "./wrap";

// ---------------------------------------------------------------- settings

export type VoiceBoxMode = "text" | "ask";
export type VoiceStyle = "typed" | "handwriting";

/** Settings → General → Voice on the board (settings/defs/voiceBoard.ts). */
export const voiceBoardPrefs = jsonStore("rupok.voiceBoard", {
  /** What a new voice box does: words on the board, or a prompt to the AI. */
  mode: "text" as VoiceBoxMode,
  /** "toggle": the box listens until finished (holding the dock button is
   * push-to-talk too). "hold": it listens only while the dock button, the
   * box's mic or Space is held. */
  talk: "toggle" as "toggle" | "hold",
  /** New boxes write in handwriting (English) instead of typed text. */
  style: "typed" as VoiceStyle,
  /** A pause this long (ms) starts a new line. 0 turns it off. */
  pauseMs: 2000,
  /** Let the voice box be used in Student view (Ask AI only: the student
   * view cannot write on the board). */
  student: false,
});

/** Text size of a voice box, world units (slide px): readable at the back. */
export const VOICE_TEXT_SIZE = 48;
const DEFAULT_W = 900;
const LINE = 1.25;
/** Inner padding of the live box (world units, matches voice-board.css). */
export const BOX_PAD = 8;

// ------------------------------------------------------------------- state

export interface VoiceBox {
  /** Page index the box is on. */
  page: number;
  /** World units (slide px): top-left and width; the height follows the text. */
  x: number;
  y: number;
  w: number;
  size: number;
  color: number;
  mode: VoiceBoxMode;
  style: VoiceStyle;
  /** Opened by a push-to-talk: finish on release, keep the current tool. */
  ptt: boolean;
}

const [box, setBox] = createSignal<VoiceBox | null>(null);
/** Final words in the box (with spoken punctuation and pause breaks). */
const [said, setSaid] = createSignal("");
/** The words being recognised now (replaced on every result). */
const [live, setLive] = createSignal("");
/** The box element (the dictation anchor: hint, focus). */
const [boxEl, setBoxEl] = createSignal<HTMLElement | null>(null);
/** "Sent to AI" confirmation: the prompt, shown for a moment. */
const [sent, setSent] = createSignal<{ text: string; at: number; ok: boolean } | null>(null);
/** The language chosen on the box while it is not listening. */
const [boxLang, setBoxLang] = createSignal<VoiceLang | null>(null);
const [pendingBreak, setPendingBreak] = createSignal(false);

export { box as voiceBox, boxEl as voiceBoxEl, live as voiceLive, said as voiceSaid, sent as voiceSent, setBoxEl as setVoiceBoxEl };

/** Where the last voice text went, so the next box can open under it. */
let lastPlace: { page: number; x: number; y: number; w: number; bottom: number } | null = null;

/** The language the box listens in. */
export const voiceLang = (): VoiceLang => (listening() ? activeLang() : boxLang() ?? activeLang() ?? voicePrefs.read().lang);

// ---------------------------------------------------------------- geometry

/** The current page's slide frame (null: an infinite board). */
export function pageFrame(): { w: number; h: number } | null {
  return pages()[pageIndex()]?.frame ?? null;
}

/** Fit a box onto the page: narrower than the page, inside it left to right
 * and top to bottom (as far as its height allows). */
export function clampBox(b: { x: number; y: number; w: number }, h: number, size: number, frame = pageFrame()): { x: number; y: number; w: number } {
  if (!frame) return b;
  const minW = Math.min(frame.w, Math.max(size * 5, 240));
  const w = Math.min(frame.w, Math.max(minW, b.w));
  const x = Math.min(Math.max(0, b.x), frame.w - w);
  const y = Math.min(Math.max(0, b.y), Math.max(0, frame.h - h));
  return { x, y, w };
}

/** Ink colour for new voice text: the pen colour, readable on this board. */
function inkColor(): number {
  const bg = pages()[pageIndex()]?.bg_color ?? 0xffffffff;
  return inkForBoard(color(), (bg >>> 8) < 0x808080);
}

/** The visible part of the board, in world units. */
function visibleWorld(): { x: number; y: number; w: number; h: number } {
  const [cx, cy, z] = controller.board.camera();
  const r = controller.root.getBoundingClientRect();
  const [top, right, bottom, left] = controller.insets;
  return { x: cx + left / z, y: cy + top / z, w: Math.max(1, r.width - left - right) / z, h: Math.max(1, r.height - top - bottom) / z };
}

/** Where a box opens when nobody said where: under the last voice text on
 * this page while there is room, else the middle of the visible page. */
function defaultPlace(size: number): { x: number; y: number; w: number } {
  const frame = pageFrame();
  if (lastPlace && lastPlace.page === pageIndex()) {
    const y = lastPlace.bottom + size * 0.5;
    if (!frame || y + size * 2 < frame.h) return { x: lastPlace.x, y, w: lastPlace.w };
  }
  const v = visibleWorld();
  const w = Math.min(DEFAULT_W, (frame?.w ?? v.w) * 0.6, v.w * 0.8);
  return { x: v.x + (v.w - w) / 2, y: v.y + v.h * 0.4, w };
}

// -------------------------------------------------------------- the text

/** Joined preview of the interim words after what was said. */
function preview(): string {
  const l = live().trim();
  if (!l) return "";
  const base = said() + (pendingBreak() && said() ? "\n" : "");
  const p = voicePrefs.read();
  const j = joinDictation(base, l, "", { lang: voiceLang(), autoPunctuation: p.autoPunctuation });
  return (pendingBreak() && said() ? "\n" : "") + j;
}

/** The box's font size after shrinking to keep a long text on the page. */
const fitted = createMemo(() => {
  const b = box();
  if (!b) return null;
  const frame = pageFrame();
  let size = b.size;
  let lines: LiveLine[] = [];
  for (let i = 0; i < 12; i++) {
    lines = splitLines(said(), preview(), b.w - 2 * BOX_PAD - WRAP_SLACK, boardMeasure(size));
    const h = Math.max(1, lines.length) * size * LINE + 2 * BOX_PAD;
    if (!frame || h <= frame.h || size <= 20) break;
    size = Math.max(20, size * 0.88);
  }
  const h = Math.max(1, lines.length) * size * LINE + 2 * BOX_PAD;
  const at = clampBox(b, h, size);
  return { ...at, h, size, lines };
});
export { fitted as voiceLayout };

// --------------------------------------------------------- the dictation

let heardAt = 0;
let inUtterance = false;
let wantListen = false;
let restarts: number[] = [];
let endWaiters: (() => void)[] = [];

const listening = () => isDictating(target);

function onWords() {
  const now = performance.now();
  const pause = voiceBoardPrefs.read().pauseMs;
  if (!inUtterance) {
    inUtterance = true;
    const s = said();
    if (pause > 0 && s && !s.endsWith("\n") && heardAt && now - heardAt >= pause) setPendingBreak(true);
  }
  heardAt = now;
}

const target: CustomTarget = {
  get element() { return boxEl(); },
  before: () => {
    // A final result with no interim before it starts an utterance here.
    onWords();
    const s = said();
    return pendingBreak() && s ? `${s}\n` : s;
  },
  interim: (t: string) => {
    if (t.trim()) onWords();
    setLive(t);
  },
  insert: (t: string) => {
    batch(() => {
      const brk = pendingBreak() && said() && !said().endsWith("\n");
      setSaid((s) => tidyText(s + (brk ? "\n" + t.replace(/^ +/, "") : t)));
      setPendingBreak(false);
      setLive("");
    });
    inUtterance = false;
    heardAt = performance.now();
  },
};

function onEnd(reason: "stopped" | "cancelled" | "error") {
  setLive("");
  inUtterance = false;
  const waiters = endWaiters;
  endWaiters = [];
  for (const w of waiters) w();
  // A recogniser that stops by itself (silence, a network hiccup) comes back
  // while the box still wants to listen, at most 3 times in 10 s.
  if (reason === "stopped" && wantListen && box()) {
    const now = performance.now();
    restarts = restarts.filter((t) => now - t < 10000);
    if (restarts.length < 3) {
      restarts.push(now);
      setTimeout(() => { if (wantListen && box() && voiceStatus() === "idle") begin(); }, 120);
      return;
    }
  }
  if (reason !== "cancelled") wantListen = false;
}

function begin(): boolean {
  const lang = boxLang() ?? undefined;
  return startDictation(target, { lang, anchor: boxEl(), bubble: false, onEnd });
}

/** Start listening in the open box. False when no speech provider works
 * here (the OS dictation hint is shown) or voice input is off. */
export function listen(): boolean {
  if (!box()) return false;
  if (listening()) return true;
  if (!voicePrefs.read().enabled) {
    showToast("Voice input is off in Settings › General › Voice input");
    return false;
  }
  wantListen = true;
  heardAt = 0;
  const ok = begin();
  if (!ok) wantListen = false;
  return ok;
}

/** Stop listening; the box stays open with its words. */
export function pauseListening() {
  wantListen = false;
  if (listening()) stopDictation();
}

export function toggleListening() {
  if (listening()) pauseListening();
  else listen();
}

export const voiceListening = listening;

/** Wait until the running session has delivered its last words. */
function untilEnded(ms = 3000): Promise<void> {
  if (voiceStatus() === "idle" || !listening()) return Promise.resolve();
  return new Promise((resolve) => {
    const t = setTimeout(resolve, ms);
    endWaiters.push(() => { clearTimeout(t); resolve(); });
  });
}

// ------------------------------------------------------------- open/close

export interface OpenOptions {
  x?: number; y?: number; w?: number;
  mode?: VoiceBoxMode; style?: VoiceStyle; lang?: VoiceLang;
  /** Push-to-talk: finish on release and keep the current tool. */
  ptt?: boolean;
  /** Start listening at once (default true). */
  listen?: boolean;
}

/** Open a voice box (finishing any open one first). Returns false when
 * voice text cannot be used here. */
export async function openVoiceBox(o: OpenOptions = {}): Promise<boolean> {
  if (box()) await finishVoiceBox();
  const p = voiceBoardPrefs.read();
  const m: VoiceBoxMode = isView() ? "ask" : o.mode ?? p.mode;
  if (isView() && !p.student) return false;
  const size = VOICE_TEXT_SIZE;
  const d = defaultPlace(size);
  const at = clampBox({ x: o.x ?? d.x, y: o.y ?? d.y, w: o.w ?? d.w }, size * LINE + 2 * BOX_PAD, size);
  if (o.lang) setBoxLang(o.lang);
  void loadBoardFont(size, "Aঅ");
  batch(() => {
    setSaid("");
    setLive("");
    setPendingBreak(false);
    setSent(null);
    setBox({ page: pageIndex(), ...at, size, color: inkColor(), mode: m, style: o.style ?? p.style, ptt: !!o.ptt });
  });
  restarts = [];
  if (o.listen === false) return true;
  // The box element is mounted by now (the OS dictation hint anchors to it).
  await Promise.resolve();
  listen();
  return true;
}

/** Change the open box (drag, resize, the mode and style toggles). */
export function updateVoiceBox(patch: Partial<Pick<VoiceBox, "x" | "y" | "w" | "mode" | "style">>) {
  const b = box();
  if (!b) return;
  if (isView() && patch.mode === "text") return;
  setBox({ ...b, ...patch });
}

/** Switch the box's language (EN / বাং): restarts a running session. */
export function setVoiceLang(lang: VoiceLang) {
  setBoxLang(lang);
  if (listening()) setDictationLang(lang);
}

export interface FinishResult {
  text: string;
  /** The new text item, or the first handwriting stroke group. */
  item_id?: string;
  kind: "text" | "handwriting" | "prompt" | "none";
  note?: string;
}

let finishing: Promise<FinishResult> | null = null;

/** Finish the box: its words become a text item (one undo step), handwriting,
 * or a prompt to the AI; `cancel` throws them away. */
export function finishVoiceBox(o: { cancel?: boolean } = {}): Promise<FinishResult> {
  if (finishing) return finishing;
  finishing = (async () => {
    const b = box();
    if (!b) return { text: "", kind: "none" as const };
    wantListen = false;
    if (o.cancel) cancelDictation();
    else if (listening()) {
      stopDictation();
      await untilEnded();
    }
    const layout = fitted();
    const text = tidyText(said()).trim();
    batch(() => {
      setBox(null);
      setSaid("");
      setLive("");
      setPendingBreak(false);
    });
    if (o.cancel || !text || !layout) return { text: o.cancel ? "" : text, kind: "none" as const };
    if (b.mode === "ask") return sendToAi(text);
    // The words go on the page being shown.
    const w = layout.w - 2 * BOX_PAD;
    const x = layout.x + BOX_PAD, y = layout.y + BOX_PAD;
    lastPlace = { page: pageIndex(), x: layout.x, y: layout.y, w: layout.w, bottom: layout.y + layout.h };
    if (b.style === "handwriting") {
      const r = writeHandwriting(text, x, y, w, layout.size, b.color);
      if (r) return r;
    }
    await loadBoardFont(layout.size, text);
    const lines = wrapLines(text, w - WRAP_SLACK, boardMeasure(layout.size));
    const id = controller.board.add_text_box(x, y, w, lines.join("\n"), layout.size, b.color) ?? undefined;
    controller.sync(true);
    return { text, item_id: id, kind: "text" as const };
  })();
  const p = finishing;
  void p.finally(() => { finishing = null; });
  return p;
}

/** Handwriting style: the synthesiser that "Solve this" uses writes the lines
 * (English; Bangla has no handwriting glyphs yet, so it stays typed). */
function writeHandwriting(text: string, x: number, y: number, w: number, size: number, rgba: number): FinishResult | null {
  if (/[ঀ-৿]/.test(text)) {
    showToast("Handwriting is English only for now: written as typed text");
    return null;
  }
  // The synthesiser's size is a capital's height; the line advance is ~2×.
  const cap = size * 0.72;
  const lines = wrapLines(text, w, (s) => s.length * cap * 0.62).slice(0, 40).map((l) => l.slice(0, 200));
  try {
    const r = JSON.parse(controller.board.live_apply(JSON.stringify({
      op: "write_handwriting", lines, anchor: { x, y }, size: cap, color: hex(rgba), style: "teacher", step_mode: "all",
    }), "Voice text"));
    controller.sync(true);
    void import("../solve").then((m) => m.watchReveal());
    return { text, item_id: r.item_ids?.[0], kind: "handwriting", note: r.note };
  } catch (e) {
    console.warn("rupok: voice handwriting failed, writing typed text", e);
    return null;
  }
}

/** Ask AI: the words go to the in-app chat with the page (or the selection)
 * attached. Teach keeps the chat closed; a small bubble confirms. */
async function sendToAi(text: string): Promise<FinishResult> {
  if (!liveHasChat()) {
    // Not connected yet: open the AI panel with the words in the composer.
    setComposerDraft({ text, n: Date.now() });
    openAiPanel();
    setSent({ text, at: Date.now(), ok: false });
    clearSentSoon();
    return { text, kind: "prompt", note: "the AI chat is not connected; the prompt is waiting in the chat box" };
  }
  const sel = controller.board.selection_json();
  const n = (JSON.parse(sel || "[]") as string[]).length;
  const att = n ? selectionAttachment(n) : pageAttachment(controller.board.page_index() + 1);
  await sendPrompt(text, [att]);
  if (mode() !== "teach") openAiPanel();
  setSent({ text, at: Date.now(), ok: true });
  clearSentSoon();
  return { text, kind: "prompt" };
}

let sentTimer = 0;
function clearSentSoon() {
  clearTimeout(sentTimer);
  sentTimer = window.setTimeout(() => setSent(null), 3200);
}

// ------------------------------------------------------- the dock button

const HOLD_MS = 350;
let pressTimer = 0;
let pressed = false;
let held = false;

/** Pointer down on the dock's Voice text button: a hold is push-to-talk. */
export function dockPress(e: PointerEvent) {
  if (e.button !== 0) return;
  pressed = true;
  held = false;
  clearTimeout(pressTimer);
  pressTimer = window.setTimeout(() => {
    if (!pressed) return;
    held = true;
    const b = box();
    if (b) { if (!listening()) listen(); }
    else void openVoiceBox({ ptt: true });
  }, HOLD_MS);
  (e.currentTarget as HTMLElement | null)?.setPointerCapture?.(e.pointerId);
}

/** Pointer up: a push-to-talk ends, its words go on the board. */
export function dockRelease() {
  clearTimeout(pressTimer);
  if (!pressed) return;
  pressed = false;
  if (!held) return;
  const b = box();
  // Push-to-talk boxes finish on release; a toggle box just stops listening.
  if (b?.ptt || voiceBoardPrefs.read().talk === "hold") void finishVoiceBox();
  else pauseListening();
}

/** The click that ends a hold is not a tap. */
export function dockClickConsumed(): boolean {
  if (held) { held = false; return true; }
  return false;
}

/** A tap on the tool: picking it (VoiceBoard shows where to place a box). */
export function pickVoiceTool() {
  if (tool() === "voice" && box()) { void finishVoiceBox(); return; }
  controller.setTool("voice");
}

/** Leaving the tool finishes a box it opened (a push-to-talk box stays). */
export function onToolChange(t: string) {
  const b = box();
  if (b && !b.ptt && t !== "voice") void finishVoiceBox();
}

// ---------------------------------------------------------- agents (live)

const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
const LANGS: Record<string, VoiceLang> = { en: "en-US", "en-us": "en-US", "en-gb": "en-GB", "en-in": "en-IN", bn: "bn-BD", "bn-bd": "bn-BD", "bn-in": "bn-IN" };

/** live.ts: an agent's `start_voice_text` / `stop_voice_text`. */
export async function onVoiceOp(op: any): Promise<any> {
  if (op.op === "stop_voice_text") {
    if (!box()) return { status: "idle", text: "", note: "no voice box was open" };
    const r = await finishVoiceBox({ cancel: op.cancel === true });
    return { status: op.cancel === true ? "cancelled" : "done", ...r };
  }
  if (!voicePrefs.read().enabled) throw new Error("Voice input is turned off on this board (Settings › General › Voice input).");
  if (voiceChoice() === "hint") throw new Error("This board has no speech recognition (use Chrome or Edge, or the desktop app with the offline voice model).");
  const modeArg = op.mode === "ask" ? "ask" : op.mode == null || op.mode === "text" ? undefined : null;
  if (modeArg === null) throw new Error(`mode must be "text" or "ask", not ${JSON.stringify(op.mode)}`);
  const style = op.style === "handwriting" ? "handwriting" : op.style == null || op.style === "typed" ? undefined : null;
  if (style === null) throw new Error(`style must be "typed" or "handwriting", not ${JSON.stringify(op.style)}`);
  const lang = op.lang == null ? undefined : LANGS[String(op.lang).toLowerCase()];
  if (op.lang != null && !lang) throw new Error(`lang must be en-US or bn-BD (also en-GB, en-IN, bn-IN), not ${JSON.stringify(op.lang)}`);
  const w = num(op.w);
  if (w !== undefined && w <= 0) throw new Error("w must be positive");
  const ok = await openVoiceBox({ x: num(op.x), y: num(op.y), w, mode: modeArg ?? undefined, style: style ?? undefined, lang, ptt: false });
  if (!ok) throw new Error("Voice text is not available in this mode.");
  const b = fitted();
  return {
    status: listening() ? "listening" : "open",
    page: pageIndex() + 1,
    box: b ? { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.w) } : null,
    mode: box()?.mode, style: box()?.style, lang: voiceLang(), provider: voiceChoice(),
    note: "The teacher's words appear in the box as they speak; call stop_voice_text to put them on the board.",
  };
}

/** For live.ts: is this op a voice op? */
export const isVoiceOp = (op: any) => op?.op === "start_voice_text" || op?.op === "stop_voice_text";

/** Test hook (e2e): the state at a glance. */
if (typeof window !== "undefined") {
  (window as any).__rupokVoiceBoard = { box, said, live, listening, finish: finishVoiceBox, open: openVoiceBox, layout: fitted, op: onVoiceOp };
}
