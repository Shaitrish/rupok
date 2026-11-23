// "Draw over any app" pages. See apps/desktop/src/overlay.rs for the windows.

import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Icon } from "../components/Icon";
import { closeOverlay, desktopEmitTo, desktopInvoke, desktopListen, isDesktop } from "../platform";
import { prefs, showToast, toast } from "../state";
import OverlayToolbar from "./OverlayToolbar";
import {
  CLICK_THROUGH_EVENT, CMD_EVENT, INK_WINDOW, type OverlayCommand, OverlayInk, type OverlayStatus, STATUS_EVENT,
  TOOLBAR_WINDOW,
} from "./ink";
import type { OverlayRole } from "./role";

/** Theme, accent and density on <html>, as in the board (App.tsx). */
function useTheme() {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const [sysDark, setSysDark] = createSignal(media.matches);
  media.addEventListener("change", (e) => setSysDark(e.matches));
  createEffect(() => {
    const p = prefs();
    const root = document.documentElement;
    root.dataset.theme = p.theme === "system" ? (sysDark() ? "dark" : "light") : p.theme;
    root.dataset.accent = p.accent;
    root.dataset.density = p.density;
  });
}

function onKey(handler: (e: KeyboardEvent) => void) {
  window.addEventListener("keydown", handler);
  onCleanup(() => window.removeEventListener("keydown", handler));
}

const exit = () => (isDesktop ? void closeOverlay() : location.assign("/board/"));

/** The ink layer. In a browser it also shows the toolbar. */
function InkLayer() {
  let root!: HTMLDivElement;
  let canvas!: HTMLCanvasElement;
  let input!: HTMLDivElement;
  const ink = new OverlayInk();
  const [error, setError] = createSignal<string | null>(null);
  const [through, setThrough] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [permissionHelp, setPermissionHelp] = createSignal(false);

  const status = (s: OverlayStatus) => {
    if (isDesktop) void desktopEmitTo(TOOLBAR_WINDOW, STATUS_EVENT, s);
  };
  const save = async () => {
    if (!isDesktop) {
      showToast("Save to lesson works in the Rupok desktop app");
      return;
    }
    if (saving()) return;
    setSaving(true);
    status({ saving: true });
    try {
      showToast(await ink.saveToLesson(), 4000);
      if (ink.needsScreenPermission) {
        // Make the help clickable even if clicks were passing through.
        void desktopInvoke("overlay_set_click_through", { enabled: false });
        setPermissionHelp(true);
      }
    } catch (e: any) {
      showToast(`Could not save to the lesson: ${e?.message ?? e}`, 5000);
    } finally {
      setSaving(false);
      status({ saving: false });
    }
  };
  const run = (cmd: OverlayCommand) => {
    if (cmd.type === "tool") ink.setTool(cmd);
    else if (cmd.type === "undo") ink.undo();
    else if (cmd.type === "redo") ink.redo();
    else if (cmd.type === "clear") ink.clear();
    else if (cmd.type === "save") void save();
  };

  onKey((e) => {
    const k = e.key.toLowerCase();
    if (e.key === "Escape") exit();
    else if ((e.ctrlKey || e.metaKey) && k === "z") e.shiftKey ? ink.redo() : ink.undo();
    else if ((e.ctrlKey || e.metaKey) && k === "y") ink.redo();
    else return;
    e.preventDefault();
  });

  onMount(async () => {
    if (isDesktop) {
      void desktopListen<OverlayCommand>(CMD_EVENT, run);
      void desktopListen<boolean>(CLICK_THROUGH_EVENT, setThrough);
    }
    try {
      await ink.init(root, canvas);
      ink.attachInput(input);
      status({ ready: true });
    } catch (e: any) {
      console.error(e);
      setError(String(e?.message ?? e));
    }
  });

  return (
    <div class="overlay-board" classList={{ through: through() }} ref={root}>
      <canvas class="ink" ref={canvas} />
      <div class="input-layer" ref={input} />
      <Show when={!isDesktop}>
        <div class="ov-dock">
          <OverlayToolbar send={run} clickThrough={through()} onClickThrough={setThrough} saving={saving()} onExit={exit} />
        </div>
      </Show>
      <Show when={toast()}>
        <div class="toast surface" role="status"><Icon name="info" size={20} />{toast()}</div>
      </Show>
      <Show when={error()}>
        <div class="toast surface ov-error" role="alert"><Icon name="info" size={20} />The overlay could not start the GPU renderer: {error()}</div>
      </Show>
      <Show when={permissionHelp()}>
        <ScreenPermissionHelp onClose={() => setPermissionHelp(false)} />
      </Show>
    </div>
  );
}

/** macOS: why the screen was not saved, and the way to System Settings. */
function ScreenPermissionHelp(props: { onClose: () => void }) {
  const open = () => {
    props.onClose();
    // Hides the overlay (it would cover System Settings) and opens the pane.
    void desktopInvoke("overlay_open_screen_settings").catch((e) => showToast(String(e?.message ?? e), 5000));
  };
  return (
    <div class="ov-permission dialog" role="alertdialog" aria-labelledby="ov-permission-title" aria-describedby="ov-permission-text">
      <h2 id="ov-permission-title">Let Rupok see the screen</h2>
      <p id="ov-permission-text" class="sub">
        Your ink was saved as a new page. To save the screen behind it too, macOS needs to allow Rupok to record the screen.
        Rupok only looks at the screen when you press Save to lesson.
      </p>
      <ol>
        <li>Click <b>Open System Settings</b>. The overlay hides.</li>
        <li>In <b>Screen &amp; System Audio Recording</b> (or <b>Screen Recording</b>), turn on <b>Rupok</b>.</li>
        <li>Quit Rupok with <b>⌘Q</b> and open it again. macOS applies the change after a restart.</li>
      </ol>
      <div class="ov-permission-actions">
        <button class="btn" onClick={props.onClose}>Not now</button>
        <button class="btn solid" onClick={open}>Open System Settings</button>
      </div>
    </div>
  );
}

/** The desktop app's floating toolbar window: it sizes the window to fit. */
function ToolbarWindow() {
  let dock!: HTMLDivElement;
  const [through, setThrough] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [resync, setResync] = createSignal(0);
  const send = (cmd: OverlayCommand) => void desktopEmitTo(INK_WINDOW, CMD_EVENT, cmd);

  onKey((e) => {
    if (e.key === "Escape") exit();
    else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") send({ type: e.shiftKey ? "redo" : "undo" });
    else return;
    e.preventDefault();
  });

  onMount(() => {
    void desktopListen<boolean>(CLICK_THROUGH_EVENT, setThrough);
    void desktopListen<OverlayStatus>(STATUS_EVENT, (s) => {
      if (s.saving !== undefined) setSaving(s.saving);
      if (s.ready) setResync((n) => n + 1);
    });
    const fit = () => {
      const r = dock.getBoundingClientRect();
      void desktopInvoke("overlay_toolbar_fit", { width: r.width, height: r.height });
    };
    new ResizeObserver(fit).observe(dock);
  });

  return (
    <div class="ov-dock" ref={dock}>
      <OverlayToolbar
        draggable
        send={send}
        clickThrough={through()}
        onClickThrough={(on) => void desktopInvoke("overlay_set_click_through", { enabled: on })}
        saving={saving()}
        onExit={exit}
        resync={resync()}
      />
    </div>
  );
}

export default function OverlayApp(props: { role: OverlayRole }) {
  document.documentElement.dataset.overlay = props.role;
  useTheme();
  return props.role === "toolbar" ? <ToolbarWindow /> : <InkLayer />;
}
