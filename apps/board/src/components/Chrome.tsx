import { For, Show, createSignal } from "solid-js";
import { controller } from "../controller";
import {
  BOARD_COLORS, camera, canRedo, canUndo, pageIndex, pages, prefs, replay, revision, savedRevision, setPrefs, showToast, stats,
  title,
} from "../state";
import { Icon, Mark } from "./Icon";
import { FILE_EXT, WORDMARK } from "../brand";

function pick(accept: string, multiple = false): Promise<File[]> {
  return new Promise((resolve) => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = accept;
    i.multiple = multiple;
    i.onchange = () => resolve(Array.from(i.files ?? []));
    i.click();
  });
}

export function TopBar(props: { onObject: () => void; onSettings: () => void }) {
  const dark = () => document.documentElement.dataset.theme === "dark";
  return (
    <header class="topbar">
      <div class="cluster surface">
        <button class="btn icon" title="Pages panel" aria-label="Toggle pages panel" onClick={() => setPrefs({ showPages: !prefs().showPages })}>
          <Icon name="sidebar" />
        </button>
        <div class="brand">
          <Mark size={28} />
          <span class="wordmark"><b>{WORDMARK.accent}</b>{WORDMARK.rest}</span>
        </div>
        <div class="vsep" />
        <input
          class="title-input"
          value={title()}
          size={Math.min(28, Math.max(10, title().length + 1))}
          onChange={(e) => controller.board.set_title(e.currentTarget.value.trim() || "Untitled lesson")}
          onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          aria-label="Lesson title"
        />
        <Show when={revision() !== savedRevision()}>
          <span class="dirty-dot" title={`Changes are autosaved in this browser. Save to keep a .${FILE_EXT} file.`} />
        </Show>
      </div>
      <div class="grow" />
      <div class="cluster surface">
        <button class="btn icon" disabled={!canUndo()} onClick={() => controller.board.undo()} title="Undo  ·  Ctrl Z" aria-label="Undo"><Icon name="undo" /></button>
        <button class="btn icon" disabled={!canRedo()} onClick={() => controller.board.redo()} title="Redo  ·  Ctrl Shift Z" aria-label="Redo"><Icon name="redo" /></button>
        <div class="vsep" />
        <button class="btn" onClick={async () => controller.importFiles(await pick(".pdf,application/pdf"))} title="Import a PDF — every page becomes a slide you can write on">
          <Icon name="pdf" /><span class="label">PDF</span>
        </button>
        <button class="btn" onClick={async () => { const [f] = await pick("image/*"); if (f) controller.addImageFile(f); }} title="Insert an image">
          <Icon name="image" /><span class="label">Image</span>
        </button>
        <button class="btn" onClick={props.onObject} title="Add a Smart Object — live HTML made by your AI">
          <Icon name="object" /><span class="label">Smart Object</span>
        </button>
        <button class="btn" onClick={() => controller.board.replay_start()} title="Replay this page stroke by stroke">
          <Icon name="replay" /><span class="label">Replay</span>
        </button>
      </div>
      <div class="cluster surface">
        <button class="btn icon" onClick={() => controller.openFile()} title="Open  ·  Ctrl O" aria-label="Open"><Icon name="open" /></button>
        <button class="btn icon" onClick={() => setPrefs({ theme: dark() ? "light" : "dark" })} title="Light / dark" aria-label="Toggle theme">
          <Icon name={dark() ? "sun" : "moon"} />
        </button>
        <button class="btn icon" onClick={() => controller.toggleFullscreen()} title="Fullscreen  ·  F" aria-label="Fullscreen"><Icon name="fullscreen" /></button>
        <button class="btn icon" onClick={props.onSettings} title="Settings" aria-label="Settings"><Icon name="settings" /></button>
        <div class="vsep" />
        <button class="btn solid" onClick={() => controller.saveFile()} title={`Save .${FILE_EXT}  ·  Ctrl S`}>
          <Icon name="save" /><span class="label">Save</span>
        </button>
      </div>
    </header>
  );
}

export function PagesPanel() {
  const current = () => pages()[pageIndex()];
  const thumbBg = (c: number) => "#" + (c >>> 8).toString(16).padStart(6, "0");
  return (
    <aside class="pages surface" aria-label="Pages">
      <div class="pages-head">
        <span>Pages · {pages().length}</span>
        <button class="btn icon" title="Add page after this one" aria-label="Add page" onClick={() => controller.board.add_page(current()?.pattern ?? "plain")}>
          <Icon name="plus" size={20} />
        </button>
      </div>
      <div class="pages-list">
        <For each={pages()}>
          {(p, i) => (
            <div
              class="page-thumb"
              classList={{ on: i() === pageIndex() }}
              style={{ background: thumbBg(p.bg_color) }}
              onClick={() => controller.board.goto_page(i())}
              title={p.title || `Page ${i() + 1}`}
            >
              <span class="num">{i() + 1}</span>
              <span class="meta" style={{ color: (p.bg_color >>> 8) < 0x808080 ? "rgba(255,255,255,0.55)" : undefined }}>
                {p.item_count ? `${p.item_count}` : ""}
              </span>
              <Show when={pages().length > 1}>
                <button
                  class="btn icon del"
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
          )}
        </For>
      </div>
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
    </aside>
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
  return (
    <div class="statusbar">
      <div class="zoom surface">
        <button class="btn icon" onClick={() => controller.zoomBy(0.8)} title="Zoom out  ·  −" aria-label="Zoom out"><Icon name="zoomOut" size={20} /></button>
        <button class="btn pct" onClick={() => controller.board.fit_page()} title="Fit page  ·  0">{Math.round(camera()[2] * 100)}%</button>
        <button class="btn icon" onClick={() => controller.zoomBy(1.25)} title="Zoom in  ·  +" aria-label="Zoom in"><Icon name="zoomIn" size={20} /></button>
      </div>
      <Show when={prefs().showStats && stats()}>
        {(s) => (
          <div class="stats surface" title="Renderer statistics">
            {s().backend.split(" · ")[0].replace("BrowserWebGpu", "WebGPU").replace("Gl", "WebGL2")} · {s().strokes} strokes · {s().fps} fps
          </div>
        )}
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
