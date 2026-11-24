// Students ask AI: in Student view, when the teacher allows it (Settings →
// AI & agents → AI in action → "Students can ask AI", off by default), a
// student asks the connected assistant to show them something ("a 3D model
// of a water molecule"). What the AI makes lands on the student's own layer:
// shown over the page, only on this screen, never written into the
// teacher's lesson, and gone when they leave Student view. Asks are limited
// per student (per Student view session) with a visible count.
//
// The assistant is the one connected to this board (the teacher's, on a
// classroom board, or the student's own Rupok and agent at home). A future
// Rupok AI would plug in the same way. Without one, the box explains how.

import { createRoot, createSignal, createEffect, on } from "solid-js";
import type { Controller } from "../controller";
import { live, liveHasChat } from "../live";
import { notePrompt } from "../chat";
import { isView } from "../state";
import { activeCard, celebrateAt, defaultRect, landCard } from "./progress";
import { aiPrefs, type Rect } from "./state";

export interface LayerItem { id: string; kind: "object" | "text"; title: string; html?: string; lines?: string[]; rect: Rect; page: number }

export const [studentLayer, setStudentLayer] = createSignal<LayerItem[]>([]);
export const [askOpen, setAskOpen] = createSignal(false);
const [asks, setAsks] = createSignal<number[]>(loadAsks());
const WINDOW_MS = 10 * 60_000;
const KEY = "rupok.studentAsks";
let pendingSince = 0;
let seq = 0;
let c: Controller | null = null;

function loadAsks(): number[] {
  try { return (JSON.parse(sessionStorage.getItem(KEY) || "[]") as number[]).filter((t) => Date.now() - t < WINDOW_MS); } catch { return []; }
}

export const studentsMayAsk = () => isView() && aiPrefs.read().studentsAsk;
/** Asks left in the current 10 minutes, and when the next one frees up. */
export function asksLeft(): { left: number; limit: number; resetMin: number } {
  const limit = Math.max(1, Math.min(20, aiPrefs.read().studentLimit || 3));
  const recent = asks().filter((t) => Date.now() - t < WINDOW_MS);
  const oldest = recent[0];
  return { left: Math.max(0, limit - recent.length), limit, resetMin: oldest ? Math.ceil((WINDOW_MS - (Date.now() - oldest)) / 60_000) : 0 };
}

export function studentPrompt(q: string, page: number): string {
  return [
    `A student in my class asks, on page ${page} of the lesson: "${q.replace(/"/g, "'").slice(0, 300)}"`,
    "",
    "Help them see it. Make ONE thing, quickly, without asking questions:",
    "- preferably a Smart Object (add_smart_object with path \"live\", about 1280×720, themed, no network) that shows it, interactive if that helps;",
    "- or, for a short worked answer, write_handwriting with a few short lines.",
    "Call show_progress first (kind \"object\" or \"handwriting\") so the student sees you working.",
    "The board is in Student view: what you add goes to the student's own layer only, not to the teacher's lesson, so don't add pages or change anything else.",
    "Keep it accurate, friendly and at a school student's level. Then say in one sentence what you made.",
  ].join("\n");
}

/** Send a student's question. "noagent" / "limit" / "busy" when it cannot go. */
export function studentAsk(q: string): "sent" | "noagent" | "limit" | "empty" {
  const text = q.trim();
  if (!text) return "empty";
  if (!liveHasChat()) return "noagent";
  if (asksLeft().left <= 0) return "limit";
  const next = [...asks().filter((t) => Date.now() - t < WINDOW_MS), Date.now()];
  setAsks(next);
  try { sessionStorage.setItem(KEY, JSON.stringify(next)); } catch { /* storage off */ }
  pendingSince = Date.now();
  const page = (c?.board.page_index() ?? 0) + 1;
  const prompt = studentPrompt(text, page);
  live.sendChat({ t: "prompt", text: prompt, context: [] });
  notePrompt(text, "student");
  return "sent";
}

/** live.studentRoute: in Student view, a write from the AI answering a
 * student's ask goes to their layer. Anything else is refused as usual. */
export function routeStudentOp(op: any, _agent: string): any | undefined {
  if (!isView() || !pendingSince || Date.now() - pendingSince > 5 * 60_000) return undefined;
  const page = (c?.board.page_index() ?? 0) + 1;
  const num = (v: unknown, d: number) => (typeof v === "number" && Number.isFinite(v) ? v : d);
  let item: LayerItem | null = null;
  if (op.op === "add_smart_object" && typeof op.html === "string") {
    if (op.html.length > 2 * 1024 * 1024) throw new Error("the Smart Object is too large");
    const d = defaultRect("object");
    const w = num(op.width, d.w), h = num(op.height, d.h);
    item = { id: `s${++seq}`, kind: "object", title: String(op.title ?? "For you"), html: op.html, page, rect: { x: num(op.x, (1920 - w) / 2), y: num(op.y, (1080 - h) / 2), w, h } };
  } else if (op.op === "write_handwriting" || op.op === "add_text") {
    const lines: string[] = Array.isArray(op.lines) ? op.lines.map(String) : String(op.text ?? "").split("\n");
    if (!lines.some((l) => l.trim())) throw new Error("nothing to show");
    const d = defaultRect("other");
    item = { id: `s${++seq}`, kind: "text", title: "For you", lines: lines.slice(0, 40), page, rect: { x: num(op.x, d.x), y: num(op.y, d.y), w: d.w, h: d.h } };
  }
  if (!item) return undefined;
  setStudentLayer((l) => [...l, item!].slice(-4));
  const card = activeCard(["student"]);
  if (card) landCard(card.id, item.rect, page);
  else celebrateAt(item.rect, page);
  return {
    status: "student_layer", item_id: item.id, page,
    note: "Shown to the student only, on their own layer over the page. It is not saved in the teacher's lesson.",
  };
}

/** A student's ask is being answered (its board edits go to their layer). */
export const studentAskPending = () => isView() && !!pendingSince && Date.now() - pendingSince < 5 * 60_000;

export function clearStudentLayer() {
  setStudentLayer([]);
}

export function initStudent(controller: Controller) {
  c = controller;
  createRoot(() => {
    // Leaving Student view clears the student's layer and their question.
    createEffect(on(isView, (v) => { if (!v) { clearStudentLayer(); setAskOpen(false); pendingSince = 0; } }, { defer: true }));
  });
}
