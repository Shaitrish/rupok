// Solve this UI: the teacher's preview card ("Check before showing the
// class"), the writing chip (Next line / Show all), the selection toolbar's
// Solve button and the "Teach Rupok my handwriting" calibration screen.

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { renderMarkdown, onMathReady } from "../markdown";
import { isView, mode } from "../state";
import { Icon } from "../components/Icon";
import {
  CALIBRATION_CHARS, SAMPLES_PER_CHAR, type PendingSolution, type Sample, calibrateOpen, glyphs, pending, reveal,
  saveGlyphs, setCalibrateOpen, solvePrefs, solving, taughtCount,
} from "./state";
import { approve, askChange, discard, revealAll, revealNext, solve } from "./index";
import { MicButton } from "../components/MicButton";
import "./solve.css";

// --------------------------------------------------------- preview card

/** The synthesised handwriting, drawn as it will be written (a private
 * preview: nothing is on the board yet). */
function HandPreview(props: { lines: string[]; teacher: boolean }) {
  const data = createMemo(() => {
    try {
      return JSON.parse(controller.board.handwriting_preview_json(JSON.stringify(props.lines), 16, props.teacher, 1));
    } catch {
      return null;
    }
  });
  const [t, setT] = createSignal(0);
  let raf = 0;
  onMount(() => {
    const d = data();
    if (!d) return;
    const total = d.duration_ms as number;
    const t0 = performance.now();
    // Play at up to 3× so the teacher is not kept waiting.
    const k = Math.max(1, total / 6000, 3);
    const step = () => {
      const now = (performance.now() - t0) * k;
      setT(now);
      if (now < total) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
  });
  onCleanup(() => cancelAnimationFrame(raf));
  const paths = createMemo(() => {
    const d = data();
    if (!d) return [];
    const now = t();
    const out: string[] = [];
    for (const s of d.strokes as number[][]) {
      const start = s[0];
      if (start > now) break;
      const pts: string[] = [];
      for (let i = 1; i + 2 < s.length; i += 3) {
        if (start + s[i + 2] > now) break;
        pts.push(`${s[i].toFixed(1)},${s[i + 1].toFixed(1)}`);
      }
      if (pts.length) out.push(pts.join(" "));
    }
    return out;
  });
  return (
    <Show when={data()}>
      {(d) => (
        <svg class="solve-hand" viewBox={`-6 -6 ${d().width + 12} ${d().height + 12}`} style={{ "aspect-ratio": `${d().width + 12} / ${d().height + 12}` }} role="img" aria-label="Handwriting preview">
          <For each={paths()}>{(p) => <polyline points={p} />}</For>
        </svg>
      )}
    </Show>
  );
}

function PreviewCard(props: { p: PendingSolution }) {
  const [tick, setTick] = createSignal(0);
  onCleanup(onMathReady(() => setTick((n) => n + 1)));
  const html = () => (tick(), renderMarkdown(props.p.lines.map((l) => l || " ").join("\n\n")));
  const teacher = () => (props.p.op.style ?? solvePrefs.read().style) !== "neat";
  const [note, setNote] = createSignal("");
  let field: HTMLInputElement | undefined;
  return (
    <section class="solve-card surface" role="dialog" aria-label="Check the solution before showing the class">
      <header class="solve-card-head">
        <span class="solve-badge"><Icon name="smartink" size={16} /></span>
        <div>
          <div class="solve-title">Check before showing the class</div>
          <div class="solve-sub">{props.p.agent}'s solution · {props.p.lines.length} {props.p.lines.length === 1 ? "line" : "lines"} · only you see this</div>
        </div>
        <button class="btn icon" aria-label="Discard" title="Discard" onClick={() => discard(props.p.id)}><Icon name="close" size={18} /></button>
      </header>
      <div class="solve-lines chat-md" innerHTML={html()} />
      <HandPreview lines={props.p.lines} teacher={teacher()} />
      <div class="solve-actions">
        <button class="btn solid solve-write" onClick={() => approve(props.p.id)}>
          <Icon name="pen" size={18} /><span class="label">Write it on the board</span>
        </button>
        <button class="btn" onClick={() => discard(props.p.id)}>Discard</button>
      </div>
      <form class="solve-change" onSubmit={(e) => { e.preventDefault(); askChange(props.p.id, note()); }}>
        <input
          ref={field}
          class="st-input solve-change-input"
          placeholder="Ask for a change (e.g. use the balance method)"
          value={note()}
          onInput={(e) => setNote(e.currentTarget.value)}
          onKeyDown={(e) => e.stopPropagation()}
          aria-label="Ask the assistant for a different solution"
        />
        <MicButton target={() => field} size={36} />
        <button class="btn soft" type="submit" disabled={!note().trim()}>Ask again</button>
      </form>
      <Show when={props.p.op.step_mode === "one-line-per-click"}>
        <div class="solve-hint">Step by step: press Next (clicker, → or swipe) for each line.</div>
      </Show>
    </section>
  );
}

// ------------------------------------------------------------ reveal chip

function RevealChip() {
  const s = () => reveal();
  return (
    <Show when={s()}>
      {(st) => (
        <div class="solve-chip surface" role="status" aria-live="polite">
          <span class="solve-dot" classList={{ waiting: st()[2] === 1 }} />
          <Show
            when={st()[2] === 1}
            fallback={<span class="solve-chip-label">Writing… {st()[0]}/{st()[1]}</span>}
          >
            <button class="btn soft solve-next" onClick={() => revealNext()}>
              <span class="label">Next line</span><Icon name="next" size={16} />
            </button>
            <span class="solve-chip-label">{st()[3]}/{st()[4]}</span>
          </Show>
          <button class="btn solve-all" onClick={() => revealAll()}>Show all</button>
        </div>
      )}
    </Show>
  );
}

// ------------------------------------------------------- toolbar button

/** "Solve this" in the selection toolbar. */
export function SolveButton() {
  const show = () => !isView() || solvePrefs.read().students;
  return (
    <Show when={show()}>
      <button class="btn totext solve-btn" title="Ask your AI to solve this, written in handwriting" disabled={solving()} onClick={() => void solve()}>
        <Icon name="smartink" size={18} />
        <span>{solving() ? "Asking…" : "Solve this"}</span>
      </button>
    </Show>
  );
}

/** "Solve what I select" next to the Teach chip. */
export function SolveChipAction() {
  return (
    <Show when={mode() === "teach"}>
      <button class="solve-peek" onClick={() => void solve()} title="Lasso a problem and the AI solves it">
        <Icon name="smartink" size={16} />
        <span>Solve what I select</span>
      </button>
    </Show>
  );
}

// --------------------------------------------------------- calibration

/** Cell geometry (CSS px): guide lines for the writing frame. */
const CELL_W = 96, CELL_H = 120, CAP = 34, XLINE = 55, BASE = 85;
const K = 14 / (BASE - XLINE); // writing units per px

function toUnits(strokes: number[][]): Sample {
  return strokes.map((s) => {
    const out: number[] = [];
    for (let i = 0; i + 1 < s.length; i += 2) out.push(Math.round(s[i] * K * 100) / 100, Math.round((s[i + 1] - BASE) * K * 100) / 100);
    return out;
  });
}
function toPx(sample: Sample): number[][] {
  return sample.map((s) => s.map((v, i) => (i % 2 ? v / K + BASE : v / K + CELL_W / 2 - 10)));
}

function Cell(props: { ch: string; strokes: number[][]; taught?: Sample; onChange: (s: number[][]) => void }) {
  let cv!: HTMLCanvasElement;
  let cur: number[] | null = null;
  const draw = () => {
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = CELL_W * dpr; cv.height = CELL_H * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, CELL_W, CELL_H);
    const css = getComputedStyle(cv);
    const guide = css.getPropertyValue("--solve-guide") || "#ccc";
    const ink = css.getPropertyValue("--solve-ink") || "#1c2024";
    const ghost = css.getPropertyValue("--solve-ghost") || "#bbb";
    ctx.lineWidth = 1;
    ctx.strokeStyle = guide;
    ctx.setLineDash([3, 4]);
    for (const y of [CAP, XLINE]) { ctx.beginPath(); ctx.moveTo(6, y); ctx.lineTo(CELL_W - 6, y); ctx.stroke(); }
    ctx.setLineDash([]);
    ctx.beginPath(); ctx.moveTo(6, BASE); ctx.lineTo(CELL_W - 6, BASE); ctx.stroke();
    ctx.lineCap = "round"; ctx.lineJoin = "round";
    const line = (s: number[]) => { ctx.beginPath(); ctx.moveTo(s[0], s[1]); for (let i = 2; i + 1 < s.length; i += 2) ctx.lineTo(s[i], s[i + 1]); if (s.length === 2) ctx.lineTo(s[0] + 0.1, s[1]); ctx.stroke(); };
    if (!props.strokes.length && props.taught) {
      ctx.strokeStyle = ghost; ctx.lineWidth = 2.5;
      for (const s of toPx(props.taught)) line(s);
    }
    ctx.strokeStyle = ink; ctx.lineWidth = 3;
    for (const s of props.strokes) line(s);
    if (cur) line(cur);
  };
  onMount(draw);
  const pos = (e: PointerEvent) => { const r = cv.getBoundingClientRect(); return [((e.clientX - r.left) * CELL_W) / r.width, ((e.clientY - r.top) * CELL_H) / r.height]; };
  return (
    <div class="solve-cell" classList={{ done: props.strokes.length > 0, taught: !!props.taught }}>
      <span class="solve-cell-ch">{props.ch}</span>
      <canvas
        ref={cv}
        style={{ width: `${CELL_W}px`, height: `${CELL_H}px` }}
        aria-label={`Write ${props.ch}`}
        onPointerDown={(e) => { e.preventDefault(); cv.setPointerCapture(e.pointerId); cur = pos(e); draw(); }}
        onPointerMove={(e) => { if (!cur) return; const [x, y] = pos(e); cur.push(x, y); draw(); }}
        onPointerUp={() => { if (cur && cur.length >= 2) props.onChange([...props.strokes, cur]); cur = null; queueMicrotask(draw); }}
        onPointerCancel={() => { cur = null; draw(); }}
      />
      <button class="solve-cell-clear" aria-label={`Clear ${props.ch}`} onClick={() => { props.onChange([]); queueMicrotask(draw); }}>Clear</button>
    </div>
  );
}

export function Calibrate() {
  const [drawn, setDrawn] = createSignal<Record<string, number[][]>>({});
  const close = () => { setDrawn({}); setCalibrateOpen(false); };
  const save = () => {
    const g = { ...glyphs() };
    for (const [ch, strokes] of Object.entries(drawn())) {
      if (!strokes.length) continue;
      g[ch] = [...(g[ch] ?? []), toUnits(strokes)].slice(-SAMPLES_PER_CHAR);
    }
    saveGlyphs(g);
    close();
  };
  const count = () => Object.values(drawn()).filter((s) => s.length).length;
  return (
    <Show when={calibrateOpen()}>
      <div class="solve-scrim" onClick={(e) => e.target === e.currentTarget && close()}>
        <section class="solve-cal surface" role="dialog" aria-modal="true" aria-label="Teach Rupok my handwriting">
          <header class="solve-cal-head">
            <div>
              <div class="solve-title">Teach Rupok my handwriting</div>
              <div class="solve-sub">Write each character once, between the lines, the way you write on the board. Rupok uses it to write solutions in your hand and to read your maths better. Kept on this device.</div>
            </div>
            <button class="btn icon" aria-label="Close" onClick={close}><Icon name="close" size={18} /></button>
          </header>
          <div class="solve-cal-body">
            <For each={CALIBRATION_CHARS}>
              {(grp) => (
                <>
                  <div class="solve-cal-group">{grp.group}</div>
                  <div class="solve-cal-grid">
                    <For each={grp.chars}>
                      {(ch) => (
                        <Cell
                          ch={ch}
                          strokes={drawn()[ch] ?? []}
                          taught={glyphs()[ch]?.[glyphs()[ch].length - 1]}
                          onChange={(s) => setDrawn((d) => ({ ...d, [ch]: s }))}
                        />
                      )}
                    </For>
                  </div>
                </>
              )}
            </For>
          </div>
          <footer class="solve-cal-foot">
            <span class="solve-sub">{count()} written now · {taughtCount()} taught before</span>
            <Show when={taughtCount() > 0}>
              <button class="btn" onClick={() => { if (confirm("Forget your handwriting on this device?")) { saveGlyphs({}); close(); } }}>Forget my handwriting</button>
            </Show>
            <button class="btn solid" disabled={count() === 0} onClick={save}><span class="label">Save</span></button>
          </footer>
        </section>
      </div>
    </Show>
  );
}

/** Everything Solve this floats over the board. */
export function SolveLayer() {
  return (
    <>
      <div class="solve-cards">
        <For each={pending()}>{(p) => <PreviewCard p={p} />}</For>
      </div>
      <RevealChip />
      <Calibrate />
    </>
  );
}
