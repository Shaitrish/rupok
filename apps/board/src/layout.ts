// Layouts: "Modern" (the Smart Dock and the top bar) and "Classic" (three
// big islands along the bottom edge, no top bar, the ☰ menu; familiar to
// teachers coming from classroom whiteboard software). Classic is not a
// separate mode of the app: it is a dock style (modes.ts → DockConfig
// `style: "islands"`) that the "Classic" preset sets, so the teacher and
// their AI rearrange it with the same tools as any dock (Customize dock,
// set_dock_layout). The switch (Settings → Appearance → Layout, the
// first-run choice, set_setting `appearance.layout`) is DockEditor.tsx →
// setLayout, which keeps the teacher's Modern dock to come back to.

import { CLASSIC_VIEW_DOCK, type DockConfig, type Mode } from "./modes";
import { mode, modeConfig } from "./state";

export type Layout = "modern" | "classic";

/** The islands of a mode, or null when it uses the Smart Dock. The Student
 * view follows Prepare (its own dock is fixed). */
export function islandsOf(m: Mode = mode()): DockConfig | null {
  if (m === "view") return modeConfig("prepare").dock.style === "islands" ? CLASSIC_VIEW_DOCK : null;
  const d = modeConfig(m).dock;
  return d.style === "islands" ? d : null;
}

/** The mode on screen uses the Classic layout. */
export const isClassic = (m: Mode = mode()): boolean => islandsOf(m) !== null;

/** The layout the board uses (Prepare decides; Teach may differ if arranged so). */
export const layout = (): Layout => (modeConfig("prepare").dock.style === "islands" ? "classic" : "modern");

// The ☰ menu's "Toolbar at top / bottom" calls the islands' flip (Dock.tsx).
let flip: (() => void) | null = null;
export const registerIslandFlip = (f: (() => void) | null) => (flip = f);
export const flipIslandsNow = () => flip?.();
let fold: ((hide: boolean) => void) | null = null;
export const registerIslandFold = (f: ((hide: boolean) => void) | null) => (fold = f);
/** ☰ → Hide toolbar / Show toolbar: every island folds to a pill (or back). */
export const foldIslandsNow = (hide: boolean) => fold?.(hide);
