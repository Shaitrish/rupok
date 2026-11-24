// First-run quick tour: state and storage. The callouts are in Tour.tsx.
//
// It starts once, the first time the board opens in Prepare mode, and is
// remembered when finished or skipped (localStorage "rupok.tour"). Help → Quick
// tour and Settings → About restart it.

import { createSignal } from "solid-js";
import { studentEntry } from "../state";

const KEY = "rupok.tour";
const DONE = "1"; // bump to show a new tour to everyone once

/** The open step, or null when the tour is closed. */
export const [tourStep, setTourStep] = createSignal<number | null>(null);

export function tourDone(): boolean {
  try {
    return localStorage.getItem(KEY) === DONE;
  } catch {
    return true; // no storage: don't show it on every visit
  }
}

function markDone() {
  try {
    localStorage.setItem(KEY, DONE);
  } catch { /* fine */ }
}

/** Should the tour start by itself? Never for students, never twice. */
export function wantsAutoTour(): boolean {
  if (studentEntry || tourDone()) return false;
  // Automated browsers (e2e suites) don't get it unless they ask: /board/?tour.
  if (navigator.webdriver && !new URLSearchParams(location.search).has("tour")) return false;
  return true;
}

export function startTour() {
  setTourStep(0);
}

/** Finished or skipped: remembered, never starts by itself again. */
export function endTour() {
  setTourStep(null);
  markDone();
}

// For the console and e2e tests.
if (typeof window !== "undefined") (window as any).rupokTour = { start: startTour, end: endTour, step: tourStep };
