// Settings → Pen & ink (writing feel and Smart ink).

import { type SmartShapes, prefs, setPrefs } from "../../state";
import { controller } from "../../controller";
import { pref } from "../bind";
import { registerSettings } from "../registry";
import { setCalibrateOpen } from "../../solve/state";

registerSettings([
  {
    id: "pen.pressure", category: "pen", order: 10, type: "slider", label: "Pen pressure",
    description: "Soft makes thin lines easy; firm needs a harder press for thick ones.",
    keywords: ["stylus", "pressure curve", "চাপ", "কলম"],
    min: 0.3, max: 2, step: 0.05, format: (v) => (v < 0.8 ? "Soft" : v > 1.2 ? "Firm" : "Normal"),
    ...pref("pressure"),
  },
  {
    id: "pen.smoothing", category: "pen", order: 20, type: "slider", label: "Smoothing",
    description: "Higher removes shake on large boards; lower feels more direct.",
    keywords: ["stabilizer", "shaky", "jitter", "smooth", "মসৃণ"],
    min: 0, max: 1, step: 0.05, format: (v) => `${Math.round(v * 100)}%`,
    ...pref("smoothing"),
  },
  {
    id: "pen.hand", category: "pen", order: 30, type: "segmented", label: "Writing hand",
    description: "Menus open away from this hand, so your arm never covers them.",
    keywords: ["left-handed", "right-handed", "handedness", "বাম হাত", "ডান হাত"],
    options: [{ value: "right", label: "Right" }, { value: "left", label: "Left" }],
    ...pref("hand"),
  },
  {
    id: "pen.smartShapes", category: "pen", group: "Smart ink", order: 110, type: "segmented", label: "Smart shapes",
    description: "Rough circles, boxes, lines and arrows become clean. Handwriting stays as written; Undo brings your drawing back.",
    keywords: ["shape recognition", "snap", "auto shapes", "hold to snap", "আকৃতি", "smart ink"],
    options: [{ value: "off", label: "Off" }, { value: "auto", label: "On lift" }, { value: "hold", label: "Hold to snap" }],
    get: () => prefs().smartShapes,
    set: (v) => {
      const s = v as SmartShapes;
      setPrefs(s === "off" ? { smartShapes: s } : { smartShapes: s, smartLast: s });
      controller.applyPrefs();
    },
    default: "off",
  },
  {
    id: "pen.scratchOut", category: "pen", group: "Smart ink", order: 120, type: "toggle", label: "Scratch out to erase",
    description: "Scribble a tight zig-zag over ink to erase it, while smart shapes are on.",
    keywords: ["eraser", "scribble", "delete", "মুছুন", "ইরেজার"],
    enabled: () => prefs().smartShapes !== "off",
    ...pref("scratchOut"),
  },
  ...(
    [
      ["smartGraphs", "Graphs", "A big + or L becomes x/y axes; a curve drawn over them gets its equation, and Make it live opens the grapher.", ["axes", "graph", "curve", "equation", "plot", "অক্ষ", "লেখচিত্র", "সমীকরণ"]],
      ["smartFlowchart", "Flowcharts", "Ovals and slanted boxes snap, and arrows between shapes stay attached when you move them.", ["flowchart", "connector", "arrow", "terminator", "ফ্লোচার্ট", "তীর"]],
      ["smartGates", "Logic gates", "A gate body with a pin, bubble or second curve becomes a clean AND, OR, NOT, NAND, NOR or XOR gate.", ["and", "or", "not", "xor", "nand", "nor", "logic", "ICT", "লজিক গেট"]],
      ["smartNumberLine", "Number lines", "A long line with evenly spaced ticks gets numbers, zero at the middle tick.", ["number line", "ticks", "integers", "সংখ্যারেখা"]],
      ["smartGeometry", "Angles and lines", "An arc in a corner shows the angle in degrees, a small square marks a right angle, and lines snap parallel or perpendicular.", ["angle", "degrees", "right angle", "parallel", "perpendicular", "কোণ", "সমান্তরাল", "লম্ব"]],
      ["smartTables", "Tables and Venn diagrams", "Lines across a rectangle make a table; overlapping circles become a Venn diagram.", ["table", "grid", "venn", "sets", "সারণি", "ভেনচিত্র"]],
      ["smartMath", "Maths", "While you write maths, its typeset form floats above it; tap to convert. Works offline.", ["maths", "math", "equation", "latex", "formula", "handwriting", "গণিত", "সমীকরণ"]],
      ["smartScience", "Circuits and chemistry", "Resistors, cells, bulbs, switches, benzene rings and ⇌. Off by default, since these marks mean other things in other subjects.", ["circuit", "resistor", "battery", "cell", "bulb", "switch", "benzene", "equilibrium", "বর্তনী", "রোধ", "বেনজিন"]],
    ] as const
  ).map(([key, label, description, keywords], i) => ({
    id: `pen.${key}`, category: "pen" as const, group: "Smart ink", order: 130 + i * 10, type: "toggle" as const,
    label, description, keywords: [...keywords],
    enabled: () => prefs().smartShapes !== "off",
    ...pref(key),
  })),
  {
    id: "pen.teachHandwriting", category: "pen", group: "Smart ink", order: 220, type: "action",
    label: "Teach Rupok my handwriting",
    description: "Write each digit, letter and sign once so your maths is read better (Solve this also writes in your hand). Kept on this device.",
    keywords: ["calibrate", "calibration", "train", "my handwriting", "হাতের লেখা", "শেখান"],
    button: "Start", icon: "pen",
    run: () => {
      void import("../../solve").then((m) => m.initSolve());
      setCalibrateOpen(true);
    },
  },
]);
