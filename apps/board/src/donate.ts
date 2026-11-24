// "Support Rupok": the donate link (Settings → About, ⋯ → Support Rupok) and
// the small card the desktop app shows on first launch and once more about
// two weeks later (components/DonateCard.tsx).
//
// The card's rules:
// - desktop app only (in a browser, `/board/?donate-card` turns it on for
//   testing, with short delays);
// - at most MAX_SHOWS times ever; "Don't show again" or "Donate" ends it;
// - only in Prepare mode, never in Teach or Student view, never during a
//   replay, in fullscreen or while the teacher is writing; it hides at once
//   if Teach starts;
// - a corner card, never a modal, never over the middle of the board.
// Nothing is sent anywhere: the state lives in localStorage on this device.

import { createSignal } from "solid-js";
import { SITE_URL } from "./brand";
import { isFullscreen } from "./fullscreen";
import { desktopInvoke, isDesktop } from "./platform";
import { mode, replay, studentEntry } from "./state";

export const DONATE_URL = `${SITE_URL}/donate/`;

const KEY = "rupok.donate";
const DAY = 24 * 60 * 60 * 1000;
/** Shows ever: the first launch, then once after SECOND_AFTER_DAYS. */
export const MAX_SHOWS = 2;
export const SECOND_AFTER_DAYS = 14;

export interface DonateState {
  /** First launch (ms since the epoch). */
  first: number;
  /** How many times the card has been on screen. */
  shown: number;
  /** When it was last on screen. */
  last: number;
  /** "Don't show again", or the teacher already chose Donate. */
  never: boolean;
}

export function loadDonateState(): DonateState | null {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    return v && typeof v.first === "number" ? { first: v.first, shown: v.shown | 0, last: Number(v.last) || 0, never: !!v.never } : null;
  } catch {
    return null;
  }
}

function saveDonateState(s: DonateState) {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch { /* storage unavailable: the card simply may show again */ }
}

/** Is the card due? `null` state is a first launch. Pure, for tests. */
export function donateCardDue(s: DonateState | null, now: number): boolean {
  if (!s) return true;
  if (s.never || s.shown >= MAX_SHOWS) return false;
  if (s.shown === 0) return true; // the first launch closed before it showed
  return now - s.first >= SECOND_AFTER_DAYS * DAY && now - s.last >= 7 * DAY;
}

/** Open rupok.app/donate/ in the system browser (desktop) or a new tab. */
export async function openDonatePage() {
  if (isDesktop) {
    try {
      await desktopInvoke("open_donate_page"); // the URL is fixed on the Rust side
      return;
    } catch { /* an older shell without the command: try the webview */ }
  }
  window.open(DONATE_URL, "_blank", "noopener");
}

// ------------------------------------------------------------------- card

const [cardShown, setCardShown] = createSignal(false);
/** The card is on screen: shown this session, and the board is in a calm state. */
export const donateCardOpen = () => cardShown() && calm();
const calm = () => mode() === "prepare" && !replay() && !studentEntry;

const forced = typeof location !== "undefined" && new URLSearchParams(location.search).has("donate-card");
let lastPointer = 0;
let started = false;

/** Start the schedule once the board is up (App.tsx). */
export function startDonateCard() {
  if (started || (!isDesktop && !forced) || studentEntry) return;
  started = true;
  const now = Date.now();
  let s = loadDonateState();
  if (!donateCardDue(s, now)) return;
  if (!s) {
    s = { first: now, shown: 0, last: 0, never: false };
    saveDonateState(s);
  }
  const state = s;
  // First launch: let the teacher look around first. Tests: almost at once.
  const delay = forced ? 800 : 25_000;
  const idle = forced ? 600 : 8_000;
  addEventListener("pointerdown", () => (lastPointer = Date.now()), { capture: true, passive: true });
  const tryShow = () => {
    if (!calm() || Date.now() - lastPointer < idle || document.fullscreenElement) return void setTimeout(tryShow, forced ? 300 : 5_000);
    void isFullscreen().catch(() => false).then((full) => {
      if (full) return void setTimeout(tryShow, 5_000);
      if (!calm()) return void setTimeout(tryShow, forced ? 300 : 5_000);
      state.shown += 1;
      state.last = Date.now();
      saveDonateState(state);
      setCardShown(true);
    });
  };
  setTimeout(tryShow, delay);
}

function finish(never: boolean) {
  setCardShown(false);
  if (!never) return;
  const s = loadDonateState() ?? { first: Date.now(), shown: 1, last: Date.now(), never: true };
  s.never = true;
  saveDonateState(s);
}

export const donateNow = () => {
  finish(true);
  void openDonatePage();
};
export const donateNotNow = () => finish(false);
export const donateNever = () => finish(true);
