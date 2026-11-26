// Settings → About → Performance lab: every ink-speed fix as a switch that
// applies at once, so a board can be tried in each mode in one visit
// (docs/board-test.md), and the board speed test that compares them.

import { prefs, setPrefs } from "../../state";
import { controller } from "../../controller";
import { closeSettings } from "../open";
import { registerSettings } from "../registry";
import { nativeInkAvailable, nativeInkFailure } from "../../nativeInk";
import { startSpeedTest } from "../../speedTest";

const LAB = "Performance lab · experimental";
const windows = /Windows/i.test(navigator.userAgent);

/** A pref that takes effect on the running board (no reload). */
function live<K extends "renderScale" | "wetLayer" | "inkTrail" | "nativeWetInk" | "flatPanels">(key: K) {
  return {
    get: () => prefs()[key] as any,
    set: (v: any) => {
      setPrefs({ [key]: v });
      controller.applyPerf();
    },
  };
}

registerSettings([
  {
    id: "about.speedTest", category: "about", group: "Troubleshooting", order: 105, type: "action", label: "Board speed test",
    description: "Draws on the board by itself for about a minute, once in each drawing mode, then asks you to write for 10 seconds in each. The report is shown, and saved to Documents/Rupok in the app. Your lesson is put back as it was.",
    keywords: ["speed test", "benchmark", "slow", "lag", "fps", "report", "diagnostics", "smartboard", "4k", "গতি", "ধীর"],
    button: "Run board speed test", icon: "play",
    run: () => {
      closeSettings();
      void startSpeedTest("compare");
    },
  },
  {
    id: "about.renderScale", category: "about", group: LAB, order: 200, type: "segmented", label: "Ink sharpness",
    description: "How many of the screen's pixels the ink canvas draws. Auto starts a 4K board at 75 % and steps down if frames come late. Lower is faster and a little softer.",
    keywords: ["render scale", "resolution", "4k", "sharp", "fast", "quality", "performance", "slow", "lag", "smartboard"],
    options: [{ value: "auto", label: "Auto" }, { value: "100", label: "100 % · sharp" }, { value: "75", label: "75 %" }, { value: "50", label: "50 % · fast" }],
    ...live("renderScale"),
    default: "auto",
  },
  {
    id: "about.wetLayer", category: "about", group: LAB, order: 210, type: "segmented", label: "Wet-ink layer",
    description: "The stroke being written is drawn in a small canvas under the pen, so writing does not redraw the whole board. Smaller is cheaper per frame; larger holds longer strokes before the board must redraw once. Off draws every frame at full size, as up to v0.2.4.",
    keywords: ["wet ink", "tile", "layer", "incremental", "performance", "slow", "lag"],
    options: [{ value: "off", label: "Off" }, { value: "small", label: "Small" }, { value: "medium", label: "Medium" }, { value: "large", label: "Large" }],
    ...live("wetLayer"),
    default: "medium",
  },
  {
    id: "about.inkTrail", category: "about", group: LAB, order: 220, type: "toggle", label: "Windows ink trail",
    description: "Windows draws the last stretch from the ink to the pen itself, ahead of the board's frames.",
    keywords: ["ink trail", "delegated ink", "latency", "windows ink", "prediction"],
    when: () => windows,
    ...live("inkTrail"),
    default: true,
  },
  {
    id: "about.nativeWetInk", category: "about", group: LAB, order: 230, type: "toggle", label: "Native wet ink · prototype",
    get description() {
      const why = nativeInkFailure();
      return why
        ? `It switched itself off: ${why}`
        : "The Windows app draws the stroke being written in its own layer over the board, outside the browser's frame pipeline. A plain line for the pen and ballpoint styles; it switches itself off if anything fails.";
    },
    keywords: ["native", "direct2d", "directcomposition", "wet ink", "latency", "windows", "prototype"],
    when: () => nativeInkAvailable,
    ...live("nativeWetInk"),
    default: false,
  },
  {
    id: "about.flatPanels", category: "about", group: LAB, order: 240, type: "toggle", label: "Flat panels",
    description: "No frosted-glass blur behind the dock and the bars: less work for the graphics card each time the ink under them changes.",
    keywords: ["blur", "backdrop", "frosted", "glass", "performance", "panels"],
    ...live("flatPanels"),
    default: false,
  },
]);
