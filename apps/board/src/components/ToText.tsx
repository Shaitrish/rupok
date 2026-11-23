// "To text" in the selection toolbar: converts the selected handwriting to a
// text item through the first available recognizer (handwriting.ts). With
// none available it explains how to turn one on instead of being a dead
// button.

import { For, Show, createSignal } from "solid-js";
import { controller } from "../controller";
import { type InkSample, PROVIDERS, pickProvider, textPlacement } from "../handwriting";
import { prefs, selectionIds, showToast } from "../state";
import { fromInk, looksLikeMath, mathPlacement, recognize } from "../mathInk";
import { Icon } from "./Icon";

let settingsOpener: ((section?: string) => void) | null = null;
/** App registers how to open Settings (at a section). */
export function registerSettingsOpener(f: (section?: string) => void) {
  settingsOpener = f;
}

/** Offline maths converts straight away from this confidence; below it the
 * AI reader gets a go when there is one. */
const OFFLINE_SURE = 0.7;
/** Without an AI reader, the offline reading is used from here. */
const OFFLINE_MIN = 0.45;

export function ToTextButton() {
  const [busy, setBusy] = createSignal(false);
  const [help, setHelp] = createSignal(false);
  /** Open the popover upwards when there is no room below the toolbar. */
  const [up, setUp] = createSignal(false);
  let wrap: HTMLDivElement | undefined;
  let abort: AbortController | null = null;
  const toggleHelp = () => {
    const r = wrap?.getBoundingClientRect();
    setUp(!!r && r.bottom + 300 > window.innerHeight);
    setHelp(!help());
  };

  const run = async () => {
    if (busy()) {
      abort?.abort();
      return;
    }
    const json = controller.board.selection_ink_json();
    if (json === "null") return;
    const ink = JSON.parse(json) as InkSample;
    const ids = selectionIds();
    // Maths first, offline: no key and no network. Messy maths goes to the
    // AI reader when there is one.
    const provider = await pickProvider();
    if (prefs().smartMath) {
      const { xy, counts } = fromInk(ink.strokes);
      const best = recognize(xy, counts)[0];
      if (looksLikeMath(best) && (best.confidence >= OFFLINE_SURE || (!provider && best.confidence >= OFFLINE_MIN))) {
        const at = mathPlacement(ink.bounds, best.latex);
        if (controller.board.replace_selection_with_latex(best.latex, at.x, at.y, at.w, at.size)) {
          controller.sync(true);
          showToast("Converted to maths, offline. Undo brings the ink back.");
        }
        return;
      }
    }
    if (!provider) {
      toggleHelp();
      return;
    }
    const ctl = new AbortController();
    abort = ctl;
    setBusy(true);
    try {
      const r = await provider.recognize(ink, ctl.signal);
      const text = r.text.trim();
      if (!text) {
        showToast("Couldn't read that handwriting. Try writing a little larger.");
        return;
      }
      // The teacher may have moved on while we waited: convert what was selected.
      controller.board.select_json(JSON.stringify(ids));
      let id: string | undefined;
      if (r.kind === "math" && r.latex.trim()) {
        const at = mathPlacement(ink.bounds, r.latex);
        id = controller.board.replace_selection_with_latex(r.latex.trim(), at.x, at.y, at.w, at.size);
      } else {
        const at = textPlacement(ink.bounds, text);
        id = controller.board.replace_selection_with_text(text, at.x, at.y, at.w, at.size);
      }
      if (id) showToast(`Converted to ${r.kind === "math" ? "maths" : "text"}. Undo brings the ink back.`);
    } catch (e: any) {
      if (!ctl.signal.aborted) showToast(`Could not convert: ${e?.message ?? e}`, 5000);
    } finally {
      setBusy(false);
      abort = null;
    }
  };

  return (
    <div class="totext-wrap" ref={wrap}>
      <button
        class="btn totext"
        classList={{ busy: busy() }}
        title={busy() ? "Reading the handwriting… tap to cancel" : "Turn this handwriting into text"}
        aria-label="Convert handwriting to text"
        aria-busy={busy()}
        onClick={run}
      >
        <Icon name="totext" />
        <span class="label">{busy() ? "Reading…" : "To text"}</span>
      </button>
      <Show when={help()}>
        <div class="totext-help surface" classList={{ up: up() }} role="dialog" aria-label="Turn on handwriting to text">
          <strong>Turn handwriting into text</strong>
          <p>This needs a handwriting reader. Either of these works:</p>
          <ul>
            <For each={PROVIDERS.filter((p) => p.id !== "cloud")}>
              {(p) => (
                <li>
                  <b>{p.label}</b>: {p.reads}.{" "}
                  {p.id === "claude"
                    ? "Add your key in Settings → AI. It stays on this device; the ink image goes only to api.anthropic.com."
                    : "Built into Chrome on Chromebooks, nothing to set up."}
                </li>
              )}
            </For>
          </ul>
          <div class="totext-actions">
            <button class="btn" onClick={() => setHelp(false)}>Close</button>
            <button class="btn solid" onClick={() => { setHelp(false); settingsOpener?.("ai"); }}>Open Settings</button>
          </div>
        </div>
      </Show>
    </div>
  );
}
