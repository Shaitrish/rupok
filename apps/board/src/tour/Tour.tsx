// First-run quick tour: five small callouts anchored to the real controls.
// Not a wizard: no scrim, the board stays usable, and the callout follows its
// control when the dock or the bar moves. Anchors are data-tour attributes
// (plus the mode switcher's data-mode), with fallbacks for collapsed layouts.
// Hidden in Teach and Student view and while a dialog or the Library is open.

import { For, Show, createEffect, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { mode } from "../state";
import { libraryOpen } from "../library/session";
import { endTour, setTourStep, startTour, tourStep, wantsAutoTour } from "./tour";
import "./tour.css";

interface Step {
  /** Selectors tried in order; the first visible one is the anchor. */
  at: string[];
  /** Also outlined with the anchor when visible (the dock's colours). */
  with?: string;
  title: string;
  body: string;
  /** Said instead when the control has collapsed into the ⋯ menu (small screens). */
  inMore?: string;
}

const STEPS: Step[] = [
  {
    at: ['[data-tour="pen"]', ".dock-bubble"],
    with: '[data-tour="color"]',
    title: "Pen and colours",
    body: "Write with the pen on the Smart Dock. Tap the pen again for sizes and all colours; your recent colours sit right next to it.",
  },
  {
    at: ['[data-tour="pages-add"]', '[aria-label="Add page"]', '[aria-label="Toggle pages panel"]', '[aria-label="Next page"]'],
    title: "Pages",
    body: "Each slide is a page. Press + to add a page after this one, and drag the thumbnails to change the order.",
  },
  {
    at: ['[data-tour="hdr-insert"]', '[data-tour="hdr-more"]'],
    title: "Insert",
    body: "Bring in images, PDF slides, Smart Objects (live simulations, 3D models, quizzes) and videos.",
    inMore: "Insert is in the ⋯ menu: images, PDF slides, Smart Objects (live simulations, 3D models, quizzes) and videos.",
  },
  {
    at: ['[data-mode="teach"]', '[data-tour="hdr-more"]'],
    title: "Teach mode",
    body: "Press F5 or Teach to present: the slide fills the screen and only the essential tools stay. Esc brings you back.",
    inMore: "Teach is in the ⋯ menu (or press F5): the slide fills the screen and only the essential tools stay. Esc brings you back.",
  },
  {
    at: ['[data-tour="library"]', '[data-tour="hdr-more"]'],
    title: "Save and Library",
    body: "Your work is kept as you go. The Library holds all your lessons, and Ctrl S saves a .rpk file to share with students.",
  },
];

const GAP = 14; // between the control and the callout
const MARGIN = 16; // from the screen edge

type Box = { left: number; top: number; right: number; bottom: number; more: boolean };
const MORE = '[data-tour="hdr-more"]';

function visible(el: Element | null): el is HTMLElement {
  if (!(el instanceof HTMLElement)) return false;
  const r = el.getBoundingClientRect();
  if (r.width < 4 || r.height < 4) return false;
  if (r.right < 0 || r.bottom < 0 || r.left > innerWidth || r.top > innerHeight) return false;
  const cs = getComputedStyle(el);
  return cs.visibility !== "hidden" && cs.display !== "none";
}

function anchorBox(step: Step): Box | null {
  let el: HTMLElement | null = null;
  let at = "";
  for (const sel of step.at) {
    el = Array.from(document.querySelectorAll(sel)).find(visible) as HTMLElement | undefined ?? null;
    at = sel;
    if (el) break;
  }
  if (!el) return null;
  const r = el.getBoundingClientRect();
  const box = { left: r.left, top: r.top, right: r.right, bottom: r.bottom, more: at === MORE };
  if (step.with && el.matches(step.at[0])) {
    // The colours only when they sit next to the anchor (same dock row or column).
    for (const c of Array.from(document.querySelectorAll(step.with)).filter(visible)) {
      const q = c.getBoundingClientRect();
      if (Math.abs(q.left - box.right) > 120 && Math.abs(q.top - box.bottom) > 120) continue;
      box.left = Math.min(box.left, q.left);
      box.top = Math.min(box.top, q.top);
      box.right = Math.max(box.right, q.right);
      box.bottom = Math.max(box.bottom, q.bottom);
    }
  }
  return box;
}

/** Something modal is up (Settings, a dialog, the Library home): wait. */
const blocked = () =>
  mode() !== "prepare" || libraryOpen() || !!document.querySelector('.scrim, [aria-modal="true"]');

export function Tour() {
  const [box, setBox] = createSignal<Box | null>(null);
  const [paused, setPaused] = createSignal(false);
  const [size, setSize] = createSignal({ w: 320, h: 180 });
  let card: HTMLDivElement | undefined;

  const step = createMemo(() => {
    const i = tourStep();
    return i === null ? null : STEPS[i] ?? null;
  });

  const measure = () => {
    const s = step();
    if (!s) return;
    setPaused(blocked());
    setBox(anchorBox(s));
    if (card) setSize({ w: card.offsetWidth, h: card.offsetHeight });
  };

  onMount(() => {
    // First run: wait for the board to settle and for any welcome dialog.
    if (wantsAutoTour()) {
      let tries = 0;
      const t = setInterval(() => {
        if (++tries > 120 || tourStep() !== null) return clearInterval(t);
        if (!blocked() && anchorBox(STEPS[0])) {
          clearInterval(t);
          startTour();
        }
      }, 1000);
      onCleanup(() => clearInterval(t));
    }
    // The dock and the bar move (drag, collapse, resize): follow them.
    const tick = setInterval(() => tourStep() !== null && measure(), 400);
    addEventListener("resize", measure);
    onCleanup(() => {
      clearInterval(tick);
      removeEventListener("resize", measure);
    });
  });
  createEffect(() => {
    void tourStep();
    queueMicrotask(measure);
  });

  // Where the callout goes: the side of the control with the most room,
  // preferring below for the top bar and above for the dock.
  const place = createMemo(() => {
    const b = box();
    const { w, h } = size();
    const vw = innerWidth;
    const vh = innerHeight;
    if (!b) return { left: (vw - w) / 2, top: vh - h - 96, side: "none" as const, arrow: 0 };
    const cx = (b.left + b.right) / 2;
    const cy = (b.top + b.bottom) / 2;
    const room = { below: vh - b.bottom, above: b.top, right: vw - b.right, left: b.left };
    const order: (keyof typeof room)[] =
      b.bottom - b.top > 2 * (b.right - b.left) && (b.left < vw * 0.25 || b.right > vw * 0.75)
        ? b.left < vw / 2 ? ["right", "left", "below", "above"] : ["left", "right", "below", "above"]
        : cy < vh / 2 ? ["below", "above", "right", "left"] : ["above", "below", "right", "left"];
    const need = { below: h + GAP + MARGIN, above: h + GAP + MARGIN, right: w + GAP + MARGIN, left: w + GAP + MARGIN };
    const side = order.find((s) => room[s] >= need[s]) ?? order.reduce((a, s) => (room[s] > room[a] ? s : a));
    const clampX = (x: number) => Math.max(MARGIN, Math.min(vw - w - MARGIN, x));
    const clampY = (y: number) => Math.max(MARGIN, Math.min(vh - h - MARGIN, y));
    let left: number;
    let top: number;
    if (side === "below" || side === "above") {
      left = clampX(cx - w / 2);
      top = side === "below" ? b.bottom + GAP : b.top - GAP - h;
      top = clampY(top);
      return { left, top, side, arrow: Math.max(20, Math.min(w - 20, cx - left)) };
    }
    top = clampY(cy - h / 2);
    left = side === "right" ? b.right + GAP : b.left - GAP - w;
    left = clampX(left);
    return { left, top, side, arrow: Math.max(20, Math.min(h - 20, cy - top)) };
  });

  const next = () => {
    const i = tourStep() ?? 0;
    if (i >= STEPS.length - 1) endTour();
    else setTourStep(i + 1);
  };
  const back = () => setTourStep(Math.max(0, (tourStep() ?? 0) - 1));

  return (
    <Show when={step() && !paused() && step()}>
      {(s) => (
        <>
          <Show when={box()}>
            {(b) => (
              <div
                class="tour-ring"
                aria-hidden="true"
                style={{ left: `${b().left - 4}px`, top: `${b().top - 4}px`, width: `${b().right - b().left + 8}px`, height: `${b().bottom - b().top + 8}px` }}
              />
            )}
          </Show>
          <div
            ref={card}
            class="tour-card"
            classList={{ [`side-${place().side}`]: true }}
            role="dialog"
            aria-modal="false"
            aria-labelledby="tour-title"
            aria-describedby="tour-body"
            data-tour-step={tourStep() ?? 0}
            style={{ left: `${place().left}px`, top: `${place().top}px`, "--arrow": `${place().arrow}px` }}
            onKeyDown={(e) => e.key === "Escape" && (e.stopPropagation(), endTour())}
          >
            <span class="tour-arrow" aria-hidden="true" />
            <div class="tour-head">
              <span class="tour-count">{(tourStep() ?? 0) + 1} / {STEPS.length}</span>
              <div class="tour-dots" aria-hidden="true">
                <For each={STEPS}>{(_, i) => <span classList={{ on: i() === tourStep() }} />}</For>
              </div>
            </div>
            <h2 id="tour-title" class="tour-title">{s().title}</h2>
            <p id="tour-body" class="tour-body">{box()?.more && s().inMore ? s().inMore : s().body}</p>
            <div class="tour-actions">
              <button class="tour-btn ghost" onClick={endTour}>Skip tour</button>
              <span class="tour-grow" />
              <Show when={(tourStep() ?? 0) > 0}>
                <button class="tour-btn" onClick={back}>Back</button>
              </Show>
              <button class="tour-btn solid" onClick={next}>{(tourStep() ?? 0) >= STEPS.length - 1 ? "Done" : "Next"}</button>
            </div>
          </div>
        </>
      )}
    </Show>
  );
}
