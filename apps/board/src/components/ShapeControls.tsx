// Shape controls in the selection toolbar: fill (none or a soft tint),
// dashed or solid outline, and for a table its rows and columns and typing
// in a cell. Each change is one undo step in the engine (engine/insert.rs).

import { For, Show, createMemo, createSignal } from "solid-js";
import { controller } from "../controller";
import { DEFAULT_FILL_ALPHA, FILL_ALPHAS, fillColors, parseRgba } from "../shapes";
import { camera, revision, selectionMeta, setEditingText } from "../state";
import "./ShapesPicker.css";

const hexa = (c: number) => "#" + (c >>> 0).toString(16).padStart(8, "0");

/** "Type in a cell" mode of the selected table. */
const [cellMode, setCellMode] = createSignal(false);
export const tableCellMode = cellMode;
export const setTableCellMode = setCellMode;

const Plus = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><path d="M12 5 V19 M5 12 H19" /></svg>
);
const Minus = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true"><path d="M5 12 H19" /></svg>
);

/** The buttons. The caller's toolbar shows one popup row at a time: the
 * fill row (its own) or the table row (drawn here). */
export function ShapeControls(props: {
  fillsOpen: boolean;
  onFills: (open: boolean) => void;
  tableOpen: boolean;
  onTable: (open: boolean) => void;
  /** Popup rows open above the toolbar. */
  up: boolean;
}) {
  const st = () => selectionMeta().shapeStyle ?? null;
  const fill = () => st()?.fill ?? null;
  return (
    <Show when={st()}>
      {(s) => (
        <>
          <Show when={s().fillable}>
            <button
              class="btn sel-fill-btn"
              classList={{ on: props.fillsOpen }}
              title="Fill colour"
              aria-label="Fill"
              aria-expanded={props.fillsOpen}
              onClick={() => props.onFills(!props.fillsOpen)}
            >
              <span class="fill-dot" classList={{ none: !fill() }} style={{ background: fill() ?? undefined }} />
              <span>Fill</span>
            </button>
          </Show>
          <button
            class="btn icon"
            classList={{ on: s().dash }}
            title={s().dash ? "Solid outline" : "Dashed outline"}
            aria-label="Dashed outline"
            aria-pressed={s().dash}
            onClick={() => controller.board.set_selection_dash(!s().dash)}
          >
            <span class="sel-dash" />
          </button>
          <Show when={s().table}>
            {(t) => (
              <>
                <button
                  class="btn sel-table"
                  classList={{ on: props.tableOpen }}
                  title="Rows, columns and cell text"
                  aria-label="Table"
                  aria-expanded={props.tableOpen}
                  onClick={() => props.onTable(!props.tableOpen)}
                >
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M3.5 5 H20.5 V19 H3.5 Z M3.5 9.7 H20.5 M3.5 14.3 H20.5 M9.2 5 V19 M14.8 5 V19" /></svg>
                  {t()[0]} × {t()[1]}
                </button>
                <Show when={props.tableOpen}>
                  <div class="sel-palette sel-tablerow" classList={{ up: props.up }} role="group" aria-label="Table rows and columns">
                    <button class="btn sel-table" title="Remove the last row" aria-label="Remove a row" disabled={t()[0] <= 1} onClick={() => controller.board.table_edit("row-")}>
                      <Minus />Row
                    </button>
                    <button class="btn sel-table" title="Add a row" aria-label="Add a row" disabled={t()[0] >= 10} onClick={() => controller.board.table_edit("row+")}>
                      <Plus />Row
                    </button>
                    <button class="btn sel-table" title="Remove the last column" aria-label="Remove a column" disabled={t()[1] <= 1} onClick={() => controller.board.table_edit("col-")}>
                      <Minus />Col
                    </button>
                    <button class="btn sel-table" title="Add a column" aria-label="Add a column" disabled={t()[1] >= 10} onClick={() => controller.board.table_edit("col+")}>
                      <Plus />Col
                    </button>
                    <button
                      class="btn sel-table"
                      classList={{ on: cellMode() }}
                      title="Tap a cell to type in it"
                      aria-label="Type in a cell"
                      aria-pressed={cellMode()}
                      onClick={() => setCellMode(!cellMode())}
                    >
                      Aa · Type
                    </button>
                  </div>
                </Show>
              </>
            )}
          </Show>
        </>
      )}
    </Show>
  );
}

/** No fill, the outline's colour (soft), the pen palette, and an opacity
 * step. `value` is the current fill (0xRRGGBBAA, 0 = none); `onPick` gets
 * the new one. Shared by the selection toolbar and the Shapes picker. */
export function FillChooser(props: { value: number; outline: number; onPick: (rgba: number) => void; class?: string }) {
  const alpha = () => (props.value ? props.value & 0xff : DEFAULT_FILL_ALPHA);
  const rgb = () => (props.value & 0xffffff00) >>> 0;
  const with_ = (c: number, a: number) => ((c & 0xffffff00) | a) >>> 0;
  return (
    <div class={`fill-chooser ${props.class ?? ""}`}>
      <div class="fill-swatches" role="radiogroup" aria-label="Fill colour">
        <button
          class="btn icon sel-fill"
          role="radio"
          aria-checked={!props.value}
          classList={{ on: !props.value }}
          title="No fill"
          aria-label="No fill"
          onClick={() => props.onPick(0)}
        >
          <span class="fill-dot none" />
        </button>
        <For each={fillColors(props.outline)}>
          {(c, i) => (
            <button
              class="btn icon sel-fill"
              role="radio"
              aria-checked={!!props.value && rgb() === c.rgb}
              classList={{ on: !!props.value && rgb() === c.rgb, own: i() === 0 }}
              title={c.label}
              aria-label={i() === 0 ? "Soft outline colour" : c.label}
              onClick={() => props.onPick(with_(c.rgb, i() === 0 && !props.value ? DEFAULT_FILL_ALPHA : alpha()))}
            >
              <span class="fill-dot" style={{ background: hexa(with_(c.rgb, i() === 0 && !props.value ? DEFAULT_FILL_ALPHA : alpha())) }} />
            </button>
          )}
        </For>
      </div>
      <div class="seg fill-alpha" role="radiogroup" aria-label="Fill opacity">
        <For each={FILL_ALPHAS}>
          {(o) => (
            <button
              role="radio"
              aria-checked={!!props.value && alpha() === o.a}
              classList={{ on: !!props.value && alpha() === o.a }}
              disabled={!props.value}
              title={`${o.label} fill`}
              onClick={() => props.onPick(with_(props.value, o.a))}
            >
              {o.label}
            </button>
          )}
        </For>
      </div>
    </div>
  );
}

/** The fill row under (or above) the toolbar. */
export function FillRow(props: { up: boolean }) {
  return (
    <div class="sel-palette sel-fills" classList={{ up: props.up }}>
      <FillChooser
        value={parseRgba(selectionMeta().shapeStyle?.fill)}
        outline={parseRgba(selectionMeta().style?.color) || 0x1c2024ff}
        onPick={(rgba) => controller.board.set_selection_fill(rgba)}
      />
    </div>
  );
}

/** Tap targets over the selected table's cells while "type in a cell" is on. */
export function TableCells() {
  const cells = createMemo(() => {
    revision();
    if (!cellMode() || !selectionMeta().shapeStyle?.table) return null;
    const j = JSON.parse(controller.board.table_cells_json()) as { rows: number; cols: number; cells: number[][] } | null;
    return j;
  });
  return (
    <Show when={cells()}>
      {(c) => {
        const [cx, cy, z] = [() => camera()[0], () => camera()[1], () => camera()[2]];
        return (
          <div class="table-cells">
            <For each={c().cells}>
              {(r, i) => (
                <button
                  class="table-cell"
                  title="Type here"
                  aria-label={`Cell row ${Math.floor(i() / c().cols) + 1}, column ${(i() % c().cols) + 1}`}
                  style={{
                    left: `${(r[0] - cx()) * z() + 3}px`,
                    top: `${(r[1] - cy()) * z() + 3}px`,
                    width: `${Math.max(8, r[2] * z() - 6)}px`,
                    height: `${Math.max(8, r[3] * z() - 6)}px`,
                  }}
                  onClick={() => {
                    const id = controller.board.table_cell_text(Math.floor(i() / c().cols), i() % c().cols, "Text");
                    setCellMode(false);
                    if (id) {
                      controller.setTool("lasso");
                      setEditingText(id);
                    }
                  }}
                >
                  Aa
                </button>
              )}
            </For>
          </div>
        );
      }}
    </Show>
  );
}
