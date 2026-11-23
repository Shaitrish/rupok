// Deep links into Settings: `openSettings("touch.palmEraser")` opens the
// dialog on that setting's category, scrolls to it and highlights it. The
// target can also be a category ("ai") or nothing (the last category).
// Popovers ("To text", gesture hints, the dock's "Customize dock…") use this
// so a teacher lands on the exact control that explains or fixes things.

import { createSignal } from "solid-js";

export interface SettingsRequest {
  target?: string;
  /** Bumped on every call, so opening the same target twice re-highlights. */
  seq: number;
}

const [request, setRequest] = createSignal<SettingsRequest | null>(null);
let seq = 0;

/** The open request, or null while Settings is closed. */
export const settingsRequest = request;
export const settingsOpen = () => request() !== null;

export function openSettings(target?: string) {
  setRequest({ target, seq: ++seq });
}

export function closeSettings() {
  setRequest(null);
}

// Handy for agents, e2e tests and the console: `rupokSettings.open("ai")`.
if (typeof window !== "undefined") {
  (window as any).rupokSettings = { open: openSettings, close: closeSettings };
}
