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
//   frames while writing
//                   the time between consecutive board frames while a pointer
//                   is down, as a rate. The browser asks for the next frame
//                   only once the GPU and the compositor have taken the last,
//                   so this is the rate that reaches the screen. (Frames per
//                   second of wall time would count the pauses between
//                   strokes as slow frames, as the overlay did up to v0.2.4.)
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

/** Frame intervals while a pointer is down, the frames by canvas, and the
 * pen-down time they cover. One for the overlay, one for a speed test run. */
interface Writing {
  intervals: number[];
  latency: number[];
  mainFrames: number;
  tileFrames: number;
  samples: number;
  downMs: number;
}
const fresh = (): Writing => ({ intervals: [], latency: [], mainFrames: 0, tileFrames: 0, samples: 0, downMs: 0 });
let live = fresh();
let run: Writing | null = null;
let penDown = false;
let downAt = 0;
let lastWriteFrame = 0;

function push(ring: number[], v: number) {
  ring.push(v);
  if (ring.length > RING) ring.splice(0, ring.length - RING);
}

function closeDown(now: number) {
  if (!penDown) return;
  live.downMs += now - downAt;
  if (run) run.downMs += now - downAt;
  downAt = now;
}

/** A pointer went down on the board (`true`) or the last one lifted. */
export function notePen(down: boolean) {
  const now = performance.now();
  if (down === penDown) return;
  if (down) {
    downAt = now;
    lastWriteFrame = 0;
  } else {
    closeDown(now);
  }
  penDown = down;
}

/** Whether a pointer is down on the board. */
export const penIsDown = () => penDown;

/** Samples the board was given for one pointer event (coalesced list). */
export function noteSamples(e: { pointerType: string }, list: { timeStamp: number }[]) {
  events++;
  lastKind = e.pointerType;
  for (const c of list) pending.push(c.timeStamp);
  samples += list.length;
  live.samples += list.length;
  if (run) run.samples += list.length;
}

/** After each board frame; `drew` when the GPU drew one, `ms` its CPU time,
 * `kind` what it presented (1 the main canvas, 2 the wet tile, 3 both). */
export function noteFrame(drew: boolean, ms: number, kind = 1) {
  if (!drew) return;
  drawn++;
  frameMsSum += ms;
  const now = performance.now();
  if (penDown) {
    for (const w of run ? [live, run] : [live]) {
      if (lastWriteFrame) w.intervals.push(now - lastWriteFrame);
      if (kind & 1) w.mainFrames++;
      else w.tileFrames++;
    }
    if (live.intervals.length > 4000) live.intervals.splice(0, 2000);
    lastWriteFrame = now;
  }
  if (!pending.length) return;
  for (const ts of pending) {
    push(frameLat, now - ts);
    if (run) run.latency.push(now - ts);
  }
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
  /** Pen samples per second of pen-down time, and per drawn frame. */
  inputHz: number;
  perFrame: number;
  /** Frames drawn per second of wall time, and the board's CPU time per drawn frame. */
  fps: number;
  frameMs: number;
  /** Frames per second while writing (median frame interval), NaN when nothing was written. */
  writeFps: number;
  /** Frames while writing that presented the main canvas, and the wet tile alone. */
  mainFrames: number;
  tileFrames: number;
  pointer: string;
}

/** Frame rate while writing, from the intervals between frames. */
export interface WritingStats {
  frames: number;
  fpsMedian: number;
  /** The rate the slowest 5 % of frames fall below. */
  fpsP5: number;
  fpsMin: number;
  /** Frame intervals in buckets of display refreshes: on time, 1 missed, 2, 3–5, more. */
  histogram: [number, number, number, number, number];
  latencyP50: number;
  latencyP95: number;
  mainFrames: number;
  tileFrames: number;
  inputHz: number;
  seconds: number;
}

function writingStats(w: Writing, refreshMs: number): WritingStats {
  const hist: WritingStats["histogram"] = [0, 0, 0, 0, 0];
  for (const d of w.intervals) {
    const missed = Math.max(0, Math.round(d / refreshMs) - 1);
    hist[missed === 0 ? 0 : missed === 1 ? 1 : missed === 2 ? 2 : missed <= 5 ? 3 : 4]++;
  }
  const rate = (ms: number) => (ms > 0 ? 1000 / ms : NaN);
  return {
    frames: w.intervals.length,
    fpsMedian: rate(pct(w.intervals, 0.5)),
    fpsP5: rate(pct(w.intervals, 0.95)),
    fpsMin: rate(pct(w.intervals, 1)),
    histogram: hist,
    latencyP50: pct(w.latency, 0.5),
    latencyP95: pct(w.latency, 0.95),
    mainFrames: w.mainFrames,
    tileFrames: w.tileFrames,
    inputHz: w.downMs > 0 ? (w.samples * 1000) / w.downMs : NaN,
    seconds: w.downMs / 1000,
  };
}

/** Start recording a speed test run (frames and pen samples while writing). */
export function startRun() {
  closeDown(performance.now());
  run = fresh();
}

/** End the run and return its numbers. */
export function endRun(refreshMs: number): WritingStats {
  closeDown(performance.now());
  const w = run ?? fresh();
  run = null;
  return writingStats(w, refreshMs);
}

/** Rates since the previous call; percentiles over the last ~600 samples. */
export function snapshot(): LatencySnapshot {
  const now = performance.now();
  const dt = Math.max(1, now - since) / 1000;
  closeDown(now);
  const w = live;
  live = fresh();
  const s: LatencySnapshot = {
    frameP50: pct(frameLat, 0.5),
    frameP95: pct(frameLat, 0.95),
    downP50: pct(downPresent, 0.5),
    inputHz: w.downMs > 50 ? (w.samples * 1000) / w.downMs : 0,
    perFrame: events ? samples / Math.max(1, drawn) : 0,
    fps: drawn / dt,
    frameMs: drawn ? frameMsSum / drawn : 0,
    writeFps: w.intervals.length >= 3 ? 1000 / pct(w.intervals, 0.5) : NaN,
    mainFrames: w.mainFrames,
    tileFrames: w.tileFrames,
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
