import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { APP_SLUG } from "./brand";

export type Tool =
  | "pen" | "highlighter" | "eraser" | "lasso" | "hand" | "laser" | "interact" | "text"
  | "rect" | "ellipse" | "line" | "arrow";

export interface DomItem {
  id: string;
  kind: "object" | "text" | "image";
  x: number; y: number; w: number; h: number; rot: number; z: number;
  title?: string; bundle?: string; step?: number; steps?: number; state?: string;
  text?: string; size?: number; color?: string; asset?: string;
  selected: boolean;
}

export interface PageInfo {
  id: string;
  title: string;
  frame: { w: number; h: number } | null;
  pattern: "plain" | "grid" | "dots" | "lines";
  bg_color: number;
  item_count: number;
}

export const [tool, setTool] = createSignal<Tool>("pen");
export const [color, setColor] = createSignal<number>(0x1c2024ff);
export const [width, setWidth] = createSignal<number>(4);
export const [hlColor, setHlColor] = createSignal<number>(0xffe629ff);
/** Last chosen size per tool family. */
export const [sizes, setSizes] = createSignal<Record<string, number>>({ pen: 4, highlighter: 22, eraser: 32 });
export function sizeKey(t: string): string {
  return t === "highlighter" || t === "eraser" ? t : "pen";
}
export const [camera, setCamera] = createSignal<[number, number, number]>([0, 0, 1]);
export const [pages, setPages] = createSignal<PageInfo[]>([]);
export const [pageIndex, setPageIndex] = createSignal(0);
export const [items, setItems] = createStore<DomItem[]>([]);
export const [selection, setSelection] = createSignal<number[] | null>(null); // world bounds
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
export type Accent = "iris" | "teal" | "blue" | "violet" | "grass" | "ruby" | "amber";
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
}
// WebGL2 is the default until WebGPU presentation is verified on real boards (see ROADMAP).
const defaults: Prefs = {
  theme: "system", accent: "iris", iconStyle: "linear", toolbar: "bottom", density: "comfortable",
  touchMode: "auto", smoothing: 0.5, pressure: 0.8, showStats: false, forceWebgl: true, showPages: true,
};

function loadPrefs(): Prefs {
  try {
    return { ...defaults, ...JSON.parse(localStorage.getItem(PREF_KEY) || "{}") };
  } catch {
    return defaults;
  }
}
export const [prefs, setPrefsSignal] = createSignal<Prefs>(loadPrefs());
export function setPrefs(p: Partial<Prefs>) {
  const next = { ...prefs(), ...p };
  setPrefsSignal(next);
  try {
    localStorage.setItem(PREF_KEY, JSON.stringify(next));
  } catch { /* storage unavailable */ }
}

export const ACCENTS: { id: Accent; label: string; color: string }[] = [
  { id: "iris", label: "Iris", color: "#5b5bd6" },
  { id: "teal", label: "Teal", color: "#12a594" },
  { id: "blue", label: "Blue", color: "#0090ff" },
  { id: "violet", label: "Violet", color: "#6e56cf" },
  { id: "grass", label: "Grass", color: "#46a758" },
  { id: "ruby", label: "Ruby", color: "#e54666" },
  { id: "amber", label: "Amber", color: "#ffc53d" },
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
