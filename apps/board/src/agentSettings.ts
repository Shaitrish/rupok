// Which settings the teacher's AI agent may read and change (get_settings /
// set_setting). An allowlist: anything not here is the teacher's alone.
// docs/agent-capabilities.md lists every setting with this decision (and the
// reason for "teacher only"); crates/rupok-cli/tests/agent_capabilities.rs
// fails CI when the two disagree or a new setting is in neither.
//
// Never add AI keys, the cloud account, payments, storage locations,
// microphone or permission switches here.

import { allSettings, settingById, type SettingDef } from "./settings";

export const AGENT_SETTINGS: readonly string[] = [
  // Appearance
  "appearance.theme", "appearance.accent", "appearance.density", "appearance.dockSide", "appearance.iconStyle",
  "appearance.pagesPanel", "appearance.pagesPinned",
  // Pen and ink
  "pen.style", "pen.markerOpacity", "pen.pressure", "pen.smoothing", "pen.hand", "pen.eraserMode", "pen.keepInkOnPage",
  "pen.smartShapes", "pen.scratchOut", "pen.stampQuickMenu", "pen.stampSounds",
  // Shapes after drawing
  "modes.shapeAfterTeach", "modes.shapeAfterPrepare",
  // Touch and gestures
  "touch.fingers", "touch.palmEraser", "touch.palmSize", "touch.eraserScale", "touch.handCluster", "touch.thumbScrub",
  "touch.swipe", "touch.swipeFingers", "touch.flick", "touch.swipeSensitivity", "touch.tapUndo", "touch.tapRedo",
  "touch.doubleTapFit", "touch.longPressDock", "touch.largeTargets", "touch.hints",
  // PDF defaults
  "files.pdfLayout", "files.pdfInk", "files.pdfPrintFriendly",
  // Voice (language and style only; the microphone and providers stay the teacher's)
  "general.voiceLang", "general.voicePunctuation", "general.voiceMode",
  // AI presentation (never permissions, keys or the preview check)
  "ai.solveStyle", "ai.solveSpeed", "ai.solveTeachStep", "ai.chatAttach", "ai.chatPosition",
  // Statistics overlay
  "about.stats",
];

type Simple = Extract<SettingDef, { type: "toggle" | "segmented" | "select" | "slider" | "colour" }>;
const SIMPLE = new Set(["toggle", "segmented", "select", "slider", "colour"]);
const isSimple = (s: SettingDef | undefined): s is Simple => !!s && SIMPLE.has(s.type);

function describe(s: Simple) {
  const base = { key: s.id, label: s.label, type: s.type, value: s.get() as unknown, group: s.category, description: s.description };
  switch (s.type) {
    case "toggle": return { ...base, allowed: [true, false] };
    case "slider": return { ...base, min: s.min, max: s.max, step: s.step };
    default: return { ...base, allowed: s.options.map((o) => o.value), labels: Object.fromEntries(s.options.map((o) => [o.value, o.label])) };
  }
}

/** The settings an agent may change, with values and what is allowed. */
export function agentSettings() {
  return allSettings().filter((s) => AGENT_SETTINGS.includes(s.id) && isSimple(s)).map((s) => describe(s as Simple));
}

/** Change one allowed setting; returns `{key, from, to}` and an undo. */
export function setAgentSetting(key: string, value: unknown): { result: { key: string; from: unknown; to: unknown }; undo: () => void } {
  if (!AGENT_SETTINGS.includes(key)) {
    throw new Error(`\`${key}\` cannot be changed by an agent; get_settings lists the settings that can (AI keys, accounts, payments, storage and permissions are the teacher's alone)`);
  }
  const s = settingById(key);
  if (!isSimple(s) || (s.when && !s.when())) throw new Error(`\`${key}\` is not available on this board`);
  if (s.enabled && !s.enabled()) throw new Error(`\`${key}\` is switched off by another setting right now`);
  const from = s.get();
  let to: unknown = value;
  switch (s.type) {
    case "toggle":
      if (typeof value !== "boolean") throw new Error(`${key} takes true or false`);
      (s.set as (v: boolean) => void)(value);
      break;
    case "slider": {
      const v = typeof value === "number" ? value : Number(value);
      if (!Number.isFinite(v) || v < s.min || v > s.max) throw new Error(`${key} takes a number from ${s.min} to ${s.max}`);
      to = Math.round(v / s.step) * s.step;
      (s.set as (v: number) => void)(to as number);
      break;
    }
    default: {
      const ok = s.options.map((o) => o.value);
      const v = typeof value === "string" ? (ok.find((o) => o.toLowerCase() === value.toLowerCase()) ?? s.options.find((o) => o.label.toLowerCase() === value.toLowerCase())?.value) : undefined;
      if (!v) throw new Error(`${key} takes one of: ${ok.join(", ")}`);
      to = v;
      (s.set as (v: string) => void)(v);
    }
  }
  const undo = () => (s.set as (v: unknown) => void)(from);
  return { result: { key, from, to }, undo };
}
