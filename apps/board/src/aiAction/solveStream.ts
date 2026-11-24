// Solve this, live: the agent writes its solution in the chat inside a
// ```board block, one step per line, and the board writes each line in
// handwriting the moment it is complete, below the problem (and below the
// lines before it, in the same group), instead of waiting for the whole
// answer. Used when the teacher does not check solutions first, or in class
// mode. Each line is its own write; the toast's Undo takes back them all.

import type { Controller } from "../controller";
import { chatStatus } from "../chat";
import { solvePrefs } from "../solve/state";
import { landCard, patchCard, removeCard } from "./progress";
import { setAiToast } from "./state";

interface Stream {
  ids: string[];
  agent: string;
  card: string | null;
  buf: string;
  inside: boolean;
  group?: string;
  lines: number;
  page?: number;
  vv?: Uint8Array;
}

let c: Controller | null = null;
let s: Stream | null = null;
/** For tests and the e2e: the lines written so far in the current stream. */
export const streamed: string[] = [];
/** Where each streamed line went (for the e2e harness). */
export const streamLog: any[] = [];

export function initSolveStream(controller: Controller) {
  c = controller;
}

/** What the Solve this prompt asks for in streaming mode. */
export const STREAM_INSTRUCTIONS = [
  "2. Write the solution in your reply as you work it out, inside a fenced block that starts with a line ```board and ends with a line ```.",
  "   One short step per line (under about 30 characters), maths in $…$ (e.g. \"$2x = 4$\", \"$x = \\frac{4}{2}$\").",
  "   Rupok writes each line on the board in handwriting, right below the problem, the moment you finish the line, so check each step before you start the next one.",
  "   Don't copy the problem out again; finish with the answer on its own line. Do not call write_handwriting for it.",
].join("\n");

/** Start streaming the next agent reply's ```board lines below `ids`. */
export function startStream(ids: string[], card: string | null) {
  streamed.length = 0;
  streamLog.length = 0;
  s = { ids, agent: chatStatus().label ?? "AI", card, buf: "", inside: false, lines: 0 };
}

export const streaming = () => !!s;

function writeLine(line: string) {
  if (!c || !s) return;
  const p = solvePrefs.read();
  const op = {
    op: "write_handwriting",
    lines: [line],
    anchor: s.group ? { group: s.group } : s.ids.length ? { item_ids: s.ids } : undefined,
    style: p.style,
    speed: p.speed,
    step_mode: "all",
  };
  let r: any;
  try {
    r = JSON.parse(c.board.live_apply(JSON.stringify(op), s.agent));
  } catch (e) {
    console.warn("rupok: could not write a streamed line", e);
    return;
  }
  c.sync(true);
  void import("../solve").then((m) => m.watchReveal());
  streamed.push(line);
  streamLog.push({ line, page: r.page, bounds: r.bounds, note: r.note, anchor: op.anchor });
  s.lines++;
  s.vv = c.board.live_version();
  if (typeof r.group === "string") s.group = r.group;
  if (typeof r.page === "number") s.page = r.page;
  if (s.card) {
    const b = r.bounds;
    if (s.lines === 1 && b) landCard(s.card, { x: b.x, y: b.y, w: b.w, h: b.h }, r.page);
    s.card = s.lines === 1 ? null : s.card;
  }
}

function feed(text: string) {
  if (!s) return;
  s.buf += text;
  for (let nl = s.buf.indexOf("\n"); nl >= 0 && s; nl = s.buf.indexOf("\n")) {
    const line = s.buf.slice(0, nl).replace(/\r$/, "");
    s.buf = s.buf.slice(nl + 1);
    if (!s.inside) {
      if (/^\s*```+\s*board\b/i.test(line)) {
        s.inside = true;
        if (s.card) patchCard(s.card, (x) => { x.step = "Writing the solution…"; });
      }
    } else if (/^\s*```/.test(line)) {
      s.inside = false;
    } else if (line.trim()) {
      writeLine(line.trim());
    }
  }
}

function finish() {
  if (!s) return;
  if (s.inside && s.buf.trim() && !/^\s*```/.test(s.buf)) writeLine(s.buf.trim());
  const done = s;
  s = null;
  if (done.card) removeCard(done.card);
  if (!done.lines || !c) return;
  const n = done.lines;
  const vv = done.vv;
  const board = c.board;
  setAiToast({
    id: Date.now(), agent: done.agent, label: `wrote ${n} line${n > 1 ? "s" : ""} in handwriting`,
    undo: () => {
      // Only while those lines are still the newest change.
      const now = board.live_version();
      if (vv && now.length === vv.length && now.every((b, i) => b === vv[i])) for (let i = 0; i < n; i++) board.undo();
      setAiToast(null);
    },
  });
}

/** chatHooks.update / done. */
export function onStreamUpdate(u: any) {
  if (!s || u?.sessionUpdate !== "agent_message_chunk") return;
  const t = u.content?.type === "text" ? String(u.content.text ?? "") : "";
  if (t) feed(t);
}
export function onStreamDone() {
  finish();
}

/** Stop streaming (the teacher started another request). */
export function cancelStream() {
  s = null;
}
