// Settings → Gestures, stored in this browser under `rupok.gestures` (its own
// key, apart from `rupok.prefs`). The format is `GestureConfig` in gestures.ts
// and the `gestures` object of a UI config file (docs/ui-config.md).

import { createSignal } from "solid-js";
import { APP_SLUG } from "./brand";
import { DEFAULT_GESTURES, type GestureConfig, type GestureId, resolveGestures, sanitizeGestures } from "./gestures";

const KEY = `${APP_SLUG}.gestures`;
const SEEN_KEY = `${APP_SLUG}.gestures.seen`;

function load(): GestureConfig {
  try {
    return resolveGestures(JSON.parse(localStorage.getItem(KEY) || "{}"));
  } catch {
    return { ...DEFAULT_GESTURES };
  }
}

const [gestures, setGesturesSignal] = createSignal<GestureConfig>(load());
export { gestures };

export function setGestures(patch: Partial<GestureConfig>) {
  const next = { ...gestures(), ...patch };
  setGesturesSignal(next);
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch { /* storage unavailable */ }
}

export function resetGestures() {
  setGestures({ ...DEFAULT_GESTURES });
}

/** The `gestures` object of an exported UI config. The calibration stays
 * out: it belongs to this board, and the config may go to another one. */
export function exportGestures(): Omit<GestureConfig, "calibration"> {
  const { calibration: _, ...rest } = gestures();
  return rest;
}

/** Apply the `gestures` object of an imported UI config; returns whether
 * anything valid was in it. Unknown keys and bad values are ignored. */
export function importGestures(raw: unknown): boolean {
  const patch = sanitizeGestures(raw);
  if (!Object.keys(patch).length) return false;
  setGestures(patch);
  return true;
}

// ------------------------------------------------------------ first-time hints

function loadSeen(): Set<string> {
  try {
    const v = JSON.parse(localStorage.getItem(SEEN_KEY) || "[]");
    return new Set(Array.isArray(v) ? v.filter((x) => typeof x === "string") : []);
  } catch {
    return new Set();
  }
}
const seen = loadSeen();

/** True the first time a gesture fires (then remembered). */
export function firstTime(id: GestureId): boolean {
  if (seen.has(id)) return false;
  seen.add(id);
  try {
    localStorage.setItem(SEEN_KEY, JSON.stringify([...seen]));
  } catch { /* storage unavailable */ }
  return true;
}

/** Settings → "Show the hints again". */
export function resetHints() {
  seen.clear();
  try {
    localStorage.removeItem(SEEN_KEY);
  } catch { /* storage unavailable */ }
}
