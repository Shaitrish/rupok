// Settings → Library: Teacher/Student, your name, where lessons live, and
// browser storage. Used in the board's Settings dialog and in the Library.

import { For, Show, createResource, createSignal } from "solid-js";
import { LIcon, bytes } from "./LibraryIcon";
import { isDesktop } from "../platform";
import { showToast } from "../state";
import { libPrefs, setLibPrefs } from "../library/prefs";
import { bump, store, version } from "../library/session";
import { canUseFolder } from "../library/store";
import { changeDesktopFolder, persistStorage, resetDesktopFolder, storageEstimate, useBrowserStorage, useFolder } from "../library/actions";
import type { DesktopDriver } from "../library/desktop";

/** Segmented control (same look as Settings' own; local to avoid an import cycle). */
function Seg<T extends string>(props: { value: T; options: readonly (readonly [T, string])[]; onChange: (v: T) => void }) {
  return (
    <div class="seg" role="radiogroup">
      <For each={props.options}>
        {([v, label]) => (
          <button role="radio" aria-checked={props.value === v} classList={{ on: props.value === v }} onClick={() => props.onChange(v)}>
            {label}
          </button>
        )}
      </For>
    </div>
  );
}

/** `only="storage"`: just where lessons live and browser storage (Settings →
 * General shows the role and name as its own rows). */
export function LibrarySettings(props: { only?: "storage" } = {}) {
  const [ask, setAsk] = createSignal<{ n: number; answer: (copy: boolean) => void } | null>(null);
  const [usage, { refetch }] = createResource(() => version(), () => storageEstimate());
  const desktopInfo = () => (store()?.kind === "desktop" ? (store()!.driver as DesktopDriver).info : null);
  const where = () => (store()?.kind === "folder" ? "folder" : "browser");

  const pickFolder = async () => {
    try {
      const ok = await useFolder((n) => new Promise((resolve) => setAsk({ n, answer: (c) => { setAsk(null); resolve(c); } })));
      if (ok) showToast(`Your library is now the folder “${store()?.label}”`);
    } catch (e: any) {
      showToast(`Could not use that folder: ${e?.message ?? e}`, 5000);
    }
  };

  return (
    <div class="lib-settings">
      <Show when={props.only !== "storage"}>
      <div class="settings-grid">
        <div class="field">
          <label>I use Rupok as a</label>
          <Seg
            value={libPrefs().role ?? "teacher"}
            options={[["teacher", "Teacher"], ["student", "Student"]] as const}
            onChange={(v) => { setLibPrefs({ role: v, setupDone: true }); bump(); }}
          />
          <span class="hint">Students start on lessons shared with them and keep their own copies.</span>
        </div>
        <div class="field">
          <label for="lib-author">Your name</label>
          <input id="lib-author" class="input" placeholder="Shown on lessons you make" value={libPrefs().author} onChange={(e) => setLibPrefs({ author: e.currentTarget.value.trim().slice(0, 120) })} />
          <span class="hint">Saved inside lessons you make or copy, so copies can credit you.</span>
        </div>
      </div>
      </Show>

      <div class="field">
        <label>Where your lessons live</label>
        <Show
          when={isDesktop}
          fallback={
            <>
              <Show when={canUseFolder()} fallback={<div class="loc"><LIcon name="browser" /><span>In this browser’s private storage</span></div>}>
                <Seg
                  value={where()}
                  options={[["browser", "This browser"], ["folder", "A folder on this computer"]] as const}
                  onChange={(v) => void (v === "folder" ? pickFolder() : useBrowserStorage().then(() => showToast("Your library is now in this browser")))}
                />
              </Show>
              <Show when={where() === "folder"}>
                <div class="loc"><LIcon name="laptop" /><span>Folder <code>{store()?.label}</code></span>
                  <button class="btn" onClick={() => void pickFolder()}>Change…</button>
                </div>
              </Show>
              <span class="hint">
                {where() === "folder"
                  ? "Lessons are plain .rpk files in that folder, so the Rupok desktop app, Google Drive or OneDrive for desktop, and AI agents can use them too."
                  : canUseFolder()
                    ? "Lessons are kept privately by this browser. Choose a folder on this computer to sync it with Drive or the desktop app."
                    : "Lessons are kept privately by this browser."}
              </span>
            </>
          }
        >
          <div class="loc"><LIcon name="laptop" /><code title={desktopInfo()?.root}>{desktopInfo()?.root}</code></div>
          <div class="acts">
            <button class="btn" onClick={() => void changeDesktopFolder().then((ok) => ok && showToast("Library folder changed"))}><LIcon name="folder" /><span class="label">Change folder…</span></button>
            <Show when={desktopInfo()?.custom}>
              <button class="btn" onClick={() => void resetDesktopFolder()}>Use Documents/Rupok</button>
            </Show>
          </div>
          <span class="hint">AI agents see the same lessons: <code>rupok mcp --workspace "{desktopInfo()?.root}"</code>. Sync apps (Drive, OneDrive) can sync this folder.</span>
        </Show>
        <Show when={ask()}>
          {(a) => (
            <div class="lib-banner" style={{ margin: "8px 0 0" }}>
              <LIcon name="copy" />
              <span class="grow">Copy your <b>{a().n} lesson{a().n === 1 ? "" : "s"}</b> from this browser into the folder?</span>
              <button class="btn" onClick={() => a().answer(false)}>Start empty</button>
              <button class="btn solid" onClick={() => a().answer(true)}>Copy lessons</button>
            </div>
          )}
        </Show>
      </div>

      <Show when={usage()}>
        {(u) => (
          <div class="field">
            <label>Browser storage</label>
            <div class="lib-meter"><i style={{ width: `${Math.min(100, Math.max(1, (u().usage / Math.max(1, u().quota)) * 100))}%` }} /></div>
            <span class="hint">
              {bytes(u().usage)} used of {bytes(u().quota)} available to {location.host || "this app"}.{" "}
              {u().persisted ? "The browser keeps it even when space runs low." : "The browser may clear it when space runs low."}
            </span>
            <Show when={!u().persisted}>
              <div class="acts">
                <button class="btn" onClick={async () => { const ok = await persistStorage(); showToast(ok ? "Your lessons are kept safe in this browser" : "The browser did not allow it. A folder on this computer is safest."); void refetch(); }}>
                  <LIcon name="shield" /><span class="label">Keep my lessons safe</span>
                </button>
              </div>
            </Show>
          </div>
        )}
      </Show>
    </div>
  );
}
