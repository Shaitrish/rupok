// The top bar (rendered by TopBar in Chrome.tsx).
//
//   [Library | Pages · Mode switcher | Title · Saved]   [Undo Redo | Insert ▾ · Replay] [AI · Settings · ⋯]
//
// Only what a teacher uses during a lesson is on the bar; files, export,
// theme, fullscreen and help live in the ⋯ menu. Every element follows the
// mode's chrome keys (modes.ts, docs/ui-config.md). When the bar runs out of
// room it collapses by priority: labels first, then Replay, Insert, Undo/Redo,
// Settings, the AI status, the mode switcher and the pages button move into ⋯.
// The bar never overflows.
//
// Other features' actions are called, never re-implemented: openExport
// (ExportDialog), openSettings (settings/open.ts), the Library button
// (LibraryButton.tsx), the AI panel (AiStatus.tsx / LivePanel.tsx).

import { Show, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import {
  canRedo, canUndo, isView, lessonOpened, mode, modeConfig, prefs, revision, savedRevision, setPrefs, showToast, studentEntry, title,
} from "../state";
import { MODES } from "../modes";
import { ModeSwitcher } from "./ModeSwitcher";
import { Icon, Mark } from "./Icon";
import { openExport } from "./ExportDialog";
import { FILE_EXT, WORDMARK } from "../brand";
import { LibraryTopButton } from "./LibraryButton";
import { openSettings } from "../settings/open";
import { openDonatePage } from "../donate";
import { current, flush } from "../library/session";
import { openShareDialog } from "./ShareDialog";
import { MenuButton, type MenuEntry } from "./HeaderMenu";
import { AiStatus, aiConnected, aiStatusText, toggleAi } from "./AiStatus";
import { pagesShown, togglePages } from "../viewport";
import "./Header.css";
import { MicButton } from "./MicButton";
import { insertVideo, insertWebEmbed, pickMedia, pickModel } from "./MediaDialog";

function pickFiles(accept: string, multiple = false): Promise<File[]> {
  return new Promise((resolve) => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = accept;
    i.multiple = multiple;
    i.onchange = () => resolve(Array.from(i.files ?? []));
    i.click();
  });
}

const pickPdf = async () => controller.importFiles(await pickFiles(".pdf,application/pdf"));
const pickImage = async () => {
  const [f] = await pickFiles("image/*");
  if (f) controller.insertImage(f);
};

/** Signed in to Rupok Cloud: a share-link dialog (ShareDialog.tsx). Otherwise,
 * the file through the system share sheet, or saved to send by hand. */
async function shareLesson() {
  if (await openShareDialog(() => void shareLessonFile())) return;
  await shareLessonFile();
}

/** Share the lesson file with the system share sheet (WhatsApp, Mail, Drive…);
 * without one, save the file so it can be sent by hand. */
async function shareLessonFile() {
  const name = (controller.board.title() || "lesson").replace(/[\\/:*?"<>|]+/g, "-") + `.${FILE_EXT}`;
  try {
    const file = new File([controller.board.save() as Uint8Array<ArrayBuffer>], name, { type: "application/octet-stream" });
    if (navigator.canShare?.({ files: [file] })) {
      await navigator.share({ files: [file], title: controller.board.title() || "Lesson" });
      return;
    }
  } catch (e: any) {
    if (e?.name === "AbortError") return;
  }
  await controller.saveFile();
  showToast(`Send the .${FILE_EXT} file to your students: they open it in Rupok`, 5000);
}

/** Collapse thresholds: at this level or above the element moves into ⋯.
 * Level 1 only hides labels. */
const AT = { replay: 2, viewOpen: 2, insert: 3, viewDownload: 3, history: 4, theme: 4, settings: 5, ai: 6, switcher: 7, pagesToggle: 8 } as const;
const MAX_LEVEL = 8;

export function Header(props: { onObject: () => void; onSettings: () => void }) {
  const c = () => modeConfig().chrome;
  let titleInput: HTMLInputElement | undefined; // voice input target (MicButton)
  // From prefs (reactive), not from <html data-theme>, which an effect sets later.
  const dark = () => {
    const t = prefs().theme;
    return t === "dark" || (t === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  };
  /** Student view: a lesson is on the board (opened, or switched to from Prepare). */
  const hasLesson = () => !studentEntry || lessonOpened();
  const view = isView;

  // ---------------------------------------------------------- collapsing
  const [level, setLevel] = createSignal(0);
  const moved = (k: keyof typeof AT) => level() >= AT[k];
  let bar!: HTMLElement;
  const overflowing = () => bar.scrollWidth > bar.clientWidth + 1;
  // The bar's own collapse adds and removes elements, which the observers
  // below see; a fit that would land on the same layout is skipped, so the
  // bar does not re-fit itself in a loop (it did, many times a second, on
  // phones, re-creating an open ⋯ menu under the finger).
  const layoutKey = () => `${bar.clientWidth}|${Array.from(bar.children).map((c) => (c as HTMLElement).scrollWidth).join(",")}`;
  let fitted = "";
  const fit = () => {
    if (!bar) return;
    let l = 0;
    setLevel(0);
    while (overflowing() && l < MAX_LEVEL) setLevel(++l);
    fitted = layoutKey();
  };
  let raf = 0;
  const refit = () => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => {
      if (bar && fitted && layoutKey() === fitted) return;
      fit();
    });
  };
  onMount(() => {
    const ro = new ResizeObserver(refit);
    ro.observe(bar);
    // Content changes (title, "Save a copy", AI undo) change the clusters' widths.
    for (const el of Array.from(bar.children)) ro.observe(el);
    const mo = new MutationObserver(() => {
      for (const el of Array.from(bar.children)) ro.observe(el);
      refit();
    });
    mo.observe(bar, { childList: true });
    window.addEventListener("resize", refit);
    onCleanup(() => {
      ro.disconnect();
      mo.disconnect();
      window.removeEventListener("resize", refit);
      cancelAnimationFrame(raf);
    });
    fit();
  });
  createEffect(on([modeConfig, mode, title, () => prefs().density], () => { fitted = ""; refit(); }, { defer: true }));

  // -------------------------------------------------------------- what shows
  const showPagesToggle = () => c().pagesToggle && c().pages && !moved("pagesToggle");
  const showSwitcher = () => c().switcher && !moved("switcher");
  const leftTail = () => showPagesToggle() || c().logo || showSwitcher();
  const showHistory = () => c().history && !view() && !moved("history");
  const showInsert = () => c().insert && !view() && !moved("insert");
  const showReplay = () => c().replay && !moved("replay");
  const editOn = () => showHistory() || showInsert() || showReplay();
  const showAi = () => c().ai && !view() && !moved("ai");
  const showSettings = () => c().settings && !view() && !moved("settings");
  /** ⋯ is on by config, or forced on once something has collapsed into it. */
  const overflowOn = () => c().overflow || level() >= 2;
  // Student view: what students need, on the bar.
  const viewOpen = () => view() && c().open && !moved("viewOpen");
  const viewDownload = () => view() && hasLesson() && !moved("viewDownload");
  const viewTheme = () => view() && c().theme && !moved("theme");
  // Without the ⋯ menu (overflow: false), its everyday items are icons again.
  const plainIcons = () => !overflowOn() && !view();
  const rightOn = () =>
    viewOpen() || viewDownload() || viewTheme() || showAi() || showSettings() || overflowOn() ||
    (plainIcons() && (c().open || c().save || c().theme || c().fullscreen));

  // ------------------------------------------------------------ save state
  const dirty = () => revision() !== savedRevision();
  const saveState = () => {
    if (!dirty()) return { cls: "", text: "Saved", tip: current() ? "Saved to your library" : `Saved  ·  Ctrl S saves a .${FILE_EXT} file` };
    if (current()) return { cls: "dirty saving", text: "Saving…", tip: "Saving to your library…" };
    return { cls: "dirty", text: "Unsaved", tip: `Changes are kept in this browser. Click or press Ctrl S to save a .${FILE_EXT} file.` };
  };
  const saveNow = () => (current() ? void flush() : void controller.saveFile());

  // ------------------------------------------------------------- the menus
  const toggleTheme = () => setPrefs({ theme: dark() ? "light" : "dark" });
  const insertEntries = (): MenuEntry[] => [
    { id: "insert-image", label: "Image…", icon: "image", run: () => void pickImage() },
    { id: "insert-pdf", label: "PDF…", icon: "pdf", run: () => void pickPdf() },
    { id: "insert-object", label: "Smart Object…", icon: "object", run: props.onObject },
    { id: "insert-video", label: "Video / YouTube…", icon: "video", run: insertVideo },
    { id: "insert-audio", label: "Audio…", icon: "audio", run: () => void pickMedia("audio") },
    { id: "insert-model", label: "3D model (GLB)…", icon: "cube", run: () => void pickModel() },
    { id: "insert-web", label: "Web embed…", icon: "globe", run: insertWebEmbed },
    { id: "insert-text", label: "Text", icon: "text", key: "T", run: () => controller.setTool("text") },
  ];

  const overflowEntries = createMemo((): MenuEntry[] => {
    const k = c();
    const out: MenuEntry[] = [];
    const sep = () => out.length && out[out.length - 1].kind !== "sep" && out.push({ kind: "sep" });
    // 1. Whatever collapsed off the bar, in bar order.
    if (k.switcher && moved("switcher")) {
      out.push({ kind: "label", label: "Mode" });
      for (const m of MODES) out.push({ id: `mode-${m.id}`, label: m.label, icon: m.icon, key: m.key, checked: mode() === m.id, run: () => controller.setMode(m.id) });
      sep();
    }
    if (k.pagesToggle && k.pages && moved("pagesToggle")) {
      out.push({ id: "pages", label: "Pages panel", icon: "sidebar", state: pagesShown() ? "On" : "Off", run: togglePages });
    }
    if (!view()) {
      if (k.history && moved("history")) {
        out.push({ id: "undo", label: "Undo", icon: "undo", key: "Ctrl Z", disabled: !canUndo(), run: () => controller.board.undo() });
        out.push({ id: "redo", label: "Redo", icon: "redo", key: "Ctrl Shift Z", disabled: !canRedo(), run: () => controller.board.redo() });
      }
      if (k.insert && moved("insert")) {
        sep();
        out.push({ kind: "label", label: "Insert" }, ...insertEntries());
        sep();
      }
    }
    if (k.replay && moved("replay")) out.push({ id: "replay", label: "Replay this page", icon: "replay", run: () => controller.board.replay_start() });
    if (!view() && (!k.ai || moved("ai"))) out.push({ id: "ai", label: aiConnected() ? aiStatusText() : "Connect AI…", icon: "smartink", dot: aiConnected(), run: toggleAi });
    if (!view() && k.settings && moved("settings")) out.push({ id: "settings", label: "Settings", icon: "settings", run: props.onSettings });
    sep();

    // 2. Files.
    if (view()) {
      if (k.open && moved("viewOpen")) out.push({ id: "open", label: "Open lesson…", icon: "open", key: "Ctrl O", run: () => controller.openFile() });
      if (hasLesson() && moved("viewDownload")) out.push({ id: "export-pdf", label: "Download PDF…", icon: "download", key: "Ctrl Shift E", run: openExport });
    } else {
      if (k.open) out.push({ id: "open", label: "Open file…", icon: "open", key: "Ctrl O", run: () => controller.openFile() });
      if (k.save) {
        out.push({ id: "save", label: `Save .${FILE_EXT} file`, icon: "save", key: "Ctrl S", run: () => void controller.saveFile() });
        out.push({
          id: "save-as", label: `Download a copy (.${FILE_EXT})…`, icon: "download",
          run: () => { controller.fileHandle = null; void controller.saveFile(); },
        });
        out.push({ id: "export-pdf", label: "Export PDF…", icon: "pdf", key: "Ctrl Shift E", run: openExport });
        out.push({ id: "share", label: "Share lesson…", icon: "export", run: () => void shareLesson() });
      }
    }
    sep();

    // 3. View. Theme and fullscreen are always reachable here.
    if (!viewTheme()) out.push({ id: "theme", label: dark() ? "Light mode" : "Dark mode", icon: dark() ? "sun" : "moon", run: toggleTheme });
    out.push({ id: "fullscreen", label: "Fullscreen", icon: "fullscreen", key: "F", run: () => controller.toggleFullscreen() });

    // 4. Help. Settings stays reachable even when a config hides its button.
    if (!view()) {
      sep();
      if (!k.settings) out.push({ id: "settings", label: "Settings", icon: "settings", run: props.onSettings });
      out.push({ id: "shortcuts", label: "Keyboard shortcuts", icon: "keyboard", run: () => openSettings("keyboard") });
      out.push({ id: "about", label: "Help & about", icon: "info", run: () => openSettings("about") });
      out.push({ id: "donate", label: "Support Rupok", icon: "heart", run: () => void openDonatePage() });
    }
    while (out.length && out[out.length - 1].kind === "sep") out.pop();
    return out;
  });

  return (
    <header class="topbar" ref={bar} classList={{ compact: level() >= 1, "narrow-title": level() >= AT.switcher }} aria-label="Lesson">
      <div class="cluster surface hdr-left">
        <LibraryTopButton />
        <Show when={leftTail() || c().title}>
          <div class="vsep" />
        </Show>
        <Show when={showPagesToggle()}>
          <button class="btn icon" title="Pages panel" aria-label="Toggle pages panel" aria-pressed={pagesShown()} onClick={togglePages}>
            <Icon name="sidebar" />
          </button>
        </Show>
        <Show when={c().logo}>
          <div class="brand">
            <Mark size={28} />
            <span class="wordmark"><b>{WORDMARK.accent}</b>{WORDMARK.rest}</span>
          </div>
        </Show>
        <Show when={showSwitcher()}>
          <ModeSwitcher />
        </Show>
        <Show when={c().title}>
          <Show when={leftTail()}>
            <div class="vsep" />
          </Show>
          <Show
            when={!view()}
            fallback={
              <>
                <span class="title-text" title={title()}>{title()}</span>
                <span class="view-chip"><Icon name="eye" size={16} />View only</span>
              </>
            }
          >
            <span class="title-wrap">
            <input
              ref={titleInput}
              class="title-input"
              value={title()}
              size={Math.min(28, Math.max(10, title().length + 1))}
              onChange={(e) => controller.board.set_title(e.currentTarget.value.trim() || "Untitled lesson")}
              onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
              aria-label="Lesson title"
              title="Lesson title: click to rename"
            />
            <MicButton target={() => titleInput} size={32} class="title-mic" label="Dictate the lesson title" />
            </span>
          </Show>
        </Show>
        <Show when={c().save && !view()}>
          <button class={`save-state ${saveState().cls}`} onClick={saveNow} title={saveState().tip} aria-label={`${saveState().text}. ${saveState().tip}`}>
            <Show when={dirty()}><span class="save-dot" aria-hidden="true" /></Show>
            <span class="save-text">{saveState().text}</span>
          </button>
        </Show>
      </div>

      <div class="grow" />

      <Show when={editOn()}>
        <div class="cluster surface hdr-edit">
          <Show when={showHistory()}>
            <button class="btn icon" disabled={!canUndo()} onClick={() => controller.board.undo()} title="Undo  ·  Ctrl Z" aria-label="Undo"><Icon name="undo" /></button>
            <button class="btn icon" disabled={!canRedo()} onClick={() => controller.board.redo()} title="Redo  ·  Ctrl Shift Z" aria-label="Redo"><Icon name="redo" /></button>
            <Show when={showInsert() || showReplay()}>
              <div class="vsep" />
            </Show>
          </Show>
          <Show when={showInsert()}>
            <MenuButton
              id="hdr-insert"
              label="Insert"
              title="Insert an image, a PDF, a Smart Object or text"
              icon="insert"
              text="Insert"
              caret
              align="left"
              entries={insertEntries}
            />
          </Show>
          <Show when={showReplay()}>
            <button class="btn" onClick={() => controller.board.replay_start()} title="Replay this page stroke by stroke" aria-label="Replay">
              <Icon name="replay" /><span class="label">Replay</span>
            </button>
          </Show>
        </div>
      </Show>

      <Show when={rightOn()}>
      <div class="cluster surface hdr-right">
        {/* Student view: open a lesson, download it as a PDF, light / dark. */}
        <Show when={viewOpen()}>
          <button class="btn" classList={{ solid: !hasLesson() }} onClick={() => controller.openFile()} title={`Open a .${FILE_EXT} lesson  ·  Ctrl O`} aria-label="Open lesson">
            <Icon name="open" /><span class="label">Open lesson</span>
          </button>
        </Show>
        <Show when={viewDownload()}>
          <button class="btn" onClick={openExport} title="Download this lesson as a PDF  ·  Ctrl Shift E" aria-label="Download PDF">
            <Icon name="download" /><span class="label">Download PDF</span>
          </button>
        </Show>
        <Show when={viewTheme()}>
          <button class="btn icon" onClick={toggleTheme} title="Light / dark" aria-label="Toggle theme"><Icon name={dark() ? "sun" : "moon"} /></button>
        </Show>
        {/* A config without the ⋯ menu gets the everyday file buttons back as icons. */}
        <Show when={plainIcons()}>
          <Show when={c().open}>
            <button class="btn icon" onClick={() => controller.openFile()} title="Open  ·  Ctrl O" aria-label="Open"><Icon name="open" /></button>
          </Show>
          <Show when={c().save}>
            <button class="btn icon" onClick={openExport} title="Export PDF  ·  Ctrl Shift E" aria-label="Export PDF"><Icon name="export" /></button>
          </Show>
          <Show when={c().theme}>
            <button class="btn icon" onClick={toggleTheme} title="Light / dark" aria-label="Toggle theme"><Icon name={dark() ? "sun" : "moon"} /></button>
          </Show>
          <Show when={c().fullscreen}>
            <button class="btn icon" onClick={() => controller.toggleFullscreen()} title="Fullscreen  ·  F" aria-label="Fullscreen"><Icon name="fullscreen" /></button>
          </Show>
        </Show>
        <Show when={showAi()}>
          <AiStatus />
        </Show>
        <Show when={showSettings()}>
          <button class="btn icon" onClick={props.onSettings} title="Settings" aria-label="Settings"><Icon name="settings" /></button>
        </Show>
        <Show when={overflowOn()}>
          <MenuButton
            id="hdr-more"
            label="More"
            title="More: files, export, theme, fullscreen, help"
            icon="more"
            align="right"
            dot={!view() && aiConnected() && (!c().ai || moved("ai"))}
            entries={overflowEntries}
          />
        </Show>
      </div>
      </Show>
    </header>
  );
}

/** Teach has no top bar: while an AI is connected (or pairing), its status
 * floats top right so the teacher still sees it and can open the panel. */
export function FloatingAiStatus() {
  return (
    <div class="ai-float surface">
      <AiStatus />
    </div>
  );
}
