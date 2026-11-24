// The pen's path to the screen, on the JS side (docs/performance.md).
//
// The engine draws wet ink in the board's frame loop: smoothed points, a raw
// tip at the pen's newest sample, then the browser's predicted points
// (rupok_ink::StrokeBuilder::tip, Engine::update_wet). The dry stroke ends at
// the same tip, so pen-up does not move the ink. This module adds:
//
// - Delegated ink trail (navigator.ink, Chromium on Windows; Edge WebView2 in
//   the desktop app). After each pen move we tell the OS compositor where our
//   ink ends; it draws the trail from there to the pen *between* our frames,
//   on the next display refresh, ahead of the browser's own frame pipeline.
//   With the trail on, the browser's prediction is off: the OS trail follows
//   the real pen, a prediction could overshoot it.
// - Prediction policy without the trail: predicted points at most one frame
//   ahead, so a fast stroke's tip keeps up without overshooting corners, and
//   pen-up retracts at most that much.
// - The pointer kind (pen, touch, mouse) for the board's cursor: no cursor
//   over a pen's ink, a small dot for a mouse.
//
// Pen styles (brushes.ts) build on this: anything that changes how the wet
// stroke looks must change the dry stroke the same way (see the wet/dry test
// in crates/rupok-render/tests/wet_dry.rs and bench/latency). The OS trail is
// one solid, opaque line, so styles it cannot imitate (dashes, the rainbow,
// a see-through marker) go without it and keep the browser's prediction.

import { createSignal } from "solid-js";
import { camera, color, pageIndex, pages, prefs, tool, width } from "./state";
import { brushOf } from "./brushes";

/** At most this far ahead (ms) of the newest real sample when predicting. */
const PREDICT_MS = 12;

interface InkPresenter {
  updateInkTrailStartPoint(e: PointerEvent, style: { color: string; diameter: number }): void;
}
interface Ink {
  requestPresenter(opts?: { presentationArea?: Element | null }): Promise<InkPresenter>;
}

/** The last pointer kind over the board ("pen", "touch", "mouse"). */
export const [pointerKind, setPointerKind] = createSignal<string>("mouse");

const windows = typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
const ink: Ink | undefined = typeof navigator !== "undefined" ? (navigator as any).ink : undefined;
let presenter: InkPresenter | null = null;
let requested = false;
let trailFailed = false;

/** Delegated ink trail state, for the latency overlay. */
export function trailState(): "on" | "available" | "unsupported" | "failed" {
  if (trailFailed) return "failed";
  if (!ink) return "unsupported";
  // Chromium exposes navigator.ink everywhere, but only Windows draws it.
  if (!windows) return "unsupported";
  return presenter ? "on" : "available";
}

/** Ask for the OS presenter once, over the ink canvas. */
export function attachTrail(canvas: HTMLCanvasElement) {
  if (requested || !ink || !windows) return;
  requested = true;
  ink.requestPresenter({ presentationArea: canvas }).then(
    (p) => (presenter = p),
    () => (trailFailed = true),
  );
}

const inking = () => tool() === "pen";
/** Pen styles the OS trail can lead: a solid line in the pen's colour. */
const TRAIL_OFF = ["dashed", "dotted", "rainbow", "marker"];
const trailFits = () => !TRAIL_OFF.includes(brushOf("pen"));
let penSeen = false;

/** Whether this pointer's press draws ink (mirrors the engine's palm
 * rejection: touch draws until a pen is seen, unless set otherwise). */
export function drawsInk(e: PointerEvent, x: number, y: number): boolean {
  if (e.pointerType === "pen") penSeen = true;
  // Ink that starts off a slide page draws nothing (engine/clip.rs).
  if (!inking() || e.button > 0 || !onPage(x, y, 0)) return false;
  if (e.pointerType !== "touch") return true;
  const m = prefs().touchMode;
  return m === "draw" || (m === "auto" && !penSeen);
}

/** CSS colour for a 0xRRGGBBAA pen colour, opaque (a trail must be). */
function css(rgba: number): string {
  return `rgb(${(rgba >>> 24) & 255}, ${(rgba >>> 16) & 255}, ${(rgba >>> 8) & 255})`;
}

/** Whether a board point (CSS px from the board's corner) lies on the page
 * with `margin` to spare, when ink is kept on the page (engine/clip.rs:
 * slide pages clip ink to [0, 0, w, h] in world units; infinite pages never). */
function onPage(x: number, y: number, margin: number): boolean {
  if (!prefs().keepInkOnPage) return true;
  const f = pages()[pageIndex()]?.frame;
  if (!f) return true;
  const [cx, cy, z] = camera();
  const wx = cx + x / z;
  const wy = cy + y / z;
  const m = margin / z;
  return wx >= m && wy >= m && wx <= f.w - m && wy <= f.h - m;
}

/** After the board took a pen move: hand the OS trail its start point.
 * `x`, `y`: the move's position on the board (CSS px from its corner). */
export function trailAfterMove(e: PointerEvent, drawing: boolean, x: number, y: number) {
  if (!presenter || !drawing || !inking() || !trailFits()) return;
  // Mirror rupok_ink::Brush::radius and the pressure curve, so the trail is
  // as wide as the ink it leads.
  const p = e.pointerType === "pen" && e.pressure > 0 ? Math.pow(Math.min(1, e.pressure), prefs().pressure) : 0.5;
  const b = brushOf("pen");
  const k = b === "ballpoint" ? 1 : b === "brush" ? 0.15 + 0.85 * p : b === "pencil" ? 0.6 + 0.4 * p : 0.35 + 0.65 * p;
  const diameter = Math.max(1, width() * camera()[2] * k);
  // Off the page the engine cuts the ink; the OS trail must not draw there
  // either, or the tip would show ink that never lands. Skipping an update
  // leaves that frame without a trail.
  if (!onPage(x, y, diameter / 2)) return;
  try {
    presenter.updateInkTrailStartPoint(e, { color: css(color()), diameter });
  } catch {
    trailFailed = true;
    presenter = null;
  }
}

/** The browser's predicted samples to draw for this move. */
export function predictedFor(e: PointerEvent): PointerEvent[] {
  if (presenter && inking() && trailFits()) return [];
  const list = e.getPredictedEvents ? e.getPredictedEvents() : [];
  return list.filter((p) => p.timeStamp - e.timeStamp <= PREDICT_MS);
}

/** Cursor over the board for drawing tools: none for pen and touch (the ink
 * is the feedback, a crosshair would hide it), a small dot for a mouse. */
export function drawCursor(): string {
  // While drawing, nothing at all (an IR board that reports itself as a
  // mouse still gets a clean tip).
  if (pointerKind() !== "mouse" || pressed()) return "none";
  return `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='9' height='9'%3E%3Ccircle cx='4.5' cy='4.5' r='2.5' fill='%231c2024' stroke='white' stroke-width='1.2'/%3E%3C/svg%3E") 4 4, default`;
}

// ------------------------------------------------- pen activity (idle work)

let downCount = 0;
let lastActive = 0;
const [pressed, setPressed] = createSignal(false);

export function notePointerDown() {
  downCount++;
  lastActive = performance.now();
  setPressed(true);
}
export function notePointerUp() {
  downCount = Math.max(0, downCount - 1);
  lastActive = performance.now();
  if (!downCount) setPressed(false);
}

/** True while a pointer is down or was within `idleMs`: heavy work
 * (autosave, recognisers) waits, so it never lands in the middle of a stroke
 * or in the first frames of the next one. */
export function penBusy(idleMs = 0): boolean {
  return downCount > 0 || performance.now() - lastActive < idleMs;
}
