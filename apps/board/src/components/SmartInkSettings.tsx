// Settings → Smart ink (shape snapping, scratch-out) and Settings → AI
// (handwriting to text). The Claude key is kept apart from prefs: see
// handwriting.ts.

import { Show, createResource, createSignal } from "solid-js";
import { controller } from "../controller";
import { chromeProvider, claudeKey, keyVersion, pickProvider, setClaudeKey } from "../handwriting";
import { prefs, setPrefs, showToast, type Prefs, type SmartShapes } from "../state";
import { Seg } from "./Dialogs";
import { Icon } from "./Icon";

export function SmartInkSettings() {
  const p = prefs;
  const set = (x: Partial<Prefs>) => {
    setPrefs(x);
    controller.applyPrefs();
  };
  const [draft, setDraft] = createSignal("");
  const [active] = createResource(keyVersion, () => pickProvider());
  const [chrome] = createResource(() => chromeProvider.available());
  const hasKey = () => (keyVersion(), claudeKey() !== "");
  const saveKey = () => {
    const k = draft().trim();
    if (!k.startsWith("sk-ant-")) {
      showToast("That doesn't look like a Claude API key (they start with sk-ant-)");
      return;
    }
    setClaudeKey(k);
    setDraft("");
    showToast("Key saved on this device");
  };
  return (
    <>
      <div class="section-title" id="settings-smart">Smart ink</div>
      <div class="settings-grid">
        <div class="field">
          <label>Smart shapes</label>
          <Seg
            value={p().smartShapes}
            options={[["off", "Off"], ["auto", "Auto"], ["hold", "Hold to snap"]] as const}
            onChange={(v: SmartShapes) => set(v === "off" ? { smartShapes: v } : { smartShapes: v, smartLast: v })}
          />
          <span class="hint">
            Auto: a rough circle, rectangle, triangle, diamond, star, line or arrow becomes clean when you lift the pen.
            Hold to snap: keep the pen still for half a second, then drag to resize or turn it. Handwriting stays as you wrote it, and Undo brings back your drawing.
          </span>
        </div>
        <div class="field">
          <label>Scratch out to erase</label>
          <Seg value={p().scratchOut ? "on" : "off"} options={[["on", "On"], ["off", "Off"]] as const} onChange={(v) => set({ scratchOut: v === "on" })} />
          <span class="hint">Scribble a tight zig-zag over ink to erase it. Works while smart shapes are on.</span>
        </div>
      </div>

      <div class="section-title" id="settings-ai">AI</div>
      <div class="settings-grid">
        <div class="field">
          <label>Handwriting to text</label>
          <span class="ai-status">
            <Show when={active()} fallback={<>Off. Add a key, or use a Chromebook.</>}>
              {(pr) => <>Using {pr().label}: {pr().reads}.</>}
            </Show>
          </span>
          <span class="hint">
            Lasso some writing and tap <b>To text</b>. It becomes a text box in the same place and colour.
            {chrome() ? " Chrome's offline recognizer is available on this device." : ""}
          </span>
        </div>
        <div class="field">
          <label for="claude-key">Claude API key (optional)</label>
          <Show
            when={!hasKey()}
            fallback={
              <div class="key-row">
                <span class="key-saved"><Icon name="info" size={16} /> Key saved on this device</span>
                <button class="btn danger" onClick={() => { setClaudeKey(""); showToast("Key removed"); }}>Remove</button>
              </div>
            }
          >
            <div class="key-row">
              <input
                id="claude-key"
                class="input"
                type="password"
                autocomplete="off"
                spellcheck={false}
                placeholder="sk-ant-…"
                value={draft()}
                onInput={(e) => setDraft(e.currentTarget.value)}
                onKeyDown={(e) => e.key === "Enter" && saveKey()}
              />
              <button class="btn solid" disabled={!draft().trim()} onClick={saveKey}>Save</button>
            </div>
          </Show>
          <span class="hint">
            Reads English, বাংলা and maths with Claude Opus 5.5 on your own Anthropic account. The key is stored only on this device,
            never in lesson files, autosave or exported settings, and is sent only to api.anthropic.com when you tap To text.
          </span>
        </div>
      </div>
    </>
  );
}
