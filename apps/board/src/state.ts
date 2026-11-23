import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { APP_SLUG } from "./brand";
import { type Mode, type ModeConfig, type ModesConfig, resolveMode, sanitizeModes } from "./modes";

export type Tool =
  | "pen" | "highlighter" | "eraser" | "lasso" | "hand" | "laser" | "interact" | "text"
  | "rect" | "ellipse" | "line" | "arrow";

export interface DomItem {
  id: string;
  kind: "object" | "text" | "image";
  x: number; y: number; w: number; h: number; rot: number; z: number;
  title?: string; bundle?: string; step?: number; steps?: number; state?: string;
  text?: string; size?: number; color?: string; asset?: string;
  /** A text item whose text is LaTeX maths. */
  latex?: boolean;
  selected: boolean;
}

export interface PageInfo {
  id: string;
  title: string;
  frame: { w: number; h: number } | null;
  pattern: "plain" | "grid" | "dots" | "lines";
  bg_color: number;
  item_count: number;
  /** Page design (design.ts): the background image and this page's own branding. */
  background?: import("./design").PageBackground;
  branding?: import("./design").Branding;
}

export const [tool, setTool] = createSignal<Tool>("pen");
export const [color, setColor] = createSignal<number>(0x1c2024ff);
export const [width, setWidth] = createSignal<number>(4);
export const [hlColor, setHlColor] = createSignal<number>(0xffe629ff);
/** Last chosen size per tool family. */
export const [sizes, setSizes] = createSignal<Record<string, number>>({ pen: 4, highlighter: 22, eraser: 64 });
export function sizeKey(t: string): string {
  return t === "highlighter" || t === "eraser" ? t : "pen";
}
export const [camera, setCamera] = createSignal<[number, number, number]>([0, 0, 1]);
export const [pages, setPages] = createSignal<PageInfo[]>([]);
export const [pageIndex, setPageIndex] = createSignal(0);
export const [items, setItems] = createStore<DomItem[]>([]);
export const [selection, setSelection] = createSignal<number[] | null>(null); // world bounds
/** Ids of the selected items (strokes and box items). */
export const [selectionIds, setSelectionIds] = createSignal<string[]>([], {
  equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]),
});
/** What the selection holds (from the engine), for the selection toolbar. */
export const [selectionMeta, setSelectionMeta] = createSignal<{
  count: number; strokes: number; boxes: number; groups: number; grouped: boolean;
  /** One resizable shape stroke: its geometric bounds [x0, y0, x1, y1] (world) and name. */
  shape?: number[] | null; shapeName?: string | null;
  /** An axes-and-curve graph "Make it live" can replace. */
  live?: boolean;
  /** Shared colour (#rrggbbaa) and stroke width when uniform; all ink is highlighter. */
  style?: { color: string | null; width: number | null; highlighter: boolean };
}>({ count: 0, strokes: 0, boxes: 0, groups: 0, grouped: false });
/** Opened as the student viewer (`/board/?view`): starts in the Student
 * mode and never touches the teacher's autosave. */
export const studentEntry = typeof location !== "undefined" && new URLSearchParams(location.search).has("view");
/** The board mode: Prepare, Teach or Student view (see modes.ts). */
export const [mode, setModeSignal] = createSignal<Mode>(studentEntry ? "view" : "prepare");
/** Student view: read-only. */
export const isView = () => mode() === "view";
/** Student entry: true once a lesson file has been opened. */
export const [lessonOpened, setLessonOpened] = createSignal(false);
export const [canUndo, setCanUndo] = createSignal(false);
export const [canRedo, setCanRedo] = createSignal(false);
export const [title, setTitle] = createSignal("Untitled lesson");
export const [replay, setReplay] = createSignal<{ t: number; d: number; playing: boolean } | null>(null);
export const [stats, setStats] = createSignal<{ backend: string; strokes: number; frameMs: number; fps: number } | null>(null);
export const [toast, setToast] = createSignal<string | null>(null);
export const [editingText, setEditingText] = createSignal<string | null>(null);
export const [activeObject, setActiveObject] = createSignal<string | null>(null);
export const [savedRevision, setSavedRevision] = createSignal(0);
export const [revision, setRevision] = createSignal(0);

let toastTimer: number | undefined;
export function showToast(msg: string, ms = 2600) {
  setToast(msg);
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => setToast(null), ms);
}

const PREF_KEY = `${APP_SLUG}.prefs`;
export type Accent = "mint" | "graphite" | "blue" | "grass" | "coral" | "sun";
export type SmartShapes = "off" | "auto" | "hold";
export interface Prefs {
  theme: "light" | "dark" | "system";
  accent: Accent;
  iconStyle: import("./icons.generated").IconVariant;
  toolbar: "bottom" | "left" | "right";
  density: "compact" | "comfortable" | "large";
  touchMode: "auto" | "draw" | "navigate";
  smoothing: number;
  pressure: number;
  showStats: boolean;
  forceWebgl: boolean;
  showPages: boolean;
  /** Smart Ink shapes (pen only): off, snap on pen-up, or hold still to snap. */
  smartShapes: SmartShapes;
  /** The on-mode the toolbar toggle switches back to. */
  smartLast: Exclude<SmartShapes, "off">;
  /** Scratch out ink to erase it (while smart shapes are on). */
  scratchOut: boolean;
  /** Smart ink families (Settings → Pen & ink → Smart ink). */
  smartGraphs: boolean;
  smartFlowchart: boolean;
  smartGates: boolean;
  smartNumberLine: boolean;
  smartGeometry: boolean;
  smartTables: boolean;
  smartScience: boolean;
  /** Typeset preview of handwritten maths while writing. */
  smartMath: boolean;
  /** Per-mode chrome and toolbar (docs/ui-config.md); missing modes use the defaults. */
  modes?: Partial<ModesConfig>;
  /** Writing hand: dock popovers open away from it, so the arm never covers them. */
  hand: "right" | "left";
  /** Eraser and palm duster: rub out only what they cover, or whole strokes. */
  eraserMode: "partial" | "stroke";
  /** Slide pages are the board: ink, shapes and placed items stay on them. */
  keepInkOnPage: boolean;
  /** The pages drawer stays open after a jump (Teach, phones, "Show pages"). */
  pagesPinned: boolean;
}
// WebGL2 is the default until WebGPU presentation is verified on real boards (see ROADMAP).
const defaults: Prefs = {
  theme: "system", accent: "mint", iconStyle: "linear", toolbar: "bottom", density: "comfortable",
  touchMode: "auto", smoothing: 0.25, pressure: 0.8, showStats: false, forceWebgl: true, showPages: true,
  smartShapes: "off", smartLast: "auto", scratchOut: true, hand: "right", eraserMode: "partial", keepInkOnPage: true, pagesPinned: false,
  smartGraphs: true, smartFlowchart: true, smartGates: true, smartNumberLine: true,
  smartGeometry: true, smartTables: true, smartScience: false, smartMath: true,
};

/** Smart ink families that are on, as the engine's comma-separated list. */
export function smartFamilies(p: Prefs): string {
  const on: [boolean, string][] = [
    [p.smartGraphs, "graphs"], [p.smartFlowchart, "flowchart"], [p.smartGates, "gates"],
    [p.smartNumberLine, "numberline"], [p.smartGeometry, "geometry"], [p.smartTables, "tables"],
    [p.smartScience, "science"],
  ];
  return on.filter(([v]) => v).map(([, n]) => n).join(",");
}

/** The default of every pref (Settings → "Reset this section"). */
export const PREF_DEFAULTS: Readonly<Prefs> = defaults;

function loadPrefs(): Prefs {
  try {
    const saved = { ...defaults, ...JSON.parse(localStorage.getItem(PREF_KEY) || "{}") };
    const known = ["mint", "graphite", "blue", "grass", "coral", "sun"];
    if (!known.includes(saved.accent)) saved.accent = "mint";
    if (!["off", "auto", "hold"].includes(saved.smartShapes)) saved.smartShapes = "off";
    if (!["auto", "hold"].includes(saved.smartLast)) saved.smartLast = "auto";
    if (saved.hand !== "left") saved.hand = "right";
    if (saved.eraserMode !== "stroke") saved.eraserMode = "partial";
    saved.keepInkOnPage = saved.keepInkOnPage !== false;
    saved.modes = sanitizeModes(saved.modes);
    return saved;
  } catch {
    return defaults;
  }
}
export const [prefs, setPrefsSignal] = createSignal<Prefs>(loadPrefs());
/** The effective configuration of a mode (stored, else default). */
export const modeConfig = (m: Mode = mode()): ModeConfig => resolveMode(prefs().modes, m);
export function setPrefs(p: Partial<Prefs>) {
  const next = { ...prefs(), ...p };
  setPrefsSignal(next);
  try {
    localStorage.setItem(PREF_KEY, JSON.stringify(next));
  } catch { /* storage unavailable */ }
}

export const ACCENTS: { id: Accent; label: string; color: string }[] = [
  { id: "mint", label: "Mint", color: "#20c997" },
  { id: "graphite", label: "Graphite", color: "#111418" },
  { id: "blue", label: "Blue", color: "#1c7ed6" },
  { id: "grass", label: "Grass", color: "#37b24d" },
  { id: "coral", label: "Coral", color: "#ff6b57" },
  { id: "sun", label: "Sun", color: "#fcc419" },
];

/** Page (board surface) colours. Ink defaults adapt to dark boards. */
export const BOARD_COLORS: { rgba: number; label: string; pattern: string }[] = [
  { rgba: 0xffffffff, label: "White", pattern: "rgba(30, 60, 120, 0.10)" },
  { rgba: 0xfbf7edff, label: "Paper", pattern: "rgba(120, 90, 40, 0.14)" },
  { rgba: 0x1f3b30ff, label: "Chalkboard", pattern: "rgba(255, 255, 255, 0.10)" },
  { rgba: 0x17191cff, label: "Blackboard", pattern: "rgba(255, 255, 255, 0.09)" },
];

export const PEN_COLORS = [0x1c2024ff, 0x3e63ddff, 0xe5484dff, 0x30a46cff, 0xf76b15ff, 0x8e4ec6ff, 0xffffffff];
export const HL_COLORS = [0xffe629ff, 0x86ead4ff, 0x8ec8f6ff, 0xf5a9b8ff, 0xffc182ff];
export const WIDTHS = [2, 4, 8, 16];

export function hex(rgba: number): string {
  return "#" + (rgba >>> 8).toString(16).padStart(6, "0");
}
