// The edge swipe's starting rule (edge.ts), pure so the unit tests run it
// without a browser (test/classic.test.ts).

/** Width of the edge strip a swipe starts in, CSS px (plus the safe area). */
export const EDGE = 20;
/** How far inward it must travel, CSS px. */
export const TRAVEL = 56;
/** Within this time from the touch, ms. */
export const WINDOW = 600;

export interface EdgeStart { id: number; x: number; y: number; t: number; side: "left" | "right" }

/** Does a pointer-down start an edge swipe? Pure, for the unit tests. */
export function edgeStart(x: number, width: number, type: string, others: number, safeLeft = 0, safeRight = 0): "left" | "right" | null {
  if (type === "mouse" || others > 0) return null;
  if (x <= EDGE + safeLeft) return "left";
  if (x >= width - EDGE - safeRight) return "right";
  return null;
}

/** Given the start and where the pointer is now: "swipe", "wait" or "no". */
export function edgeVerdict(s: EdgeStart, x: number, y: number, t: number): "swipe" | "wait" | "no" {
  const inward = s.side === "left" ? x - s.x : s.x - x;
  const side = Math.abs(y - s.y);
  if (t - s.t > WINDOW || inward < -8 || side > 40) return "no";
  if (inward >= TRAVEL && side < inward * 0.6) return "swipe";
  return "wait";
}
