// Full screen for presenting. Teach puts the window into real full screen —
// the native window in the desktop app (no title bar, no taskbar), the
// Fullscreen API in a browser — and leaving Teach restores it. Only a full
// screen Teach turned on is turned off again: a teacher who was already in
// full screen stays there.
//
// Esc: in a browser, Esc ends full screen before the page sees the key, so
// while presenting we ask for the Keyboard Lock (Chromium), which hands Esc
// to the board (clear the selection, then leave Teach); holding Esc still
// leaves full screen. If full screen ends any other way (browser UI, F11),
// Teach ends with it.

import { isDesktop } from "./platform";

type TauriWindow = {
  isFullscreen(): Promise<boolean>;
  setFullscreen(on: boolean): Promise<void>;
};

/** Full screen we turned on for Teach (and so turn off when Teach ends). */
let ours = false;
let onLeft: (() => void) | null = null;

async function tauriWindow(): Promise<TauriWindow | null> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  } catch {
    return null;
  }
}

type KeyboardLock = { lock?(keys: string[]): Promise<void>; unlock?(): void };
const keyboard = (): KeyboardLock | undefined => (navigator as Navigator & { keyboard?: KeyboardLock }).keyboard;

/** Whether the window is full screen now. */
export async function isFullscreen(): Promise<boolean> {
  if (isDesktop) {
    const w = await tauriWindow();
    return w ? w.isFullscreen().catch(() => false) : false;
  }
  return !!document.fullscreenElement;
}

/** Enter (Teach) or leave (any other mode) presenting full screen. */
export async function presentFullscreen(on: boolean): Promise<void> {
  if (on) {
    if (await isFullscreen()) return;
    ours = await setFullscreen(true);
    if (ours && !isDesktop) keyboard()?.lock?.(["Escape"]).catch(() => {});
  } else if (ours) {
    ours = false;
    if (!isDesktop) keyboard()?.unlock?.();
    await setFullscreen(false);
  }
}

/** The F key and the ⋯ menu: full screen on or off, by the teacher's hand. */
export async function toggleFullscreen(): Promise<void> {
  const on = !(await isFullscreen());
  ours = false;
  await setFullscreen(on);
}

async function setFullscreen(on: boolean): Promise<boolean> {
  if (isDesktop) {
    const w = await tauriWindow();
    if (!w) return false;
    try {
      await w.setFullscreen(on);
      return true;
    } catch (e) {
      console.warn("full screen is not available", e);
      return false;
    }
  }
  try {
    if (on) await document.documentElement.requestFullscreen({ navigationUI: "hide" });
    else if (document.fullscreenElement) await document.exitFullscreen();
    return true;
  } catch {
    // No user gesture (e.g. Teach restored at start-up) or not allowed.
    return false;
  }
}

/** Run `fn` when the full screen Teach turned on ends without Teach ending
 * (browser UI, holding Esc): the board then leaves Teach too. */
export function onPresentFullscreenLost(fn: () => void) {
  onLeft = fn;
}

if (typeof document !== "undefined") {
  document.addEventListener("fullscreenchange", () => {
    if (document.fullscreenElement || !ours) return;
    ours = false;
    keyboard()?.unlock?.();
    onLeft?.();
  });
}
