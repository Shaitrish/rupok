// Classroom touch gestures. A layer that sees touch pointers before the board
// engine does, classifies them, and either lets them through or consumes them:
//
//   palm eraser  a palm, the side of a hand or a thumb rubbing the board erases
//                like a board duster; the eraser follows the contact size
//   swipe        2 or 3 fingers flicked left / right: next / previous slide or step
//   flick        one finger, in Teach and the student view, when fingers don't draw
//   taps         two-finger tap undo, three-finger tap redo, two-finger double-tap fit
//   long-press   two fingers held still: the Smart Dock comes to that spot
//
// Detection works with and without contact size. Boards that report it
// (PointerEvent width/height) get a palm from one large contact. Boards that
// don't (most IR frames, Linux WebKitGTK; they report 0 or 1) get a palm
// from 3+ touches close together that move together, from two touches close
// together rubbing back and forth (an IR frame often sees a palm as two),
// or from one touch scrubbing fast back and forth while fingers don't draw.
//
// Touches the engine already has (a finger that started drawing or panning)
// are taken back with a cancel, and the camera returns to where the gesture
// began, so a recognised gesture leaves no stray ink or pan behind.
//
// No framework, state or wasm imports here: the recogniser is unit-tested with
// synthetic touch streams (apps/board/test/gestures.test.ts). GestureLayer.tsx
// wires it to the controller and draws the feedback.

// ------------------------------------------------------------------ config

export interface GestureCalibration {
  /** Median contact diameter of one finger, CSS px (0: not reported). */
  finger: number;
  /** Largest contact diameter of a palm, CSS px (0: not reported). */
  palm: number;
  /** Most touches the palm produced at once. */
  contacts: number;
  /** Largest distance of a palm touch from their centre, CSS px. */
  spread: number;
  reportsSize: boolean;
  /** When it was measured (ms since the epoch). */
  at: number;
}

export interface GestureConfig {
  /** Palm / hand eraser. */
  palmErase: boolean;
  /** Contact diameter (CSS px) from which one touch counts as a palm. */
  palmSize: number;
  /** Eraser radius relative to the contact (0.5–2). */
  eraserScale: number;
  /** 3+ touches close together that rub erase (boards without contact size). */
  handCluster: boolean;
  /** One touch scrubbing fast back and forth erases, when fingers don't draw. */
  thumbScrub: boolean;
  /** Multi-finger swipe left / right: next / previous. */
  swipe: boolean;
  swipeFingers: 2 | 3;
  /** One-finger flick in Teach and the student view, when fingers don't draw. */
  flick: boolean;
  /** 0 (strict: long fast swipes only) … 1 (eager). */
  swipeSensitivity: number;
  tapUndo: boolean;
  tapRedo: boolean;
  doubleTapFit: boolean;
  longPressDock: boolean;
  /** Bigger buttons, handles and switches everywhere. */
  largeTargets: boolean;
  /** A short hint the first time each gesture fires. */
  hints: boolean;
  calibration: GestureCalibration | null;
}

export const DEFAULT_GESTURES: GestureConfig = {
  palmErase: true,
  palmSize: 40,
  eraserScale: 1,
  handCluster: true,
  thumbScrub: true,
  swipe: true,
  swipeFingers: 2,
  flick: true,
  swipeSensitivity: 0.5,
  tapUndo: true,
  tapRedo: true,
  doubleTapFit: true,
  longPressDock: true,
  largeTargets: false,
  hints: true,
  calibration: null,
};

const BOOL_KEYS = [
  "palmErase", "handCluster", "thumbScrub", "swipe", "flick", "tapUndo", "tapRedo", "doubleTapFit",
  "longPressDock", "largeTargets", "hints",
] as const;
const NUM_KEYS: Record<"palmSize" | "eraserScale" | "swipeSensitivity", [number, number]> = {
  palmSize: [12, 200],
  eraserScale: [0.5, 2],
  swipeSensitivity: [0, 1],
};

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const num = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

/** Keep what is valid of a stored or imported config; unknown keys and bad
 * values are dropped, numbers are clamped. Never throws. */
export function sanitizeGestures(raw: unknown): Partial<GestureConfig> {
  const out: Partial<GestureConfig> = {};
  if (!raw || typeof raw !== "object") return out;
  const r = raw as Record<string, unknown>;
  for (const k of BOOL_KEYS) if (typeof r[k] === "boolean") out[k] = r[k] as boolean;
  for (const [k, [lo, hi]] of Object.entries(NUM_KEYS) as [keyof typeof NUM_KEYS, [number, number]][]) {
    if (num(r[k])) out[k] = clamp(r[k] as number, lo, hi);
  }
  if (r.swipeFingers === 2 || r.swipeFingers === 3) out.swipeFingers = r.swipeFingers;
  const c = r.calibration as Record<string, unknown> | null | undefined;
  if (c === null) out.calibration = null;
  else if (c && typeof c === "object" && num(c.finger) && num(c.palm)) {
    out.calibration = {
      finger: clamp(c.finger as number, 0, 400),
      palm: clamp(c.palm as number, 0, 800),
      contacts: num(c.contacts) ? clamp(Math.round(c.contacts as number), 0, 20) : 0,
      spread: num(c.spread) ? clamp(c.spread as number, 0, 800) : 0,
      reportsSize: c.reportsSize === true,
      at: num(c.at) ? (c.at as number) : 0,
    };
  }
  return out;
}

export function resolveGestures(raw: unknown): GestureConfig {
  return { ...DEFAULT_GESTURES, ...sanitizeGestures(raw) };
}

/** Thresholds derived from the config. Distances in CSS px, times in ms. */
export function thresholds(c: GestureConfig) {
  const s = clamp(c.swipeSensitivity, 0, 1);
  // Geometric interpolation: strict at 0, eager at 1.
  const ease = (strict: number, eager: number) => strict * Math.pow(eager / strict, s);
  const swipeVelocity = ease(1.2, 0.3); // px/ms over the motion (0.6 at 0.5)
  const swipeDistance = ease(200, 60); // px (≈110 at 0.5)
  const spread = c.calibration?.spread ?? 0;
  return {
    swipeVelocity,
    swipeDistance,
    flickVelocity: swipeVelocity * 1.15,
    flickDistance: swipeDistance * 1.2,
    /** tan of the largest angle from horizontal a swipe may have (30°). */
    swipeSlope: Math.tan((30 * Math.PI) / 180),
    swipeMaxMs: 700,
    /** Movement below this is "still" (taps, long-press). */
    slop: 16,
    tapMaxMs: 320,
    /** Fingers of a multi-finger tap land within this of each other. */
    tapStagger: 180,
    doubleTapMs: 350,
    longPressMs: 650,
    /** Hand cluster: touches within this distance of their centre. */
    clusterRadius: spread > 0 ? clamp(spread * 1.5, 90, 320) : 170,
    clusterMin: 3,
    /** Two touches this close (CSS px) rubbing back and forth are a palm. */
    pairGap: 110,
    pairReversals: 2,
    pairTravel: 60,
    /** Mean path a cluster travels before it erases. */
    clusterTravel: 18,
    scrubAmplitude: 22,
    scrubReversals: 3,
    scrubWindow: 900,
    scrubTravel: 120,
    /** A palm erases once it has moved this far (a resting hand doesn't). */
    armTravel: 10,
    minRadius: 16,
    maxRadius: 200,
    /** Widths at or below this mean "the board does not report size". */
    sizeFloor: 3,
    /** After the pen lifts, palm-sized contacts are the writing hand. */
    penGraceMs: 500,
  };
}

// ------------------------------------------------------------------ calibration

/** Turn the "Calibrate my board" measurements into settings. */
export function calibrate(
  finger: { size: number },
  palm: { size: number; contacts: number; spread: number },
  now = Date.now(),
): { patch: Partial<GestureConfig>; reportsSize: boolean; message: string } {
  const f = finger.size > 3 ? Math.round(finger.size) : 0;
  const p = palm.size > 3 ? Math.round(palm.size) : 0;
  const reportsSize = f > 0 && p > 0 && p >= f * 1.5 && p - f >= 12;
  const calibration: GestureCalibration = {
    finger: f, palm: p, contacts: palm.contacts, spread: Math.round(palm.spread), reportsSize, at: now,
  };
  if (reportsSize) {
    // Between the two, nearer the finger: a thumb or the side of a hand counts.
    const thr = Math.round(clamp(Math.sqrt(f * p), f * 1.35, p * 0.8));
    return {
      reportsSize,
      patch: { palmErase: true, palmSize: thr, calibration },
      message: `Your board reports touch size: a finger is about ${f} px and your palm about ${p} px. Contacts wider than ${thr} px now erase.`,
    };
  }
  if (palm.contacts >= 3) {
    return {
      reportsSize,
      patch: { palmErase: true, handCluster: true, calibration },
      message: `Your board does not report touch size, using hand-cluster detection: your palm showed as ${palm.contacts} touches, so 3 or more touches close together that rub will erase.`,
    };
  }
  return {
    reportsSize,
    patch: { palmErase: true, handCluster: true, thumbScrub: true, calibration },
    message: "Your board does not report touch size, using hand-cluster detection. Your palm showed as one touch, so rub quickly back and forth to erase (when fingers don't draw), or use the Eraser tool (E).",
  };
}

// ------------------------------------------------------------------ recogniser

export type GestureId = "palm" | "swipe" | "flick" | "tapUndo" | "tapRedo" | "doubleTapFit" | "longPressDock";

/** What one finger does on the board right now. */
export type FingerRole = "draw" | "select" | "pan";

export type Feedback =
  | { kind: "palm"; x: number; y: number; r: number }
  | { kind: "palm-end"; erased: number }
  | { kind: "swipe"; dir: 1 | -1; progress: number; valid: boolean }
  | { kind: "swipe-end"; dir: 1 | -1; committed: boolean }
  | { kind: "fired"; gesture: GestureId; x: number; y: number };

export interface GestureHost {
  oneFinger(penSeen: boolean): FingerRole;
  /** "prepare" | "teach" | "view" */
  mode(): string;
  /** The document can be edited (not the student view, not replaying). */
  canEdit(): boolean;
  /** Take a pointer the engine already has back from it. */
  cancel(id: number): void;
  camera(): number[];
  setCamera(c: number[]): void;
  /** Erase strokes under a circle swept from (x0, y0) to (x1, y1), board px. */
  erase(x0: number, y0: number, x1: number, y1: number, r: number): number;
  /** Commit the palm erase as one undo step; returns strokes erased. */
  eraseEnd(): number;
  next(): void;
  prev(): void;
  undo(): void;
  redo(): void;
  fit(): void;
  summonDock(x: number, y: number): void;
  feedback(f: Feedback): void;
  setTimer(fn: () => void, ms: number): unknown;
  clearTimer(id: unknown): void;
}

export interface GPointer {
  id: number;
  type: string; // "touch" | "pen" | "mouse"
  /** Board-local CSS px. */
  x: number;
  y: number;
  /** Event time, ms. */
  t: number;
  /** Contact size, CSS px (PointerEvent.width / height). */
  w: number;
  h: number;
  /** The engine sees this event unless it is consumed. */
  board: boolean;
}

export type Verdict = "pass" | "consume";
type Phase = "idle" | "pending" | "palm" | "swipe" | "done" | "blocked";

interface Axis {
  ext: number;
  dir: number;
}

interface Track {
  id: number;
  x0: number; y0: number; t0: number;
  x: number; y: number; t: number;
  /** Last position used for erasing. */
  ex: number; ey: number;
  size: number;
  maxSize: number;
  /** The engine has seen its pointerdown. */
  passed: boolean;
  /** Ours: the engine never sees (or no longer sees) this pointer. */
  claimed: boolean;
  travel: number;
  /** Time it first moved beyond the slop (start of motion). */
  tm: number;
  path: number[]; // x, y pairs, capped
  ax: Axis; ay: Axis;
  reversals: number[];
}

interface PalmState {
  how: "size" | "cluster" | "scrub";
  armed: boolean;
  erased: number;
  /** Cluster: last centre and how many touches it was made of. */
  cx: number; cy: number; n: number;
  r: number;
}

const PATH_CAP = 96;

export class GestureRecognizer {
  private tracks = new Map<number, Track>();
  private phase: Phase = "idle";
  private sessionCam: number[] | null = null;
  private sessionT0 = 0;
  private maxTouches = 0;
  private downTimes: number[] = [];
  private lifted: Track[] = [];
  private pens = new Set<number>();
  private lastPenUp = -Infinity;
  private palm: PalmState | null = null;
  private swipe: { dir: 1 | -1; flick: boolean; valid: boolean } | null = null;
  private longPress: unknown = null;
  private tapTimer: unknown = null;
  private lastTapEnd = -Infinity;
  /** Contact sizes of ordinary fingers (at touch-down), for "palm = much bigger than a finger". */
  private fingerSizes: number[] = [];
  penSeen = false;
  private host: GestureHost;
  private config: () => GestureConfig;

  constructor(host: GestureHost, config: () => GestureConfig) {
    this.host = host;
    this.config = config;
  }

  /** A pointer this recogniser is tracking (it wants all of its events). */
  tracking(id: number): boolean {
    return this.tracks.has(id) || this.pens.has(id);
  }

  /** True while a gesture owns the touches (palm erase, swipe…). */
  get active(): boolean {
    return this.phase === "palm" || this.phase === "swipe" || this.phase === "done";
  }

  get state(): Phase {
    return this.phase;
  }

  handle(kind: "down" | "move" | "up" | "cancel", p: GPointer): Verdict {
    if (p.type === "pen") return this.pen(kind, p);
    if (p.type !== "touch") return "pass";
    if (kind === "down") return this.down(p);
    if (kind === "move") return this.move(p);
    return this.up(p, kind === "cancel");
  }

  /** Forget everything (e.g. calibration took over the screen). */
  reset() {
    if (this.phase === "palm") this.endPalm();
    this.clearLongPress();
    this.tracks.clear();
    this.endSession();
  }

  // ----------------------------------------------------------- pen

  private pen(kind: string, p: GPointer): Verdict {
    if (kind === "down") {
      this.pens.add(p.id);
      this.penSeen = true;
      // The pen wins: it ends a palm erase, a swipe or a pending gesture.
      if (this.phase === "palm") {
        this.endPalm();
        this.phase = "done";
      } else if (this.phase === "swipe") {
        this.endSwipe(false);
        this.phase = "done";
      } else if (this.phase === "pending") {
        this.phase = "blocked";
      }
      this.clearLongPress();
    } else if (kind === "up" || kind === "cancel") {
      if (this.pens.delete(p.id)) this.lastPenUp = p.t;
    }
    return "pass";
  }

  private penBusy(t: number): boolean {
    return this.pens.size > 0 || t - this.lastPenUp < thresholds(this.config()).penGraceMs;
  }

  // ----------------------------------------------------------- touch

  private down(p: GPointer): Verdict {
    const cfg = this.config();
    if (this.tracks.size === 0) this.startSession(p);
    const size = this.sizeOf(p);
    const tr: Track = {
      id: p.id, x0: p.x, y0: p.y, t0: p.t, x: p.x, y: p.y, t: p.t, ex: p.x, ey: p.y,
      size, maxSize: size, passed: false, claimed: false, travel: 0, tm: Infinity, path: [p.x, p.y],
      ax: { ext: p.x, dir: 0 }, ay: { ext: p.y, dir: 0 }, reversals: [],
    };
    this.tracks.set(p.id, tr);
    this.downTimes.push(p.t);
    this.maxTouches = Math.max(this.maxTouches, this.tracks.size);

    // While writing, a palm-sized contact is the writing hand: ignore it.
    if (this.penBusy(p.t)) {
      if (this.phase === "pending") this.phase = "blocked";
      if (this.palmSized(tr, true)) {
        tr.claimed = true;
        return "consume";
      }
    }
    switch (this.phase) {
      case "palm": {
        tr.claimed = true;
        if (this.palm) this.palm.n = -1; // the cluster changed: no jump
        return "consume";
      }
      case "swipe":
      case "done":
        tr.claimed = true;
        return "consume";
      case "blocked":
        tr.passed = p.board;
        return "pass";
      default:
        break;
    }
    if (this.palmAllowed(p.t) && this.palmSized(tr)) {
      this.enterPalm("size");
      return "consume";
    }
    tr.passed = p.board;
    this.clearLongPress();
    if (this.tracks.size === 2 && this.maxTouches === 2 && cfg.longPressDock) {
      const th = thresholds(cfg);
      this.longPress = this.host.setTimer(() => this.fireLongPress(), th.longPressMs);
    }
    return "pass";
  }

  private move(p: GPointer): Verdict {
    const tr = this.tracks.get(p.id);
    if (!tr) return "pass";
    this.update(tr, p);
    switch (this.phase) {
      case "palm":
        this.palmMove(tr);
        return "consume";
      case "swipe":
        this.swipeMove();
        return "consume";
      case "done":
        return "consume";
      case "blocked":
        return tr.claimed ? "consume" : "pass";
      default:
        break;
    }
    if (tr.claimed) return "consume";
    const cfg = this.config();
    const th = thresholds(cfg);
    if (tr.travel > th.slop) this.clearLongPress();
    if (this.penBusy(p.t)) {
      this.phase = "blocked";
      return "pass";
    }
    const palmOk = this.palmAllowed(p.t);
    if (palmOk && this.palmSized(tr)) {
      this.enterPalm("size");
      return "consume";
    }
    const sw = this.swipeCandidate(p.t);
    if (sw) {
      this.enterSwipe(sw.dir, sw.flick);
      return "consume";
    }
    if (palmOk && cfg.handCluster && this.clusterHit(p.t)) {
      this.enterPalm("cluster");
      return "consume";
    }
    if (palmOk && cfg.thumbScrub && this.tracks.size === 1 && this.maxTouches === 1 && this.host.oneFinger(this.penSeen) === "pan" && this.scrubHit(tr, p.t)) {
      this.enterPalm("scrub");
      return "consume";
    }
    return "pass";
  }

  private up(p: GPointer, cancelled: boolean): Verdict {
    const tr = this.tracks.get(p.id);
    if (!tr) return "pass";
    this.tracks.delete(p.id);
    this.lifted.push(tr);
    this.clearLongPress();
    const verdict: Verdict = tr.claimed ? "consume" : "pass";
    if (this.phase === "palm") {
      if (this.palm) this.palm.n = -1;
      if (this.tracks.size === 0) {
        this.endPalm();
        this.phase = "done";
      }
    } else if (this.phase === "swipe") {
      if (this.tracks.size === 0) this.endSwipe(!cancelled && !!this.swipe?.valid);
    } else if (this.phase === "pending" && this.tracks.size === 0 && !cancelled) {
      this.maybeTap(p.t);
    }
    if (this.tracks.size === 0) {
      this.learnFingerSizes();
      this.endSession();
    }
    return verdict;
  }

  // ----------------------------------------------------------- session

  private startSession(p: GPointer) {
    this.phase = "pending";
    this.sessionCam = this.host.camera();
    this.sessionT0 = p.t;
    this.maxTouches = 0;
    this.downTimes = [];
    this.lifted = [];
    this.palm = null;
    this.swipe = null;
  }

  private endSession() {
    this.phase = "idle";
    this.sessionCam = null;
    this.palm = null;
    this.swipe = null;
  }

  private sizeOf(p: GPointer): number {
    const s = Math.max(p.w || 0, p.h || 0);
    return s > thresholds(this.config()).sizeFloor ? s : 0;
  }

  private update(tr: Track, p: GPointer) {
    const th = thresholds(this.config());
    const d = Math.hypot(p.x - tr.x, p.y - tr.y);
    tr.travel += d;
    if (tr.tm === Infinity && Math.hypot(p.x - tr.x0, p.y - tr.y0) > 8) tr.tm = tr.t;
    tr.x = p.x;
    tr.y = p.y;
    tr.t = p.t;
    const size = this.sizeOf(p);
    if (size) {
      // Light smoothing: contact sizes jitter frame to frame.
      tr.size = tr.size ? tr.size * 0.5 + size * 0.5 : size;
      tr.maxSize = Math.max(tr.maxSize, size);
    }
    tr.path.push(p.x, p.y);
    if (tr.path.length > PATH_CAP * 2) tr.path.splice(0, tr.path.length - PATH_CAP * 2);
    for (const [ax, v] of [[tr.ax, p.x], [tr.ay, p.y]] as [Axis, number][]) {
      const delta = v - ax.ext;
      if (ax.dir === 0) {
        if (Math.abs(delta) >= th.scrubAmplitude) {
          ax.dir = Math.sign(delta);
          ax.ext = v;
        }
      } else if (delta * ax.dir > 0) {
        ax.ext = v;
      } else if (Math.abs(delta) >= th.scrubAmplitude) {
        ax.dir = -ax.dir;
        ax.ext = v;
        tr.reversals.push(p.t);
      }
    }
  }

  // ----------------------------------------------------------- palm

  private palmAllowed(t: number): boolean {
    const cfg = this.config();
    return cfg.palmErase && this.host.canEdit() && !this.penBusy(t);
  }

  /** A contact much bigger than this board's fingers. `lenient` (palm
   * rejection while writing) skips the finger comparison. */
  private palmSized(tr: Track, lenient = false): boolean {
    const cfg = this.config();
    const size = Math.max(tr.size, tr.maxSize * 0.85);
    if (!size || size < cfg.palmSize) return false;
    if (cfg.calibration && !cfg.calibration.reportsSize) return false;
    if (lenient) return true;
    // Much bigger than this board's fingers. Until a few fingers have been
    // seen (or calibrated), ask for more, in case the board reports one
    // size for every touch.
    const typical = this.typicalFinger();
    return size >= (typical ? Math.max(cfg.palmSize, typical * 1.6) : cfg.palmSize * 1.6);
  }

  private typicalFinger(): number {
    if (this.fingerSizes.length < 3) return this.config().calibration?.finger ?? 0;
    const s = [...this.fingerSizes].sort((a, b) => a - b);
    return s[s.length >> 1];
  }

  private learnFingerSizes() {
    if (this.phase === "palm" || this.phase === "done" || this.phase === "blocked") return;
    for (const tr of this.lifted) {
      const s = tr.size || tr.maxSize;
      if (s && !tr.claimed) this.fingerSizes.push(s);
    }
    if (this.fingerSizes.length > 24) this.fingerSizes.splice(0, this.fingerSizes.length - 24);
  }

  private centre(): { x: number; y: number; spread: number } {
    let x = 0, y = 0;
    const n = this.tracks.size || 1;
    for (const t of this.tracks.values()) { x += t.x; y += t.y; }
    x /= n; y /= n;
    let spread = 0;
    for (const t of this.tracks.values()) spread = Math.max(spread, Math.hypot(t.x - x, t.y - y));
    return { x, y, spread };
  }

  private clusterHit(t: number): boolean {
    const th = thresholds(this.config());
    const n = this.tracks.size;
    if (n === 2 && this.maxTouches === 2) return this.pairRub(t);
    if (n < th.clusterMin) return false;
    const { spread } = this.centre();
    if (spread > th.clusterRadius) return false;
    let travel = 0, reversals = 0;
    for (const tr of this.tracks.values()) { travel += tr.travel; reversals += tr.reversals.length; }
    travel /= n;
    if (n >= 4) return travel >= th.armTravel;
    const cfg = this.config();
    if (cfg.swipe && cfg.swipeFingers === 3) {
      // Three fingers may still become a swipe: wait for a rub, not a stroke.
      return travel >= 40 && (reversals > 0 || t - this.sessionT0 > th.swipeMaxMs);
    }
    return travel >= th.clusterTravel;
  }

  /** Two touches close together rubbing back and forth: a palm the board
   * reports as two contacts. A pinch or a two-finger swipe never reverses. */
  private pairRub(t: number): boolean {
    const th = thresholds(this.config());
    const [a, b] = [...this.tracks.values()];
    if (Math.hypot(a.x - b.x, a.y - b.y) > th.pairGap) return false;
    const recent = (tr: Track) => tr.reversals.filter((r) => t - r <= th.scrubWindow).length;
    if (recent(a) + recent(b) < th.pairReversals || !recent(a) || !recent(b)) return false;
    return (a.travel + b.travel) / 2 >= th.pairTravel;
  }

  private scrubHit(tr: Track, t: number): boolean {
    const th = thresholds(this.config());
    const recent = tr.reversals.filter((r) => t - r <= th.scrubWindow).length;
    return recent >= th.scrubReversals && tr.travel >= th.scrubTravel;
  }

  private claimAll() {
    for (const tr of this.tracks.values()) {
      if (tr.passed && !tr.claimed) this.host.cancel(tr.id);
      tr.claimed = true;
    }
    // Undo any pan or pinch the engine did before we knew.
    if (this.sessionCam) {
      const now = this.host.camera();
      if (now.some((v, i) => Math.abs(v - this.sessionCam![i]) > 1e-4)) this.host.setCamera(this.sessionCam);
    }
  }

  private enterPalm(how: PalmState["how"]) {
    this.clearLongPress();
    this.claimAll();
    this.phase = "palm";
    const c = this.centre();
    this.palm = { how, armed: false, erased: 0, cx: c.x, cy: c.y, n: this.tracks.size, r: this.radius(how) };
    this.maybeArm();
    this.showPalm();
  }

  /** Eraser radius, board px. */
  private radius(how: PalmState["how"], tr?: Track): number {
    const cfg = this.config();
    const th = thresholds(cfg);
    let r: number;
    if (how === "cluster") {
      r = this.centre().spread + 28;
      r = Math.max(r, 44);
    } else if (how === "scrub") {
      r = Math.max(30, ((tr?.size || 0) / 2) * 1.4);
    } else {
      const s = tr ? tr.size || tr.maxSize : Math.max(0, ...[...this.tracks.values()].map((t) => t.size || t.maxSize));
      r = s / 2;
    }
    return clamp(r * cfg.eraserScale, th.minRadius, th.maxRadius);
  }

  private maybeArm() {
    const p = this.palm;
    if (!p || p.armed) return;
    const th = thresholds(this.config());
    let moved = false;
    for (const tr of this.tracks.values()) if (tr.travel >= th.armTravel) moved = true;
    if (!moved) return;
    p.armed = true;
    // Erase the ground the hand already covered before it was recognised.
    for (const tr of this.tracks.values()) {
      const r = p.how === "cluster" ? p.r * 0.7 : this.radius(p.how, tr);
      const path = tr.path;
      for (let i = 2; i < path.length; i += 2) p.erased += this.host.erase(path[i - 2], path[i - 1], path[i], path[i + 1], r);
      if (path.length === 2) p.erased += this.host.erase(path[0], path[1], path[0], path[1], r);
      tr.ex = tr.x;
      tr.ey = tr.y;
    }
    const c = this.centre();
    p.cx = c.x;
    p.cy = c.y;
    p.n = this.tracks.size;
  }

  private palmMove(tr: Track) {
    const p = this.palm;
    if (!p) return;
    if (!p.armed) {
      this.maybeArm();
      this.showPalm();
      return;
    }
    if (p.how === "cluster") {
      const c = this.centre();
      p.r = this.radius("cluster");
      if (p.n === this.tracks.size) p.erased += this.host.erase(p.cx, p.cy, c.x, c.y, p.r);
      p.cx = c.x;
      p.cy = c.y;
      p.n = this.tracks.size;
    } else {
      p.r = this.radius(p.how, tr);
      p.erased += this.host.erase(tr.ex, tr.ey, tr.x, tr.y, p.r);
    }
    tr.ex = tr.x;
    tr.ey = tr.y;
    this.showPalm();
  }

  private showPalm() {
    const p = this.palm;
    if (!p) return;
    if (p.how === "cluster") {
      const c = this.centre();
      this.host.feedback({ kind: "palm", x: c.x, y: c.y, r: this.radius("cluster") });
      return;
    }
    // The biggest contact carries the cursor.
    let best: Track | null = null;
    for (const t of this.tracks.values()) if (!best || (t.size || t.maxSize) > (best.size || best.maxSize)) best = t;
    if (best) this.host.feedback({ kind: "palm", x: best.x, y: best.y, r: this.radius(p.how, best) });
  }

  private endPalm() {
    const p = this.palm;
    const erased = this.host.eraseEnd();
    this.host.feedback({ kind: "palm-end", erased });
    this.palm = null;
    // After the rub, not under the hand; a resting palm is not "the gesture".
    if (p?.armed) this.host.feedback({ kind: "fired", gesture: "palm", x: p.cx, y: p.cy });
  }

  // ----------------------------------------------------------- swipe

  private swipeCandidate(t: number): { dir: 1 | -1; flick: boolean } | null {
    const cfg = this.config();
    const th = thresholds(cfg);
    const n = this.tracks.size;
    if (n !== this.maxTouches) return null;
    let flick = false;
    if (n === 1) {
      if (!cfg.flick) return null;
      const m = this.host.mode();
      if (m !== "teach" && m !== "view") return null;
      if (this.host.oneFinger(this.penSeen) !== "pan") return null;
      flick = true;
    } else if (!cfg.swipe || n !== cfg.swipeFingers) return null;
    // Fingers of a swipe land together.
    if (Math.max(...this.downTimes) - Math.min(...this.downTimes) > th.tapStagger) return null;
    let dx = 0, dy = 0, tm = Infinity;
    const tracks = [...this.tracks.values()];
    for (const tr of tracks) {
      dx += tr.x - tr.x0;
      dy += tr.y - tr.y0;
      tm = Math.min(tm, tr.tm);
    }
    dx /= n;
    dy /= n;
    if (tm === Infinity) return null;
    const dist = flick ? th.flickDistance : th.swipeDistance;
    const vmin = flick ? th.flickVelocity : th.swipeVelocity;
    const elapsed = Math.max(16, t - tm);
    if (Math.abs(dx) < dist || Math.abs(dy) > Math.abs(dx) * th.swipeSlope) return null;
    if (elapsed > th.swipeMaxMs || Math.abs(dx) / elapsed < vmin) return null;
    // Every finger goes the same way (not a pinch).
    for (const tr of tracks) {
      const ddx = tr.x - tr.x0;
      if (Math.sign(ddx) !== Math.sign(dx) || Math.abs(ddx) < Math.abs(dx) * 0.5) return null;
    }
    if (n >= 2) {
      const d0 = Math.hypot(tracks[0].x0 - tracks[1].x0, tracks[0].y0 - tracks[1].y0);
      const d1 = Math.hypot(tracks[0].x - tracks[1].x, tracks[0].y - tracks[1].y);
      if (Math.abs(d1 - d0) > Math.max(48, d0 * 0.35)) return null;
    }
    // Fingers moving left bring the next slide in from the right.
    return { dir: dx < 0 ? 1 : -1, flick };
  }

  private enterSwipe(dir: 1 | -1, flick: boolean) {
    this.clearLongPress();
    this.claimAll();
    this.phase = "swipe";
    this.swipe = { dir, flick, valid: true };
    this.swipeMove();
  }

  private swipeMove() {
    const s = this.swipe;
    if (!s) return;
    const th = thresholds(this.config());
    let dx = 0;
    const n = this.tracks.size || 1;
    for (const tr of this.tracks.values()) dx += tr.x - tr.x0;
    dx /= n;
    const dist = s.flick ? th.flickDistance : th.swipeDistance;
    const along = -dx * s.dir; // positive in the swipe direction
    s.valid = along >= dist * 0.6;
    this.host.feedback({ kind: "swipe", dir: s.dir, progress: clamp(along / (dist * 2), 0, 1), valid: s.valid });
  }

  private endSwipe(commit: boolean) {
    const s = this.swipe;
    if (!s) return;
    this.swipe = null;
    this.host.feedback({ kind: "swipe-end", dir: s.dir, committed: commit });
    if (!commit) return;
    if (s.dir === 1) this.host.next();
    else this.host.prev();
    const c = this.lifted[this.lifted.length - 1];
    this.host.feedback({ kind: "fired", gesture: s.flick ? "flick" : "swipe", x: c?.x ?? 0, y: c?.y ?? 0 });
  }

  // ----------------------------------------------------------- taps, long-press

  private maybeTap(t: number) {
    const cfg = this.config();
    const th = thresholds(cfg);
    const n = this.maxTouches;
    if (n < 2 || n > 3 || this.lifted.length !== n) return;
    if (t - this.sessionT0 > th.tapMaxMs) return;
    if (Math.max(...this.downTimes) - Math.min(...this.downTimes) > th.tapStagger) return;
    if (this.lifted.some((tr) => tr.travel > th.slop)) return;
    if (!this.host.canEdit() && n === 3) return;
    const x = this.lifted.reduce((s, tr) => s + tr.x, 0) / n;
    const y = this.lifted.reduce((s, tr) => s + tr.y, 0) / n;
    if (n === 3) {
      if (cfg.tapRedo && this.host.canEdit()) {
        this.host.redo();
        this.host.feedback({ kind: "fired", gesture: "tapRedo", x, y });
      }
      return;
    }
    const undo = cfg.tapUndo && this.host.canEdit();
    if (cfg.doubleTapFit && t - this.lastTapEnd <= th.doubleTapMs) {
      // The second tap of a double-tap: fit instead of undoing twice.
      if (this.tapTimer !== null) this.host.clearTimer(this.tapTimer);
      this.tapTimer = null;
      this.lastTapEnd = -Infinity;
      this.host.fit();
      this.host.feedback({ kind: "fired", gesture: "doubleTapFit", x, y });
      return;
    }
    this.lastTapEnd = t;
    if (!undo) return;
    const fire = () => {
      this.tapTimer = null;
      this.host.undo();
      this.host.feedback({ kind: "fired", gesture: "tapUndo", x, y });
    };
    if (cfg.doubleTapFit) this.tapTimer = this.host.setTimer(fire, th.doubleTapMs);
    else fire();
  }

  private fireLongPress() {
    this.longPress = null;
    const cfg = this.config();
    const th = thresholds(cfg);
    if (this.phase !== "pending" || this.tracks.size !== 2 || !cfg.longPressDock) return;
    for (const tr of this.tracks.values()) if (tr.travel > th.slop) return;
    if (this.pens.size) return;
    this.claimAll();
    this.phase = "done";
    const c = this.centre();
    this.host.summonDock(c.x, c.y);
    this.host.feedback({ kind: "fired", gesture: "longPressDock", x: c.x, y: c.y });
  }

  private clearLongPress() {
    if (this.longPress !== null) this.host.clearTimer(this.longPress);
    this.longPress = null;
  }
}

// ------------------------------------------------------------------ DOM

/**
 * Listen to the board's pointers in the capture phase on `root` (an ancestor
 * of the engine's input layer), so the recogniser decides before the
 * controller sees an event. `input` is the controller's input layer.
 * Returns a function that detaches the listeners.
 */
export function attachGestures(root: HTMLElement, input: HTMLElement, rec: GestureRecognizer): () => void {
  const onSurface = (t: EventTarget | null) => {
    if (t === input) return true;
    // "Use objects": the input layer lets the pointer through; the empty
    // board still takes gestures, Smart Objects and edited text never do.
    const el = t as Element | null;
    return !!el && input.classList.contains("interact") && root.contains(el) && !el.closest(".item.object, [contenteditable='true']");
  };
  const handler = (kind: "down" | "move" | "up" | "cancel") => (e: PointerEvent) => {
    if (synthetic) return;
    if (e.pointerType === "mouse") return;
    if (kind === "down" ? !onSurface(e.target) : !rec.tracking(e.pointerId)) return;
    const r = root.getBoundingClientRect();
    const v = rec.handle(kind, {
      id: e.pointerId, type: e.pointerType, x: e.clientX - r.left, y: e.clientY - r.top, t: e.timeStamp,
      w: e.width, h: e.height, board: e.target === input,
    });
    if (v === "consume") {
      e.stopImmediatePropagation();
      e.preventDefault();
    }
  };
  const map: [string, (e: PointerEvent) => void][] = [
    ["pointerdown", handler("down")],
    ["pointermove", handler("move")],
    ["pointerup", handler("up")],
    ["pointercancel", handler("cancel")],
  ];
  for (const [type, fn] of map) root.addEventListener(type, fn as EventListener, true);
  const menu = (e: Event) => rec.active && e.preventDefault();
  root.addEventListener("contextmenu", menu, true);
  return () => {
    for (const [type, fn] of map) root.removeEventListener(type, fn as EventListener, true);
    root.removeEventListener("contextmenu", menu, true);
  };
}

let synthetic = false;

/** Tell the controller (and so the engine) that a pointer it has was
 * cancelled: an unfinished stroke is dropped, a pan stops, its pointer
 * bookkeeping stays right. The recogniser ignores this synthetic event. */
export function cancelPointer(input: HTMLElement, id: number) {
  synthetic = true;
  try {
    input.dispatchEvent(new PointerEvent("pointercancel", { pointerId: id, pointerType: "touch", bubbles: true }));
  } finally {
    synthetic = false;
  }
}
