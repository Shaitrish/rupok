// Classroom touch gestures on the board (the recogniser is gestures.ts):
// connects it to the controller and draws its feedback — the soft eraser
// circle under the hand, the edge arrow of a swipe with a short slide
// transition, and a one-time hint the first time each gesture fires.
// Mounted inside `.board`, next to the input layer.

import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { controller } from "../controller";
import { summonDock } from "../dock";
import { type FingerRole, type GestureHost, type GestureId, GestureRecognizer, attachGestures, cancelPointer } from "../gestures";
import { firstTime, gestures } from "../gesturePrefs";
import type { IconName } from "../icons.generated";
import { isView, items, mode, pageIndex, pages, prefs, showToast, tool } from "../state";
import { Icon } from "./Icon";
import "../gestures.css";

/** Hint text, icon, and the setting (settings/defs/touch.ts) that turns it off. */
const HINTS: Record<GestureId, { icon: IconName; text: (fingers: number) => string; setting: string }> = {
  palm: { icon: "eraser", text: () => "Palm eraser — rub to erase", setting: "touch.palmEraser" },
  swipe: { icon: "next", text: (n) => `${n}-finger swipe — next or previous slide`, setting: "touch.swipe" },
  flick: { icon: "next", text: () => "Flick — next or previous slide", setting: "touch.flick" },
  tapUndo: { icon: "undo", text: () => "Two-finger tap — undo", setting: "touch.tapUndo" },
  tapRedo: { icon: "redo", text: () => "Three-finger tap — redo", setting: "touch.tapRedo" },
  doubleTapFit: { icon: "fullscreen", text: () => "Two-finger double-tap — fit the slide", setting: "touch.doubleTapFit" },
  longPressDock: { icon: "hand", text: () => "Two-finger hold — the dock comes to you", setting: "touch.longPressDock" },
};

export default function GestureLayer(props: { onSettings?: (section?: string) => void }) {
  let anchor!: HTMLDivElement;
  const [palm, setPalm] = createSignal<{ x: number; y: number; r: number } | null>(null);
  const [swipe, setSwipe] = createSignal<{ dir: 1 | -1; progress: number; valid: boolean } | null>(null);
  const [landed, setLanded] = createSignal<{ dir: 1 | -1; label: string } | null>(null);
  const [hint, setHint] = createSignal<{ id: GestureId; text: string } | null>(null);
  let hintTimer: number | undefined;
  let landedTimer: number | undefined;

  // Large touch targets: an attribute on <html>, the CSS does the rest.
  createEffect(() => {
    document.documentElement.dataset.touchTargets = gestures().largeTargets ? "large" : "normal";
  });

  const showHint = (id: GestureId) => {
    if (!gestures().hints || !firstTime(id)) return;
    setHint({ id, text: HINTS[id].text(gestures().swipeFingers) });
    clearTimeout(hintTimer);
    hintTimer = window.setTimeout(() => setHint(null), 5200);
  };

  onMount(() => {
    const root = anchor.parentElement as HTMLElement; // .board
    const input = root.querySelector(".input-layer") as HTMLElement;

    /** After a swipe: what is on screen now, and a short slide transition. */
    const navigate = (dir: 1 | -1) => {
      const before = pageIndex();
      if (dir === 1) controller.next();
      else controller.prev();
      requestAnimationFrame(() => {
        const i = pageIndex(), n = pages().length;
        let label: string;
        if (i !== before) {
          label = `Slide ${i + 1} / ${n}`;
          root.classList.remove("g-slide-next", "g-slide-prev");
          void root.offsetWidth; // restart the animation
          root.classList.add(dir === 1 ? "g-slide-next" : "g-slide-prev");
          window.setTimeout(() => root.classList.remove("g-slide-next", "g-slide-prev"), 320);
        } else {
          const stepped = [...items].find((it) => it.kind === "object" && (it.steps ?? 0) > 0);
          label = stepped && !isView() ? `Step ${stepped.step ?? 0} / ${stepped.steps}` : dir === 1 ? "Last slide" : "First slide";
        }
        setLanded({ dir, label });
        clearTimeout(landedTimer);
        landedTimer = window.setTimeout(() => setLanded(null), 1100);
      });
    };

    const host: GestureHost = {
      oneFinger(penSeen): FingerRole {
        if (isView()) return "pan";
        const t = tool();
        if (t === "hand" || t === "interact") return "pan";
        if (t === "lasso") return "select";
        // The eraser tool: a finger rubs out, also once a pen has been seen.
        if (t === "text" || t === "eraser") return "draw";
        const tm = prefs().touchMode;
        return tm === "draw" ? "draw" : tm === "navigate" ? "pan" : penSeen ? "pan" : "draw";
      },
      mode: () => mode(),
      canEdit: () => !isView() && !controller.board.read_only() && controller.board.replay_state().length === 0,
      cancel: (id) => cancelPointer(input, id),
      camera: () => Array.from(controller.board.camera()),
      setCamera: (c) => controller.board.set_camera(c[0], c[1], c[2]),
      erase: (x0, y0, x1, y1, r) => controller.board.area_erase(x0, y0, x1, y1, r),
      eraseEnd: () => controller.board.area_erase_end(),
      next: () => navigate(1),
      prev: () => navigate(-1),
      undo: () => controller.board.undo(),
      redo: () => controller.board.redo(),
      fit: () => controller.board.fit_page(),
      summonDock: (x, y) => {
        const r = root.getBoundingClientRect();
        if (!summonDock(x + r.left, y + r.top)) showToast("The Smart Dock is not shown in this mode");
      },
      feedback: (f) => {
        switch (f.kind) {
          case "palm":
            setPalm({ x: f.x, y: f.y, r: f.r });
            root.dataset.gesture = "palm";
            break;
          case "palm-end":
            setPalm(null);
            delete root.dataset.gesture;
            break;
          case "swipe":
            setSwipe({ dir: f.dir, progress: f.progress, valid: f.valid });
            root.dataset.gesture = "swipe";
            break;
          case "swipe-end":
            setSwipe(null);
            delete root.dataset.gesture;
            break;
          case "fired":
            showHint(f.gesture);
            break;
        }
      },
      setTimer: (fn, ms) => window.setTimeout(fn, ms),
      clearTimer: (id) => clearTimeout(id as number),
    };
    const rec = new GestureRecognizer(host, gestures);
    const detach = attachGestures(root, input, rec);
    onCleanup(() => {
      detach();
      rec.reset();
      clearTimeout(hintTimer);
      clearTimeout(landedTimer);
    });
  });

  return (
    <div ref={anchor} class="g-anchor">
      <Show when={palm()}>
        {(p) => (
          <div
            class="g-palm"
            aria-hidden="true"
            style={{ left: `${p().x}px`, top: `${p().y}px`, width: `${p().r * 2}px`, height: `${p().r * 2}px` }}
          />
        )}
      </Show>
      <Portal>
        <Show when={swipe()}>
          {(s) => (
            <div
              class="g-edge"
              classList={{ next: s().dir === 1, prev: s().dir === -1, ready: s().valid }}
              style={{ "--p": String(s().progress) }}
              aria-hidden="true"
            >
              <Icon name={s().dir === 1 ? "next" : "prev"} size={28} />
            </div>
          )}
        </Show>
        <Show when={landed()}>
          {(l) => (
            <div class="g-landed" classList={{ next: l().dir === 1, prev: l().dir === -1 }} role="status">
              {l().label}
            </div>
          )}
        </Show>
        <Show when={hint()}>
          {(h) => (
            <div class="g-hint surface" role="status">
              <Icon name={HINTS[h().id].icon} size={18} />
              <span>{h().text}</span>
              <span class="g-hint-sep" aria-hidden="true">·</span>
              <button
                class="g-hint-link"
                onClick={() => {
                  setHint(null);
                  props.onSettings?.(HINTS[h().id].setting);
                }}
              >
                turn off in Settings
              </button>
            </div>
          )}
        </Show>
      </Portal>
    </div>
  );
}
