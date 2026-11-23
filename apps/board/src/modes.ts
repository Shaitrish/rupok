// Board modes and their UI configuration.
//
// A mode decides which chrome is on screen and which tools the toolbar holds,
// in order. Defaults live here; a teacher's (or an AI's) changes are stored in
// prefs under `modes` and can be exported/imported as a JSON "UI config" file.
// The file format is documented in docs/ui-config.md; keep the two in sync.

import type { IconName } from "./icons.generated";
import type { Tool } from "./state";

export type Mode = "prepare" | "teach" | "view";

/** Toolbar entries: drawing tools plus a few actions. */
export type ToolbarItem = Tool | "smartink" | "undo" | "redo" | "pager" | "replay" | "exit";

/** Chrome elements a mode can show or hide. */
export type ChromeKey =
  | "switcher" | "logo" | "title" | "pagesToggle" | "history" | "insert" | "replay" | "open"
  | "theme" | "fullscreen" | "settings" | "save" | "ai" | "overflow" | "pages" | "zoom" | "stats" | "colors" | "sizes";

export interface ModeConfig {
  chrome: Record<ChromeKey, boolean>;
  /** Everything the mode offers, in order: on the dock or in its ⋯ menu. */
  tools: ToolbarItem[];
  /** The Smart Dock (components/Dock.tsx): where it starts and what sits on it. */
  dock: DockConfig;
}
export type ModesConfig = Record<Mode, ModeConfig>;

// ------------------------------------------------------------------ dock

/** Where the dock starts: "auto" follows Settings → Toolbar (bottom, left,
 * right); `{x, y}` floats it at fractions (0..1) of the free screen area. */
export type DockPosition =
  | "auto" | "bottom" | "top" | "left" | "right" | "bottom-left" | "bottom-right" | "top-left" | "top-right"
  | { x: number; y: number };
export const DOCK_POSITIONS = ["auto", "bottom", "top", "left", "right", "bottom-left", "bottom-right", "top-left", "top-right"] as const;
export type DockOrientation = "auto" | "horizontal" | "vertical";
/** A dock pin: a toolbar entry, or "shapes" for the one Shapes button
 * (rectangle, ellipse, line, arrow and the Smart shapes toggle). */
export type DockPin = ToolbarItem | "shapes";
export interface DockConfig {
  position: DockPosition;
  orientation: DockOrientation;
  /** Start as the small round bubble. */
  minimized: boolean;
  /** Tools on the dock besides the core ones; the mode's other tools go in ⋯. */
  pinned: DockPin[];
}
/** Always on the dock in Prepare and Teach; a config cannot remove them. */
export const CORE_TOOLS: ToolbarItem[] = ["pen", "highlighter", "eraser", "laser", "undo"];
export const SHAPE_TOOLS: ToolbarItem[] = ["rect", "ellipse", "line", "arrow"];
const hasCore = (m: Mode) => m !== "view";

export const MODES: { id: Mode; label: string; icon: IconName; key: string; hint: string }[] = [
  { id: "prepare", label: "Prepare", icon: "pen", key: "Alt 1", hint: "Build the lesson: every tool and panel" },
  { id: "teach", label: "Teach", icon: "monitor", key: "F5", hint: "Present: the slide fills the screen, only essential tools" },
  { id: "view", label: "Student", icon: "eye", key: "Alt 3", hint: "Student view: read-only, page through and replay" },
];

/** Everything the toolbar can hold, in the default order. */
export const TOOLBAR_ITEMS: { id: ToolbarItem; icon: IconName; label: string; key?: string; kind: "tool" | "action" | "nav" | "exit" }[] = [
  { id: "pen", icon: "pen", label: "Pen", key: "P", kind: "tool" },
  { id: "highlighter", icon: "highlighter", label: "Highlighter", key: "H", kind: "tool" },
  { id: "eraser", icon: "eraser", label: "Eraser", key: "E", kind: "tool" },
  { id: "lasso", icon: "select", label: "Select & move", key: "V", kind: "tool" },
  { id: "laser", icon: "laser", label: "Laser pointer", key: "L", kind: "tool" },
  { id: "text", icon: "text", label: "Text", key: "T", kind: "tool" },
  { id: "rect", icon: "rect", label: "Rectangle", key: "R", kind: "tool" },
  { id: "ellipse", icon: "ellipse", label: "Ellipse", key: "O", kind: "tool" },
  { id: "line", icon: "line", label: "Line", key: "N", kind: "tool" },
  { id: "arrow", icon: "arrow", label: "Arrow", key: "A", kind: "tool" },
  { id: "hand", icon: "hand", label: "Pan", key: "Space", kind: "tool" },
  { id: "interact", icon: "interact", label: "Use Smart Objects", key: "I", kind: "tool" },
  { id: "smartink", icon: "smartink", label: "Smart shapes", key: "M", kind: "action" },
  { id: "undo", icon: "undo", label: "Undo", key: "Ctrl Z", kind: "action" },
  { id: "redo", icon: "redo", label: "Redo", key: "Ctrl Shift Z", kind: "action" },
  { id: "replay", icon: "replay", label: "Replay", kind: "action" },
  { id: "pager", icon: "next", label: "Page back / next", key: "← →", kind: "nav" },
  { id: "exit", icon: "exit", label: "Exit mode", key: "Esc", kind: "exit" },
];

export const CHROME_ITEMS: { id: ChromeKey; label: string }[] = [
  { id: "switcher", label: "Mode switcher" },
  { id: "logo", label: "Logo" },
  { id: "title", label: "Lesson title" },
  { id: "pagesToggle", label: "Pages button" },
  { id: "history", label: "Undo and redo" },
  { id: "insert", label: "Insert menu (image, PDF, Smart Object, text)" },
  { id: "replay", label: "Replay button" },
  { id: "open", label: "Open" },
  { id: "theme", label: "Light / dark" },
  { id: "fullscreen", label: "Fullscreen" },
  { id: "settings", label: "Settings" },
  { id: "save", label: "Save state, Save and Export PDF" },
  { id: "ai", label: "AI assistant status" },
  { id: "overflow", label: "⋯ menu (open, export, theme, fullscreen, help)" },
  { id: "pages", label: "Pages panel" },
  { id: "zoom", label: "Zoom" },
  { id: "stats", label: "Statistics (when enabled)" },
  { id: "colors", label: "Recent colours on the dock" },
  { id: "sizes", label: "Ink sizes (tap the pen again)" },
];

/** Toolbar entries each mode may use (the student view cannot edit). */
export const ALLOWED_TOOLS: Record<Mode, ToolbarItem[]> = {
  prepare: TOOLBAR_ITEMS.map((t) => t.id).filter((t) => t !== "exit"),
  teach: TOOLBAR_ITEMS.map((t) => t.id),
  view: ["pager", "hand", "interact", "replay"],
};
/** Chrome each mode may show (the student view has no editing chrome). */
export const ALLOWED_CHROME: Record<Mode, ChromeKey[]> = {
  prepare: CHROME_ITEMS.map((c) => c.id),
  teach: CHROME_ITEMS.map((c) => c.id),
  view: ["switcher", "logo", "title", "pagesToggle", "replay", "open", "theme", "fullscreen", "overflow", "pages", "zoom", "stats"],
};

const chrome = (on: ChromeKey[]): Record<ChromeKey, boolean> =>
  Object.fromEntries(CHROME_ITEMS.map((c) => [c.id, on.includes(c.id)])) as Record<ChromeKey, boolean>;

export const DEFAULT_MODES: ModesConfig = {
  prepare: {
    chrome: chrome(["switcher", "title", "pagesToggle", "history", "insert", "replay", "open", "theme", "fullscreen", "settings", "save", "ai", "overflow", "pages", "zoom", "stats", "colors", "sizes"]),
    tools: ["pen", "highlighter", "eraser", "rect", "ellipse", "line", "arrow", "smartink", "text", "lasso", "laser", "hand", "interact", "undo", "redo"],
    dock: { position: "auto", orientation: "auto", minimized: false, pinned: ["shapes", "text", "lasso"] },
  },
  teach: {
    chrome: chrome(["colors", "sizes"]),
    tools: ["pen", "highlighter", "eraser", "lasso", "laser", "smartink", "undo", "pager", "exit"],
    dock: { position: "auto", orientation: "auto", minimized: false, pinned: ["lasso", "smartink", "pager", "exit"] },
  },
  view: {
    chrome: chrome(["switcher", "title", "pagesToggle", "open", "theme", "fullscreen", "overflow", "pages", "zoom"]),
    tools: ["pager", "hand", "interact", "replay"],
    dock: { position: "bottom", orientation: "auto", minimized: false, pinned: ["pager", "hand", "interact", "replay"] },
  },
};

const isMode = (m: unknown): m is Mode => m === "prepare" || m === "teach" || m === "view";

/** Validate one mode's config from untrusted JSON: unknown keys and tools are
 * dropped, missing chrome keys fall back to the default, duplicates removed. */
export function sanitizeMode(m: Mode, raw: unknown): ModeConfig {
  const def = DEFAULT_MODES[m];
  const out: ModeConfig = { chrome: { ...def.chrome }, tools: [...def.tools], dock: { ...def.dock, pinned: [...def.dock.pinned] } };
  if (!raw || typeof raw !== "object") return out;
  const r = raw as { chrome?: unknown; tools?: unknown; dock?: unknown };
  if (r.chrome && typeof r.chrome === "object") {
    for (const [k, v] of Object.entries(r.chrome as Record<string, unknown>)) {
      if (k in out.chrome && typeof v === "boolean") out.chrome[k as ChromeKey] = v && ALLOWED_CHROME[m].includes(k as ChromeKey);
    }
  }
  const customTools = Array.isArray(r.tools);
  if (customTools) {
    const allowed = ALLOWED_TOOLS[m];
    out.tools = [...new Set((r.tools as unknown[]).filter((t): t is ToolbarItem => allowed.includes(t as ToolbarItem)))];
  }
  // Core tools cannot be removed in Prepare and Teach.
  if (hasCore(m)) for (const t of CORE_TOOLS) if (!out.tools.includes(t)) out.tools.push(t);
  // A config written before the dock (tools, no dock.pinned) meant "these
  // tools on the toolbar": keep showing them all on the dock.
  const d = (r.dock && typeof r.dock === "object" ? r.dock : {}) as Record<string, unknown>;
  out.dock = sanitizeDock(m, d, customTools && !Array.isArray(d.pinned) ? out.tools : null, out.tools, def.dock);
  return out;
}

function sanitizeDock(m: Mode, d: Record<string, unknown>, legacy: ToolbarItem[] | null, tools: ToolbarItem[], def: DockConfig): DockConfig {
  const out: DockConfig = { ...def, pinned: [...def.pinned] };
  const p = d.position;
  if (typeof p === "string" && (DOCK_POSITIONS as readonly string[]).includes(p)) out.position = p as DockPosition;
  else if (p && typeof p === "object") {
    const { x, y } = p as { x?: unknown; y?: unknown };
    if (typeof x === "number" && typeof y === "number" && Number.isFinite(x) && Number.isFinite(y)) {
      out.position = { x: Math.min(1, Math.max(0, x)), y: Math.min(1, Math.max(0, y)) };
    }
  }
  if (d.orientation === "auto" || d.orientation === "horizontal" || d.orientation === "vertical") out.orientation = d.orientation;
  if (typeof d.minimized === "boolean") out.minimized = d.minimized;
  const src: unknown[] | null = legacy ?? (Array.isArray(d.pinned) ? d.pinned : null);
  if (src) {
    const pins: DockPin[] = [];
    for (const t of src) {
      const pin: DockPin | null = t === "shapes" || SHAPE_TOOLS.includes(t as ToolbarItem) ? "shapes" : tools.includes(t as ToolbarItem) ? (t as ToolbarItem) : null;
      if (pin && !(hasCore(m) && CORE_TOOLS.includes(pin as ToolbarItem)) && !pins.includes(pin)) pins.push(pin);
    }
    // "shapes" needs at least one shape tool in the mode.
    out.pinned = pins.filter((x) => x !== "shapes" || tools.some((t) => SHAPE_TOOLS.includes(t)));
  } else {
    out.pinned = out.pinned.filter((x) => (x === "shapes" ? tools.some((t) => SHAPE_TOOLS.includes(t)) : tools.includes(x)));
  }
  return out;
}

/** Validate a `modes` object (from prefs or an imported file). */
export function sanitizeModes(raw: unknown): Partial<ModesConfig> {
  const out: Partial<ModesConfig> = {};
  if (!raw || typeof raw !== "object") return out;
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) if (isMode(k)) out[k] = sanitizeMode(k, v);
  return out;
}

/** The effective config of a mode: the stored one, else the default. */
export function resolveMode(stored: Partial<ModesConfig> | undefined, m: Mode): ModeConfig {
  return stored?.[m] ? sanitizeMode(m, stored[m]) : DEFAULT_MODES[m];
}

/** True when any top-bar element is on, i.e. the top bar is drawn at all. */
export function hasTopBar(c: ModeConfig): boolean {
  const k = c.chrome;
  return k.switcher || k.logo || k.title || (k.pagesToggle && k.pages) || k.history || k.insert || k.replay || k.open || k.theme || k.fullscreen || k.settings || k.save || k.ai || k.overflow;
}

// ---------------------------------------------------------- UI config file

export const UI_CONFIG_FORMAT = "rupok-ui-config";
export const UI_CONFIG_VERSION = 1;

export interface UiConfigFile {
  format: typeof UI_CONFIG_FORMAT;
  version: number;
  modes?: Partial<ModesConfig>;
  appearance?: { theme?: string; accent?: string; iconStyle?: string; toolbar?: string; density?: string; hand?: string };
  /** Classroom touch gestures (gestures.ts → GestureConfig). */
  gestures?: Record<string, unknown>;
}
