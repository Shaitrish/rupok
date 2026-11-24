// The stamp editor: a small sheet to make or change one of "My stamps":
// preview, name, artwork (emoji, a label or a picture as SVG), colour, size,
// and its behaviour (animation, burst, counter, sound, fade). Smart stamps
// (logic written by the teacher's AI, run in the Smart Object sandbox) keep
// their code; here the teacher changes their name, size and behaviour.
//
// Saving checks the definition in the core (customStamps.saveStamp →
// Board.check_stamp_json) and, when the open lesson uses the stamp, updates
// the lesson's copy too as one undo step.

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { deleteStamp, moveStamp, playSound, saveStamp } from "../customStamps";
import { type CustomStamp, type StampArt, type StampBehavior, lessonStamps, myStamps, setStampEditor, stampEditor } from "../stampLib";
import { toggleFavorite } from "../stamps";
import { prefs, showToast } from "../state";
import { CustomIcon } from "./Stamps";
import { Icon } from "./Icon";
import "./StampEditor.css";

const COLORS = ["#f5b301", "#30a46c", "#e5484d", "#3e63dd", "#f76b15", "#0d9488", "#d6409f", "#1c2024"];
const ANIMS: [NonNullable<StampBehavior["animation"]>, string][] = [["none", "None"], ["pop", "Pop"], ["sparkle", "Sparkle"], ["bounce", "Bounce"], ["spin", "Spin"], ["fade-in", "Fade in"]];
const BURSTS: [NonNullable<StampBehavior["burst"]>, string][] = [["none", "None"], ["stars", "Stars"], ["confetti", "Confetti"], ["hearts", "Hearts"], ["sparkles", "Sparkles"]];
const SOUNDS: [NonNullable<StampBehavior["sound"]>, string][] = [["none", "None"], ["chime", "Chime"], ["pop", "Pop"], ["ding", "Ding"], ["tada", "Ta-da"], ["clap", "Clap"], ["whoosh", "Whoosh"]];
const FADES: [number, string][] = [[0, "Never"], [3, "3 s"], [5, "5 s"], [10, "10 s"], [30, "30 s"]];

export function StampEditorHost() {
  return (
    <Show when={stampEditor()} keyed>
      {(req) => <StampEditor id={req.id} />}
    </Show>
  );
}

function StampEditor(props: { id?: string }) {
  const orig = myStamps().find((s) => s.id === props.id) ?? lessonStamps()[props.id ?? ""];
  const [d, setD] = createSignal<CustomStamp>(
    orig ? structuredClone(orig) : { id: "", name: "", art: "emoji", emoji: "🦉", color: "#f5b301", size: 96, behavior: { animation: "pop" } },
  );
  const [error, setError] = createSignal<string | null>(null);
  const [replay, setReplay] = createSignal(0);
  const set = (p: Partial<CustomStamp>) => setD({ ...d(), ...p });
  const setB = (p: Partial<StampBehavior>) => setD({ ...d(), behavior: { ...(d().behavior ?? {}), ...p } });
  const smart = () => d().art === "smart";
  const fav = () => prefs().stampFavorites.includes(d().id);
  const close = () => setStampEditor(null);
  let sheet!: HTMLDivElement;

  onMount(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && (e.preventDefault(), close());
    window.addEventListener("keydown", key);
    onCleanup(() => window.removeEventListener("keydown", key));
    queueMicrotask(() => sheet.querySelector<HTMLInputElement>("#se-name")?.focus());
  });

  const save = async () => {
    setError(null);
    try {
      const clean = await saveStamp({ ...d(), id: d().id || undefined });
      // The open lesson's copy follows, so placed stamps change too.
      if (lessonStamps()[clean.id]) {
        const { html: _h, ...rest } = clean;
        try {
          controller.board.live_apply(JSON.stringify({ op: "update_stamp", ...rest }), "");
          controller.sync(true);
        } catch { /* the library copy is saved; the lesson keeps its own */ }
      }
      showToast(`Saved “${clean.name}” in My stamps`);
      close();
    } catch (e: any) {
      setError(String(e?.message ?? e));
    }
  };
  const remove = async () => {
    if (!confirm(`Delete “${d().name}” from My stamps? Lessons that use it keep it.`)) return;
    await deleteStamp(d().id);
    close();
  };
  const loadSvg = () => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = ".svg,image/svg+xml";
    i.onchange = async () => {
      const f = i.files?.[0];
      if (f) set({ svg: await f.text(), art: "svg" });
    };
    i.click();
  };
  const preview = createMemo(() => ({ ...d(), name: d().name || "Stamp" }));
  const play = () => {
    setReplay((n) => n + 1);
    playSound(d().behavior?.sound);
  };

  const Chips = <T extends string | number>(p: { label: string; value: T; options: [T, string][]; onPick: (v: T) => void; extra?: (v: T) => void }) => (
    <div class="se-field">
      <span class="se-label" id={`se-${p.label}`}>{p.label}</span>
      <div class="se-chips" role="radiogroup" aria-labelledby={`se-${p.label}`}>
        <For each={p.options}>
          {([v, l]) => (
            <button role="radio" aria-checked={p.value === v} classList={{ on: p.value === v }} onClick={() => (p.onPick(v), p.extra?.(v))}>
              {l}
            </button>
          )}
        </For>
      </div>
    </div>
  );

  return (
    <div class="se-backdrop" onPointerDown={(e) => e.target === e.currentTarget && close()}>
      <div class="se-sheet" ref={sheet} role="dialog" aria-modal="true" aria-labelledby="se-title">
        <header class="se-head">
          <h2 id="se-title">{orig ? "Edit stamp" : "New stamp"}</h2>
          <div class="grow" />
          <button class="btn icon se-close" onClick={close} aria-label="Close"><Icon name="close" /></button>
        </header>
        <div class="se-body">
          <div class="se-preview" classList={{ dark: false }}>
            <Show when={replay() >= 0} keyed>
              <div class={`se-preview-art anim-${d().behavior?.animation ?? "pop"}`} data-n={replay()}>
                <CustomIcon stamp={preview()} size={Math.min(140, Math.max(56, d().size ?? 96))} />
                <Show when={d().behavior?.counter}>
                  <span class="stamp-count se-count">1</span>
                </Show>
              </div>
            </Show>
            <button class="btn se-play" onClick={play} title="Play the animation and sound">
              <Icon name="play" size={18} /><span>Try it</span>
            </button>
          </div>

          <div class="se-field">
            <label class="se-label" for="se-name">Name</label>
            <input id="se-name" class="input" maxLength={40} placeholder="e.g. Super star" value={d().name} onInput={(e) => set({ name: e.currentTarget.value })} />
          </div>
          <div class="se-field">
            <label class="se-label" for="se-kw">Search words</label>
            <input
              id="se-kw"
              class="input"
              placeholder="praise, gold, well done"
              value={(d().keywords ?? []).join(", ")}
              onInput={(e) => set({ keywords: e.currentTarget.value.split(",").map((k) => k.trim()).filter(Boolean).slice(0, 12) })}
            />
          </div>

          <Show
            when={!smart()}
            fallback={
              <p class="se-note">
                <Icon name="object" size={18} />
                <span>A Smart stamp: its logic was written by your AI and runs safely in the Smart Object sandbox (offline, no network). Ask the AI to change what it does; here you can rename it and change its size and behaviour.</span>
              </p>
            }
          >
            <Chips<StampArt>
              label="Picture"
              value={d().art}
              options={[["emoji", "Emoji"], ["text", "Label"], ["svg", "Drawing (SVG)"]]}
              onPick={(v) => set({ art: v })}
            />
            <Show when={d().art === "emoji"}>
              <div class="se-field">
                <label class="se-label" for="se-emoji">Emoji</label>
                <input id="se-emoji" class="input se-emoji" maxLength={16} value={d().emoji ?? ""} onInput={(e) => set({ emoji: e.currentTarget.value })} />
              </div>
            </Show>
            <Show when={d().art === "text"}>
              <div class="se-field">
                <label class="se-label" for="se-text">Label</label>
                <input id="se-text" class="input" maxLength={24} placeholder="Well done!" value={d().text ?? ""} onInput={(e) => set({ text: e.currentTarget.value })} />
              </div>
            </Show>
            <Show when={d().art === "svg"}>
              <div class="se-field">
                <label class="se-label" for="se-svg">SVG</label>
                <textarea
                  id="se-svg"
                  class="input se-svg"
                  rows={4}
                  spellcheck={false}
                  placeholder={'<svg viewBox="0 0 100 100">…</svg>  (fill="currentColor" takes the colour)'}
                  value={d().svg ?? ""}
                  onInput={(e) => set({ svg: e.currentTarget.value })}
                />
                <button class="btn" onClick={loadSvg}><Icon name="open" size={18} /><span>Open an .svg file</span></button>
              </div>
            </Show>
            <div class="se-field">
              <span class="se-label" id="se-colour">Colour</span>
              <div class="se-colors" role="radiogroup" aria-labelledby="se-colour">
                <For each={COLORS}>
                  {(c) => (
                    <button
                      role="radio"
                      aria-checked={d().color === c}
                      aria-label={`Colour ${c}`}
                      classList={{ on: d().color === c }}
                      style={{ "--c": c }}
                      onClick={() => set({ color: c })}
                    />
                  )}
                </For>
                <input type="color" aria-label="Any colour" value={d().color ?? "#f5b301"} onInput={(e) => set({ color: e.currentTarget.value })} />
              </div>
            </div>
          </Show>

          <div class="se-field">
            <label class="se-label" for="se-size">Size · {Math.round(d().size ?? (smart() ? d().height ?? 200 : 96))} px</label>
            <input id="se-size" class="slider" type="range" min="48" max="320" step="4" value={d().size ?? (smart() ? d().height ?? 200 : 96)} onInput={(e) => set({ size: Number(e.currentTarget.value) })} />
          </div>

          <Chips label="Animation" value={d().behavior?.animation ?? "pop"} options={ANIMS} onPick={(v) => setB({ animation: v })} extra={() => setReplay((n) => n + 1)} />
          <Chips label="Burst" value={d().behavior?.burst ?? "none"} options={BURSTS} onPick={(v) => setB({ burst: v })} />
          <Chips label="Sound" value={d().behavior?.sound ?? "none"} options={SOUNDS} onPick={(v) => setB({ sound: v })} extra={(v) => playSound(v)} />
          <Show when={!prefs().stampSounds && (d().behavior?.sound ?? "none") !== "none"}>
            <p class="se-hint">Stamp sounds are off in Settings → Pen &amp; ink → Stamps.</p>
          </Show>
          <Chips label="Fade away" value={d().behavior?.fade_after ?? 0} options={FADES} onPick={(v) => setB({ fade_after: v || undefined })} />
          <label class="check se-check">
            <input type="checkbox" checked={!!d().behavior?.counter} onChange={(e) => setB({ counter: e.currentTarget.checked || undefined })} />
            <span>Count them: a number badge (1, 2, 3…) on each one on the page</span>
          </label>

          <Show when={error()}>
            <p class="se-error" role="alert"><Icon name="info" size={16} />{error()}</p>
          </Show>

          <Show when={orig && myStamps().some((s) => s.id === d().id)}>
            <div class="se-manage">
              <button class="btn" aria-pressed={fav()} onClick={() => toggleFavorite(d().id)}>
                <Icon name="heart" size={18} active={fav()} /><span>{fav() ? "In the quick menu" : "Add to the quick menu"}</span>
              </button>
              <button class="btn" onClick={() => moveStamp(d().id, -1)} aria-label="Move earlier in My stamps"><Icon name="prev" size={18} /><span>Earlier</span></button>
              <button class="btn" onClick={() => moveStamp(d().id, 1)} aria-label="Move later in My stamps"><span>Later</span><Icon name="next" size={18} /></button>
            </div>
          </Show>
        </div>
        <footer class="se-foot">
          <Show when={orig && myStamps().some((s) => s.id === d().id)}>
            <button class="btn danger" onClick={remove}><Icon name="trash" size={18} /><span>Delete</span></button>
          </Show>
          <div class="grow" />
          <button class="btn" onClick={close}>Cancel</button>
          <button class="btn solid" onClick={save} disabled={!d().name.trim()}>Save</button>
        </footer>
      </div>
    </div>
  );
}
