// Class mode: in front of a class the AI should just do routine board work.
// - Permission requests for Rupok's board tools are approved at once, except
//   destructive ones (deleting, clearing, replacing the teacher's ink,
//   changing settings or the mode), which still ask.
// - Every AI edit shows "Claude added a Smart Object · Undo"; Undo takes back
//   exactly that step.
// - A destructive op that reaches the board without a fresh approval (an
//   agent in a no-ask mode, or an outside MCP agent) is confirmed on the
//   board first.
// On by default in Teach (Settings → AI & agents → AI in action).

import { live, opLabel } from "../live";
import { classModeOn, setAiToast, setConfirmReq } from "./state";
import { studentAskPending } from "./student";

/** Ops that remove or replace the teacher's work, or change settings or the mode. */
const DESTRUCTIVE = new Set([
  "delete_item", "delete_items", "delete_page", "remove_page", "clear_page", "clear_board",
  "replace_items_with_text", "replace_strokes_with_text",
  "set_setting", "set_settings", "set_prefs", "set_mode", "set_app_mode", "set_dock",
]);

/** Is this op destructive? The app theme (`set_board_theme` with `ui`) is a setting. */
export function isDestructiveOp(op: any): boolean {
  if (!op || typeof op.op !== "string") return false;
  if (DESTRUCTIVE.has(op.op)) return true;
  return op.op === "set_board_theme" && typeof op.ui === "string";
}

/** The Rupok tool an ACP permission request is about, from its title
 * ("mcp__rupok__add_text", "rupok: add_text") or input. */
export function toolOf(tc: any): string | null {
  const t = String(tc?.title ?? "").toLowerCase();
  const m = /(?:mcp__rupok__|rupok[:/ ._-]+)([a-z_]+)/.exec(t) ?? /\b([a-z]+(?:_[a-z0-9]+)+)\b/.exec(t);
  return m ? m[1] : null;
}

const isBoardTool = (tc: any) => /rupok|board/i.test(`${tc?.title ?? ""} ${JSON.stringify(tc?.rawInput ?? "")}`);

/** A request that sounds destructive even when the tool name is hidden
 * behind a friendly title ("Delete an item on your board"). */
function soundsDestructive(tc: any): boolean {
  const name = toolOf(tc);
  if (name && (DESTRUCTIVE.has(name) || (name === "set_board_theme" && tc?.rawInput?.ui))) return true;
  return /\b(delete|remove|clear|erase|wipe|replace|settings?|mode)\b/i.test(String(tc?.title ?? ""));
}

/** chatHooks.autoApprove: approve routine board edits in class mode. */
export function classAutoApprove(tc: any): string | null {
  // A student's ask in Student view: its edits can only reach their own layer.
  if (studentAskPending() && isBoardTool(tc) && !soundsDestructive(tc)) return "Approved automatically (a student's ask, on their own layer)";
  if (!classModeOn() || !isBoardTool(tc) || soundsDestructive(tc)) return null;
  return "Approved automatically (Class mode)";
}

// --------------------------------------------- approvals and confirmations

/** Destructive tools the teacher just approved in the chat (no second ask). */
const approved = new Map<string, number>();
export function noteAnswered(title: string, kind: string) {
  if (!kind.startsWith("allow")) return;
  const name = toolOf({ title });
  approved.set(name ?? title.toLowerCase(), Date.now() + (kind === "allow_always" ? 30 * 60_000 : 60_000));
}
function freshlyApproved(op: any): boolean {
  const t = approved.get(op.op);
  return !!t && t > Date.now();
}

let confirmSeq = 0;
/** live.gate: confirm a destructive op with the teacher in class mode. */
export async function gateOp(op: any, agent: string): Promise<void> {
  if (!classModeOn() || !isDestructiveOp(op) || freshlyApproved(op)) return;
  const ok = await new Promise<boolean>((resolve) => {
    const id = ++confirmSeq;
    const until = Date.now() + 25_000;
    const done = (v: boolean) => { setConfirmReq((r) => (r?.id === id ? null : r)); resolve(v); };
    setConfirmReq({ id, agent, label: describeOp(op), detail: op.page ? `Page ${op.page}` : undefined, answer: done, until });
    setTimeout(() => done(false), 25_000);
  });
  if (!ok) throw new Error("The teacher did not allow this. Do not try again unless they ask.");
}

/** "deleted an item", "changed a setting"… for the confirm card. */
export function describeOp(op: any): string {
  switch (op?.op) {
    case "delete_item": return "delete an item on this page";
    case "delete_page": case "remove_page": return `delete page ${op.page ?? ""}`.trim();
    case "clear_page": case "clear_board": return "clear this page";
    case "replace_items_with_text": return "replace your handwriting with typed text";
    case "set_board_theme": return `switch the app to ${op.ui} mode`;
    case "set_mode": case "set_app_mode": return `switch to ${op.mode ?? "another"} mode`;
    default: return op?.op?.startsWith("set_") ? "change your settings" : `run ${op?.op ?? "an action"}`;
  }
}

// ------------------------------------------------------------- Undo toast

let toastSeq = 0;
let toastTimer: number | undefined;
/** After an AI edit in class mode: "Claude added a Smart Object · Undo". */
export function toastFor(agent: string, op: any) {
  if (!classModeOn()) return;
  const label = opLabel(op).replace(/^is /, "");
  if (/reading|looking/.test(label)) return;
  const id = ++toastSeq;
  setAiToast({ id, agent, label, undo: () => { live.undoLast(); setAiToast(null); } });
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => setAiToast((t) => (t?.id === id ? null : t)), 6500);
}
