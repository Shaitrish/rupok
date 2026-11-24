// The ink sections of the dock's pen, highlighter and shapes popovers:
// Style (pen styles with a live sample stroke each), Size and Opacity.
// Colour and Pressure stay in Dock.tsx.

import { For, Show, createEffect, on, onMount } from "solid-js";
import { controller } from "../controller";
import { type BrushId, BRUSHES, TOOL_BRUSHES, brushOf, drawBrushPreview, setBrushOf } from "../brushes";
import { prefs, setPrefs } from "../state";
import "./Stamps.css";

export type InkKind = "pen" | "highlighter" | "shape";

/** The ink colour, unless it would vanish on the popover (graphite in dark
 * mode, white in light mode): then the text colour. */
function legible(c: number): number {
  const r = (c >>> 24) & 255, g = (c >>> 16) & 255, b = (c >>> 8) & 255;
  const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  const t = document.documentElement.dataset.theme;
  const dark = t === "dark" || (!t && matchMedia("(prefers-color-scheme: dark)").matches);
  if (dark && lum < 0.3) return 0xedeef0ff;
  if (!dark && lum > 0.85) return 0x1c2024ff;
  return c;
}

/** One style swatch: its sample stroke in the current colour and size. */
function Swatch(props: { id: BrushId; color: number; size: number; on: boolean; onPick: () => void }) {
  let cv!: HTMLCanvasElement;
  const info = () => BRUSHES.find((b) => b.id === props.id)!;
  const draw = () => drawBrushPreview(cv, props.id, legible(props.color), props.size);
  onMount(draw);
  createEffect(on([() => props.color, () => props.size, () => prefs().markerOpacity], draw, { defer: true }));
  return (
    <button
      class="ink-style"
      classList={{ on: props.on }}
      role="radio"
      aria-checked={props.on}
      aria-label={info().label}
      title={`${info().label}: ${info().hint}`}
      data-brush={props.id}
      onClick={() => props.onPick()}
    >
      <canvas ref={cv} aria-hidden="true" />
      <span>{info().label}</span>
    </button>
  );
}

/** Style grid for one tool; picking applies to new strokes at once. */
export function StylePicker(props: { kind: InkKind; color: number; size: number; onPick?: (id: BrushId) => void }) {
  const current = () => brushOf(props.kind);
  const pick = (id: BrushId) => {
    setBrushOf(props.kind, id);
    controller.applyPrefs();
    props.onPick?.(id);
  };
  return (
    <div class="ink-styles" role="radiogroup" aria-label="Pen style" data-kind={props.kind}>
      <For each={TOOL_BRUSHES[props.kind]}>
        {(id) => <Swatch id={id} color={props.color} size={props.size} on={current() === id} onPick={() => pick(id)} />}
      </For>
    </div>
  );
}

/** Size: preset dots for one tap and a slider for everything between, on
 * one row under a "Size · n" label. */
export function SizeControl(props: { presets: number[]; value: number; min: number; max: number; onChange: (w: number) => void }) {
  return (
    <>
      <div class="ink-label">Size · {Math.round(props.value * 10) / 10}</div>
      <div class="ink-size">
        <div class="dock-sizes" role="group" aria-label="Sizes">
          <For each={props.presets}>
            {(w, i) => (
              <button
                class="tool"
                classList={{ on: props.value === w }}
                title={`Size ${w}`}
                aria-label={`Size ${w}`}
                aria-pressed={props.value === w}
                onClick={() => props.onChange(w)}
              >
                <span class="size-dot" style={{ width: `${4 + i() * 4}px`, height: `${4 + i() * 4}px` }} />
              </button>
            )}
          </For>
        </div>
        <input
          class="slider"
          type="range"
          min={props.min}
          max={props.max}
          step="1"
          value={props.value}
          aria-label="Size"
          onInput={(e) => props.onChange(Number(e.currentTarget.value))}
        />
      </div>
    </>
  );
}

/** Opacity of the marker (each stroke stores its own). */
export function MarkerOpacity() {
  return (
    <Show when={brushOf("pen") === "marker" || brushOf("shape") === "marker"}>
      <label class="dock-slider">
        <span>Opacity · {Math.round(prefs().markerOpacity * 100)}%</span>
        <input
          class="slider"
          type="range"
          min="0.3"
          max="1"
          step="0.05"
          value={prefs().markerOpacity}
          aria-label="Marker opacity"
          onInput={(e) => {
            setPrefs({ markerOpacity: Number(e.currentTarget.value) });
            controller.applyPrefs();
          }}
        />
      </label>
    </Show>
  );
}

/** Pen styles as one row of swatches (the selection toolbar): restyles
 * the selected ink. Magic ink is not a style ink can keep. */
export function StyleRow(props: { current: string | null; color: number; onPick: (id: BrushId) => void }) {
  return (
    <div class="ink-styles row" role="radiogroup" aria-label="Pen style">
      <For each={TOOL_BRUSHES.pen.filter((b) => b !== "magic")}>
        {(id) => <Swatch id={id} color={props.color} size={4} on={props.current === id} onPick={() => props.onPick(id)} />}
      </For>
    </div>
  );
}
