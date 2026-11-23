// The app-wide voice input layer (mounted once in App.tsx):
//   - Alt+V in any focused field: tap to start/stop, hold for push-to-talk;
//   - Esc cancels a running dictation;
//   - the live-text bubble next to the field being dictated into;
//   - the OS dictation hint when no speech provider exists here;
//   - the mic chip next to a board text box while it is being edited.

import { For, Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { MicButton } from "../components/MicButton";
import { openSettings } from "../settings/open";
import { editingText } from "../state";
import {
  activeAnchor, activeLang, cancelDictation, hint, interim, privacyShort, privacyText, setDictationLang, setHint,
  startDictation, stopDictation, voiceEnv, voicePrefs, voiceStatus,
} from "./dictation";
import { osDictationHint } from "./select";
import { VOICE_LANGS, type VoiceLang } from "./types";
import "./voice.css";

const HOLD_MS = 450;

/** Track an element's screen rectangle while `el()` is set. */
function useRect(el: () => HTMLElement | null | undefined) {
  const [rect, setRect] = createSignal<DOMRect | null>(null);
  let raf = 0;
  const loop = () => {
    const e = el();
    if (!e || !e.isConnected) { setRect(null); raf = 0; return; }
    const r = e.getBoundingClientRect();
    const p = rect();
    if (!p || p.x !== r.x || p.y !== r.y || p.width !== r.width || p.height !== r.height) setRect(r);
    raf = requestAnimationFrame(loop);
  };
  createEffect(() => {
    if (el()) { if (!raf) raf = requestAnimationFrame(loop); }
    else { cancelAnimationFrame(raf); raf = 0; setRect(null); }
  });
  onCleanup(() => cancelAnimationFrame(raf));
  return rect;
}

/** Place a box of about `w`×`h` below the anchor (above when there is no
 * room), inside the window. */
function place(r: DOMRect, w: number, h: number) {
  const vw = window.innerWidth, vh = window.innerHeight;
  const left = Math.max(8, Math.min(vw - w - 8, r.left));
  const below = r.bottom + 8;
  const top = below + h > vh - 8 ? Math.max(8, r.top - h - 8) : below;
  return { left: `${left}px`, top: `${top}px` };
}

function Bubble() {
  const rect = useRect(() => (voiceStatus() !== "idle" ? activeAnchor() ?? document.body : null));
  const listening = () => voiceStatus() === "listening";
  const lang = () => activeLang();
  return (
    <Show when={voiceStatus() !== "idle" && rect()}>
      {(r) => (
        <div class="voice-bubble surface" role="status" aria-live="polite" style={place(r(), 360, 96)} onMouseDown={(e) => e.preventDefault()}>
          <div class="voice-bubble-row">
            <span class="voice-dot" classList={{ on: listening() }} aria-hidden="true" />
            <span class="voice-text" classList={{ placeholder: !interim() }}>
              {interim() || (listening() ? (lang().startsWith("bn") ? "বলুন… (Listening)" : "Listening…") : "Finishing…")}
            </span>
          </div>
          <div class="voice-bubble-foot">
            <div class="voice-langs" role="radiogroup" aria-label="Dictation language">
              <For each={VOICE_LANGS.filter((l) => l.value === "en-US" || l.value === "bn-BD" || l.value === voicePrefs.read().lang)}>
                {(l) => (
                  <button
                    type="button"
                    role="radio"
                    aria-checked={lang() === l.value}
                    classList={{ on: lang() === l.value }}
                    onClick={() => setDictationLang(l.value as VoiceLang)}
                  >
                    {l.short}
                  </button>
                )}
              </For>
            </div>
            <span class="voice-privacy" title={privacyText()}>{privacyShort()}</span>
            <span class="grow" />
            <button type="button" class="voice-link" onClick={() => stopDictation()}>Done</button>
            <span class="voice-esc"><kbd>Esc</kbd> cancel</span>
          </div>
        </div>
      )}
    </Show>
  );
}

function Hint() {
  const rect = useRect(() => hint());
  const info = () => osDictationHint(voiceEnv().os);
  const canWhisper = () => voiceEnv().desktop && voiceEnv().whisperBuilt && !voiceEnv().whisperModel;
  let box!: HTMLDivElement;
  onMount(() => {
    const away = (e: PointerEvent) => {
      if (hint() && box && !box.contains(e.target as Node) && !(e.target as HTMLElement)?.closest?.("[data-mic]")) setHint(null);
    };
    window.addEventListener("pointerdown", away, true);
    onCleanup(() => window.removeEventListener("pointerdown", away, true));
  });
  return (
    <Show when={hint() && rect()}>
      {(r) => (
        <div ref={box} class="voice-hint surface" role="dialog" aria-label="Voice typing" style={place(r(), 340, 170)} onMouseDown={(e) => e.preventDefault()}>
          <div class="voice-hint-title">Speak instead of typing</div>
          <p>{info().text}</p>
          <Show when={info().keys.length}>
            <div class="voice-hint-keys">
              <For each={info().keys}>{(k, i) => <>{i() > 0 && <span>+</span>}<kbd>{k}</kbd></>}</For>
            </div>
          </Show>
          <div class="voice-hint-actions">
            <Show when={canWhisper()}>
              <button type="button" class="btn soft" onClick={() => { setHint(null); openSettings("general.voiceModel"); }}>Set up offline voice</button>
            </Show>
            <button type="button" class="btn" onClick={() => { setHint(null); openSettings("general.voiceInput"); }}>Voice settings</button>
            <span class="grow" />
            <button type="button" class="btn solid" onClick={() => setHint(null)}>OK</button>
          </div>
        </div>
      )}
    </Show>
  );
}

/** The mic chip by a board text box while it is being edited. */
function TextChip() {
  const el = () => {
    const id = editingText();
    return id ? document.querySelector<HTMLElement>(`.item.text[data-item="${CSS.escape(id)}"]`) : null;
  };
  // The text box element appears a frame after editingText is set.
  const [node, setNode] = createSignal<HTMLElement | null>(null);
  createEffect(() => {
    const id = editingText();
    if (!id) { setNode(null); return; }
    queueMicrotask(() => setNode(el()));
    requestAnimationFrame(() => setNode(el()));
  });
  const rect = useRect(node);
  return (
    <Show when={voicePrefs.read().enabled && node() && rect()}>
      {(r) => (
        <div
          class="voice-textchip surface"
          style={{ left: `${Math.min(window.innerWidth - 96, r().right + 8)}px`, top: `${Math.max(8, r().top + r().height / 2 - 22)}px` }}
          onPointerDown={(e) => { e.preventDefault(); e.stopPropagation(); }}
          onMouseDown={(e) => e.preventDefault()}
        >
          <MicButton target={() => node()} size={36} showLang label="Dictate into the text box" />
        </div>
      )}
    </Show>
  );
}

export default function VoiceLayer() {
  onMount(() => {
    let keyStart = 0;
    let startedByKey = false;
    const down = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (voiceStatus() !== "idle") {
          e.preventDefault();
          e.stopImmediatePropagation();
          cancelDictation();
        } else if (hint()) {
          e.preventDefault();
          e.stopImmediatePropagation();
          setHint(null);
        }
        return;
      }
      if (e.code === "KeyV" && e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        if (!voicePrefs.read().enabled) return;
        e.preventDefault(); // Option+V would type "√" on a Mac
        e.stopImmediatePropagation();
        if (e.repeat) return;
        if (voiceStatus() !== "idle") {
          startedByKey = false;
          stopDictation();
          return;
        }
        keyStart = performance.now();
        startedByKey = startDictation();
      }
    };
    const up = (e: KeyboardEvent) => {
      if (!startedByKey || !(e.code === "KeyV" || e.key === "Alt")) return;
      startedByKey = false;
      const held = performance.now() - keyStart;
      if (voicePrefs.read().mode === "hold" || held >= HOLD_MS) stopDictation();
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    onCleanup(() => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    });
  });
  return (
    <>
      <TextChip />
      <Bubble />
      <Hint />
    </>
  );
}
