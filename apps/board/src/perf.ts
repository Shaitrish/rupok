// How much the board asks of the GPU (docs/performance.md, "Big screens and
// weak GPUs"): the render scale of the ink canvas and its governor, the wet
// tile's size, and which renderer to start.
//
// A 4K smartboard on integrated graphics cannot fill 8.3 million pixels
// several times per frame at 60 fps. Two things keep it from having to:
//
// - The wet tile (crates/rupok-render, `InkRenderer::plan`): while the pen
//   writes, only a small canvas under the pen is drawn and presented.
// - The render scale: the ink canvas is drawn at a fraction of the screen's
//   pixels and stretched by CSS, which makes the frames that do need the
//   whole canvas (pan, zoom, erase, pen-up) cheaper.
//
// Everything here can be switched at runtime (Settings → About →
// Performance lab) so a board can be tried in each mode in one visit.

import { createSignal } from "solid-js";
import { prefs } from "./state";
import { graphicsAdapter } from "./inkLatency";

export type RenderScalePref = "auto" | "100" | "75" | "50";
export type RendererPref = "auto" | "webgpu" | "webgl";

/** The scales Auto moves between. */
const AUTO_STEPS = [1, 0.75, 0.5];
/** Auto starts at the sharpest step with at most this many canvas pixels
 * (a little over 2560 × 1440): a 4K board starts at 75 %. */
const AUTO_START_PIXELS = 5_000_000;

const [autoStep, setAutoStep] = createSignal(-1);
/** The sharpest step Auto may return to (lowered when a step up was slow). */
let ceiling = 0;
/** The board size (in screen pixels) Auto's step was chosen for. */
let basis = 0;

function startStep(cssW: number, cssH: number, dpr: number): number {
  const px = cssW * cssH * dpr * dpr;
  const i = AUTO_STEPS.findIndex((s) => px * s * s <= AUTO_START_PIXELS);
  return i < 0 ? AUTO_STEPS.length - 1 : i;
}

/** The render scale in use: canvas pixels per screen pixel, per axis. */
export function renderScale(cssW: number, cssH: number, dpr: number): number {
  const p = prefs().renderScale;
  if (p === "100") return 1;
  if (p === "75") return 0.75;
  if (p === "50") return 0.5;
  // Start again when the board's size changed a lot (the window went full
  // screen, or to another display): what was learned was about another size.
  const px = cssW * cssH * dpr * dpr;
  if (autoStep() < 0 || Math.abs(px - basis) > basis * 0.25) {
    basis = px;
    ceiling = startStep(cssW, cssH, dpr);
    setAutoStep(ceiling);
    gov.reset();
  }
  return AUTO_STEPS[autoStep()];
}

/** Forget what Auto learned (the window moved to another screen, or the
 * setting changed). */
export function resetAutoScale() {
  setAutoStep(-1);
  gov.reset();
}

/** "Auto (75 %)", "100 %"…, for the overlay and the speed test. */
export function renderScaleLabel(scale: number): string {
  const pct = `${Math.round(scale * 100)} %`;
  return prefs().renderScale === "auto" ? `Auto (${pct})` : pct;
}

export type WetLayerPref = "off" | "small" | "medium" | "large";
/** The wet tile as a share of the canvas (width, height) and its limits in
 * canvas pixels. Medium on a 4K board is 1024 × 768: a tenth of the screen. */
const TILES: Record<Exclude<WetLayerPref, "off">, [number, number, number, number]> = {
  small: [0.19, 0.25, 768, 512],
  medium: [0.27, 0.36, 1280, 896],
  large: [0.4, 0.5, 1536, 1024],
};

/** The wet tile's size for a main canvas of `w` × `h` pixels: a handwriting
 * area with room to move. Every frame while writing costs about its area
 * (measured: GPU time per frame follows it), and a stroke that leaves it
 * costs one full-size frame (the renderer puts the next tile ahead of the
 * pen), so it is as small as a few written words. */
export function tileSize(w: number, h: number, size: Exclude<WetLayerPref, "off"> = "medium"): [number, number] {
  const [fw, fh, maxW, maxH] = TILES[size];
  const step = (v: number, lo: number, hi: number, full: number) => Math.min(full, Math.max(lo, Math.min(hi, Math.round(v / 64) * 64)));
  return [step(w * fw, 384, maxW, w), step(h * fh, 256, maxH, h)];
}

// ------------------------------------------------------------- the governor

/** Watches real frame intervals while the board draws. Many slow frames:
 * one step down. A long clean run below the starting scale: one step back
 * up, once; if that was slow again the board stays down for the session. */
class Governor {
  private last = 0;
  private lastDrew = false;
  private idle: number[] = [];
  private n = 0;
  private slow = 0;
  private goodWindows = 0;
  private wanted = 0;
  private steppedUp = false;
  /** Display refresh interval (ms), from frames in which nothing was drawn. */
  refreshMs = 1000 / 60;

  reset() {
    this.n = this.slow = this.goodWindows = this.wanted = 0;
    this.steppedUp = false;
  }

  /** Once per animation frame; `drew` when the board presented a canvas. */
  frame(now: number, drew: boolean) {
    const dt = this.last ? now - this.last : 0;
    this.last = now;
    if (dt > 0 && dt < 100 && !drew && !this.lastDrew) {
      this.idle.push(dt);
      if (this.idle.length >= 40) {
        const v = this.idle.sort((a, b) => a - b);
        this.refreshMs = Math.min(1000 / 24, Math.max(1000 / 360, v[v.length >> 1]));
        this.idle = [];
      }
    }
    // An interval between two drawn frames: how long the GPU and the
    // compositor took to take one and ask for the next.
    if (drew && this.lastDrew && dt > 0) {
      this.n++;
      if (dt > this.refreshMs * 1.6) this.slow++;
      if (this.n >= 90) {
        const share = this.slow / this.n;
        this.n = this.slow = 0;
        if (share > 0.3) {
          this.wanted = 1;
          this.goodWindows = 0;
        } else if (share < 0.03) {
          if (++this.goodWindows >= 20) {
            this.wanted = -1;
            this.goodWindows = 0;
          }
        } else {
          this.goodWindows = 0;
        }
      }
    }
    this.lastDrew = drew;
  }

  /** Apply a pending step (call when no pointer is down). True if the scale changed. */
  settle(): boolean {
    const w = this.wanted;
    this.wanted = 0;
    if (!w || prefs().renderScale !== "auto" || autoStep() < 0) return false;
    const cur = autoStep();
    if (w > 0) {
      // Slow right after a step up: never go back up this session.
      if (this.steppedUp) ceiling = Math.min(AUTO_STEPS.length - 1, cur + 1);
      this.steppedUp = false;
      if (cur >= AUTO_STEPS.length - 1) return false;
      setAutoStep(cur + 1);
      return true;
    }
    if (cur <= ceiling) return false;
    this.steppedUp = true;
    setAutoStep(cur - 1);
    return true;
  }
}

export const gov = new Governor();

/** The display's refresh rate as the board's frame loop sees it. */
export const refreshHz = () => Math.round(1000 / gov.refreshMs);

// ------------------------------------------------------------- the renderer

const TRY_KEY = "rupok.gpu.trying";

/** Chromium with WebGPU on a Windows or Linux computer (Chrome, Edge, the
 * Windows app's WebView2): where WebGPU is what we measured. Safari,
 * Firefox, Macs, phones and Chromebooks stay on WebGL2 until tested. */
export const webgpuTested = () => {
  const ua = navigator.userAgent;
  return "gpu" in navigator && /Chrome\//.test(ua) && /Windows NT|X11; Linux/.test(ua) && !/Android|CrOS|Mobile/.test(ua);
};

/** Whether to start the board on WebGL2. "Auto" is WebGPU in Chromium on
 * Windows and Linux where it runs on a real GPU (it was about twice as fast as WebGL2 on the first
 * 4K board we measured: wgpu's WebGL2 path adds a full-canvas copy to every
 * frame), and WebGL2 otherwise: without a GPU (a software adapter), or when
 * the last WebGPU start did not finish. */
export async function startOnWebgl(): Promise<boolean> {
  const r = prefs().renderer;
  if (r === "webgl") return true;
  if (r === "webgpu") return false;
  if (!webgpuTested() || graphicsAdapter().software) return true;
  try {
    // The mark holds for a week, then WebGPU gets another try.
    const at = Number(localStorage.getItem(TRY_KEY) || 0);
    if (at > 0 && Date.now() - at < 7 * 86400_000) return true;
  } catch { /* storage unavailable */ }
  try {
    const a = await (navigator as any).gpu.requestAdapter();
    if (!a) return true;
    const i = a.info ?? {};
    const name = `${i.vendor ?? ""} ${i.architecture ?? ""} ${i.device ?? ""} ${i.description ?? ""}`;
    return !!(a.isFallbackAdapter || i.isFallbackAdapter) || /swiftshader|llvmpipe|softpipe|basic render|warp/i.test(name);
  } catch {
    return true;
  }
}

/** Around a WebGPU start in Auto: a start that hangs or crashes the page
 * leaves the mark, and the next start uses WebGL2. */
export function markGpuStart(done: boolean) {
  try {
    if (done) localStorage.removeItem(TRY_KEY);
    else localStorage.setItem(TRY_KEY, String(Date.now()));
  } catch { /* storage unavailable */ }
}
