// The Smart Dock's runtime state and geometry (the UI is components/Dock.tsx,
// the shell shared with the overlay is components/DockShell.tsx).
//
// What a teacher or an AI configures lives in the mode config
// (modes.ts → `dock`). What the dock learns while it is used lives here, in
// its own localStorage key: where it was dragged (per mode and per screen-size
// bucket), whether it is minimised, the three recent colours and the last
// shape. When the mode's dock config changes (Settings, an imported UI
// config), the learnt place and minimised state of that mode are dropped, so
// the new config takes effect.
//
// `summonDock(x, y)` is the public "bring the dock here" entry point for
// gestures (gestures.ts owns the two-finger long-press).

import { createSignal } from "solid-js";
import { APP_SLUG } from "./brand";
import { type DockOrientation, type DockPosition, type Mode, type ModeConfig, hasTopBar } from "./modes";
import { type Tool, prefs } from "./state";
import { cssPx, isPhone, keyboard, safeArea } from "./viewport";

/** Which screen edge the dock sits on; "float" is anywhere. */
export type DockEdge = "bottom" | "top" | "left" | "right" | "float";
/** A dock place: an edge plus the dock's position as fractions (0..1) of the
 * free range along each axis (0 = flush left/top, 1 = flush right/bottom).
 * Fractions of the free range keep the dock on screen by construction, at
 * any window size and any dock width. */
export interface DockPlace {
  edge: DockEdge;
  x: number;
  y: number;
  /** Orientation of a floating dock (edges decide their own). */
  vertical?: boolean;
}

// ------------------------------------------------------------------ store

const KEY = `${APP_SLUG}.dock`;
type Kind = "pen" | "highlighter";
interface ModeDock { cfg: string; min?: boolean; places: Record<string, DockPlace> }
interface DockStore {
  modes: Partial<Record<Mode, ModeDock>>;
  /** Three colour slots per ink tool; slots keep their position. */
  slots: Record<Kind, number[]>;
  /** Slot colours, most recently used first (decides which slot a new colour replaces). */
  used: Record<Kind, number[]>;
  shape: Tool;
  /** Opacity of new highlighter strokes. */
  hlOpacity: number;
}

const GRAPHITE = 0x1c2024ff;
const WHITE = 0xffffffff;
const DEFAULT_SLOTS: Record<Kind, number[]> = { pen: [GRAPHITE, 0x3e63ddff, 0xe5484dff], highlighter: [0xffe629ff, 0x86ead4ff, 0xf5a9b8ff] };
const SHAPES: Tool[] = ["rect", "ellipse", "line", "arrow"];
const EDGES: DockEdge[] = ["bottom", "top", "left", "right", "float"];

const unit = (n: unknown, d: number) => (typeof n === "number" && Number.isFinite(n) ? Math.min(1, Math.max(0, n)) : d);
const isColor = (c: unknown): c is number => typeof c === "number" && Number.isInteger(c) && c >= 0 && c <= 0xffffffff;

function cleanPlace(p: unknown): DockPlace | null {
  if (!p || typeof p !== "object") return null;
  const r = p as Record<string, unknown>;
  if (!EDGES.includes(r.edge as DockEdge)) return null;
  return { edge: r.edge as DockEdge, x: unit(r.x, 0.5), y: unit(r.y, 1), ...(r.vertical === true ? { vertical: true } : {}) };
}

/** Untrusted (localStorage): anything odd falls back to the defaults. */
function load(): DockStore {
  const out: DockStore = { modes: {}, slots: structuredClone(DEFAULT_SLOTS), used: structuredClone(DEFAULT_SLOTS), shape: "rect", hlOpacity: 0.38 };
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) || "{}") as Record<string, any>;
    for (const m of ["prepare", "teach", "view"] as Mode[]) {
      const md = raw.modes?.[m];
      if (!md || typeof md !== "object" || typeof md.cfg !== "string") continue;
      const places: Record<string, DockPlace> = {};
      for (const [k, v] of Object.entries(md.places ?? {}).slice(0, 24)) {
        const p = cleanPlace(v);
        if (p && /^[a-z0-9-]{1,16}$/.test(k)) places[k] = p;
      }
      out.modes[m] = { cfg: md.cfg.slice(0, 400), places, ...(typeof md.min === "boolean" ? { min: md.min } : {}) };
    }
    for (const k of ["pen", "highlighter"] as Kind[]) {
      const s = raw.slots?.[k];
      if (Array.isArray(s) && s.length === 3 && s.every(isColor)) out.slots[k] = s;
      const u = raw.used?.[k];
      if (Array.isArray(u) && u.every(isColor)) out.used[k] = u.filter((c: number) => out.slots[k].includes(c)).slice(0, 3);
    }
    if (SHAPES.includes(raw.shape)) out.shape = raw.shape;
    if (typeof raw.hlOpacity === "number" && raw.hlOpacity >= 0.1 && raw.hlOpacity <= 1) out.hlOpacity = raw.hlOpacity;
  } catch { /* storage unavailable or corrupt */ }
  return out;
}

const [store, setStore] = createSignal<DockStore>(load());
function save(next: DockStore) {
  setStore(next);
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch { /* storage unavailable */ }
}

// -------------------------------------------------------- place, minimised

/** Viewport bucket, so a dock placed on the 86" panel and one placed on the
 * laptop that prepared the lesson don't overwrite each other. */
export function screenBucket(w: number, h: number): string {
  const size = w < 1024 ? "s" : w < 1600 ? "m" : w < 2200 ? "l" : "xl";
  return h > w * 1.1 ? `${size}-tall` : size;
}

/** The learnt state is valid only for the config it was learnt under. */
const cfgKey = (c: ModeConfig) => JSON.stringify([c.dock.position, c.dock.orientation, c.dock.minimized, prefs().toolbar]);

export function presetPlace(pos: DockPosition, toolbar: "bottom" | "left" | "right"): DockPlace {
  if (typeof pos === "object") return { edge: "float", x: pos.x, y: pos.y };
  switch (pos === "auto" ? toolbar : pos) {
    case "top": return { edge: "top", x: 0.5, y: 0 };
    case "left": return { edge: "left", x: 0, y: 0.5 };
    case "right": return { edge: "right", x: 1, y: 0.5 };
    case "bottom-left": return { edge: "bottom", x: 0, y: 1 };
    case "bottom-right": return { edge: "bottom", x: 1, y: 1 };
    case "top-left": return { edge: "top", x: 0, y: 0 };
    case "top-right": return { edge: "top", x: 1, y: 0 };
    default: return { edge: "bottom", x: 0.5, y: 1 };
  }
}

function modeDock(m: Mode, c: ModeConfig): ModeDock | undefined {
  const md = store().modes[m];
  return md && md.cfg === cfgKey(c) ? md : undefined;
}

export function dockPlace(m: Mode, c: ModeConfig, bucket: string): DockPlace {
  return modeDock(m, c)?.places[bucket] ?? presetPlace(c.dock.position, prefs().toolbar);
}

export function setDockPlace(m: Mode, c: ModeConfig, bucket: string, p: DockPlace) {
  const s = store();
  const md = modeDock(m, c) ?? { cfg: cfgKey(c), places: {} };
  save({ ...s, modes: { ...s.modes, [m]: { ...md, places: { ...md.places, [bucket]: p } } } });
}

export function dockMinimized(m: Mode, c: ModeConfig): boolean {
  return modeDock(m, c)?.min ?? c.dock.minimized;
}

export function setDockMinimized(m: Mode, c: ModeConfig, min: boolean) {
  const s = store();
  const md = modeDock(m, c) ?? { cfg: cfgKey(c), places: {} };
  save({ ...s, modes: { ...s.modes, [m]: { ...md, min } } });
}

/** Forget where the dock was dragged in this mode (back to its configured place). */
export function resetDock(m: Mode) {
  const s = store();
  const modes = { ...s.modes };
  delete modes[m];
  save({ ...s, modes });
}

/** Forget every learnt place and minimised state (all modes, all screen sizes). */
export function resetDockAll() {
  save({ ...store(), modes: {} });
}

export function isVertical(p: DockPlace, o: DockOrientation): boolean {
  if (o !== "auto") return o === "vertical";
  return p.edge === "left" || p.edge === "right" || (p.edge === "float" && !!p.vertical);
}

// ------------------------------------------------------------- geometry

/** Screen margins the dock keeps clear of: the inset, the top bar, the pages
 * panel, the zoom chip. */
export interface DockFrame { w: number; h: number; top: number; left: number; right: number; bottom: number }

const INSET = 12;
const GAP = 8;

/** Margins for a dock on `edge` in the current mode (CSS px). `x` is where
 * along the width the dock is (0..1): only a dock towards the right end
 * keeps clear of the zoom chip, so a centred dock stays centred. */
export function dockFrame(c: ModeConfig, edge: DockEdge, vertical: boolean, w: number, h: number, pagesOn: boolean, x = 0.5): DockFrame {
  const topBar = hasTopBar(c);
  // Phones: a tighter inset, the taller phone top bar, and the safe area
  // (notch, home indicator); the on-screen keyboard pushes a bottom dock up.
  const phone = isPhone();
  const inset = phone ? 8 : INSET;
  const s = safeArea();
  const kb = keyboard();
  const f: DockFrame = {
    w, h, top: s.top + inset + (topBar ? cssPx("--bar", 48) + GAP : 0), left: s.left + inset, right: s.right + inset, bottom: s.bottom + inset + kb,
  };
  // The pages panel runs from under the top bar to 76px above the bottom (a drawer over the board on phones).
  if (pagesOn && !phone && (vertical || edge === "top")) f.left = INSET + 176 + GAP;
  // The zoom chip sits in the bottom-right corner; on a phone it floats above
  // the dock (styles.css), so a bottom dock keeps the thumb's edge.
  // Tablets do the same under 1100px, so a centred dock has the full width.
  if (c.chrome.zoom && !phone) {
    if (edge === "bottom" && !vertical && w < 1100) { /* the zoom chip sits above the dock */ }
    else if (edge === "bottom" && !vertical && x > 0.75) f.right = INSET + 136 + GAP;
    else if (vertical && x > 0.75) f.bottom = INSET + 40 + GAP;
  }
  return f;
}

/** Pixel rect of a dock of size `sw × sh` at place `p` (mirrors the CSS in DockShell). */
export function placeRect(p: DockPlace, sw: number, sh: number, f: DockFrame) {
  const fw = Math.max(0, f.w - f.left - f.right - sw);
  const fh = Math.max(0, f.h - f.top - f.bottom - sh);
  return { x: f.left + fw * p.x, y: f.top + fh * p.y, w: sw, h: sh };
}

const EDGE_ZONE = 64;
const MAGNET = 24;

/** Where a dragged dock lands: the grip stays under the pointer; near a
 * screen edge it docks there (vertical on the left and right), near the ends
 * and the middle of an edge it snaps, elsewhere it floats. `grip` is the grip
 * centre relative to the dock's top-left; `size` is the current dock size. */
export function snapPlace(
  px: number, py: number, grip: { x: number; y: number }, size: { w: number; h: number },
  frameFor: (edge: DockEdge, vertical: boolean, x?: number) => DockFrame, wasVertical: boolean, o: DockOrientation,
): DockPlace {
  const { w, h } = frameFor("float", wasVertical);
  const nearL = px < EDGE_ZONE, nearR = px > w - EDGE_ZONE, nearB = py > h - EDGE_ZONE;
  const nearT = py < frameFor("top", false).top + EDGE_ZONE / 2;
  let edge: DockEdge = "float";
  const side = nearL ? "left" : nearR ? "right" : null;
  const end = nearB ? "bottom" : nearT ? "top" : null;
  // In a corner, keep the orientation the dock already has.
  if (side && end) edge = wasVertical ? side : end;
  else edge = side ?? end ?? "float";
  const vertical = isVertical({ edge, x: 0, y: 0, vertical: wasVertical }, o);
  // Turning on its side swaps the dock's width and height.
  const [sw, sh] = vertical === wasVertical ? [size.w, size.h] : [size.h, size.w];
  const f = frameFor(edge, vertical, px / Math.max(1, w));
  const fw = f.w - f.left - f.right - sw, fh = f.h - f.top - f.bottom - sh;
  const frac = (v: number, free: number) => (free > 0 ? Math.min(1, Math.max(0, v / free)) : 0.5);
  let x = frac(px - grip.x - f.left, fw);
  let y = frac(py - grip.y - f.top, fh);
  const magnet = (v: number, free: number) => {
    for (const a of [0, 0.5, 1]) if (Math.abs(v - a) * Math.max(free, 1) < MAGNET) return a;
    return v;
  };
  if (edge === "bottom") y = 1;
  if (edge === "top") y = 0;
  if (edge === "left") x = 0;
  if (edge === "right") x = 1;
  if (edge !== "left" && edge !== "right") x = magnet(x, fw);
  if (edge !== "top" && edge !== "bottom") y = magnet(y, fh);
  return edge === "float" ? { edge, x, y, ...(vertical ? { vertical: true } : {}) } : { edge, x, y };
}

/** A floating place that centres the dock on (cx, cy), clamped on screen. */
export function placeAround(cx: number, cy: number, size: { w: number; h: number }, f: DockFrame, vertical: boolean): DockPlace {
  const fw = f.w - f.left - f.right - size.w, fh = f.h - f.top - f.bottom - size.h;
  const frac = (v: number, free: number) => (free > 0 ? Math.min(1, Math.max(0, v / free)) : 0.5);
  const x = frac(cx - size.w / 2 - f.left, fw);
  const y = frac(cy - size.h / 2 - f.top, fh);
  if (y === 1 && !vertical) return { edge: "bottom", x, y };
  return { edge: "float", x, y, ...(vertical ? { vertical: true } : {}) };
}

/** The screen edge the board should keep clear for the dock (controller insets). */
export const [dockSide, setDockSide] = createSignal<"bottom" | "top" | "left" | "right" | "none">(
  (() => {
    try {
      const p = presetPlace("auto", prefs().toolbar);
      return p.edge === "float" ? "none" : p.edge;
    } catch {
      return "bottom";
    }
  })(),
);

// --------------------------------------------------------------- colours

const same = (a: number, b: number) => a >>> 8 === b >>> 8;
/** Graphite and white are one "contrast ink" slot: shown as white on dark boards. */
const equiv = (a: number, b: number) => same(a, b) || ([a, b].every((c) => same(c, GRAPHITE) || same(c, WHITE)));

export function inkForBoard(c: number, darkBoard: boolean): number {
  if (darkBoard && same(c, GRAPHITE)) return WHITE;
  if (!darkBoard && same(c, WHITE)) return GRAPHITE;
  return c;
}

export const recentColors = (k: Kind) => store().slots[k];

/** A colour was used: keep the slot if it is already there (slots never move
 * under the teacher's finger), else replace the least recently used slot. */
export function useColor(k: Kind, c: number) {
  const s = store();
  const slots = [...s.slots[k]];
  let at = slots.findIndex((x) => equiv(x, c));
  if (at < 0) {
    const used = s.used[k];
    const lru = slots.map((x, i) => ({ i, rank: used.findIndex((u) => same(u, x)) })).sort((a, b) => (b.rank < 0 ? 99 : b.rank) - (a.rank < 0 ? 99 : a.rank))[0];
    at = lru.i;
    slots[at] = c;
  }
  const used = [slots[at], ...s.used[k].filter((u) => !same(u, slots[at]) && slots.some((x) => same(x, u)))].slice(0, 3);
  save({ ...s, slots: { ...s.slots, [k]: slots }, used: { ...s.used, [k]: used } });
}

export const hlOpacity = () => store().hlOpacity;
export function setHlOpacityStored(a: number) {
  save({ ...store(), hlOpacity: Math.min(1, Math.max(0.1, a)) });
}

export const lastShape = () => store().shape;
export function setLastShape(t: Tool) {
  if (SHAPES.includes(t) && store().shape !== t) save({ ...store(), shape: t });
}

// ---------------------------------------------------------------- summon

let summonHandler: ((x: number, y: number) => void) | null = null;

/** The mounted dock registers how it comes to a point. */
export function registerDockSummon(f: ((x: number, y: number) => void) | null) {
  summonHandler = f;
}

/** "Bring the dock here": moves the dock (expanded) next to (x, y), in client
 * px, e.g. where the teacher's fingers are on an 86" panel. Returns false when
 * no dock is on screen. Public API for gestures.ts. */
export function summonDock(x: number, y: number): boolean {
  if (!summonHandler || !Number.isFinite(x) || !Number.isFinite(y)) return false;
  summonHandler(x, y);
  return true;
}

// ---------------------------------------------------------- the editor

/** The "Customize dock" editor (components/DockEditor.tsx): the mode it
 * edits, or null when closed. Opened from Settings → Teaching modes, the
 * dock's ⋯ menu, or by holding the dock's grip. */
export const [dockEditor, setDockEditor] = createSignal<Mode | null>(null);
export function openDockEditor(m: Mode) {
  setDockEditor(m === "view" ? "prepare" : m);
}
