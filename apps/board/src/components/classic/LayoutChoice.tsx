// First run: "Which layout do you prefer?" Modern (the Smart Dock and the
// top bar) or Classic (three big islands along the bottom). Asked once;
// Settings → Appearance → Layout changes it any time.

import { For, Show, createSignal, onMount } from "solid-js";
import { isView, prefs, setPrefs, studentEntry } from "../../state";
import { libraryOpen } from "../../library/session";
import { type Layout, layout } from "../../layout";
import { setLayout } from "../DockEditor";
import { Icon } from "../Icon";

/** Should the question show by itself? Never for students or automated
 * browsers (e2e suites ask for it with /board/?layout). */
function wanted(): boolean {
  if (studentEntry || prefs().layoutChosen) return false;
  if (navigator.webdriver && !new URLSearchParams(location.search).has("layout")) return false;
  return true;
}

/** A schematic of each layout: the slide, and where the buttons sit. */
function Preview(props: { kind: Layout }) {
  return (
    <svg viewBox="0 0 240 135" class="lc-art" aria-hidden="true">
      <rect x="0" y="0" width="240" height="135" rx="10" class="lc-canvas" />
      <rect x="22" y={props.kind === "modern" ? 26 : 12} width="196" height={props.kind === "modern" ? 82 : 92} rx="4" class="lc-page" />
      <path d={props.kind === "modern" ? "M44 64 q14 -16 28 0 t28 0" : "M44 54 q14 -16 28 0 t28 0"} class="lc-ink" />
      <Show
        when={props.kind === "classic"}
        fallback={
          <>
            <rect x="8" y="6" width="224" height="14" rx="5" class="lc-chrome" />
            <rect x="60" y="114" width="120" height="15" rx="7" class="lc-chrome" />
            <For each={[0, 1, 2, 3, 4, 5, 6]}>{(i) => <circle cx={72 + i * 16} cy="121.5" r="3.5" class="lc-dot" />}</For>
          </>
        }
      >
        <rect x="8" y="110" width="60" height="19" rx="8" class="lc-chrome" />
        <rect x="76" y="110" width="98" height="19" rx="8" class="lc-chrome" />
        <rect x="182" y="110" width="50" height="19" rx="8" class="lc-chrome" />
        <For each={[0, 1, 2, 3]}>{(i) => <rect x={14 + i * 13} y="115" width="9" height="9" rx="2" class="lc-dot" />}</For>
        <For each={[0, 1, 2, 3, 4, 5]}>{(i) => <rect x={83 + i * 15} y="115" width="9" height="9" rx="2" class={i === 1 ? "lc-dot on" : "lc-dot"} />}</For>
        <For each={[0, 1, 2]}>{(i) => <rect x={188 + i * 14} y="115" width="9" height="9" rx="2" class="lc-dot" />}</For>
      </Show>
    </svg>
  );
}

const CHOICES: { id: Layout; title: string; text: string }[] = [
  { id: "modern", title: "Modern", text: "A top bar for the lesson and one dock that moves to your hand." },
  { id: "classic", title: "Classic", text: "Big buttons in three islands along the bottom, a ☰ menu. Familiar from classroom whiteboard software." },
];

export function LayoutChoice() {
  const [open, setOpen] = createSignal(false);
  const [pick, setPick] = createSignal<Layout>(layout());
  onMount(() => {
    if (!wanted()) return;
    // After the Library home, on the board.
    let tries = 0;
    const t = setInterval(() => {
      if (++tries > 600 || prefs().layoutChosen) return clearInterval(t);
      if (!libraryOpen() && !isView()) {
        clearInterval(t);
        setOpen(true);
      }
    }, 500);
  });
  const done = (l: Layout) => {
    setLayout(l);
    setPrefs({ layoutChosen: true });
    setOpen(false);
  };
  return (
    <Show when={open()}>
      <div class="scrim lc-scrim">
      <div class="dialog lc-dialog" role="dialog" aria-modal="true" aria-labelledby="lc-title">
        <div class="badge"><Icon name="monitor" size={24} active /></div>
        <h2 id="lc-title">Which layout do you prefer?</h2>
        <p class="lc-sub">You can change it any time in Settings → Appearance → Layout. Both have every tool.</p>
        <div class="lc-options" role="radiogroup" aria-label="Layout">
          <For each={CHOICES}>
            {(c) => (
              <button
                class="lc-option"
                classList={{ on: pick() === c.id }}
                role="radio"
                aria-checked={pick() === c.id}
                data-layout={c.id}
                onClick={() => setPick(c.id)}
                onDblClick={() => done(c.id)}
              >
                <Preview kind={c.id} />
                <span class="lc-name">{c.title}</span>
                <span class="lc-text">{c.text}</span>
              </button>
            )}
          </For>
        </div>
        <div class="lc-actions">
          <button class="btn solid lc-go" onClick={() => done(pick())}>
            Use {pick() === "classic" ? "Classic" : "Modern"}
          </button>
        </div>
      </div>
      </div>
    </Show>
  );
}
