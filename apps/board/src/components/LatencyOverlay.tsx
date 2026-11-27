// The performance and pen latency overlay (Settings → About → Troubleshooting).
// Live numbers a teacher can read on the real board while writing; what each
// one means is in docs/performance.md ("Reading the overlay").

import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { graphicsAdapter, snapshot, type LatencySnapshot } from "../inkLatency";
import { refreshHz, renderScaleLabel } from "../perf";
import { trailState } from "../wetInk";
import { nativeInkState } from "../nativeInk";

const ms = (v: number) => (Number.isFinite(v) ? `${Math.round(v)} ms` : "—");

export default function LatencyOverlay() {
  const [s, setS] = createSignal<LatencySnapshot | null>(null);
  const [info, setInfo] = createSignal({ backend: "", strokes: 0, canvas: [0, 0], tile: null as number[] | null });
  // The last frame rate measured while writing stays up between strokes.
  const [writing, setWriting] = createSignal({ fps: NaN, main: 0, tile: 0, hz: 0 });
  const adapter = graphicsAdapter();
  onMount(() => {
    snapshot();
    const id = window.setInterval(() => {
      const v = snapshot();
      setS(v);
      if (Number.isFinite(v.writeFps)) setWriting({ fps: v.writeFps, main: v.mainFrames, tile: v.tileFrames, hz: v.inputHz });
      try {
        const st = JSON.parse(controller.board.stats_json());
        setInfo({ backend: String(st.backend), strokes: Number(st.strokes), canvas: st.canvas ?? [0, 0], tile: st.tile ?? null });
      } catch {
        /* the board is not up yet */
      }
    }, 1000);
    onCleanup(() => clearInterval(id));
  });
  const backend = () => info().backend.split(" · ")[0].replace("BrowserWebGpu", "WebGPU").replace("Gl", "WebGL2");
  const trail = () => ({ on: "on (Windows draws it)", available: "ready (starts on first stroke)", unsupported: "not on this system", failed: "failed", off: "off" })[trailState()];
  const frames = () => {
    const w = writing();
    if (!Number.isFinite(w.fps)) return "write to measure";
    // Which canvas the frames went to: the small wet tile, or the whole board.
    const split = w.main + w.tile > 0 ? ` · ${w.tile} small, ${w.main} full` : "";
    return `${Math.round(w.fps)} of ${refreshHz()} fps${split}`;
  };
  const canvas = () => {
    const i = info();
    const t = i.tile ? ` · wet layer ${i.tile[0]}×${i.tile[1]}` : " · wet layer off";
    return `${i.canvas[0]}×${i.canvas[1]} · ${renderScaleLabel(controller.scale)}${t}`;
  };
  return (
    <Show when={s()}>
      {(v) => (
        <div class="latency surface" role="status" aria-label="Pen latency">
          <div class="lat-big">
            <span class="lat-label">Pen → frame</span>
            <b>{ms(v().frameP50)}</b>
            <span class="lat-sub">p95 {ms(v().frameP95)}</span>
          </div>
          <div class="lat-row"><span>Frames while writing</span><b>{frames()}</b></div>
          <div class="lat-row"><span>Pen-down → screen</span><b>{ms(v().downP50)}</b></div>
          <div class="lat-row"><span>Input</span><b>{Math.round(writing().hz)} Hz{v().pointer ? ` · ${v().pointer}` : ""}</b></div>
          <div class="lat-row"><span>Board CPU per frame</span><b>{v().frameMs.toFixed(1)} ms</b></div>
          <div class="lat-row"><span>Canvas</span><b>{canvas()}</b></div>
          <div class="lat-row"><span>OS ink trail</span><b>{trail()}</b></div>
          <Show when={nativeInkState() !== "unsupported"}>
            <div class="lat-row"><span>Native wet ink</span><b>{nativeInkState()}</b></div>
          </Show>
          <div class="lat-row"><span>Renderer</span><b>{backend()} · {info().strokes} strokes</b></div>
          <div class="lat-row lat-gpu" classList={{ bad: adapter.software }} title={adapter.name}>
            {adapter.software ? "Software rendering: " : ""}{adapter.name}
          </div>
        </div>
      )}
    </Show>
  );
}
