// Starred pages (the Classic slide panel): a bookmark on this device, so a
// teacher finds the pages of a lesson she comes back to. Not part of the
// lesson file (docs/agent-capabilities.md: teacher only).

import { createSignal } from "solid-js";
import { APP_SLUG } from "../../brand";

const KEY = `${APP_SLUG}.pageStars`;
const MAX = 500;

function load(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) || "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(-MAX) : [];
  } catch {
    return [];
  }
}

const [stars, setStars] = createSignal<string[]>(load());
export const isStarred = (pageId: string) => stars().includes(pageId);

export function toggleStar(pageId: string) {
  const next = isStarred(pageId) ? stars().filter((x) => x !== pageId) : [...stars(), pageId].slice(-MAX);
  setStars(next);
  try {
    localStorage.setItem(KEY, JSON.stringify(next));
  } catch { /* storage unavailable: kept for this session */ }
}
