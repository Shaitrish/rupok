// The Shapes picker: every shape a teacher can insert, in one grid (basic
// shapes, lines, teaching diagrams), the table size grid, recent shapes, the
// fill for new shapes and the square lock. It opens from the dock's Shapes
// button, Insert ▾ → Shape… / Table… and the S key (shapes.ts).
//
// Pick a shape, then drag on the board to draw it (Shift or the lock keeps
// it square), or tap to place it at a default size.

import { For, Show, createSignal } from "solid-js";
import {
  SHAPE_GROUPS, currentShape, pickShape, recentShapes, setShapeFill, setShapeLock, shapeLabel,
  shapeStore, tableId,
} from "../shapes";
import { color, tool } from "../state";
import { FillChooser } from "./ShapeControls";
import { ShapeGlyph } from "./ShapeGlyph";
import "./ShapesPicker.css";

const hexa = (c: number) => "#" + (c >>> 0).toString(16).padStart(8, "0");

/** Word's table grid: hover (or drag a finger) to size it, tap to pick. */
function TableGrid(props: { onPick: (rows: number, cols: number) => void; onBack?: () => void }) {
  const [size, setSize] = createSignal<[number, number]>([3, 4]);
  const cells = Array.from({ length: 100 }, (_, i) => [Math.floor(i / 10) + 1, (i % 10) + 1] as const);
  let grid!: HTMLDivElement;
  const at = (e: PointerEvent): [number, number] | null => {
    const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    const r = Number(el?.dataset.r), c = Number(el?.dataset.c);
    return r && c ? [r, c] : null;
  };
  return (
    <div class="shapes-table">
      <div class="shapes-table-head">
        <Show when={props.onBack}>
          <button class="tool shapes-back" title="Back to all shapes" aria-label="Back to all shapes" onClick={() => props.onBack?.()}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 5 L8 12 L15 19" /></svg>
          </button>
        </Show>
        <span class="shapes-table-size" aria-live="polite">
          Table · {size()[0]} × {size()[1]}
        </span>
      </div>
      <div
        ref={grid}
        class="shapes-grid10"
        role="grid"
        aria-label="Table size: rows by columns"
        onPointerMove={(e) => {
          const rc = at(e);
          if (rc) setSize(rc);
        }}
        onPointerDown={(e) => {
          // A finger slides across the cells (no capture by the first one).
          (e.target as Element).releasePointerCapture?.(e.pointerId);
          const rc = at(e);
          if (rc) setSize(rc);
        }}
        onPointerUp={(e) => {
          const rc = at(e);
          if (rc) props.onPick(rc[0], rc[1]);
        }}
      >
        <For each={cells}>
          {([r, c]) => (
            <button
              class="shapes-cell"
              classList={{ on: r <= size()[0] && c <= size()[1] }}
              data-r={r}
              data-c={c}
              aria-label={`${r} by ${c} table`}
              tabIndex={r === 1 && c === 1 ? 0 : -1}
              onFocus={() => setSize([r, c])}
              onKeyDown={(e) => {
                const [rr, cc] = size();
                const move: Record<string, [number, number]> = {
                  ArrowRight: [rr, Math.min(10, cc + 1)], ArrowLeft: [rr, Math.max(1, cc - 1)],
                  ArrowDown: [Math.min(10, rr + 1), cc], ArrowUp: [Math.max(1, rr - 1), cc],
                };
                if (move[e.key]) {
                  e.preventDefault();
                  setSize(move[e.key]);
                  (grid.querySelector(`[data-r="${move[e.key][0]}"][data-c="${move[e.key][1]}"]`) as HTMLElement | null)?.focus();
                } else if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  props.onPick(rr, cc);
                }
              }}
            />
          )}
        </For>
      </div>
    </div>
  );
}

export function ShapesPicker(props: { onDone: () => void; focus?: "table" }) {
  const [view, setView] = createSignal<"all" | "table">(props.focus === "table" ? "table" : "all");
  const active = (id: string) => tool() === "shape" && currentShape() === id;
  const choose = (id: string) => {
    pickShape(id);
    props.onDone();
  };
  const fillCss = () => (shapeStore.read().fill ? hexa(shapeStore.read().fill) : null);
  const Tile = (p: { id: string; label?: string; onClick?: () => void }) => (
    <button
      class="tool shapes-tile"
      classList={{ on: active(p.id) }}
      title={p.label ?? shapeLabel(p.id)}
      aria-label={p.label ?? shapeLabel(p.id)}
      aria-pressed={active(p.id)}
      data-shape={p.id}
      onClick={() => (p.onClick ? p.onClick() : choose(p.id))}
    >
      <ShapeGlyph id={p.id} fill={fillCss()} />
    </button>
  );
  return (
    <div class="shapes-picker">
      <Show
        when={view() === "all"}
        fallback={<TableGrid onPick={(r, c) => choose(tableId(r, c))} onBack={props.focus === "table" ? undefined : () => setView("all")} />}
      >
        <Show when={recentShapes().length}>
          <div class="shapes-label">Recent</div>
          <div class="shapes-row" role="group" aria-label="Recent shapes">
            <For each={recentShapes()}>{(id) => <Tile id={id} />}</For>
          </div>
        </Show>
        <For each={SHAPE_GROUPS}>
          {(g) => (
            <>
              <div class="shapes-label">{g.label}</div>
              <div class="shapes-row" role="group" aria-label={`${g.label} shapes`}>
                <Show when={g.id === "teaching"}>
                  <Tile id="table" label="Table… (pick rows and columns)" onClick={() => setView("table")} />
                </Show>
                <For each={g.items}>{(s) => <Tile id={s.id} label={s.label} />}</For>
              </div>
            </>
          )}
        </For>
      </Show>
      <div class="dock-hr" />
      <div class="shapes-label">New shapes start · {shapeStore.read().fill ? "filled" : "no fill"}</div>
      <FillChooser class="shapes-fills" value={shapeStore.read().fill >>> 0} outline={color()} onPick={setShapeFill} />
      <button
        class="dock-row"
        role="menuitemcheckbox"
        aria-checked={shapeStore.read().lock}
        title="Keep boxes square and circles round, and lines at 15° steps (or hold Shift)"
        onClick={() => setShapeLock(!shapeStore.read().lock)}
      >
        <ShapeGlyph id="square" size={20} />
        <span class="grow">Keep square · Shift</span>
        <span class="dock-check" classList={{ on: shapeStore.read().lock }} />
      </button>
      <p class="shapes-hint">Drag on the board to draw · tap to place</p>
    </div>
  );
}

