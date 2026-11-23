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
// line width stays the same.

import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { controller } from "../controller";
import { type DomItem, camera, editingText, items, replay, selection, selectionIds, selectionMeta, setItems, isView } from "../state";
import { Icon } from "./Icon";
import { ToTextButton } from "./ToText";
import { makeItLive } from "./MakeItLive";
import { SolveButton } from "../solve/SolveUI";

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
      b = selection();
      pad = LOOSE_PAD;
    }
    if (!b) return null;
    const dpr = window.devicePixelRatio || 1;
    const snap = (v: number) => Math.round(v * dpr) / dpr;
    const x0 = snap((b[0] - cx) * z - pad), y0 = snap((b[1] - cy) * z - pad);
    const x1 = snap((b[2] - cx) * z + pad), y1 = snap((b[3] - cy) * z + pad);
    return { x: x0, y: y0, w: Math.max(1, x1 - x0), h: Math.max(1, y1 - y0) };
  });

  const handles = createMemo<Handle[]>(() => {
    const it = single();
    const sh = shapeSel();
    if (sh) {
      const [x0, y0, x1, y1] = sh.box;
      // A level or upright line resizes along its length only.
      if (y1 - y0 < 1) return ["e", "w"];
      if (x1 - x0 < 1) return ["n", "s"];
      const r = rect();
      return BOX_HANDLES.filter((h) => h.length === 2 || !r || (h === "n" || h === "s" ? r.w >= EDGE_HANDLE_MIN : r.h >= EDGE_HANDLE_MIN));
    }
    if (!it) return [];
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

  const startResize = (h: Handle, e: PointerEvent) => {
    if (shapeSel()) return startShapeResize(h, e);
    const it = single();
    if (!it || e.button > 0) return;
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
              classList={{ loose: !single(), resizing: resizing() }}
              style={{ transform: `translate(${r().x}px, ${r().y}px)`, width: `${r().w}px`, height: `${r().h}px` }}
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
              </div>
            </Show>
          </>
        )}
      </Show>
    </div>
  );
}
