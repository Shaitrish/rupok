// Phones and tablets: the safe area (notch, home indicator), the on-screen
// keyboard, the chrome sizes the CSS sets per screen size, and the pages
// drawer used on phones.
//
// CSS owns the look (styles.css: the phone block sets --ctl, --bar and
// --tb); JS reads those numbers back so the board's insets and the dock's
// frame (dock.ts) match what is drawn.

import { createSignal } from "solid-js";
import { prefs, setPrefs } from "./state";

/** Phone layout: narrow portrait or short landscape (mirrors the CSS media query). */
export const PHONE_QUERY = "(max-width: 600px), (max-height: 500px)";
const phoneMq = typeof matchMedia !== "undefined" ? matchMedia(PHONE_QUERY) : null;
const [phone, setPhone] = createSignal(!!phoneMq?.matches);
phoneMq?.addEventListener("change", (e) => {
  setPhone(e.matches);
  setPagesDrawer(false);
});
/** True on a phone-sized screen (portrait or landscape). */
export const isPhone = phone;

// ------------------------------------------------------------ safe area

export interface Safe { top: number; right: number; bottom: number; left: number }
let probe: HTMLDivElement | null = null;
/** The safe-area insets in CSS px (0 on screens without a notch). */
export function safeArea(): Safe {
  if (typeof document === "undefined") return { top: 0, right: 0, bottom: 0, left: 0 };
  if (!probe) {
    probe = document.createElement("div");
    probe.setAttribute("aria-hidden", "true");
    probe.style.cssText =
      "position:fixed;left:0;top:0;width:0;height:0;visibility:hidden;pointer-events:none;" +
      "padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)";
    document.body.appendChild(probe);
  }
  const s = getComputedStyle(probe);
  return { top: parseFloat(s.paddingTop) || 0, right: parseFloat(s.paddingRight) || 0, bottom: parseFloat(s.paddingBottom) || 0, left: parseFloat(s.paddingLeft) || 0 };
}

/** A length custom property of <html> in px (e.g. "--bar"), else `fallback`. */
export function cssPx(name: string, fallback: number): number {
  if (typeof document === "undefined") return fallback;
  const v = parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name));
  return Number.isFinite(v) ? v : fallback;
}

// ------------------------------------------------------------- keyboard

/** Height of the on-screen keyboard (CSS px), from visualViewport; also `--kb` on <html>. */
export const [keyboard, setKeyboard] = createSignal(0);
function trackKeyboard() {
  const vv = window.visualViewport;
  if (!vv) return;
  const update = () => {
    // The layout viewport stays put while the keyboard covers its bottom
    // (iOS, and Chrome's default "resizes-visual"): the difference is the keyboard.
    const kb = Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop));
    const h = kb > 80 ? kb : 0; // ignore toolbars and zoom jitter
    if (h !== keyboard()) {
      setKeyboard(h);
      document.documentElement.style.setProperty("--kb", `${h}px`);
      document.documentElement.toggleAttribute("data-keyboard", h > 0);
    }
  };
  vv.addEventListener("resize", update);
  vv.addEventListener("scroll", update);
  update();
}
if (typeof window !== "undefined") trackKeyboard();

// --------------------------------------------------------- pages drawer

/** Phones: the pages panel is a drawer over the board, closed by default
 * (the Prepare preference `showPages` is for larger screens). */
const [pagesDrawer, setPagesDrawer] = createSignal(false);
export { pagesDrawer, setPagesDrawer };
/** Is the pages panel showing: the drawer on a phone, the preference elsewhere. */
export const pagesShown = () => (phone() ? pagesDrawer() : prefs().showPages);
export function togglePages() {
  if (phone()) setPagesDrawer(!pagesDrawer());
  else setPrefs({ showPages: !prefs().showPages });
}

// iOS Safari ignores user-scalable=no: stop its page pinch here. The board
// reads pointer events, which this does not touch, so its own pinch works.
if (typeof document !== "undefined") {
  for (const t of ["gesturestart", "gesturechange"]) document.addEventListener(t, (e) => e.preventDefault(), { passive: false });
}
