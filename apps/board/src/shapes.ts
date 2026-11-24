// The Shapes picker's state: the current shape, recent shapes, the fill for
// new shapes, the square lock, what happens after a shape is placed, and the
// request that opens the picker (the dock's Shapes button, Insert ▾ → Shape…
// or Table…, the S key). The picker itself is components/ShapesPicker.tsx;
// the geometry is the engine's (crates/rupok-wasm/src/catalog.rs).

import { createEffect, createRoot, createSignal, on } from "solid-js";
import { controller } from "./controller";
import { jsonStore } from "./settings/bind";
import type { Mode } from "./modes";
import { PEN_COLORS, color, mode, revision, sizes, tool, width } from "./state";

export interface ShapeDef { id: string; label: string; key?: string }
export interface ShapeGroup { id: "basic" | "lines" | "teaching"; label: string; items: ShapeDef[] }

/** The picker, in order. Ids are the engine's (`catalog::IDS`). */
export const SHAPE_GROUPS: ShapeGroup[] = [
  {
    id: "basic", label: "Basic", items: [
      { id: "rect", label: "Rectangle" }, { id: "rrect", label: "Rounded rectangle" }, { id: "square", label: "Square" },
      { id: "ellipse", label: "Ellipse" }, { id: "circle", label: "Circle" }, { id: "triangle", label: "Triangle" },
      { id: "right-triangle", label: "Right triangle" }, { id: "diamond", label: "Diamond" }, { id: "pentagon", label: "Pentagon" },
      { id: "hexagon", label: "Hexagon" }, { id: "star", label: "Star" }, { id: "parallelogram", label: "Parallelogram" },
      { id: "trapezoid", label: "Trapezoid" },
    ],
  },
  {
    id: "lines", label: "Lines", items: [
      { id: "line", label: "Line" }, { id: "arrow", label: "Arrow" }, { id: "double-arrow", label: "Double arrow" },
      { id: "dashed-line", label: "Dashed line" }, { id: "curved-arrow", label: "Curved arrow" },
      { id: "connector", label: "Connector (sticks to shapes)" },
    ],
  },
  {
    id: "teaching", label: "Teaching", items: [
      { id: "numberline", label: "Number line" }, { id: "axes", label: "Coordinate axes" },
      { id: "venn2", label: "Venn diagram, 2 sets" }, { id: "venn3", label: "Venn diagram, 3 sets" },
      { id: "terminator", label: "Flowchart: start / end" }, { id: "process", label: "Flowchart: process" },
      { id: "decision", label: "Flowchart: decision" }, { id: "data", label: "Flowchart: input / output" },
      { id: "callout", label: "Speech bubble" }, { id: "brace", label: "Brace {" }, { id: "brace-down", label: "Brace under" },
    ],
  },
];

const ALL = new Map(SHAPE_GROUPS.flatMap((g) => g.items.map((s) => [s.id, s] as const)));

/** A table id is `table:RxC` (1–10 each). */
export const tableId = (rows: number, cols: number) => `table:${rows}x${cols}`;
export function parseTable(id: string): [number, number] | null {
  const m = /^table:(\d+)x(\d+)$/.exec(id);
  if (!m) return null;
  const r = Number(m[1]), c = Number(m[2]);
  return r >= 1 && r <= 10 && c >= 1 && c <= 10 ? [r, c] : null;
}
export const isShapeId = (id: string) => ALL.has(id) || parseTable(id) !== null;
export function shapeLabel(id: string): string {
  const t = parseTable(id);
  return t ? `Table ${t[0]} × ${t[1]}` : ALL.get(id)?.label ?? id;
}

/** After placing a shape: keep the tool, select the shape, or back to the pen. */
export type AfterPlace = "keep" | "select" | "pen";

/** Fill opacities (the alpha byte): solid, half, soft. */
export const FILL_ALPHAS: { a: number; label: string }[] = [
  { a: 0xff, label: "Solid" },
  { a: 0x80, label: "50%" },
  { a: 0x33, label: "20%" },
];
/** A new fill starts soft, so ink and overlaps show through. */
export const DEFAULT_FILL_ALPHA = 0x33;

/** The fill colours on offer: the outline's own colour first, then the
 * pen palette (alpha is chosen separately). 0xRRGGBB00. */
export function fillColors(outline: number): { rgb: number; label: string }[] {
  const pal = PEN_COLORS.map((c) => (c & 0xffffff00) >>> 0);
  const own = (outline & 0xffffff00) >>> 0;
  return [
    { rgb: own, label: "Outline colour, soft" },
    ...pal.filter((c) => c !== own).map((c) => ({ rgb: c, label: `Colour #${(c >>> 8).toString(16).padStart(6, "0")}` })),
  ];
}

/** `#rrggbbaa` (from the engine) as 0xRRGGBBAA, or 0. */
export const parseRgba = (css: string | null | undefined): number =>
  css && /^#[0-9a-f]{8}$/i.test(css) ? parseInt(css.slice(1), 16) >>> 0 : 0;

export const shapeStore = jsonStore("rupok.shapes", {
  /** The shape tool's current shape. */
  last: "rect",
  /** Recently used, newest first (quick access row). */
  recent: [] as string[],
  /** Fill of new closed shapes; 0 = none. */
  fill: 0,
  /** Square boxes, circles and 15° lines without holding Shift. */
  lock: false,
  afterPrepare: "select" as AfterPlace,
  afterTeach: "pen" as AfterPlace,
});

export const currentShape = () => (isShapeId(shapeStore.read().last) ? shapeStore.read().last : "rect");
export const recentShapes = () => shapeStore.read().recent.filter(isShapeId).slice(0, 6);
export const afterPlace = (m: Mode = mode()): AfterPlace =>
  m === "teach" ? shapeStore.read().afterTeach : shapeStore.read().afterPrepare;

/** The engine's tool name for the shape tool. */
export const shapeToolName = () => `shape:${currentShape()}`;

/** Use shape `id`: the shape tool with that shape, remembered as recent. */
export function pickShape(id: string) {
  if (!isShapeId(id)) return;
  const recent = [id, ...shapeStore.read().recent.filter((r) => r !== id)].slice(0, 6);
  shapeStore.write({ last: id, recent });
  controller.setTool("shape");
  // Shapes draw in the pen's colour and size.
  controller.board.set_color(color());
  controller.board.set_width(sizes().pen ?? width());
}

export function setShapeFill(rgba: number) {
  shapeStore.write({ fill: rgba >>> 0 });
  controller.board.set_shape_fill(rgba >>> 0);
}

let shiftDown = false;
export function setShapeLock(on: boolean) {
  shapeStore.write({ lock: on });
  controller.board.set_shape_lock(on || shiftDown);
}

// ------------------------------------------------------- open the picker

/** A request to open the picker; `focus: "table"` opens it at the table grid. */
const [request, setRequest] = createSignal<{ n: number; focus?: "table" } | null>(null);
export const shapesRequest = request;
let reqN = 0;
export function openShapesPicker(focus?: "table") {
  setRequest({ n: ++reqN, focus });
}
export const clearShapesRequest = () => setRequest(null);

// ------------------------------------------------- engine hookup (once)

/** Push the picker state to the engine, follow Shift, and act after a
 * placement (select it or go back to the pen). Called once at start-up. */
export function initShapes() {
  const b = controller.board;
  b.set_shape_fill(shapeStore.read().fill >>> 0);
  b.set_shape_lock(shapeStore.read().lock);
  const shift = (e: KeyboardEvent) => {
    if (e.key !== "Shift" || shiftDown === e.shiftKey) return;
    shiftDown = e.type === "keydown";
    b.set_shape_lock(shapeStore.read().lock || shiftDown);
  };
  window.addEventListener("keydown", shift);
  window.addEventListener("keyup", shift);
  window.addEventListener("blur", () => {
    shiftDown = false;
    b.set_shape_lock(shapeStore.read().lock);
  });
  createRoot(() => {
    createEffect(on(revision, () => {
      const json = b.take_placed_shape();
      if (!json || tool() !== "shape") return;
      const after = afterPlace();
      if (after === "pen") controller.setTool("pen");
      else if (after === "select") {
        controller.setTool("lasso");
        b.select_json(json);
      }
    }, { defer: true }));
  });
}
