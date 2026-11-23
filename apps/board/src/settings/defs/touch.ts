// Settings → Touch & gestures: finger behaviour and the classroom gestures
// (gestures.ts, stored in `rupok.gestures` by gesturePrefs.ts).

import { DEFAULT_GESTURES, type GestureConfig } from "../../gestures";
import { gestures, resetHints, setGestures } from "../../gesturePrefs";
import { prefs, setPrefs, showToast } from "../../state";
import { GestureCalibration } from "../../components/settings/GestureCalibration";
import { pref } from "../bind";
import { type SettingDef, registerSettings } from "../registry";

/** get/set/default for one gesture setting. */
function g<K extends Exclude<keyof GestureConfig, "calibration">>(key: K) {
  return {
    get: () => gestures()[key] as any,
    set: (v: any) => setGestures({ [key]: v } as Partial<GestureConfig>),
    default: DEFAULT_GESTURES[key] as any,
  };
}
const palmOn = () => gestures().palmErase;
const swipeOn = () => gestures().swipe || gestures().flick;

const defs: SettingDef[] = [
  {
    id: "touch.fingers", category: "touch", order: 10, type: "segmented", label: "Finger touches",
    description: "Auto: fingers draw on touch-only boards and pan once a pen is used.",
    keywords: ["touch mode", "pan", "zoom", "palm rejection", "draw with finger", "আঙুল"],
    options: [{ value: "auto", label: "Auto" }, { value: "draw", label: "Draw" }, { value: "navigate", label: "Pan & zoom" }],
    ...pref("touchMode"),
  },

  // Palm eraser
  {
    id: "touch.palmEraser", category: "touch", group: "Palm eraser", order: 100, type: "toggle", label: "Palm eraser",
    description: "Rub with your palm or the side of your hand, like a board duster. Ink only.",
    keywords: ["eraser", "duster", "hand", "rub", "wipe", "ইরেজার", "মুছুন", "হাত"],
    ...g("palmErase"),
  },
  {
    id: "touch.calibrate", category: "touch", group: "Palm eraser", order: 105, type: "section", label: "Calibrate my board",
    description: "Measure a finger and a palm so the palm eraser fits your board.",
    keywords: ["calibration", "palm", "touch size", "ক্যালিব্রেট"],
    component: GestureCalibration,
  },
  {
    id: "touch.palmSize", category: "touch", group: "Palm eraser", order: 110, type: "slider", label: "Palm size",
    description: "A touch at least this wide erases (on boards that report touch size).",
    keywords: ["palm", "threshold", "contact size"],
    min: 16, max: 160, step: 2, format: (v) => `${Math.round(v)} px`, enabled: palmOn,
    ...g("palmSize"),
  },
  {
    id: "touch.eraserScale", category: "touch", group: "Palm eraser", order: 120, type: "slider", label: "Eraser size",
    description: "The eraser follows the size of your hand on the board.",
    keywords: ["eraser", "palm", "scale"],
    min: 0.5, max: 2, step: 0.1, format: (v) => `${v.toFixed(1)}×`, enabled: palmOn,
    ...g("eraserScale"),
  },
  {
    id: "touch.handCluster", category: "touch", group: "Palm eraser", order: 130, type: "toggle", label: "Hand detection",
    description: "3 or more touches close together count as a hand.",
    keywords: ["palm", "eraser", "cluster", "multi-touch"],
    enabled: palmOn, ...g("handCluster"),
  },
  {
    id: "touch.thumbScrub", category: "touch", group: "Palm eraser", order: 140, type: "toggle", label: "Thumb scrub",
    description: "Rub fast back and forth with one touch to erase, when fingers don't draw.",
    keywords: ["eraser", "thumb", "rub"],
    enabled: palmOn, ...g("thumbScrub"),
  },

  // Slides
  {
    id: "touch.swipe", category: "touch", group: "Swipe between slides", order: 200, type: "toggle", label: "Swipe to change slides",
    description: "Flick fingers left for the next slide or step, right for the previous.",
    keywords: ["next slide", "previous", "page", "স্লাইড", "সোয়াইপ"],
    ...g("swipe"),
  },
  {
    id: "touch.swipeFingers", category: "touch", group: "Swipe between slides", order: 210, type: "segmented", label: "Swipe fingers",
    description: "How many fingers a slide swipe takes.",
    keywords: ["swipe", "two fingers", "three fingers"],
    options: [{ value: "2", label: "2 fingers" }, { value: "3", label: "3 fingers" }],
    get: () => String(gestures().swipeFingers),
    set: (v) => setGestures({ swipeFingers: v === "3" ? 3 : 2 }),
    default: String(DEFAULT_GESTURES.swipeFingers),
    enabled: () => gestures().swipe,
  },
  {
    id: "touch.flick", category: "touch", group: "Swipe between slides", order: 220, type: "toggle", label: "One-finger flick in Teach",
    description: "In Teach and the student view, when fingers don't draw.",
    keywords: ["swipe", "flick", "next slide"],
    ...g("flick"),
  },
  {
    id: "touch.swipeSensitivity", category: "touch", group: "Swipe between slides", order: 230, type: "slider", label: "Swipe sensitivity",
    description: "Strict needs a longer, faster swipe, so a quick pan never changes the slide.",
    keywords: ["swipe", "strict", "eager"],
    min: 0, max: 1, step: 0.05, format: (s) => (s < 0.34 ? "Strict" : s > 0.66 ? "Eager" : "Normal"), enabled: swipeOn,
    ...g("swipeSensitivity"),
  },

  // Taps
  {
    id: "touch.tapUndo", category: "touch", group: "Taps and holds", order: 300, type: "toggle", label: "Two-finger tap to undo",
    description: "Same as Undo or Ctrl Z.", keywords: ["undo", "tap", "আনডু"], ...g("tapUndo"),
  },
  {
    id: "touch.tapRedo", category: "touch", group: "Taps and holds", order: 310, type: "toggle", label: "Three-finger tap to redo",
    description: "Same as Redo or Ctrl Shift Z.", keywords: ["redo", "tap"], ...g("tapRedo"),
  },
  {
    id: "touch.doubleTapFit", category: "touch", group: "Taps and holds", order: 320, type: "toggle", label: "Two-finger double-tap to fit",
    description: "Fits the slide to the screen, like the 0 key.", keywords: ["zoom", "fit", "reset view"], ...g("doubleTapFit"),
  },
  {
    id: "touch.longPressDock", category: "touch", group: "Taps and holds", order: 330, type: "toggle", label: "Two-finger hold brings the dock",
    description: "Hold two fingers still and the Smart Dock comes next to them.", keywords: ["dock", "summon", "toolbar", "ডক"], ...g("longPressDock"),
  },

  // Help
  {
    id: "touch.largeTargets", category: "touch", group: "Help", order: 400, type: "toggle", label: "Large touch targets",
    description: "Bigger buttons and handles for big boards. Also sets Button size to Large board.",
    keywords: ["accessibility", "big buttons", "smartboard", "বড়"],
    get: () => gestures().largeTargets,
    set: (v) => {
      setGestures({ largeTargets: v });
      if (v && prefs().density !== "large") setPrefs({ density: "large" });
    },
    default: DEFAULT_GESTURES.largeTargets,
  },
  {
    id: "touch.hints", category: "touch", group: "Help", order: 410, type: "toggle", label: "First-time hints",
    description: "A short hint the first time each gesture works.", keywords: ["tips", "help", "onboarding"], ...g("hints"),
  },
  {
    id: "touch.showHintsAgain", category: "touch", group: "Help", order: 420, type: "action", label: "Show the hints again",
    description: "Every gesture hint shows once more.", keywords: ["tips", "reset hints"], button: "Show again",
    run: () => { resetHints(); showToast("Gesture hints will show again"); },
  },
];
registerSettings(defs);
