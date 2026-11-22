import { Show, createEffect, createSignal, onMount } from "solid-js";
import { controller } from "./controller";
import World from "./components/World";
import Toolbar from "./components/Toolbar";
import { PagesPanel, ReplayBar, StatusChips, TopBar } from "./components/Chrome";
import { ObjectDialog, SettingsDialog } from "./components/Dialogs";
import { prefs, replay, toast, tool } from "./state";
import { Icon, Mark } from "./components/Icon";
import { APP_NAME, FILE_EXT, TAGLINE, WORDMARK } from "./brand";

export default function App() {
  let root!: HTMLDivElement;
  let canvas!: HTMLCanvasElement;
  let world!: HTMLDivElement;
  let input!: HTMLDivElement;
  let eraser!: HTMLDivElement;
  const [ready, setReady] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [objectDialog, setObjectDialog] = createSignal(false);
  const [settings, setSettings] = createSignal(false);
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
  // Re-fit when the chrome layout changes (toolbar side, size, pages panel).
  createEffect(() => {
    const p = prefs();
    void p.toolbar; void p.density; void p.showPages;
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
      setReady(true);
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
        <div class="eraser-cursor" ref={eraser} style={{ display: "none", width: "28px", height: "28px" }} />
      </div>
      <Show when={ready()}>
        <TopBar onObject={() => setObjectDialog(true)} onSettings={() => setSettings(true)} />
        <Show when={prefs().showPages}>
          <PagesPanel />
        </Show>
        <Show when={!replay()} fallback={<ReplayBar />}>
          <Toolbar />
        </Show>
        <StatusChips />
      </Show>
      <Show when={objectDialog()}>
        <ObjectDialog onClose={() => setObjectDialog(false)} />
      </Show>
      <Show when={settings()}>
        <SettingsDialog onClose={() => setSettings(false)} />
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
    default: return "crosshair";
  }
}
