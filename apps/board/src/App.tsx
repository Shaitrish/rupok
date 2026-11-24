import { Show, createEffect, createSignal, onMount } from "solid-js";
import { controller } from "./controller";
import World from "./components/World";
import Dock from "./components/Dock";
import GestureLayer from "./components/GestureLayer";
import { PagesPanel, StatusChips, TopBar } from "./components/Chrome";
import { ObjectDialog } from "./components/Dialogs";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { closeSettings, openSettings, settingsRequest } from "./settings/open";
import { isView, prefs, sizes, title, toast, tool } from "./state";
import { handleLink } from "./components/MediaDialog";
import { Icon, Mark } from "./components/Icon";
import { APP_NAME, FILE_EXT, TAGLINE, WORDMARK } from "./brand";
import { Suspense, lazy } from "solid-js";
import { attachDesktopBridge } from "./platform";
import { onLaunchFiles, takePendingLesson } from "./pending";
import { overlayRole } from "./overlay/role";
import { registerSettingsOpener } from "./components/ToText";
import LivePanel from "./components/LivePanel";
import ChatPanel from "./components/ChatPanel";
import VoiceLayer from "./voice/VoiceLayer";
import { LibraryRoot } from "./components/Library";
import { current, startLibrary } from "./library/session";
import { courseFromPath } from "./library/paths";
import { libPrefs } from "./library/prefs";
import { setBrandVars } from "./design";
import { isPhone, pagesDrawer, pagesShown } from "./viewport";
import { PageDesignHost } from "./components/PageDesign";
import { drawCursor } from "./wetInk";

const OverlayApp = lazy(() => import("./overlay/OverlayApp"));

export default function App() {
  // "Draw over any app": the desktop overlay windows, or /board/?overlay in a browser.
  if (overlayRole) return <Suspense><OverlayApp role={overlayRole} /></Suspense>;
  let root!: HTMLDivElement;
  let canvas!: HTMLCanvasElement;
  let world!: HTMLDivElement;
  let input!: HTMLDivElement;
  let eraser!: HTMLDivElement;
  const [ready, setReady] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [objectDialog, setObjectDialog] = createSignal(false);
  // Settings deep links (settings/open.ts): popovers open the exact setting.
  registerSettingsOpener(openSettings);
  const [dragging, setDragging] = createSignal(false);

  // Theme, accent and density are attributes on <html>; CSS tokens do the rest.
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
  const [sysDark, setSysDark] = createSignal(systemDark.matches);
  systemDark.addEventListener("change", (e) => setSysDark(e.matches));
  createEffect(() => {
    const p = prefs();
    const root = document.documentElement;
    root.dataset.theme = p.theme === "system" ? (sysDark() ? "dark" : "light") : p.theme;
    root.dataset.accent = p.accent;
    root.dataset.density = p.density;
    if (ready()) queueMicrotask(() => controller.objects.broadcastTheme());
  });
  // What branding placeholders stand for ({lesson}, {teacher}, {subject}…; design.ts).
  createEffect(() => {
    const c = courseFromPath(current()?.path ?? "");
    setBrandVars({ lesson: title(), teacher: libPrefs().author, subject: c?.subject, course: c?.course, chapter: c?.chapter });
  });
  // The eraser circle goes with the eraser tool.
  createEffect(() => {
    if (tool() !== "eraser" && eraser) eraser.style.display = "none";
  });
  // Re-fit when the chrome layout changes (toolbar side, size, pages panel).
  createEffect(() => {
    const p = prefs();
    void p.toolbar; void p.density; void p.showPages; void isPhone();
    if (ready()) controller.updateInsets(true);
  });

  onMount(async () => {
    try {
      await controller.init(root, canvas, world);
      controller.attachInput(input);
      controller.attachKeys();
      controller.eraserCursor = (x, y, show) => {
        eraser.style.display = show ? "block" : "none";
        eraser.style.left = `${x}px`;
        eraser.style.top = `${y}px`;
      };
      // The eraser circle also shows while a finger or pen rubs (no hover on
      // touch) and goes when it lifts or leaves the board.
      const eraserAt = (e: PointerEvent) => {
        if (tool() !== "eraser") return;
        const r = root.getBoundingClientRect();
        controller.eraserCursor?.(e.clientX - r.left, e.clientY - r.top, true);
        eraser.classList.add("rubbing");
      };
      const eraserOff = (e: PointerEvent) => {
        eraser.classList.remove("rubbing");
        if (e.type !== "pointerup" || e.pointerType === "touch") eraser.style.display = "none";
      };
      input.addEventListener("pointerdown", eraserAt);
      for (const t of ["pointerup", "pointercancel", "pointerleave"]) input.addEventListener(t, eraserOff as EventListener);
      setReady(true);
      void import("./solve").then((m) => m.initSolve()); // Solve this: AI handwriting, preview, reveal
      startLibrary(controller); // the Library home and saving lessons to their files (before files below arrive)
      attachDesktopBridge(controller); // desktop: files opened by the OS, overlay snapshots
      onLaunchFiles((files) => void controller.importFiles(files)); // installed PWA: "Open with Rupok"
      void takePendingLesson().then((f) => f && controller.importFiles([f])); // a file picked on /open/
    } catch (e: any) {
      console.error(e);
      setError(String(e?.message ?? e));
    }
    window.addEventListener("dragover", (e) => { e.preventDefault(); setDragging(true); });
    window.addEventListener("dragleave", (e) => { if (!e.relatedTarget) setDragging(false); });
    window.addEventListener("drop", (e) => {
      e.preventDefault();
      setDragging(false);
      if (e.dataTransfer?.files.length) controller.importFiles(Array.from(e.dataTransfer.files));
      else if (!isView()) handleLink(e.dataTransfer?.getData("text/uri-list").split("\n")[0] || e.dataTransfer?.getData("text/plain") || "");
    });
    window.addEventListener("paste", (e) => {
      const t = e.target as HTMLElement;
      if (t?.isContentEditable || t?.tagName === "INPUT" || t?.tagName === "TEXTAREA") return;
      const files = Array.from(e.clipboardData?.files ?? []);
      if (files.length) {
        controller.importFiles(files);
        return;
      }
      const text = e.clipboardData?.getData("text/plain") ?? "";
      if (!isView() && handleLink(text)) return; // YouTube and trusted embeds (media/)
      if (/^\s*(<!doctype html|<html|<svg|<div|<canvas|<style|<script)/i.test(text)) {
        const title = text.match(/<title>([^<]*)<\/title>/i)?.[1]?.trim() || "Pasted Smart Object";
        controller.addObject(text, title, 1280, 720, 0);
      }
    });
  });

  return (
    <div class="app">
      <div class="board" ref={root}>
        <World ref={(el) => (world = el)} />
        <canvas class="ink" ref={canvas} />
        <div class="input-layer" classList={{ interact: tool() === "interact" }} ref={input} style={{ cursor: cursorFor(tool()) }} />
        <div class="eraser-cursor" ref={eraser} style={{ display: "none", width: `${sizes().eraser ?? 64}px`, height: `${sizes().eraser ?? 64}px` }} />
        <GestureLayer onSettings={openSettings} />
      </div>
      <Show when={ready()}>
        <TopBar onObject={() => setObjectDialog(true)} onSettings={() => openSettings()} />
        <Show when={pagesShown() || pagesDrawer()}>
          <PagesPanel />
        </Show>
        <PageDesignHost />
        {/* The Smart Dock; it also holds the replay controls while a replay runs. */}
        <Dock onSettings={openSettings} />
        <StatusChips />
        {/* Connect AI: the teacher's own agent edits the board live (live.ts). */}
        <LivePanel />
        {/* AI chat with the teacher's own agent (chat.ts, `rupok agent`). */}
        <ChatPanel />
        <VoiceLayer />
        <LibraryRoot />
      </Show>
      <Show when={objectDialog()}>
        <ObjectDialog onClose={() => setObjectDialog(false)} />
      </Show>
      <Show when={settingsRequest()}>
        {(r) => <SettingsDialog request={r()} onClose={closeSettings} />}
      </Show>
      <Show when={toast()}>
        <div class="toast surface" role="status"><Icon name="info" size={18} />{toast()}</div>
      </Show>
      <Show when={dragging()}>
        <div class="drop-hint">Drop PDF, images, HTML Smart Objects or a .{FILE_EXT} lesson</div>
      </Show>
      <Show when={!ready()}>
        <div class="loading">
          <div class="inner">
            <Mark size={64} />
            <div class="loading-word">{WORDMARK.rest}</div>
            <div class="loading-tag">{TAGLINE}</div>
            <Show when={error()}>
              <div class="err">
                {APP_NAME} could not start the GPU renderer:{"\n"}{error()}{"\n\n"}Try a recent Chrome or Edge.
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}

function cursorFor(t: string): string {
  switch (t) {
    case "hand": return "grab";
    case "text": return "text";
    case "lasso": return "default";
    case "eraser": return "none";
    case "interact": return "default";
    // Pen, highlighter, shapes, laser: no crosshair over the ink (wetInk.ts).
    default: return drawCursor();
  }
}
