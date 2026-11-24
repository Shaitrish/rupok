import { For, Show, createMemo, createSignal } from "solid-js";
import { controller } from "../controller";
import {
  BOARD_COLORS, camera, pageIndex, pages, prefs, replay, showToast, isView, modeConfig,
} from "../state";
import { hasTopBar } from "../modes";
import LatencyOverlay from "./LatencyOverlay";
import { Icon } from "./Icon";
import { SelectionOverlay } from "./Selection";
import { MathPreview } from "./MathPreview";
import { ThumbCanvas } from "./PageThumb";
import { ExportHost } from "./ExportDialog";
import { FloatingAiStatus, Header } from "./Header";
import { livePhase } from "../live";
import { isPhone, setPagesDrawer } from "../viewport";

// The top bar itself lives in Header.tsx (with HeaderMenu.tsx for Insert ▾
// and ⋯, and AiStatus.tsx for the AI). Add top-bar actions there: an item on
// the bar, or an entry in the ⋯ menu (overflowEntries).
export function TopBar(props: { onObject: () => void; onSettings: () => void }) {
  const aiAround = () => livePhase() !== "idle";
  return (
    <>
      <Show when={hasTopBar(modeConfig())}>
        <Header onObject={props.onObject} onSettings={props.onSettings} />
      </Show>
      <Show when={!hasTopBar(modeConfig()) && !isView() && aiAround()}>
        <FloatingAiStatus />
      </Show>
      {/* Mounted here because the top bar component is always rendered; the
          overlay itself is a full-screen layer. */}
      <SelectionOverlay />
      <MathPreview />
      <ExportHost />
    </>
  );
}

const sameIds = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

export function PagesPanel() {
  const current = () => pages()[pageIndex()];
  const thumbBg = (c: number) => "#" + (c >>> 8).toString(16).padStart(6, "0");
  // Keyed by page id, so thumbnails survive edits, reorders and inserts.
  const ids = createMemo(() => pages().map((p) => p.id), [], { equals: sameIds });
  let list!: HTMLDivElement;

  // ------------------------------------------------- drag to reorder
  // Mouse: press and move. Pen and touch: press and hold, then move (a
  // plain swipe keeps scrolling the list).
  const [drag, setDrag] = createSignal<{ from: number; gap: number; dy: number } | null>(null);
  let suppressClick = false;

  const gapAt = (clientY: number) => {
    const y = clientY - list.getBoundingClientRect().top + list.scrollTop;
    const thumbs = list.querySelectorAll<HTMLElement>(".page-thumb");
    for (let k = 0; k < thumbs.length; k++) {
      if (y < thumbs[k].offsetTop + thumbs[k].offsetHeight / 2) return k;
    }
    return thumbs.length;
  };

  const startDrag = (e: PointerEvent, from: number) => {
    if (isView() || e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    const el = e.currentTarget as HTMLElement;
    const pid = e.pointerId;
    const [x0, y0, scroll0] = [e.clientX, e.clientY, list.scrollTop];
    const mouse = e.pointerType === "mouse";
    let active = false;
    const begin = () => {
      active = true;
      setDrag({ from, gap: from, dy: 0 });
      try { el.setPointerCapture(pid); } catch { /* pointer already gone */ }
    };
    const timer = mouse ? 0 : window.setTimeout(begin, 320);
    const move = (ev: PointerEvent) => {
      if (ev.pointerId !== pid) return;
      if (!active) {
        const d = Math.hypot(ev.clientX - x0, ev.clientY - y0);
        if (mouse && d > 5) begin();
        else if (!mouse && d > 8) return end(false);
        if (!active) return;
      }
      ev.preventDefault();
      const r = list.getBoundingClientRect();
      if (ev.clientY < r.top + 32) list.scrollTop -= 12;
      else if (ev.clientY > r.bottom - 32) list.scrollTop += 12;
      setDrag({ from, gap: gapAt(ev.clientY), dy: ev.clientY - y0 + list.scrollTop - scroll0 });
    };
    const end = (commit: boolean) => {
      clearTimeout(timer);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      const d = drag();
      setDrag(null);
      if (!active) return;
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
      if (commit && d) {
        const to = d.gap > d.from ? d.gap - 1 : d.gap;
        if (to !== d.from) controller.board.move_page(d.from, to);
      }
    };
    const up = (ev: PointerEvent) => ev.pointerId === pid && end(true);
    const cancel = (ev: PointerEvent) => ev.pointerId === pid && end(false);
    window.addEventListener("pointermove", move, { passive: false });
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  };

  /** Where the dragged page would land: a line in the gap between thumbnails. */
  const dropLine = createMemo(() => {
    const d = drag();
    if (!d || d.gap === d.from || d.gap === d.from + 1) return null;
    const thumbs = list.querySelectorAll<HTMLElement>(".page-thumb");
    if (!thumbs.length) return null;
    if (d.gap < thumbs.length) return thumbs[d.gap].offsetTop - 5;
    const last = thumbs[thumbs.length - 1];
    return last.offsetTop + last.offsetHeight + 3;
  });

  // The list element is re-created when a mode hides and shows the panel.
  const attachList = (el: HTMLDivElement) => {
    list = el;
    // Once a touch drag has begun, stop the list from scrolling under it.
    el.addEventListener("touchmove", (e) => drag() && e.preventDefault(), { passive: false });
    el.addEventListener("contextmenu", (e) => e.preventDefault());
  };

  return (
    <Show when={modeConfig().chrome.pages}>
    {/* Phones: a drawer over the board; tapping outside closes it. */}
    <Show when={isPhone()}>
      <div class="pages-scrim" onClick={() => setPagesDrawer(false)} />
    </Show>
    <aside class="pages surface" classList={{ viewing: isView(), top: !hasTopBar(modeConfig()) }} aria-label="Pages">
      <div class="pages-head">
        <span>Pages · {pages().length}</span>
        <Show when={!isView()}>
          <button class="btn icon" title="Add page after this one" aria-label="Add page" onClick={() => controller.board.add_page(current()?.pattern ?? "plain")}>
            <Icon name="plus" size={20} />
          </button>
        </Show>
      </div>
      <div class="pages-list" classList={{ sorting: !!drag() }} ref={attachList}>
        <For each={ids()}>
          {(id, i) => {
            const p = () => pages()[i()];
            const lifted = () => drag()?.from === i();
            return (
              <div
                class="page-thumb"
                classList={{ on: i() === pageIndex(), lifted: lifted() }}
                data-page={id}
                style={{
                  background: thumbBg(p()?.bg_color ?? 0xffffffff),
                  transform: lifted() ? `translateY(${drag()!.dy}px)` : undefined,
                }}
                onClick={() => {
                  if (suppressClick) return;
                  controller.board.goto_page(i());
                  if (isPhone()) setPagesDrawer(false);
                }}
                onPointerDown={(e) => startDrag(e, i())}
                title={p()?.title || `Page ${i() + 1}`}
              >
                <ThumbCanvas index={i()} current={i() === pageIndex()} itemCount={p()?.item_count ?? 0} />
                <span class="num">{i() + 1}</span>
                <Show when={!isView()}>
                  <div class="thumb-actions">
                    <button
                      class="btn icon"
                      title="Duplicate page"
                      aria-label="Duplicate page"
                      onClick={(e) => {
                        e.stopPropagation();
                        controller.board.duplicate_page(i());
                      }}
                    >
                      <Icon name="duplicate" size={16} />
                    </button>
                    <Show when={pages().length > 1}>
                      <button
                        class="btn icon thumb-del"
                        title="Delete page (click twice)"
                        aria-label="Delete page"
                        onClick={(e) => {
                          e.stopPropagation();
                          controller.board.goto_page(i());
                          if (confirmTwice()) controller.board.delete_page();
                        }}
                      >
                        <Icon name="trash" size={16} />
                      </button>
                    </Show>
                  </div>
                </Show>
              </div>
            );
          }}
        </For>
        <Show when={dropLine() !== null}>
          <div class="drop-line" style={{ top: `${dropLine()}px` }} />
        </Show>
      </div>
      <Show when={!isView()}>
        <div class="page-style" role="group" aria-label="Page pattern">
          <For each={["plain", "grid", "dots", "lines"] as const}>
            {(pat) => (
              <button
                classList={{ on: current()?.pattern === pat }}
                title={pat[0].toUpperCase() + pat.slice(1)}
                onClick={() => controller.board.set_background(pat, current()?.bg_color ?? 0xffffffff)}
              >
                <span class={`pat-ico ${pat}`} />
              </button>
            )}
          </For>
        </div>
        <div class="page-style" role="group" aria-label="Board colour">
          <For each={BOARD_COLORS}>
            {(b) => (
              <button
                classList={{ on: (current()?.bg_color ?? 0xffffffff) === b.rgba }}
                title={b.label}
                onClick={() => controller.setBoardColor(b.rgba)}
              >
                <span class="bg-dot" style={{ background: thumbBg(b.rgba) }} />
              </button>
            )}
          </For>
        </div>
      </Show>
    </aside>
    </Show>
  );
}

let lastDelete = 0;
function confirmTwice(): boolean {
  const now = Date.now();
  if (now - lastDelete < 2000) return true;
  lastDelete = now;
  showToast("Click delete again to remove this page");
  return false;
}

export function ReplayBar() {
  const [speed, setSpeed] = createSignal(2);
  const fmt = (ms: number) => {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  return (
    <Show when={replay()}>
      {(r) => (
        <div class="replaybar surface" role="group" aria-label="Replay">
          <button class="btn icon soft" onClick={() => controller.board.replay_play(!r().playing, speed())} aria-label={r().playing ? "Pause" : "Play"}>
            <Icon name={r().playing ? "pause" : "play"} active />
          </button>
          <input
            class="range"
            type="range"
            min="0"
            max={r().d}
            step="10"
            value={r().t}
            style={{ "--p": `${r().d ? (r().t / r().d) * 100 : 0}%` }}
            onInput={(e) => controller.board.replay_seek(Number(e.currentTarget.value))}
            aria-label="Replay position"
          />
          <span class="time">{fmt(r().t)} / {fmt(r().d)}</span>
          <div class="seg">
            <For each={[1, 2, 4, 8]}>
              {(s) => (
                <button classList={{ on: speed() === s }} onClick={() => { setSpeed(s); controller.board.replay_play(r().playing, s); }}>
                  {s}×
                </button>
              )}
            </For>
          </div>
          <button class="btn icon" title="Close replay  ·  Esc" aria-label="Close replay" onClick={() => controller.board.replay_stop()}>
            <Icon name="close" />
          </button>
        </div>
      )}
    </Show>
  );
}

export function StatusChips() {
  const c = () => modeConfig().chrome;
  return (
    <div class="statusbar">
      <Show when={c().zoom}>
      <div class="zoom surface">
        <button class="btn icon" onClick={() => controller.zoomBy(0.8)} title="Zoom out  ·  −" aria-label="Zoom out"><Icon name="zoomOut" size={20} /></button>
        <button class="btn pct" onClick={() => controller.board.fit_page()} title="Fit page  ·  0">{controller.zoomPct(camera()[2])}%</button>
        <button class="btn icon" onClick={() => controller.zoomBy(1.25)} title="Zoom in  ·  +" aria-label="Zoom in"><Icon name="zoomIn" size={20} /></button>
      </div>
      </Show>
      {/* Performance and pen latency overlay (Settings → About → Troubleshooting). */}
      <Show when={c().stats && prefs().showStats}>
        <LatencyOverlay />
      </Show>
    </div>
  );
}

export function downloadQuiz() {
  const rows = controller.objects.quizLog.map((q) => ({ time: new Date(q.at).toISOString(), object: q.title, ...(q.payload as object) }));
  const blob = new Blob([JSON.stringify(rows, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "quiz-answers.json";
  a.click();
}
