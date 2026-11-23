// Settings → AI & agents → Solve this: AI that solves a problem on the board
// and writes the solution in handwriting (solve/).

import { createEffect } from "solid-js";
import type { SettingProps } from "../registry";
import { registerSettings } from "../registry";
import { setCalibrateOpen, solvePrefs, taughtCount } from "../../solve/state";

const d = solvePrefs.defaults;
const GROUP = "Solve this";

function CalibrateRow(props: SettingProps) {
  // Plain DOM keeps this file .ts (no JSX); the status follows the samples.
  const wrap = document.createElement("div");
  wrap.className = "st-inline";
  const status = document.createElement("span");
  status.className = "st-status";
  status.id = props.descId + "-n";
  const btn = document.createElement("button");
  btn.className = "st-btn";
  btn.setAttribute("aria-labelledby", props.labelId);
  btn.setAttribute("aria-describedby", props.descId);
  const sync = () => {
    const n = taughtCount();
    status.textContent = n ? `${n} characters taught` : "Not taught yet";
    btn.textContent = n ? "Write them again" : "Start";
  };
  createEffect(sync);
  btn.onclick = () => {
    void import("../../solve").then((m) => m.initSolve());
    setCalibrateOpen(true);
  };
  wrap.append(status, btn);
  return wrap;
}

registerSettings([
  {
    id: "ai.solveStyle", category: "ai", group: GROUP, order: 60, type: "segmented", label: "AI handwriting",
    description: "Lasso a problem and tap Solve this: your AI writes the solution on the board. In your own hand once you have taught it, else a neat hand.",
    keywords: ["solve", "solution", "handwriting", "write", "font", "style", "সমাধান", "হাতের লেখা"],
    options: [
      { value: "teacher", label: "My handwriting" },
      { value: "neat", label: "Neat" },
    ],
    get: () => solvePrefs.read().style,
    set: (v) => solvePrefs.write({ style: v as "teacher" | "neat" }),
    default: d.style,
  },
  {
    id: "ai.solveCalibrate", category: "ai", group: GROUP, order: 61, type: "custom", label: "Teach Rupok my handwriting",
    description: "Write the digits, common letters and maths symbols once. Rupok writes solutions in your hand and reads your maths better. Kept on this device.",
    keywords: ["calibrate", "calibration", "my handwriting", "train", "samples", "ক্যালিব্রেট"],
    component: CalibrateRow,
  },
  {
    id: "ai.solveSpeed", category: "ai", group: GROUP, order: 62, type: "slider", label: "Writing speed",
    description: "How fast the AI writes on the board. 1× is brisk board writing.",
    keywords: ["speed", "animation", "fast", "slow", "pace"],
    min: 0.5, max: 3, step: 0.25, format: (v) => `${v}×`,
    get: () => solvePrefs.read().speed,
    set: (v) => solvePrefs.write({ speed: v }),
    default: d.speed,
  },
  {
    id: "ai.solvePreview", category: "ai", group: GROUP, order: 63, type: "toggle", label: "Check before showing the class",
    description: "The solution comes to you first. It goes on the board when you tap Write it on the board.",
    keywords: ["preview", "approve", "review", "check", "private"],
    get: () => solvePrefs.read().preview,
    set: (v) => solvePrefs.write({ preview: v }),
    default: d.preview,
  },
  {
    id: "ai.solveTeachStep", category: "ai", group: GROUP, order: 64, type: "toggle", label: "One line per click in Teach",
    description: "In Teach, each press of Next (clicker, arrow key or swipe) writes the next line, so you set the pace.",
    keywords: ["step", "reveal", "clicker", "next", "line by line", "teach"],
    get: () => solvePrefs.read().teachStep,
    set: (v) => solvePrefs.write({ teachStep: v }),
    default: d.teachStep,
  },
  {
    id: "ai.solveStudents", category: "ai", group: GROUP, order: 65, type: "toggle", label: "Let students use Solve this",
    description: "Off: Solve this is hidden in Student view. On: students can ask for a solution, which only they see.",
    keywords: ["student", "student view", "allow", "homework"],
    get: () => solvePrefs.read().students,
    set: (v) => solvePrefs.write({ students: v }),
    default: d.students,
  },
]);
