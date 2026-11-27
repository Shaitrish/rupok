// The Classic layout's slide panel: the pages drawer (Chrome.tsx →
// PagesPanel) slid in from a screen edge, and the edge swipe that brings it.
//
// The swipe's starting rule keeps it apart from ink and from the other
// gestures (gestures.ts):
//   - one pointer only (two or three fingers are the slide swipe and the taps);
//   - a finger or a pen, never the mouse (the mouse has the page button);
//   - it starts on the board itself, within EDGE px of the left or right
//     screen edge (a narrow strip: ink rarely starts there);
//   - it moves inward at least TRAVEL px, mostly sideways, within WINDOW ms.
// When it is a swipe, the few millimetres of ink it began are cancelled
// (one pointercancel, as a palm does) and the panel opens on that side.

import { createSignal } from "solid-js";
import { setPagesDrawer } from "../../viewport";
import { isClassic } from "../../layout";
import { edgeStart, edgeVerdict, type EdgeStart } from "./edgeRule";
export { EDGE, TRAVEL, WINDOW } from "./edgeRule";

/** Which edge the slide panel is on (null: the usual drawer). */
export const [pagesSide, setPagesSide] = createSignal<"left" | "right" | null>(null);

export function openPagesPanel(side: "left" | "right") {
  setPagesSide(side);
  setPagesDrawer(true);
}

/** Watch the board for edge swipes (App.tsx mounts it once). */
export function attachEdgeSwipe(root: HTMLElement): () => void {
  const down = new Set<number>();
  let track: (EdgeStart & { target: Element }) | null = null;
  const swallowed = new Set<number>();
  let ours = false;
  const onDown = (e: PointerEvent) => {
    const others = down.size;
    down.add(e.pointerId);
    track = null;
    if (!isClassic()) return;
    const t = e.target as Element | null;
    if (!t || !root.contains(t) || !t.closest(".input-layer")) return;
    const side = edgeStart(e.clientX, window.innerWidth, e.pointerType, others);
    if (side) track = { id: e.pointerId, x: e.clientX, y: e.clientY, t: e.timeStamp, side, target: t };
  };
  const onMove = (e: PointerEvent) => {
    if (swallowed.has(e.pointerId)) {
      e.stopImmediatePropagation();
      return;
    }
    if (!track || e.pointerId !== track.id) return;
    if (down.size > 1) { track = null; return; }
    const v = edgeVerdict(track, e.clientX, e.clientY, e.timeStamp);
    if (v === "no") track = null;
    else if (v === "swipe") {
      const s = track;
      track = null;
      swallowed.add(s.id);
      e.stopImmediatePropagation();
      // Drop the ink this press began: the board and the gesture recogniser see a cancel.
      ours = true;
      try {
        s.target.dispatchEvent(new PointerEvent("pointercancel", { pointerId: s.id, pointerType: e.pointerType, bubbles: true }));
      } finally {
        ours = false;
      }
      openPagesPanel(s.side);
    }
  };
  const onUp = (e: PointerEvent) => {
    if (ours) return; // the cancel we send below
    down.delete(e.pointerId);
    if (track?.id === e.pointerId) track = null;
    if (swallowed.delete(e.pointerId)) e.stopImmediatePropagation();
  };
  window.addEventListener("pointerdown", onDown, true);
  window.addEventListener("pointermove", onMove, true);
  window.addEventListener("pointerup", onUp, true);
  window.addEventListener("pointercancel", onUp, true);
  return () => {
    window.removeEventListener("pointerdown", onDown, true);
    window.removeEventListener("pointermove", onMove, true);
    window.removeEventListener("pointerup", onUp, true);
    window.removeEventListener("pointercancel", onUp, true);
  };
}
