// The performance and pen latency overlay (Settings → About → Troubleshooting).
// Live numbers a teacher can read on the real board while writing; what each
// one means is in docs/performance.md ("Reading the overlay").

import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { graphicsAdapter, snapshot, type LatencySnapshot } from "../inkLatency";
import { trailState } from "../wetInk";

const ms = (v: number) => (Number.isFinite(v) ? `${Math.round(v)} ms` : "—");

export default function LatencyOverlay() {
  const [s, setS] = createSignal<LatencySnapshot | null>(null);
  const [info, setInfo] = createSignal({ backend: "", strokes: 0 });
  const adapter = graphicsAdapter();
  onMount(() => {
    snapshot();
    const id = window.setInterval(() => {
      setS(snapshot());
      try {
        const st = JSON.parse(controller.board.stats_json());
        setInfo({ backend: String(st.backend), strokes: Number(st.strokes) });
      } catch {
        /* the board is not up yet */
      }
    }, 1000);
    onCleanup(() => clearInterval(id));
  });
  const backend = () => info().backend.split(" · ")[0].replace("BrowserWebGpu", "WebGPU").replace("Gl", "WebGL2");
  const trail = () => ({ on: "on (Windows draws it)", available: "ready (starts on first stroke)", unsupported: "not on this system", failed: "failed" })[trailState()];
  return (
    <Show when={s()}>
      {(v) => (
        <div class="latency surface" role="status" aria-label="Pen latency">
          <div class="lat-big">
            <span class="lat-label">Pen → frame</span>
            <b>{ms(v().frameP50)}</b>
            <span class="lat-sub">p95 {ms(v().frameP95)}</span>
          </div>
          <div class="lat-row"><span>Pen-down → screen</span><b>{ms(v().downP50)}</b></div>
          <div class="lat-row"><span>Input</span><b>{Math.round(v().inputHz)} Hz{v().pointer ? ` · ${v().pointer}` : ""}</b></div>
          <div class="lat-row"><span>Frames</span><b>{Math.round(v().fps)} fps · {v().frameMs.toFixed(1)} ms</b></div>
          <div class="lat-row"><span>OS ink trail</span><b>{trail()}</b></div>
          <div class="lat-row"><span>Renderer</span><b>{backend()} · {info().strokes} strokes</b></div>
          <div class="lat-row lat-gpu" classList={{ bad: adapter.software }} title={adapter.name}>
            {adapter.software ? "Software rendering: " : ""}{adapter.name}
          </div>
        </div>
      )}
    </Show>
  );
}
