// Settings → Gestures: classroom touch gestures (gestures.ts) one by one,
// their sensitivity, large touch targets, and "Calibrate my board", which
// measures what the board reports for a finger and a palm and picks the
// palm-eraser thresholds. Every gesture names its button or shortcut.

import { type JSX, Show, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { type GestureConfig, calibrate } from "../gestures";
import { gestures, resetGestures, resetHints, setGestures } from "../gesturePrefs";
import { prefs, setPrefs, showToast } from "../state";
import { Seg } from "./Dialogs";
import { Icon } from "./Icon";

type BoolKey = { [K in keyof GestureConfig]: GestureConfig[K] extends boolean ? K : never }[keyof GestureConfig];

function Toggle(props: { on: boolean; label: string; onChange: (v: boolean) => void }) {
  return (
    <button class="g-toggle" role="switch" aria-checked={props.on} aria-label={props.label} onClick={() => props.onChange(!props.on)} />
  );
}

function Row(props: { k: BoolKey; title: string; children: JSX.Element; onChange?: (v: boolean) => void }) {
  const set = (v: boolean) => (props.onChange ? props.onChange(v) : setGestures({ [props.k]: v } as Partial<GestureConfig>));
  return (
    <div class="g-row">
      <div class="g-row-text">
        <span class="g-row-title">{props.title}</span>
        <span class="g-row-sub">{props.children}</span>
      </div>
      <Toggle on={gestures()[props.k]} label={props.title} onChange={set} />
    </div>
  );
}

const sensitivityLabel = (s: number) => (s < 0.34 ? "strict" : s > 0.66 ? "eager" : "normal");

export function GestureSettings() {
  const g = gestures;
  const [calibrating, setCalibrating] = createSignal(false);
  const cal = () => g().calibration;
  return (
    <>
      <div class="section-title" id="settings-gestures">Gestures</div>
      <div class="g-detect" role="status">
        <Icon name="hand" size={20} />
        <div class="grow">
          <Show
            when={cal()}
            fallback={<>Not calibrated yet. A touch much wider than a finger, or 3 or more touches close together, erases. Calibrate to fit your board.</>}
          >
            {(c) => (
              <Show
                when={c().reportsSize}
                fallback={<><b>Your board does not report touch size, using hand-cluster detection.</b> {c().contacts >= 3 ? `Your palm showed as ${c().contacts} touches.` : "Rub quickly back and forth to erase with one touch."}</>}
              >
                <b>Contact size detection.</b> A finger is about {c().finger} px and your palm about {c().palm} px; touches wider than {g().palmSize} px erase.
              </Show>
            )}
          </Show>
        </div>
        <button class="btn soft" onClick={() => setCalibrating(true)}>
          <Icon name="settings" /><span>Calibrate my board</span>
        </button>
      </div>

      <div class="g-list">
        <Row k="palmErase" title="Palm eraser">
          Rub with your palm, the side of your hand or a thumb, like a board duster. Ink only; objects and images stay. One rub is one undo step. Same as the Eraser tool <kbd>E</kbd>.
        </Row>
        <Show when={g().palmErase}>
          <div class="g-sub">
            <div class="field">
              <label for="g-palm">Palm size · {Math.round(g().palmSize)} px</label>
              <input id="g-palm" class="slider" type="range" min="16" max="160" step="2" value={g().palmSize} onInput={(e) => setGestures({ palmSize: Number(e.currentTarget.value) })} />
              <span class="hint">A touch at least this wide erases (on boards that report touch size).</span>
            </div>
            <div class="field">
              <label for="g-scale">Eraser size · {g().eraserScale.toFixed(1)}×</label>
              <input id="g-scale" class="slider" type="range" min="0.5" max="2" step="0.1" value={g().eraserScale} onInput={(e) => setGestures({ eraserScale: Number(e.currentTarget.value) })} />
              <span class="hint">The eraser follows the size of your hand on the board.</span>
            </div>
            <label class="check">
              <input type="checkbox" checked={g().handCluster} onChange={(e) => setGestures({ handCluster: e.currentTarget.checked })} />
              <span>Hand detection: 3 or more touches close together</span>
            </label>
            <label class="check">
              <input type="checkbox" checked={g().thumbScrub} onChange={(e) => setGestures({ thumbScrub: e.currentTarget.checked })} />
              <span>Thumb scrub: rub fast back and forth (when fingers don't draw)</span>
            </label>
          </div>
        </Show>

        <Row k="swipe" title="Swipe to change slides">
          Flick {g().swipeFingers} fingers left for the next slide or Smart Object step, right for the previous. Same as <kbd>→</kbd> <kbd>←</kbd>, Page Down / Up or the pager.
        </Row>
        <Row k="flick" title="One-finger flick in Teach">
          In Teach and the student view, when fingers don't draw (a pen board, or the Pan or Use objects tool). Never inside a Smart Object you are using.
        </Row>
        <Show when={g().swipe || g().flick}>
          <div class="g-sub">
            <div class="field">
              <label>Swipe fingers</label>
              <Seg value={String(g().swipeFingers) as "2" | "3"} options={[["2", "2 fingers"], ["3", "3 fingers"]] as const} onChange={(v) => setGestures({ swipeFingers: v === "3" ? 3 : 2 })} />
            </div>
            <div class="field">
              <label for="g-sens">Swipe sensitivity · {sensitivityLabel(g().swipeSensitivity)}</label>
              <input id="g-sens" class="slider" type="range" min="0" max="1" step="0.05" value={g().swipeSensitivity} onInput={(e) => setGestures({ swipeSensitivity: Number(e.currentTarget.value) })} />
              <span class="hint">Strict needs a longer, faster swipe, so a quick pan never changes the slide.</span>
            </div>
          </div>
        </Show>

        <Row k="tapUndo" title="Two-finger tap: undo">
          Same as Undo or <kbd>Ctrl Z</kbd>.
        </Row>
        <Row k="tapRedo" title="Three-finger tap: redo">
          Same as Redo or <kbd>Ctrl ⇧ Z</kbd>.
        </Row>
        <Row k="doubleTapFit" title="Two-finger double-tap: fit the slide">
          Same as <kbd>0</kbd>. A two-finger tap then waits a moment before it undoes.
        </Row>
        <Row k="longPressDock" title="Two-finger hold: bring the Smart Dock">
          Hold two fingers still for a moment and the dock comes next to them. You can also drag the dock by its grip.
        </Row>
        <Row
          k="largeTargets"
          title="Large touch targets"
          onChange={(v) => {
            setGestures({ largeTargets: v });
            if (v && prefs().density !== "large") setPrefs({ density: "large" });
          }}
        >
          Bigger buttons, switches and handles for big boards. Turning it on also sets Size to Large board.
        </Row>
        <Row k="hints" title="First-time hints">
          A short hint the first time each gesture works.{" "}
          <button class="g-hint-link" style={{ height: "auto", padding: "0 2px", "font-size": "12px" }} onClick={() => { resetHints(); showToast("Gesture hints will show again"); }}>
            Show them again
          </button>
        </Row>
      </div>
      <div class="actions">
        <button class="btn" onClick={() => { resetGestures(); showToast("Gestures reset to the defaults"); }}>
          <Icon name="restart" /><span class="label">Reset gestures</span>
        </button>
      </div>
      <Show when={calibrating()}>
        <Calibration onClose={() => setCalibrating(false)} />
      </Show>
    </>
  );
}

// ------------------------------------------------------------------ calibration

const FINGER_MS = 900;
const PALM_MS = 1200;

/** "Calibrate my board": one finger, then a palm; the board's reported
 * contact sizes (PointerEvent width/height) pick the thresholds. */
export function Calibration(props: { onClose: () => void }) {
  const [step, setStep] = createSignal<"finger" | "palm" | "result">("finger");
  const [progress, setProgress] = createSignal(0);
  const [note, setNote] = createSignal("");
  const [reading, setReading] = createSignal("");
  const [result, setResult] = createSignal<string | null>(null);
  const touches = new Map<number, { x: number; y: number }>();
  let samples: number[] = [];
  let since = 0;
  let finger = 0;
  let palm = { size: 0, contacts: 0, spread: 0 };
  let raf = 0;

  const sizeOf = (e: PointerEvent) => Math.max(e.width || 0, e.height || 0);
  const median = (a: number[]) => (a.length ? [...a].sort((x, y) => x - y)[a.length >> 1] : 0);

  const finish = () => {
    const r = calibrate({ size: finger }, palm);
    setGestures(r.patch);
    setResult(r.message);
    setStep("result");
    setProgress(1);
  };

  const tick = () => {
    raf = 0;
    if (!touches.size || step() === "result") return;
    const held = performance.now() - since;
    if (step() === "finger") {
      setProgress(Math.min(1, held / FINGER_MS));
      if (held >= FINGER_MS && touches.size === 1) {
        finger = median(samples);
        samples = [];
        touches.clear();
        setStep("palm");
        setProgress(0);
        setNote("");
        setReading("");
        return;
      }
    } else {
      setProgress(Math.min(1, held / PALM_MS));
      if (held >= PALM_MS) {
        finish();
        return;
      }
    }
    raf = requestAnimationFrame(tick);
  };

  const record = (e: PointerEvent) => {
    const s = sizeOf(e);
    if (step() === "finger") {
      samples.push(s);
      setReading(s > 3 ? `Your finger: ${Math.round(s)} px wide` : "Your board reports no touch size");
      return;
    }
    palm.size = Math.max(palm.size, s);
    palm.contacts = Math.max(palm.contacts, touches.size);
    let cx = 0, cy = 0;
    for (const t of touches.values()) { cx += t.x; cy += t.y; }
    cx /= touches.size || 1;
    cy /= touches.size || 1;
    for (const t of touches.values()) palm.spread = Math.max(palm.spread, Math.hypot(t.x - cx, t.y - cy));
    setReading(`${palm.contacts} touch${palm.contacts === 1 ? "" : "es"} · widest ${palm.size > 3 ? `${Math.round(palm.size)} px` : "not reported"}`);
  };

  const down = (e: PointerEvent) => {
    if ((e.target as Element).closest("button")) return;
    e.preventDefault();
    if (step() === "result") return;
    if (e.pointerType !== "touch") {
      setNote("Use your hand on the board: a pen or mouse can't measure touch size.");
      return;
    }
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    if (step() === "finger" && touches.size >= 1) {
      setNote("Just one finger for now.");
      touches.clear();
      samples = [];
      setProgress(0);
      return;
    }
    if (!touches.size) since = performance.now();
    touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    setNote("");
    record(e);
    if (!raf) raf = requestAnimationFrame(tick);
  };
  const move = (e: PointerEvent) => {
    if (!touches.has(e.pointerId)) return;
    touches.set(e.pointerId, { x: e.clientX, y: e.clientY });
    record(e);
  };
  const up = (e: PointerEvent) => {
    if (!touches.delete(e.pointerId)) return;
    if (touches.size) return;
    const held = performance.now() - since;
    if (step() === "palm" && held >= 300) finish();
    else if (step() !== "result") {
      setProgress(0);
      if (step() === "finger") {
        samples = [];
        setNote("Keep the finger down until the ring is full.");
      }
    }
  };
  const key = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      props.onClose();
    }
  };
  onMount(() => window.addEventListener("keydown", key, true));
  onCleanup(() => {
    window.removeEventListener("keydown", key, true);
    if (raf) cancelAnimationFrame(raf);
  });

  const R = 76;
  const C = 2 * Math.PI * R;
  return (
    <Portal>
      <div
        class="g-cal"
        role="dialog"
        aria-modal="true"
        aria-label="Calibrate my board"
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
        onContextMenu={(e) => e.preventDefault()}
      >
        <div class="g-cal-card">
          <Show
            when={step() !== "result"}
            fallback={
              <>
                <div class="g-cal-step">Done</div>
                <h2>Your board is calibrated</h2>
                <div class="g-cal-result" role="status">{result()}</div>
                <div class="g-cal-actions">
                  <button class="btn" onClick={() => { finger = 0; palm = { size: 0, contacts: 0, spread: 0 }; samples = []; setResult(null); setProgress(0); setReading(""); setStep("finger"); }}>
                    <Icon name="restart" /><span>Try again</span>
                  </button>
                  <button class="btn solid" onClick={props.onClose}>Done</button>
                </div>
              </>
            }
          >
            <div class="g-cal-step">Step {step() === "finger" ? 1 : 2} of 2</div>
            <h2>{step() === "finger" ? "Touch the board with one finger" : "Now lay your palm flat on the board"}</h2>
            <p>
              {step() === "finger"
                ? "Anywhere on the screen. Hold it still until the ring is full."
                : "Anywhere, the way you would wipe the board. Hold it until the ring is full."}
            </p>
            <div class="g-cal-target" aria-hidden="true">
              <svg class="ring" viewBox="0 0 168 168">
                <circle class="ring-track" cx="84" cy="84" r={R} />
                <circle class="ring-fill" cx="84" cy="84" r={R} stroke-dasharray={String(C)} stroke-dashoffset={String(C * (1 - progress()))} />
              </svg>
              <Icon name={step() === "finger" ? "interact" : "hand"} size={56} />
            </div>
            <div class="g-cal-read" aria-live="polite">{note() || reading()}</div>
            <div class="g-cal-actions">
              <button class="btn" onClick={props.onClose}>Cancel</button>
              <Show when={step() === "palm"}>
                <button class="btn soft" onClick={finish}>My palm is down — finish</button>
              </Show>
            </div>
          </Show>
        </div>
      </div>
    </Portal>
  );
}
