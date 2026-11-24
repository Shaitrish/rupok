// "AI in action": the AI's work is visible, live and effortless in front of
// a class. See state.ts for the parts, and docs/ai-agents.md § "AI in action".

import type { Controller } from "../controller";
import { chatHooks } from "../chat";
import { live } from "../live";
import { attachProgress, initProgress, onOpFailed, onOpLanded, runShowProgress } from "./progress";
import { classAutoApprove, gateOp, noteAnswered, toastFor } from "./classMode";
import { initSolveStream, onStreamDone, onStreamUpdate, streamLog, streamed } from "./solveStream";
import { initStudent, routeStudentOp } from "./student";
import { aiPrefs, cards } from "./state";

/** Ops that change what the class sees but are not edits (no Undo toast). */
const NOT_EDITS = new Set(["go_to_page", "point_at", "show_progress", "snapshot"]);

let started = false;
export function initAiAction(c: Controller) {
  if (started) return;
  started = true;
  initProgress(c);
  initSolveStream(c);
  initStudent(c);
  attachProgress(chatHooks);
  chatHooks.update.add(onStreamUpdate);
  chatHooks.done.add(onStreamDone);
  chatHooks.autoApprove = classAutoApprove;
  chatHooks.answered.add(noteAnswered);
  live.handlers.set("show_progress", runShowProgress);
  live.gate = gateOp;
  live.studentRoute = routeStudentOp;
  live.onOpDone((e) => {
    onOpLanded(e.agent, e.op, e.result, e.page);
    const edit = !NOT_EDITS.has(e.op?.op) && !(e.op?.op === "set_board_theme" && !e.op.page_color && !e.op.pattern);
    if (edit && e.result?.status !== "preview" && e.result?.status !== "student_layer") toastFor(e.agent, e.op);
  });
  live.onOpFailed((e) => onOpFailed(e.agent, e.op, e.error, e.data));
  // For tests and the e2e harness.
  (window as any).rupokAiAction = { cards: () => cards.list, streamed, streamLog, prefs: aiPrefs, gateOp };
}
