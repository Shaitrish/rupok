// Trackpad pinch-to-zoom in WebKit on the Mac. Chrome and Firefox report a
// pinch as a Ctrl+wheel event (handled with the wheel in controller.ts);
// Safari and WKWebView (the macOS app) send non-standard gesture events
// instead. Without this, a pinch zooms the whole page in Safari (toolbars
// and all) and does nothing in the macOS app.
//
// Only on devices without touch: on iPad, gesture events come with the touch
// pointers the engine already turns into pinch-zoom.

interface GestureLike extends UIEvent {
  scale: number;
  clientX: number;
  clientY: number;
}

/** Zoom by `factor` around the point (x, y), in CSS px from the board's top left. */
export type ZoomAt = (factor: number, x: number, y: number) => void;

export function attachTrackpadPinch(root: HTMLElement, zoomAt: ZoomAt): void {
  if (!("GestureEvent" in window) || navigator.maxTouchPoints > 0) return;
  let last = 1;
  let lastCtrlWheel = -Infinity;
  // A WebKit that also sends Ctrl+wheel for the pinch is zoomed by the wheel handler.
  root.addEventListener("wheel", (e) => e.ctrlKey && (lastCtrlWheel = performance.now()), { passive: true });
  const opts = { passive: false } as AddEventListenerOptions;
  document.addEventListener("gesturestart", (e) => {
    e.preventDefault(); // no page zoom
    last = 1;
  }, opts);
  document.addEventListener("gesturechange", (e) => {
    e.preventDefault();
    const g = e as GestureLike;
    const factor = g.scale / last;
    last = g.scale;
    if (!root.contains(e.target as Node) || !Number.isFinite(factor) || factor <= 0) return;
    if (performance.now() - lastCtrlWheel < 250) return;
    const r = root.getBoundingClientRect();
    zoomAt(factor, g.clientX - r.left, g.clientY - r.top);
  }, opts);
  document.addEventListener("gestureend", (e) => e.preventDefault(), opts);
}
