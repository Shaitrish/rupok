// Solve this: the teacher (or a student, when allowed) lassos a problem on
// the board and asks the AI to solve it. The board sends the AI a picture of
// the problem plus what the offline maths recogniser read (n-best LaTeX), and
// a prompt asking for a checked, step-by-step solution through the
// `write_handwriting` tool. The board then writes the solution below the
// problem in handwriting, stroke by stroke (engine/reveal.rs).
//
// - Preview (on by default, "Check before showing the class"): the agent's
//   `write_handwriting` call is held here and shown to the teacher; it goes
//   on the board when they tap "Write it on the board".
// - Teach: with step-by-step reveal, each Next (clicker, arrow key, swipe)
//   writes the next line.
// - One undo removes the whole solution; it is attributed to the agent.
//
// Entry points: the selection toolbar's "Solve this", the AI chat's /solve,
// the Teach chip's "Solve what I select", and `rupokSolve.solve()` in tests.

import { createEffect, createRoot, on } from "solid-js";
import { render } from "solid-js/web";
import { controller } from "../controller";
import { MathInk } from "../wasm/rupok.js";
import { live, liveHasChat } from "../live";
import { chatBusy, chatOpen, openAiPanel, setChat } from "../chat";
import { renderSnapshot } from "../snapshot";
import { isView, mode, selectionIds, showToast } from "../state";
import {
  type PendingSolution, glyphs, pending, reveal, setCalibrateOpen, setPending, setReveal, setSolving, solvePrefs, solving,
} from "./state";
import { SolveLayer } from "./SolveUI";

const c = controller;
let nextId = 0;

// ---------------------------------------------------------------- glyphs

/** Give the teacher's handwriting to the writer (wasm) and the reader. */
function pushGlyphs() {
  try { c.board.set_handwriting_glyphs(JSON.stringify(glyphs())); } catch (e) { console.warn("rupok: handwriting glyphs", e); }
  reader = null; // rebuilt with the new templates on next use
}

let reader: MathInk | null = null;
/** The offline maths recogniser, taught the teacher's own forms. */
function mathReader(): MathInk {
  if (reader) return reader;
  const r = new MathInk();
  for (const [ch, samples] of Object.entries(glyphs())) {
    for (const s of samples) {
      const counts = new Uint32Array(s.map((st) => st.length / 2));
      const xy = new Float32Array(s.flat());
      try { r.add_user_template(ch, xy, counts); } catch { /* a symbol the reader does not know */ }
    }
  }
  reader = r;
  return r;
}

/** What the recogniser reads in the selected ink: n-best LaTeX. */
export function readSelection(): { latex: string; confidence: number }[] {
  let ink: any = null;
  try { ink = JSON.parse(c.board.selection_ink_json()); } catch { return []; }
  if (!ink?.strokes?.length) return [];
  const xy: number[] = [];
  const counts: number[] = [];
  for (const s of ink.strokes as { pts: number[] }[]) {
    let n = 0;
    for (let i = 0; i + 1 < s.pts.length; i += 3) { xy.push(s.pts[i], s.pts[i + 1]); n++; }
    counts.push(n);
  }
  if (counts.length > 160) return [];
  try {
    const cands = JSON.parse(mathReader().recognize_json(new Float32Array(xy), new Uint32Array(counts)));
    return (cands as any[]).slice(0, 4).map((x) => ({ latex: String(x.latex), confidence: Math.round(Number(x.confidence) * 100) / 100 }));
  } catch {
    return [];
  }
}

// ---------------------------------------------------------------- prompt

/** The prompt sent with a Solve this request. */
export function solvePrompt(o: { ids: string[]; page: number; readings: { latex: string; confidence: number }[]; preview: boolean; extra?: string }): string {
  const read = o.readings.length
    ? o.readings.map((r, i) => `${i + 1}. $${r.latex}$ (confidence ${r.confidence.toFixed(2)})`).join("\n")
    : "(the recogniser could not read it: use the picture)";
  return [
    `Solve this problem from my board, step by step, for my class.${o.extra ? ` ${o.extra}` : ""}`,
    "",
    `The picture shows what I selected on page ${o.page} (item ids: ${o.ids.slice(0, 60).join(", ")}${o.ids.length > 60 ? ", …" : ""}).`,
    "Rupok's offline handwriting recogniser read it as (best guess first; check against the picture):",
    read,
    "",
    "Please:",
    "1. Work out the solution, then check every step and all the arithmetic before writing anything.",
    `2. Call write_handwriting once, with anchor {"item_ids": [the ids above]} so it goes right below the problem. One short step per line (under about 30 characters), maths in $…$ (e.g. "$2x = 4$", "$x = \\frac{4}{2}$"). Don't copy the problem out again; finish with the answer on its own line.`,
    "3. Keep to the level of the problem: the method a school student would use, no shortcuts they haven't met.",
    "4. While you explain, you may use point_at to circle the part of the board you are talking about.",
    "5. Then explain the steps in this chat in 2 to 4 short sentences.",
    o.preview
      ? "The teacher checks the solution before the class sees it: when write_handwriting answers status \"preview\", it is waiting for their approval. Do not call it again, and never write the solution any other way."
      : "It is written on the board straight away, so be sure it is right first.",
  ].join("\n");
}

// ---------------------------------------------------------------- flow

const allowed = () => !isView() || solvePrefs.read().students;

/** Wait until the chat relay is connected (the teacher may be pairing). */
function waitForChat(ms: number): Promise<boolean> {
  if (liveHasChat()) return Promise.resolve(true);
  return new Promise((resolve) => {
    createRoot((dispose) => {
      const t = setTimeout(() => { dispose(); resolve(false); }, ms);
      createEffect(on(liveHasChat, (ok) => { if (ok) { clearTimeout(t); dispose(); resolve(true); } }));
    });
  });
}

let armed = false;
/** "Solve what I select": switch to the lasso; the next selection is solved. */
export function armSolve() {
  if (!allowed()) return showToast("Ask your teacher to turn on Solve this for students.");
  armed = true;
  c.setTool("lasso");
  showToast("Lasso the problem to solve.", 3200);
}

/** Solve the selected problem (or arm the lasso when nothing is selected). */
export async function solve(opts: { prompt?: string; source?: string } = {}): Promise<boolean> {
  if (!allowed()) { showToast("Ask your teacher to turn on Solve this for students."); return false; }
  const ids = selectionIds();
  if (!ids.length) { armSolve(); return false; }
  if (solving()) return false;
  setSolving(true);
  try {
    const page = c.board.page_index() + 1;
    const readings = readSelection();
    const preview = solvePrefs.read().preview || isView();
    const text = solvePrompt({ ids, page, readings, preview, extra: opts.prompt });
    // Context: the selection as JSON, and a picture of it.
    const ctx: any[] = [];
    try {
      const sel = c.board.live_apply(JSON.stringify({ op: "get_selection" }), "Teacher");
      ctx.push({ kind: "selection", uri: `rupok://board/page/${page}/selection`, text: sel });
    } catch { /* read failed */ }
    try {
      const snap = await renderSnapshot(c, { page, item_ids: ids, width: 1024 }, "Teacher");
      ctx.push({ kind: "image", data: snap.png });
    } catch { /* no picture */ }
    if (!chatOpen()) openAiPanel();
    if (!(await waitForChat(90_000))) {
      showToast("Connect your AI assistant to solve problems (the AI button, top right).", 4200);
      return false;
    }
    if (chatBusy()) {
      showToast("The assistant is still busy. Try again when it has finished.");
      return false;
    }
    const label = readings[0] ? `Solve this: ${readings[0].latex}` : "Solve this";
    setChat("items", (l) => [...l, { kind: "user", id: `solve${++nextId}`, text: label, context: [`Selection (${ids.length})`] } as any]);
    live.sendChat({ t: "prompt", text, context: ctx });
    lastProblem = ids.slice();
    return true;
  } finally {
    setSolving(false);
  }
}

let lastProblem: string[] = [];

// ------------------------------------------------- write_handwriting ops

/** Fill the board's defaults into an agent's `write_handwriting` op. */
function withDefaults(op: any): any {
  const p = solvePrefs.read();
  const out = { ...op };
  if (out.style == null) out.style = p.style;
  if (out.speed == null) out.speed = p.speed;
  if (out.step_mode == null) out.step_mode = mode() === "teach" && p.teachStep ? "one-line-per-click" : "all";
  if (out.anchor == null && lastProblem.length && !selectionIds().length) out.anchor = { item_ids: lastProblem };
  return out;
}

/** Called by live.ts for an agent's `write_handwriting`: hold it for the
 * teacher's check (preview), or write it now. Returns the agent's result. */
export function onWriteOp(op: any, agent: string): any {
  const full = withDefaults(op);
  const lines: string[] = Array.isArray(op.lines) ? op.lines.map(String) : [];
  if (!lines.some((l) => l.trim())) throw new Error("lines must contain at least one line to write");
  if (solvePrefs.read().preview) {
    const item: PendingSolution = { id: ++nextId, agent, op: full, lines, at: Date.now() };
    setPending((l) => [...l, item]);
    return {
      status: "preview",
      lines: lines.length,
      note: "The teacher is checking your solution before the class sees it; it is written on the board when they approve. Do not call write_handwriting again for it. Explain the steps in the chat now.",
    };
  }
  return writeNow(full, agent);
}

function writeNow(op: any, agent: string): any {
  let result: any;
  try {
    result = JSON.parse(c.board.live_apply(JSON.stringify({ ...op, op: "write_handwriting" }), agent));
  } catch (e) {
    let body: any;
    try { body = JSON.parse(String(e)); } catch { body = { error: String((e as any)?.message ?? e) }; }
    throw new Error(body.error ?? "the board could not write that");
  }
  c.sync(true);
  watchReveal();
  return result;
}

/** "Write it on the board": the teacher approved a held solution. */
export function approve(id: number) {
  const p = pending().find((x) => x.id === id);
  if (!p) return;
  setPending((l) => l.filter((x) => x.id !== id));
  try {
    const r = writeNow(p.op, p.agent);
    if (r?.note && /new page/.test(r.note)) showToast("No room here: the solution continues on a new page.");
  } catch (e: any) {
    showToast(String(e?.message ?? e));
  }
}

/** The teacher wants a different solution: discard this one and tell the agent why. */
export function askChange(id: number, note: string) {
  const t = note.trim();
  if (!t) return;
  discard(id);
  if (!liveHasChat()) return showToast("The assistant is not connected.");
  if (chatBusy()) return showToast("The assistant is still busy. Try again when it has finished.");
  setChat("items", (l) => [...l, { kind: "user", id: `solve${++nextId}`, text: t, context: [] } as any]);
  live.sendChat({
    t: "prompt",
    text: `I did not write that solution on the board. Please change it: ${t}\nThen call write_handwriting again with the new lines (same anchor).`,
    context: [],
  });
}

export function discard(id: number) {
  setPending((l) => l.filter((x) => x.id !== id));
}

// --------------------------------------------------------------- reveal

let raf = 0;
/** Follow the engine's reveal (for the chip and Next). */
export function watchReveal() {
  cancelAnimationFrame(raf);
  const step = () => {
    const s = Array.from(c.board.reveal_state());
    setReveal(s.length ? s : null);
    if (s.length) raf = requestAnimationFrame(step);
  };
  step();
}

/** Presenter Next while writing: next line (step mode) or show all. */
export function revealNext(): boolean {
  const s = reveal();
  if (!s) return false;
  const [, , , gate, lines] = s;
  if (gate < lines) c.board.reveal_next();
  else c.board.reveal_finish();
  watchReveal();
  return true;
}

export function revealAll() {
  c.board.reveal_finish();
  watchReveal();
}

// ----------------------------------------------------------------- init

let started = false;
export function initSolve() {
  if (started) return;
  started = true;
  pushGlyphs();
  createRoot(() => {
    createEffect(on(glyphs, pushGlyphs, { defer: true }));
    // "Solve what I select": solve as soon as something is lassoed.
    createEffect(on(selectionIds, (ids) => {
      if (armed && ids.length) {
        armed = false;
        // Let the lasso gesture finish first.
        setTimeout(() => void solve(), 250);
      }
    }, { defer: true }));
  });
  c.nextHooks.push(revealNext);
  const host = document.createElement("div");
  host.className = "solve-host";
  document.body.appendChild(host);
  render(SolveLayer, host);
  (window as any).rupokSolve = { solve, armSolve, approve, discard, revealNext, revealAll, pending, reveal, readSelection, solvePrompt, prefs: solvePrefs, calibrate: () => setCalibrateOpen(true) };
}
