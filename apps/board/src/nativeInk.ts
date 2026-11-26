// The native wet-ink layer (experimental, Windows desktop app; off by default:
// Settings → About → Performance lab → Native wet ink).
//
// The browser shows a pen sample only in its next frame, after the GPU and
// the compositor have taken the previous one; on a 4K board with integrated
// graphics that is the wait a teacher feels. With this layer on, the Windows
// app draws the stroke being written itself: a transparent, click-through
// window over the board (DirectComposition + Direct2D,
// apps/desktop/src/native_ink.rs) that is given each pen sample the moment
// the browser has it (`pointerrawupdate`, before any frame) and draws just
// the new piece. The board's engine still receives every sample and builds
// the stroke as always; it only does not draw it wet. At pen-up the dry
// stroke appears in the board's own frame and the native copy is removed
// once that frame is on screen.
//
// It is a prototype with known limits (docs/performance.md, "Native wet
// ink"): a plain solid line (so only the pen and ballpoint styles use it),
// the raw pen samples (the board's smoothing shows at the hand-off), one
// pen at a time. It fails safe: any error from the native side switches it
// off for the session and the board draws wet ink itself again.

import { createSignal } from "solid-js";
import { isDesktop } from "./platform";
import { camera, color, pageIndex, pages, prefs, setPrefs, showToast, tool, width } from "./state";
import { brushOf } from "./brushes";
import { blockTrail } from "./wetInk";

export type NativeInkState = "unsupported" | "off" | "ready" | "drawing" | "failed";

const windows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
/** The Windows desktop app is the only place the layer exists. */
export const nativeInkAvailable = isDesktop && windows;

const [state, setState] = createSignal<NativeInkState>(nativeInkAvailable ? "off" : "unsupported");
export const nativeInkState = state;
const [failure, setFailure] = createSignal("");
/** Why the layer switched itself off, for Settings and the speed test. */
export const nativeInkFailure = failure;

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
let invoke: Invoke | null = null;
/** The pointer whose stroke the native layer is drawing. */
let active: number | null = null;
let hideWet: (hidden: boolean) => void = () => {};
let strokeSeq = 0;
/** Samples sent and frames of them drawn natively, for the speed test. */
export const nativeCounters = { strokes: 0, points: 0 };

function fail(e: unknown) {
  // A late answer after the layer was switched off is not a failure.
  if (state() !== "ready" && state() !== "drawing") return;
  const msg = String((e as any)?.message ?? e);
  if (state() !== "failed") {
    console.warn("native wet ink switched off:", msg);
    showToast("Native wet ink switched itself off; the board draws ink as usual.", 5000);
  }
  setFailure(msg);
  setState("failed");
  active = null;
  hideWet(false);
  blockTrail(false);
  void invoke?.("native_ink_stop").catch(() => {});
}

/** Start or stop the layer to match the setting. `setWetHidden` hides the
 * board's own wet stroke while the native layer shows it. */
export async function applyNativeInk(setWetHidden: (hidden: boolean) => void) {
  hideWet = setWetHidden;
  if (!nativeInkAvailable) return;
  const want = prefs().nativeWetInk;
  if (!want) {
    if (state() === "ready" || state() === "drawing") {
      active = null;
      hideWet(false);
      blockTrail(false);
      void invoke?.("native_ink_stop").catch(() => {});
    }
    setFailure("");
    setState("off");
    return;
  }
  if (state() === "ready" || state() === "drawing") return;
  try {
    invoke ??= (await import("@tauri-apps/api/core")).invoke as Invoke;
    await invoke("native_ink_start");
    setFailure("");
    setState("ready");
    // The native layer leads the pen itself: no OS trail on top of it.
    blockTrail(true);
  } catch (e) {
    // It did not start: say why (Settings shows it), and leave the switch
    // off so the next start is clean.
    const msg = String((e as any)?.message ?? e);
    console.warn("native wet ink did not start:", msg);
    showToast("Native wet ink could not start; the board draws ink as usual.", 5000);
    setFailure(msg);
    setState("failed");
    setPrefs({ nativeWetInk: false });
  }
}

/** Pen styles the layer can stand in for: a solid line. */
const fits = () => tool() === "pen" && ["pen", "ballpoint"].includes(brushOf("pen")) && (color() & 0xff) === 0xff;

/** The stroke's width at a sample, as the engine has it (rupok_ink::Brush::radius
 * and the pressure curve; the same mirror as the OS ink trail in wetInk.ts). */
function radius(e: PointerEvent): number {
  const p = e.pointerType === "pen" && e.pressure > 0 ? Math.pow(Math.min(1, e.pressure), prefs().pressure) : 0.5;
  const k = brushOf("pen") === "ballpoint" ? 1 : 0.35 + 0.65 * p;
  return Math.max(0.5, (width() * camera()[2] * k * (window.devicePixelRatio || 1)) / 2);
}

function point(e: PointerEvent): number[] {
  const dpr = window.devicePixelRatio || 1;
  return [e.clientX * dpr, e.clientY * dpr, radius(e)];
}

/** A press that draws ink went down at the board (`rect`: the board's client
 * rect). True when the native layer took the stroke: the board then hides
 * its own wet copy until `nativeInkUp`. */
export function nativeInkDown(e: PointerEvent, rect: DOMRect): boolean {
  if (state() !== "ready" || active !== null || !invoke || !fits()) return false;
  const dpr = window.devicePixelRatio || 1;
  // Ink is kept on the page (engine/clip.rs): the native layer clips to it too.
  let clip = [rect.left, rect.top, rect.right, rect.bottom];
  const f = prefs().keepInkOnPage ? pages()[pageIndex()]?.frame : null;
  if (f) {
    const [cx, cy, z] = camera();
    clip = [
      Math.max(clip[0], rect.left + (0 - cx) * z), Math.max(clip[1], rect.top + (0 - cy) * z),
      Math.min(clip[2], rect.left + (f.w - cx) * z), Math.min(clip[3], rect.top + (f.h - cy) * z),
    ];
  }
  const c = color();
  active = e.pointerId;
  const seq = ++strokeSeq;
  nativeCounters.strokes++;
  hideWet(true);
  setState("drawing");
  invoke("native_ink_begin", {
    seq,
    rgb: [(c >>> 24) & 255, (c >>> 16) & 255, (c >>> 8) & 255],
    clip: clip.map((v) => v * dpr),
    point: point(e),
  }).catch(fail);
  return true;
}

/** Pen samples as the browser receives them (`pointerrawupdate`): straight
 * to the native layer, not waiting for a frame. */
export function nativeInkRaw(e: PointerEvent) {
  if (active !== e.pointerId || !invoke) return;
  const list = e.getCoalescedEvents ? e.getCoalescedEvents() : [];
  const pts: number[] = [];
  for (const c of list.length ? list : [e]) pts.push(...point(c));
  nativeCounters.points += pts.length / 3;
  invoke("native_ink_points", { seq: strokeSeq, pts }).catch(fail);
}

/** The pen lifted (or the press was cancelled). The board draws the dry
 * stroke in its next frame; the native copy goes once that is on screen. */
export function nativeInkUp(pointerId: number) {
  if (active !== pointerId) return;
  active = null;
  hideWet(false);
  if (state() === "drawing") setState("ready");
  const seq = strokeSeq;
  // Two frames for the board's frame to be drawn and composited, and a
  // little more for a slow compositor: opaque ink over the same ink shows
  // nothing in between.
  requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(() => {
    if (state() === "ready" || state() === "drawing") void invoke?.("native_ink_end", { seq }).catch(fail);
  }, 40)));
}

/** Whether the native layer is drawing this pointer's stroke. */
export const nativeInkActive = (pointerId: number) => active === pointerId;
