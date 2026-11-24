// Selection frame, resize handles and the floating selection toolbar.
//
// Drawn in screen space above the input layer, so the frame stays crisp and
// the handles keep one size at any zoom. The layer itself ignores the
// pointer; only the handles and the toolbar take it. Moving the selection
// (dragging inside the frame) is the engine's job: the press falls through to
// the input layer, which works for strokes, box items and mixed selections.
// Resizing is for a single box item (object, image, text) and is previewed
// locally, then committed once on release with `set_geom` / `resize_text`.
// A single shape stroke (shape tools, Smart Ink) resizes too: the engine
// regenerates its points from the stored geometry (`resize_shape`), so the
// line width stays the same. Any other selection (handwriting, or ink with
// boxes) scales from its corners (`scale_selection`): ink is previewed by the
// engine, boxes here, and the release commits one undo step.
//
// The toolbar is built for a finger on a board (44px targets): recolour,
// change the pen style, thinner / thicker ink, smaller / bigger, To text,
// duplicate, group and delete. Each change is one undo step in the engine
// (`restyle.rs`). A single stamp scales from its corners about its centre
// and turns with the round handle above it (`set_stamp_geom`).

import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { controller } from "../controller";
import {
  type DomItem, HL_COLORS, PEN_COLORS, camera, editingText, hex, items, replay, selection, selectionIds, selectionMeta, setItems, isView,
} from "../state";
import { Icon } from "./Icon";
import { ToTextButton } from "./ToText";
import { makeItLive } from "./MakeItLive";
import { SolveButton } from "../solve/SolveUI";
import { FillRow, ShapeControls, TableCells, setTableCellMode } from "./ShapeControls";
import { StyleRow } from "./InkOptions";

/** Shapes that keep their proportions while resizing (Shift frees them). */
const KEEP_ASPECT = [
  "circle", "square", "star", "pentagon", "hexagon", "venn", "benzene", "bulb",
  "and-gate", "or-gate", "not-gate", "nand-gate", "nor-gate", "xor-gate", "xnor-gate",
];

export type Handle = "nw" | "n" | "ne" | "e" | "se" | "s" | "sw" | "w";
export interface Geom { x: number; y: number; w: number; h: number }

// Edges first so corner handles paint (and hit-test) on top of them.
const BOX_HANDLES: Handle[] = ["n", "e", "s", "w", "nw", "ne", "se", "sw"];
// Text height follows its content: corners scale the text, sides set the wrap width.
const TEXT_HANDLES: Handle[] = ["e", "w", "nw", "ne", "se", "sw"];
/** Handwriting and mixed selections scale from their corners, keeping shape. */
const SCALE_HANDLES: Handle[] = ["nw", "ne", "se", "sw"];
/** Thicker / thinner multiplies stroke widths by this; bigger / smaller scales by it. */
const WIDTH_STEP = 1.5;
const SIZE_STEP = 1.25;
/** Smallest box a resize can produce, world units. */
const MIN_SIZE = 16;
/** Frame padding around strokes and mixed selections, screen px. */
const LOOSE_PAD = 6;
/** Gap between the frame and the floating toolbar, screen px. */
const BAR_GAP = 12;
/** Side length below which edge handles hide (corners suffice), screen px. */
const EDGE_HANDLE_MIN = 72;

/** New geometry for dragging handle `h` by (dx, dy) world units. With `lock`
 * the aspect ratio is kept: corners scale about the opposite corner, sides
 * scale about the box's centre line. */
export function resizeGeom(s: Geom, h: Handle, dx: number, dy: number, lock: boolean, min = MIN_SIZE): Geom {
  let x0 = s.x, y0 = s.y, x1 = s.x + s.w, y1 = s.y + s.h;
  if (h.includes("w")) x0 = Math.min(x0 + dx, x1 - min);
  if (h.includes("e")) x1 = Math.max(x1 + dx, x0 + min);
  if (h.includes("n")) y0 = Math.min(y0 + dy, y1 - min);
  if (h.includes("s")) y1 = Math.max(y1 + dy, y0 + min);
  if (lock && s.w > 0 && s.h > 0) {
    if (h.length === 2) {
      const sx = (x1 - x0) / s.w, sy = (y1 - y0) / s.h;
      let k = Math.abs(sx - 1) > Math.abs(sy - 1) ? sx : sy;
      k = Math.max(k, min / s.w, min / s.h);
      const w = s.w * k, hh = s.h * k;
      if (h.includes("w")) x0 = x1 - w; else x1 = x0 + w;
      if (h.includes("n")) y0 = y1 - hh; else y1 = y0 + hh;
    } else if (h === "e" || h === "w") {
      const hh = Math.max(min, ((x1 - x0) * s.h) / s.w);
      const w = (hh * s.w) / s.h;
      if (h === "w") x0 = x1 - w; else x1 = x0 + w;
      y0 = s.y + s.h / 2 - hh / 2;
      y1 = y0 + hh;
    } else {
      const w = Math.max(min, ((y1 - y0) * s.w) / s.h);
      const hh = (w * s.h) / s.w;
      if (h === "n") y0 = y1 - hh; else y1 = y0 + hh;
      x0 = s.x + s.w / 2 - w / 2;
      x1 = x0 + w;
    }
  }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

const r1 = (v: number) => Math.round(v * 10) / 10;

function itemEl(id: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`.world [data-item="${CSS.escape(id)}"]`);
}

export function SelectionOverlay() {
  const [viewport, setViewport] = createSignal({ w: window.innerWidth, h: window.innerHeight });
  const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
  window.addEventListener("resize", onResize);
  onCleanup(() => window.removeEventListener("resize", onResize));

  const [resizing, setResizing] = createSignal(false);
  let bar: HTMLDivElement | undefined;

  const show = () =>
    !isView() && selectionIds().length > 0 && !replay() && !editingText() && controller.selecting();

  /** The selected item when exactly one box item is selected. */
  const single = createMemo<DomItem | undefined>(() => {
    const ids = selectionIds();
    return ids.length === 1 ? items.find((i) => i.id === ids[0]) : undefined;
  });

  /** The selection when it resizes as shapes: one shape stroke, or a
   * Smart Ink template with its labels (the engine says which). */
  const shapeSel = createMemo(() => {
    const ids = selectionIds();
    const m = selectionMeta();
    return ids.length >= 1 && !single() && m.shape?.length === 4
      ? { id: ids[0], box: m.shape, name: m.shapeName ?? "" }
      : undefined;
  });
  /** Live geometry while a shape is being resized. */
  const [shapeDrag, setShapeDrag] = createSignal<Geom | null>(null);
  /** Live bounds while handwriting or a mixed selection is being scaled. */
  const [scaleDrag, setScaleDrag] = createSignal<Geom | null>(null);
  /** The colour row of the toolbar is open. */
  const [palette, setPalette] = createSignal(false);
  /** The fill row of the toolbar is open (shapes, ShapeControls.tsx). */
  const [fills, setFills] = createSignal(false);
  /** The table row (rows, columns, type in a cell) is open. */
  const [tableRow, setTableRow] = createSignal(false);
  /** The pen-style row of the toolbar is open. */
  const [styles, setStyles] = createSignal(false);
  createEffect(() => {
    selectionIds();
    setPalette(false);
    setFills(false);
    setTableRow(false);
    setStyles(false);
    setTableCellMode(false);
  });
  /** Selected text items (they take a colour and scale their font). */
  const texts = () => {
    const ids = new Set(selectionIds());
    return items.filter((i) => ids.has(i.id) && i.kind === "text").length;
  };
  const colorable = () => selectionMeta().strokes > 0 || texts() > 0;
  const currentColor = () => selectionMeta().style?.color ?? null;
  const swatches = () => (selectionMeta().style?.highlighter ? HL_COLORS : PEN_COLORS);

  // A text box is as wide as its longest word and as tall as its lines, so
  // measure it rather than trusting the stored size.
  const textId = createMemo(() => (single()?.kind === "text" ? single()!.id : null));
  const [measured, setMeasured] = createSignal<{ id: string; w: number; h: number } | null>(null);
  createEffect(() => {
    const id = textId();
    setMeasured(null);
    if (!id) return;
    const el = itemEl(id);
    if (!el) return;
    const ro = new ResizeObserver(() => setMeasured({ id, w: el.offsetWidth, h: el.offsetHeight }));
    ro.observe(el);
    onCleanup(() => ro.disconnect());
  });

  /** Frame in screen px, snapped to device pixels so its edges stay crisp. */
  const rect = createMemo(() => {
    const [cx, cy, z] = camera();
    const it = single();
    let b: number[] | null;
    let pad = 0;
    if (it) {
      const m = measured();
      const w = m && m.id === it.id ? Math.max(it.w, m.w) : it.w;
      const h = m && m.id === it.id ? m.h : it.h;
      b = [it.x, it.y, it.x + w, it.y + h];
    } else if (shapeSel()) {
      const g = shapeDrag();
      b = g ? [g.x, g.y, g.x + g.w, g.y + g.h] : shapeSel()!.box;
      pad = LOOSE_PAD;
    } else {
      const g = scaleDrag();
      b = g ? [g.x, g.y, g.x + g.w, g.y + g.h] : selection();
      pad = LOOSE_PAD;
    }
    if (!b) return null;
    const dpr = window.devicePixelRatio || 1;
    const snap = (v: number) => Math.round(v * dpr) / dpr;
    const x0 = snap((b[0] - cx) * z - pad), y0 = snap((b[1] - cy) * z - pad);
    const x1 = snap((b[2] - cx) * z + pad), y1 = snap((b[3] - cy) * z + pad);
    return { x: x0, y: y0, w: Math.max(1, x1 - x0), h: Math.max(1, y1 - y0) };
  });

  /** A single stamp: corners scale it about its centre, the knob turns it. */
  const stamp = () => (single()?.stamp ? single() : undefined);
  const handles = createMemo<Handle[]>(() => {
    const it = single();
    if (it?.stamp) return SCALE_HANDLES;
    const sh = shapeSel();
    if (sh) {
      const [x0, y0, x1, y1] = sh.box;
      // A level or upright line resizes along its length only.
      if (y1 - y0 < 1) return ["e", "w"];
      if (x1 - x0 < 1) return ["n", "s"];
      const r = rect();
      return BOX_HANDLES.filter((h) => h.length === 2 || !r || (h === "n" || h === "s" ? r.w >= EDGE_HANDLE_MIN : r.h >= EDGE_HANDLE_MIN));
    }
    if (!it) return selectionIds().length ? SCALE_HANDLES : [];
    const r = rect();
    const all = it.kind === "text" ? TEXT_HANDLES : BOX_HANDLES;
    return all.filter((h) => {
      if (h.length === 2 || !r) return true;
      return h === "n" || h === "s" ? r.w >= EDGE_HANDLE_MIN : r.h >= EDGE_HANDLE_MIN;
    });
  });

  /** Floating toolbar: centred above the frame, else below it, else inside
   * its top edge; always on screen and clear of the top bar. */
  const barPos = createMemo(() => {
    const r = rect();
    if (!r) return null;
    const { w: vw, h: vh } = viewport();
    const bw = bar?.offsetWidth || 148;
    const bh = bar?.offsetHeight || 48;
    const [topInset, rightInset, bottomInset, leftInset] = controller.insets;
    const minTop = topInset;
    const maxTop = vh - bottomInset - bh;
    let top = r.y - BAR_GAP - bh;
    if (top < minTop) top = r.y + r.h + BAR_GAP;
    if (top > maxTop) top = Math.min(Math.max(minTop, r.y + BAR_GAP), maxTop);
    let left = r.x + r.w / 2 - bw / 2;
    left = Math.min(Math.max(left, leftInset), vw - rightInset - bw);
    return { left: Math.round(left), top: Math.round(Math.max(top, minTop)) };
  });

  /** Resize a shape stroke: preview in the engine, commit once on release. */
  const startShapeResize = (h: Handle, e: PointerEvent) => {
    const sh = shapeSel();
    if (!sh || e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const id = sh.id;
    const [x0, y0, x1, y1] = sh.box;
    const start: Geom = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    const flatX = start.w < 1, flatY = start.h < 1;
    const z = camera()[2];
    const p0 = [e.clientX, e.clientY];
    let g = start;
    let moved = false;
    setResizing(true);
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      if (!moved && Math.hypot(ev.clientX - p0[0], ev.clientY - p0[1]) < 2) return;
      moved = true;
      const dx = flatX ? 0 : (ev.clientX - p0[0]) / z, dy = flatY ? 0 : (ev.clientY - p0[1]) / z;
      const keep = KEEP_ASPECT.includes(sh.name);
      const lock = !flatX && !flatY && (keep ? !ev.shiftKey : ev.shiftKey);
      g = resizeGeom(start, h, dx, dy, lock, 4);
      if (flatX) g = { ...g, x: start.x, w: start.w };
      if (flatY) g = { ...g, y: start.y, h: start.h };
      setShapeDrag(g);
      controller.board.resize_shapes(g.x, g.y, g.x + g.w, g.y + g.h, false);
    };
    const finish = (commit: boolean) => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", cancel);
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      setResizing(false);
      setShapeDrag(null);
      if (!moved) return;
      if (commit) controller.board.resize_shapes(g.x, g.y, g.x + g.w, g.y + g.h, true);
      else controller.board.resize_shape_cancel(id);
    };
    const up = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(false);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", cancel);
  };

  /** Scale handwriting (or ink with boxes) from a corner, keeping its shape:
   * the engine previews the ink, the boxes are previewed here, and the
   * release commits one undo step. */
  const startScale = (h: Handle, e: PointerEvent) => {
    const b = selection();
    if (!b || e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const start: Geom = { x: b[0], y: b[1], w: b[2] - b[0], h: b[3] - b[1] };
    const boxes = items.filter((i) => selectionIds().includes(i.id)).map((i) => ({ ...i }));
    const z = camera()[2];
    const p0 = [e.clientX, e.clientY];
    let g = start;
    let moved = false;
    setResizing(true);
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      if (!moved && Math.hypot(ev.clientX - p0[0], ev.clientY - p0[1]) < 2) return;
      moved = true;
      g = resizeGeom(start, h, (ev.clientX - p0[0]) / z, (ev.clientY - p0[1]) / z, true, 8);
      setScaleDrag(g);
      controller.board.scale_selection(g.x, g.y, g.x + g.w, g.y + g.h, false);
      const sx = start.w > 0 ? g.w / start.w : 1, sy = start.h > 0 ? g.h / start.h : 1;
      const k = Math.sqrt(sx * sy);
      for (const it of boxes) {
        const x = g.x + (it.x - start.x) * sx, y = g.y + (it.y - start.y) * sy;
        if (it.kind === "text") setItems((i) => i.id === it.id, { x: r1(x), y: r1(y), w: r1(it.w * k), size: r1((it.size ?? 40) * k) });
        else setItems((i) => i.id === it.id, { x: r1(x), y: r1(y), w: r1(it.w * sx), h: r1(it.h * sy) });
      }
    };
    const finish = (commit: boolean) => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", cancel);
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      setResizing(false);
      setScaleDrag(null);
      if (!moved) return;
      if (!commit || !controller.board.scale_selection(g.x, g.y, g.x + g.w, g.y + g.h, true)) {
        controller.board.scale_cancel();
        controller.sync(true);
      }
    };
    const up = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(false);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", cancel);
  };

  /** Scale (`h` set) or turn (`h` null) a stamp about its centre. */
  const startStamp = (h: Handle | null, e: PointerEvent) => {
    const it = stamp();
    if (!it || e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const id = it.id;
    const start = { x: it.x, y: it.y, w: it.w, h: it.h, rot: it.rot || 0 };
    const [cx0, cy0, z] = camera();
    const c = [(start.x + start.w / 2 - cx0) * z, (start.y + start.h / 2 - cy0) * z];
    const root = controller.root.getBoundingClientRect();
    const at = (ev: PointerEvent) => [ev.clientX - root.left - c[0], ev.clientY - root.top - c[1]];
    const p0 = at(e);
    const d0 = Math.max(4, Math.hypot(p0[0], p0[1]));
    const a0 = Math.atan2(p0[1], p0[0]);
    let g = start;
    let moved = false;
    setResizing(true);
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      const p = at(ev);
      if (!moved && Math.hypot(p[0] - p0[0], p[1] - p0[1]) < 2) return;
      moved = true;
      if (h) {
        const k = Math.max(MIN_SIZE / Math.min(start.w, start.h), Math.hypot(p[0], p[1]) / d0);
        const w = start.w * k, hh = start.h * k;
        g = { ...start, x: start.x + (start.w - w) / 2, y: start.y + (start.h - hh) / 2, w, h: hh };
      } else {
        let rot = start.rot + Math.atan2(p[1], p[0]) - a0;
        // Snap to 15° steps when close (Shift turns freely).
        const step = Math.PI / 12;
        const near = Math.round(rot / step) * step;
        if (!ev.shiftKey && Math.abs(rot - near) < 0.07) rot = near;
        g = { ...start, rot: Math.atan2(Math.sin(rot), Math.cos(rot)) };
      }
      setItems((i) => i.id === id, { x: r1(g.x), y: r1(g.y), w: r1(g.w), h: r1(g.h), rot: g.rot });
    };
    const finish = (commit: boolean) => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", cancel);
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      setResizing(false);
      if (!moved) return;
      if (commit) controller.board.set_stamp_geom(id, r1(g.x), r1(g.y), r1(g.w), r1(g.h), g.rot);
      else controller.sync(true);
    };
    const up = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(false);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", cancel);
  };

  const startResize = (h: Handle, e: PointerEvent) => {
    if (stamp()) return startStamp(h, e);
    if (shapeSel()) return startShapeResize(h, e);
    const it = single();
    if (!it) return startScale(h, e);
    if (e.button > 0) return;
    e.preventDefault();
    e.stopPropagation();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const id = it.id;
    const kind = it.kind;
    const m = measured();
    const start: Geom = {
      x: it.x, y: it.y, w: it.w,
      h: kind === "text" && m && m.id === id ? m.h : it.h,
    };
    const startSize = it.size ?? 40;
    const z = camera()[2];
    const p0 = [e.clientX, e.clientY];
    let g = start;
    let size = startSize;
    let moved = false;
    setResizing(true);

    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== e.pointerId) return;
      const dx = (ev.clientX - p0[0]) / z, dy = (ev.clientY - p0[1]) / z;
      if (!moved && Math.hypot(ev.clientX - p0[0], ev.clientY - p0[1]) < 2) return;
      moved = true;
      if (kind === "text") {
        if (h === "e" || h === "w") {
          const n = resizeGeom(start, h, dx, 0, false, Math.max(MIN_SIZE, startSize));
          g = { ...n, h: start.h };
          setItems((i) => i.id === id, { x: r1(g.x), w: r1(g.w) });
        } else {
          g = resizeGeom(start, h, dx, dy, true, MIN_SIZE);
          size = Math.max(4, startSize * (g.w / start.w));
          setItems((i) => i.id === id, { x: r1(g.x), y: r1(g.y), w: r1(g.w), size: r1(size) });
        }
        return;
      }
      // Images keep their aspect unless Shift is held; other boxes the reverse.
      const lock = kind === "image" ? !ev.shiftKey : ev.shiftKey;
      g = resizeGeom(start, h, dx, dy, lock);
      setItems((i) => i.id === id, { x: r1(g.x), y: r1(g.y), w: r1(g.w), h: r1(g.h) });
    };
    const finish = (commit: boolean) => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", cancel);
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      setResizing(false);
      if (!moved) return;
      if (!commit) {
        controller.sync(true);
        return;
      }
      if (kind === "text") {
        const h = itemEl(id)?.offsetHeight ?? g.h;
        controller.board.resize_text(id, r1(g.x), r1(g.y), r1(g.w), h, r1(size));
      } else {
        controller.board.set_geom(id, r1(g.x), r1(g.y), r1(g.w), r1(g.h));
      }
    };
    const up = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === e.pointerId && finish(false);
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", cancel);
  };

  const n = () => selectionIds().length;

  return (
    <div class="sel-layer">
      <Show when={show() && rect()}>
        {(r) => (
          <>
            <div
              class="sel-frame"
              classList={{ loose: !single(), resizing: resizing(), stamped: !!stamp() }}
              style={{
                transform: `translate(${r().x}px, ${r().y}px)${stamp()?.rot ? ` rotate(${stamp()!.rot}rad)` : ""}`,
                width: `${r().w}px`,
                height: `${r().h}px`,
              }}
            >
              <For each={handles()}>
                {(h) => (
                  <div
                    class={`sel-handle ${h}`}
                    role="presentation"
                    onPointerDown={(e) => startResize(h, e)}
                  />
                )}
              </For>
              <Show when={stamp()}>
                <div class="sel-rotate" role="presentation" title="Turn  ·  Shift turns freely" onPointerDown={(e) => startStamp(null, e)} />
              </Show>
            </div>
            <Show when={!resizing()}>
              <div
                ref={bar}
                class="cluster surface sel-bar"
                role="toolbar"
                aria-label={`${n()} selected`}
                style={{ left: `${barPos()?.left ?? 0}px`, top: `${barPos()?.top ?? 0}px` }}
              >
                <Show when={selectionMeta().strokes > 0 && !shapeSel()}>
                  <ToTextButton />
                  <SolveButton />
                  <div class="vsep" />
                </Show>
                <Show when={selectionMeta().live}>
                  <button class="btn totext" title="Replace with the interactive Function grapher" onClick={() => makeItLive()}>
                    <Icon name="object" size={18} />
                    <span>Make it live</span>
                  </button>
                  <div class="vsep" />
                </Show>
                <Show when={colorable()}>
                  <button
                    class="btn icon sel-color"
                    classList={{ on: palette() }}
                    title={selectionMeta().shapeStyle ? "Outline colour" : "Colour"}
                    aria-label="Colour"
                    aria-expanded={palette()}
                    onClick={() => (setFills(false), setTableRow(false), setStyles(false), setPalette(!palette()))}
                  >
                    <span
                      class="swatch-dot"
                      classList={{ mixed: !currentColor() }}
                      style={{ background: currentColor() ? currentColor()!.slice(0, 7) : undefined }}
                    />
                  </button>
                </Show>
                <Show when={selectionMeta().strokes > 0}>
                  <button
                    class="btn icon sel-style"
                    classList={{ on: styles() }}
                    title="Pen style"
                    aria-label="Pen style"
                    aria-expanded={styles()}
                    onClick={() => {
                      setStyles(!styles());
                      setPalette(false);
                      setFills(false);
                      setTableRow(false);
                    }}
                  >
                    <Icon name="pen" />
                  </button>
                  <button class="btn icon" title="Thinner" aria-label="Thinner" onClick={() => controller.board.restyle_selection(undefined, undefined, 1 / WIDTH_STEP)}>
                    <span class="sel-line thin" aria-hidden="true" />
                  </button>
                  <button class="btn icon" title="Thicker" aria-label="Thicker" onClick={() => controller.board.restyle_selection(undefined, undefined, WIDTH_STEP)}>
                    <span class="sel-line thick" aria-hidden="true" />
                  </button>
                </Show>
                <ShapeControls
                  fillsOpen={fills()}
                  onFills={(o) => (setPalette(false), setTableRow(false), setStyles(false), setFills(o))}
                  tableOpen={tableRow()}
                  onTable={(o) => (setPalette(false), setFills(false), setStyles(false), setTableRow(o))}
                  up={(barPos()?.top ?? 0) < (rect()?.y ?? 0)}
                />
                <button class="btn icon" title="Smaller" aria-label="Smaller" onClick={() => controller.board.scale_selection_by(1 / SIZE_STEP)}>
                  <Icon name="shrink" />
                </button>
                <button class="btn icon" title="Bigger" aria-label="Bigger" onClick={() => controller.board.scale_selection_by(SIZE_STEP)}>
                  <Icon name="grow" />
                </button>
                <div class="vsep" />
                <button class="btn icon" title="Duplicate  ·  Ctrl D" aria-label="Duplicate" onClick={() => controller.duplicateSelection()}>
                  <Icon name="duplicate" />
                </button>
                <button class="btn icon" title="Bring to front" aria-label="Bring to front" onClick={() => controller.bringSelectionToFront()}>
                  <Icon name="front" />
                </button>
                <Show when={selectionMeta().count > 1}>
                  <div class="vsep" />
                  <Show
                    when={selectionMeta().grouped}
                    fallback={
                      <button class="btn icon" title="Group  ·  Ctrl G" aria-label="Group" onClick={() => controller.groupSelection()}>
                        <Icon name="group" />
                      </button>
                    }
                  >
                    <button class="btn icon" title="Ungroup  ·  Ctrl Shift G" aria-label="Ungroup" onClick={() => controller.ungroupSelection()}>
                      <Icon name="ungroup" />
                    </button>
                  </Show>
                  <Show when={selectionMeta().strokes > 1 && !selectionMeta().grouped}>
                    <button class="btn icon" title="Merge ink  ·  Ctrl E" aria-label="Merge" onClick={() => controller.mergeSelection()}>
                      <Icon name="merge" />
                    </button>
                  </Show>
                </Show>
                <div class="vsep" />
                <button class="btn icon sel-del" title="Delete  ·  Del" aria-label="Delete" onClick={() => controller.deleteSelection()}>
                  <Icon name="trash" />
                </button>
                <Show when={fills() && selectionMeta().shapeStyle?.fillable}>
                  <FillRow up={(barPos()?.top ?? 0) < (rect()?.y ?? 0)} />
                </Show>
                <Show when={styles() && selectionMeta().strokes > 0}>
                  <div class="sel-palette sel-styles" classList={{ up: (barPos()?.top ?? 0) < (rect()?.y ?? 0) }}>
                    <StyleRow
                      current={selectionMeta().style?.brush ?? null}
                      color={currentColor() ? parseInt(currentColor()!.slice(1, 9).padEnd(8, "f"), 16) >>> 0 : 0x1c2024ff}
                      onPick={(b) => controller.board.restyle_selection(undefined, undefined, undefined, b)}
                    />
                  </div>
                </Show>
                <Show when={palette() && colorable()}>
                  <div class="sel-palette" classList={{ up: (barPos()?.top ?? 0) < (rect()?.y ?? 0) }} role="group" aria-label="Colours">
                    <For each={swatches()}>
                      {(c) => (
                        <button
                          class="btn icon sel-swatch"
                          classList={{ on: !!currentColor() && currentColor()!.slice(1, 7) === hex(c).slice(1) }}
                          title={hex(c)}
                          aria-label={`Colour ${hex(c)}`}
                          onClick={() => controller.board.restyle_selection(c, undefined, undefined)}
                        >
                          <span class="swatch-dot" style={{ background: hex(c) }} />
                        </button>
                      )}
                    </For>
                  </div>
                </Show>
              </div>
            </Show>
            <TableCells />
          </>
        )}
      </Show>
    </div>
  );
}
