// Library preferences, per browser profile (localStorage `rupok.library`).
// Kept apart from the board's prefs (state.ts) so the two can evolve alone.

import { createSignal } from "solid-js";
import { APP_SLUG } from "../brand";
import type { Role } from "./types";

export type Section = "recent" | "mine" | "copies" | "shared" | "trash" | "cloud";
export type SortKey = "modified" | "name";

export interface LibraryPrefs {
  /** Teacher or student; null until the first-run choice. */
  role: Role | null;
  /** The first-run choice was made: `/board/` now opens the Library. */
  setupDone: boolean;
  /** Written into lessons you make or copy (manifest `library.author`). */
  author: string;
  view: "grid" | "list";
  sort: SortKey;
  /** Browser only: the private browser storage or a folder on this computer. */
  location: "browser" | "folder";
  /** Where new lessons are saved when signed in to Rupok Cloud. */
  saveTo: "local" | "cloud";
  /** When each lesson was last opened: `<store id>|<path>` → ms. */
  opened: Record<string, number>;
}

const KEY = `${APP_SLUG}.library`;
const defaults: LibraryPrefs = { role: null, setupDone: false, author: "", view: "grid", sort: "modified", location: "browser", saveTo: "local", opened: {} };

function load(): LibraryPrefs {
  try {
    const v = { ...defaults, ...JSON.parse(localStorage.getItem(KEY) || "{}") };
    if (v.role !== "teacher" && v.role !== "student") v.role = null;
    if (v.view !== "list") v.view = "grid";
    if (v.sort !== "name") v.sort = "modified";
    if (v.location !== "folder") v.location = "browser";
    if (v.saveTo !== "cloud") v.saveTo = "local";
    if (!v.opened || typeof v.opened !== "object") v.opened = {};
    v.author = typeof v.author === "string" ? v.author.slice(0, 120) : "";
    return v;
  } catch {
    return { ...defaults };
  }
}

export const [libPrefs, setLibPrefsSignal] = createSignal<LibraryPrefs>(load());

export function setLibPrefs(p: Partial<LibraryPrefs>) {
  const next = { ...libPrefs(), ...p };
  setLibPrefsSignal(next);
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch { /* storage unavailable */ }
}

/** Remember that a lesson was opened (for Recent). Keeps the newest 200. */
export function markOpened(key: string) {
  const opened = { ...libPrefs().opened, [key]: Date.now() };
  const keys = Object.keys(opened);
  if (keys.length > 200) {
    keys.sort((a, b) => opened[a] - opened[b]).slice(0, keys.length - 200).forEach((k) => delete opened[k]);
  }
  setLibPrefs({ opened });
}

/** Move Recent entries along with a renamed or moved item. */
export function moveOpened(storeId: string, from: string, to: string) {
  const opened = { ...libPrefs().opened };
  let changed = false;
  for (const k of Object.keys(opened)) {
    const p = k.slice(storeId.length + 1);
    if (k.startsWith(storeId + "|") && (p === from || p.startsWith(from + "/"))) {
      opened[`${storeId}|${to}${p.slice(from.length)}`] = opened[k];
      delete opened[k];
      changed = true;
    }
  }
  if (changed) setLibPrefs({ opened });
}

/** The section the Library opens on, by role. */
export const homeSection = (): Section => (libPrefs().role === "student" ? "shared" : "recent");
