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
export type ToolbarItem =
  | Tool | "smartink" | "undo" | "redo" | "pager" | "replay" | "exit"
  /** Action buttons a teacher (or their AI) may put on the dock. */
  | "celebrate" | "pages" | "solve" | "settings";

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
 * (the Shapes picker, components/ShapesPicker.tsx, and the Smart shapes toggle). */
export type DockPin = ToolbarItem | "shapes";
/** Button size: small, medium (default), large. */
export type DockSize = "s" | "m" | "l";
export const DOCK_SIZES: DockSize[] = ["s", "m", "l"];
/** A group of dock buttons, in order; groups are separated on the dock. */
export interface DockGroup {
  id: string;
  label?: string;
  items: DockPin[];
}
/** A second dock, e.g. the pager and undo on the other edge of a big board. */
export interface DockSplit {
  position: DockPosition;
  orientation: DockOrientation;
  groups: DockGroup[];
}
export interface DockConfig {
  position: DockPosition;
  orientation: DockOrientation;
  /** Start as the small round bubble. */
  minimized: boolean;
  /** Tools on the dock besides the core ones; the mode's other tools go in ⋯.
   * With `groups`, it is kept as the flat list of what the groups hold. */
  pinned: DockPin[];
  /** The teacher's own grouping and order of the dock's buttons (else the
   * dock groups them itself: ink, create, point, history…). */
  groups?: DockGroup[];
  /** A second dock on another edge, with its own groups. */
  split?: DockSplit | null;
  /** Size of every button, and per-button sizes. */
  size?: DockSize;
  sizes?: Partial<Record<DockPin, DockSize>>;
  /** Large icons, for low vision. */
  large?: boolean;
}
/** Always on the dock in Prepare and Teach; a config cannot remove them. */
export const CORE_TOOLS: ToolbarItem[] = ["pen", "highlighter", "eraser", "laser", "undo"];
export const SHAPE_TOOLS: ToolbarItem[] = ["shape", "rect", "ellipse", "line", "arrow"];
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
  { id: "shape", icon: "rect", label: "Shapes and tables", key: "S", kind: "tool" },
  { id: "stamp", icon: "stamp", label: "Stamps", key: "K", kind: "tool" },
  { id: "voice", icon: "voiceText", label: "Voice text", kind: "tool" },
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
  { id: "celebrate", icon: "celebrate", label: "Celebrate", kind: "action" },
  { id: "pages", icon: "pages", label: "Pages", key: "G", kind: "action" },
  { id: "solve", icon: "totext", label: "Solve this", kind: "action" },
  { id: "settings", icon: "settings", label: "Settings", kind: "action" },
];
/** "shapes" is a dock pin, not a toolbar entry: the Shapes button. */
export const SHAPES_PIN = { id: "shapes" as const, icon: "rect" as IconName, label: "Shapes and tables" };

export const CHROME_ITEMS: { id: ChromeKey; label: string }[] = [
  { id: "switcher", label: "Mode switcher" },
  { id: "logo", label: "Logo" },
  { id: "title", label: "Lesson title" },
  { id: "pagesToggle", label: "Pages button" },
  { id: "history", label: "Undo and redo" },
  { id: "insert", label: "Insert menu (shape, table, image, PDF, Smart Object, text)" },
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
  view: ["pager", "hand", "interact", "replay", "voice"],
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
    tools: ["pen", "highlighter", "eraser", "shape", "rect", "ellipse", "line", "arrow", "smartink", "text", "voice", "stamp", "lasso", "laser", "hand", "interact", "undo", "redo"],
    dock: { position: "auto", orientation: "auto", minimized: false, pinned: ["shapes", "text", "voice", "stamp", "lasso"] },
  },
  teach: {
    chrome: chrome(["colors", "sizes"]),
    tools: ["pen", "highlighter", "eraser", "shape", "text", "voice", "stamp", "lasso", "laser", "smartink", "undo", "pager", "exit"],
    dock: { position: "auto", orientation: "auto", minimized: false, pinned: ["shapes", "text", "voice", "stamp", "lasso", "smartink", "pager", "exit"] },
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
  // A custom layout's buttons are the mode's tools too (Shapes brings the shape tool).
  for (const pin of [...(Array.isArray(d.groups) ? d.groups : []), ...(Array.isArray((d.split as any)?.groups) ? (d.split as any).groups : [])]
    .flatMap((g: any) => (Array.isArray(g) ? g : Array.isArray(g?.items) ? g.items : []))) {
    const p = pinOf(m, pin);
    const t: ToolbarItem | null = p === "shapes" ? "shape" : p;
    if (t && !out.tools.includes(t)) out.tools.push(t);
  }
  out.dock = sanitizeDock(m, d, customTools && !Array.isArray(d.pinned) ? out.tools : null, out.tools, def.dock);
  return out;
}

/** Things a teacher should know about a dock layout before keeping it. */
export function dockWarnings(m: Mode, c: ModeConfig): string[] {
  const w: string[] = [];
  const has = (t: ToolbarItem) => c.tools.includes(t);
  if (m === "teach" && !has("exit") && !has("settings") && !c.chrome.settings) {
    w.push("There is no button back to Prepare or Settings in Teach (Esc still leaves Teach).");
  }
  if (m === "prepare" && !c.chrome.settings && !has("settings")) {
    w.push("There is no Settings button: add Settings to the dock or show it in the top bar.");
  }
  return w;
}

// ---------------------------------------------------------- dock presets

export interface DockPreset {
  name: string;
  hint: string;
  /** Dock layouts per mode (validated like any config). */
  modes: Partial<Record<Exclude<Mode, "view">, Partial<DockConfig>>>;
  /** Also switch the writing hand. */
  hand?: "left" | "right";
}

/** Named layouts. "Default" resets both modes' docks. */
export const DOCK_PRESETS: DockPreset[] = [
  { name: "Default", hint: "The dock as Rupok ships it", modes: {} },
  {
    name: "Left-handed", hint: "Tools on the left edge, popovers open to the right", hand: "left",
    modes: {
      prepare: { position: "left", orientation: "vertical" },
      teach: { position: "left", orientation: "vertical" },
    },
  },
  {
    name: "Minimal teaching", hint: "Teach with pen, eraser, undo and the pager, big buttons",
    modes: {
      teach: {
        size: "l",
        groups: [
          { id: "ink", items: ["pen", "highlighter", "eraser", "laser"] },
          { id: "history", items: ["undo"] },
          { id: "nav", items: ["pager", "exit"] },
        ],
      },
    },
  },
  {
    name: "Maths class", hint: "Shapes, smart ink and text up front; undo and the pager on the right",
    modes: {
      prepare: {
        groups: [
          { id: "ink", label: "Ink", items: ["pen", "highlighter", "eraser"] },
          { id: "maths", label: "Maths", items: ["shapes", "smartink", "text", "solve"] },
          { id: "point", items: ["lasso", "laser"] },
        ],
        split: { position: "right", orientation: "vertical", groups: [{ id: "history", items: ["undo", "redo"] }, { id: "nav", items: ["pages"] }] },
      },
      teach: {
        groups: [
          { id: "ink", items: ["pen", "highlighter", "eraser"] },
          { id: "maths", items: ["shapes", "smartink", "solve"] },
          { id: "point", items: ["laser", "lasso"] },
        ],
        split: { position: "right", orientation: "vertical", groups: [{ id: "history", items: ["undo"] }, { id: "nav", items: ["pager", "exit"] }] },
      },
    },
  },
];

/** The teacher's saved layouts from untrusted storage or a UI config file. */
export function sanitizeDockPresets(raw: unknown): DockPreset[] {
  if (!Array.isArray(raw)) return [];
  const out: DockPreset[] = [];
  for (const r of raw.slice(0, 12)) {
    if (!r || typeof r !== "object" || typeof (r as any).name !== "string" || !(r as any).name.trim()) continue;
    const name = (r as any).name.trim().slice(0, 32);
    if (DOCK_PRESETS.some((p) => p.name.toLowerCase() === name.toLowerCase())) continue;
    const modes: DockPreset["modes"] = {};
    for (const m of ["prepare", "teach"] as const) {
      const d = (r as any).modes?.[m];
      if (d && typeof d === "object") modes[m] = sanitizeMode(m, { dock: d }).dock;
    }
    const hand = (r as any).hand === "left" || (r as any).hand === "right" ? (r as any).hand : undefined;
    out.push({ name, hint: "Saved by you", modes, ...(hand ? { hand } : {}) });
  }
  return out;
}

/** A preset (built-in or saved) applied to the stored modes: each mode's dock
 * is replaced (validated), other keys kept. */
export function applyDockPreset(stored: Partial<ModesConfig> | undefined, p: DockPreset): Partial<ModesConfig> {
  const next: Partial<ModesConfig> = { ...(stored ?? {}) };
  for (const m of ["prepare", "teach"] as const) {
    const cur = resolveMode(stored, m);
    const dock = p.modes[m];
    if (p.name === "Default") next[m] = sanitizeMode(m, { chrome: cur.chrome, tools: cur.tools, dock: DEFAULT_MODES[m].dock });
    else if (dock) next[m] = sanitizeMode(m, { chrome: cur.chrome, tools: cur.tools, dock: { ...DEFAULT_MODES[m].dock, ...dock } });
  }
  return next;
}

/** A dock pin from untrusted input, or null (shape tools fold into "shapes"). */
function pinOf(m: Mode, t: unknown): DockPin | null {
  if (t === "shapes" || SHAPE_TOOLS.includes(t as ToolbarItem)) return ALLOWED_TOOLS[m].some((x) => SHAPE_TOOLS.includes(x)) ? "shapes" : null;
  return ALLOWED_TOOLS[m].includes(t as ToolbarItem) ? (t as ToolbarItem) : null;
}

function cleanGroups(m: Mode, raw: unknown, seen: Set<DockPin>): DockGroup[] {
  if (!Array.isArray(raw)) return [];
  const out: DockGroup[] = [];
  for (const [i, g] of raw.slice(0, 12).entries()) {
    const r = (Array.isArray(g) ? { items: g } : g && typeof g === "object" ? g : null) as Record<string, unknown> | null;
    if (!r || !Array.isArray(r.items)) continue;
    const items: DockPin[] = [];
    for (const t of r.items.slice(0, 32)) {
      const pin = pinOf(m, t);
      if (pin && !seen.has(pin)) {
        seen.add(pin);
        items.push(pin);
      }
    }
    if (!items.length) continue;
    const id = typeof r.id === "string" && /^[a-z0-9-]{1,24}$/.test(r.id) && !out.some((x) => x.id === r.id) ? r.id : `g${i + 1}`;
    const label = typeof r.label === "string" && r.label.trim() ? r.label.trim().slice(0, 24) : undefined;
    out.push(label ? { id, label, items } : { id, items });
  }
  return out;
}

function cleanPosition(p: unknown): DockPosition | null {
  if (typeof p === "string" && (DOCK_POSITIONS as readonly string[]).includes(p)) return p as DockPosition;
  if (p && typeof p === "object") {
    const { x, y } = p as { x?: unknown; y?: unknown };
    if (typeof x === "number" && typeof y === "number" && Number.isFinite(x) && Number.isFinite(y)) {
      return { x: Math.min(1, Math.max(0, x)), y: Math.min(1, Math.max(0, y)) };
    }
  }
  return null;
}

const cleanOrientation = (o: unknown): DockOrientation | null => (o === "auto" || o === "horizontal" || o === "vertical" ? o : null);

/** The custom layout keys of a dock (groups, split, sizes) from untrusted
 * JSON; core tools that were left out go back into the first group. */
function sanitizeLayout(m: Mode, d: Record<string, unknown>, out: DockConfig) {
  if (typeof d.size === "string" && (DOCK_SIZES as string[]).includes(d.size)) out.size = d.size as DockSize;
  if (typeof d.large === "boolean") out.large = d.large;
  if (d.sizes && typeof d.sizes === "object") {
    const sizes: Partial<Record<DockPin, DockSize>> = {};
    for (const [k, v] of Object.entries(d.sizes as Record<string, unknown>)) {
      const pin = pinOf(m, k);
      if (pin && typeof v === "string" && (DOCK_SIZES as string[]).includes(v)) sizes[pin] = v as DockSize;
    }
    if (Object.keys(sizes).length) out.sizes = sizes;
  }
  if (!Array.isArray(d.groups) && !(d.split && typeof d.split === "object")) return;
  const seen = new Set<DockPin>();
  const groups = cleanGroups(m, d.groups, seen);
  let split: DockSplit | null = null;
  if (d.split && typeof d.split === "object") {
    const s = d.split as Record<string, unknown>;
    const sg = cleanGroups(m, s.groups, seen);
    if (sg.length) split = { position: cleanPosition(s.position) ?? "right", orientation: cleanOrientation(s.orientation) ?? "auto", groups: sg };
  }
  if (hasCore(m)) {
    const missing = CORE_TOOLS.filter((t) => !seen.has(t));
    if (missing.length) {
      if (groups.length) groups[0] = { ...groups[0], items: [...missing, ...groups[0].items] };
      else groups.push({ id: "core", items: missing });
    }
  }
  if (groups.length) out.groups = groups;
  if (split) out.split = split;
}

/** Every pin a custom layout puts on the docks, main then split. */
export function layoutPins(d: DockConfig): DockPin[] {
  return [...(d.groups ?? []), ...(d.split?.groups ?? [])].flatMap((g) => g.items);
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
  sanitizeLayout(m, d, out);
  if (out.groups || out.split) {
    // A custom layout says what is on the dock; `pinned` mirrors it.
    out.pinned = layoutPins(out).filter((p) => !(hasCore(m) && CORE_TOOLS.includes(p as ToolbarItem)));
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
  /** The teacher's saved dock layouts ("My layouts"), by name. */
  dockPresets?: DockPreset[];
}
