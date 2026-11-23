// Live pen-latency measurement for the latency overlay (Settings → About →
// Troubleshooting → "Performance and pen latency overlay") and for
// bench/latency. Cheap enough to stay on: a few numbers per pointer event.
//
//   pen → frame     each pen sample's event timeStamp (when the OS handed it
//                   to the browser) to the end of the board frame that drew it
//                   (the GPU commands are submitted). The compositor then
//                   needs about one more refresh to put it on screen.
//   pen-down → screen
//                   the browser's own measure (Event Timing API) of a
//                   pointerdown to the frame that presented it, compositor
//                   included. Chrome only reports events of 16 ms or more,
//                   so this is an upper estimate.
//
// What is outside any app's reach: the digitiser and the OS before the
// timeStamp, and the display's own lag after present.
//
// See docs/performance.md.

const RING = 600;
const frameLat: number[] = [];
let pending: number[] = [];
let samples = 0;
let events = 0;
let drawn = 0;
let since = performance.now();
let frameMsSum = 0;
const downPresent: number[] = [];
let lastKind = "";

function push(ring: number[], v: number) {
  ring.push(v);
  if (ring.length > RING) ring.splice(0, ring.length - RING);
}

/** Samples the board was given for one pointer event (coalesced list). */
export function noteSamples(e: PointerEvent, list: PointerEvent[]) {
  events++;
  lastKind = e.pointerType;
  for (const c of list) pending.push(c.timeStamp);
  samples += list.length;
}

/** After each board frame; `drew` when the GPU drew one, `ms` its CPU time. */
export function noteFrame(drew: boolean, ms: number) {
  if (!drew) return;
  drawn++;
  frameMsSum += ms;
  if (!pending.length) return;
  const now = performance.now();
  for (const ts of pending) push(frameLat, now - ts);
  pending = [];
}

/** Drop samples of a frame that will never be drawn (pointer cancelled). */
export function dropPending() {
  pending = [];
}

let observing = false;
function observe() {
  if (observing) return;
  observing = true;
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries() as PerformanceEventTiming[]) {
        if (e.name === "pointerdown") push(downPresent, e.duration);
      }
    }).observe({ type: "event", durationThreshold: 16, buffered: true } as PerformanceObserverInit);
  } catch {
    // No Event Timing (Safari, WebKitGTK): the overlay shows "—".
  }
}
observe();

const pct = (xs: number[], p: number) => {
  if (!xs.length) return NaN;
  const v = [...xs].sort((a, b) => a - b);
  return v[Math.min(v.length - 1, Math.floor(p * v.length))];
};

export interface LatencySnapshot {
  /** Pen sample → board frame submitted, ms. */
  frameP50: number;
  frameP95: number;
  /** Pointerdown → presented, ms (browser-measured, ≥ 16 ms entries). */
  downP50: number;
  /** Pen samples per second and per drawn frame. */
  inputHz: number;
  perFrame: number;
  /** Frames drawn per second, and the board's CPU time per drawn frame. */
  fps: number;
  frameMs: number;
  pointer: string;
}

/** Rates since the previous call; percentiles over the last ~600 samples. */
export function snapshot(): LatencySnapshot {
  const now = performance.now();
  const dt = Math.max(1, now - since) / 1000;
  const s: LatencySnapshot = {
    frameP50: pct(frameLat, 0.5),
    frameP95: pct(frameLat, 0.95),
    downP50: pct(downPresent, 0.5),
    inputHz: samples / dt,
    perFrame: events ? samples / Math.max(1, drawn) : 0,
    fps: drawn / dt,
    frameMs: drawn ? frameMsSum / drawn : 0,
    pointer: lastKind,
  };
  since = now;
  samples = events = drawn = 0;
  frameMsSum = 0;
  return s;
}

/** The graphics adapter as the browser reports it, and whether it is software. */
export function graphicsAdapter(): { name: string; software: boolean } {
  try {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) return { name: "WebGL2 unavailable", software: true };
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    const name = String(gl.getParameter(ext ? ext.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return { name, software: /swiftshader|llvmpipe|softpipe|basic render|microsoft basic|warp/i.test(name) };
  } catch {
    return { name: "unknown", software: false };
  }
}
