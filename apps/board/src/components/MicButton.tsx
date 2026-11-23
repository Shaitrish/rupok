// MicButton: voice input (speech-to-text) for any text field.
//
//   <MicButton target={() => inputEl} />                 // dictate into a field
//   <MicButton target={() => ({ insert: (t) => … })} />  // or a custom target
//   const mic = useDictation(() => textareaEl);          // headless: mic.toggle()
//
// Click to start and again to stop (or hold it: push-to-talk; Settings ›
// Voice input › "Hold to talk" makes it hold-only). Esc cancels. Alt+V does
// the same from the keyboard in any focused field (VoiceLayer.tsx). The live
// text shows in a bubble by the field; the final text goes in at the caret.
// Renders nothing when voice input is off in Settings.

import { Show, createMemo, splitProps } from "solid-js";
import {
  type DictationOptions, type DictationTarget, activeLang, isDictating, level, startDictation, stopDictation,
  toggleDictation, toggleDictationLang, voicePrefs, voiceStatus,
} from "../voice/dictation";
import { VOICE_LANGS } from "../voice/types";
import "../voice/voice.css";

const LONG_PRESS_MS = 450;

/** Headless dictation for one target: state and actions for custom UIs. */
export function useDictation(target: () => DictationTarget | null | undefined, opts?: () => DictationOptions) {
  const t = () => target() ?? null;
  const active = createMemo(() => {
    const x = t();
    return !!x && isDictating(x);
  });
  return {
    /** Dictating into this target. */
    active,
    /** "idle" | "listening" | "finishing" for this target. */
    status: () => (active() ? voiceStatus() : "idle"),
    /** Microphone level 0–1 (offline Whisper only; 0 otherwise). */
    level: () => (active() ? level() : 0),
    start: () => startDictation(t(), opts?.()),
    stop: () => active() && stopDictation(),
    toggle: () => toggleDictation(t(), opts?.()),
    enabled: () => voicePrefs.read().enabled,
  };
}

export interface MicButtonProps {
  /** The field to dictate into (an input, textarea or contentEditable), or a
   * custom `{ insert(text), interim?(text), element? }`. Omit to use the
   * focused field. */
  target?: () => DictationTarget | null | undefined;
  /** Options per session: language, anchor, onFinal, onEnd. */
  options?: () => DictationOptions;
  /** Button size in px (default 32; at least 40 on touch layouts). */
  size?: number;
  /** Show the EN / বাং quick language toggle next to the mic. */
  showLang?: boolean;
  class?: string;
  /** Accessible name (default "Dictate"). */
  label?: string;
}

export function MicIcon(props: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={props.size ?? 18} height={props.size ?? 18} aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
      <rect x="8.5" y="2.75" width="7" height="12" rx="3.5" />
      <path d="M5 11.25a7 7 0 0 0 14 0M12 18.25v3" />
    </svg>
  );
}

export function MicButton(props: MicButtonProps) {
  const [local] = splitProps(props, ["target", "options", "size", "showLang", "class", "label"]);
  const mic = useDictation(() => local.target?.() ?? null, local.options);
  let downAt = 0;
  let startedHere = false;
  const size = () => local.size ?? 32;
  const tip = () => {
    const hold = voicePrefs.read().mode === "hold";
    return mic.active() ? "Stop dictation  ·  Esc cancels" : `${hold ? "Hold to dictate" : "Dictate"}  ·  Alt V`;
  };
  // Keep the focus (and caret) in the field: a press on the mic must not blur it.
  const keepFocus = (e: Event) => e.preventDefault();
  const onDown = (e: PointerEvent) => {
    keepFocus(e);
    if (e.button !== 0) return;
    downAt = performance.now();
    startedHere = false;
    if (!mic.active()) {
      startedHere = mic.start();
    }
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  };
  const onUp = () => {
    if (!downAt) return;
    const held = performance.now() - downAt;
    downAt = 0;
    const hold = voicePrefs.read().mode === "hold";
    if (startedHere) {
      // Push-to-talk: a long press (or hold mode) stops on release.
      if (hold || held >= LONG_PRESS_MS) mic.stop();
    } else if (mic.active()) mic.stop();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      mic.toggle();
    }
  };
  const langShort = () => VOICE_LANGS.find((l) => l.value === activeLang())?.short ?? "EN";
  return (
    <Show when={mic.enabled()}>
      <span class={`mic-wrap ${local.class ?? ""}`} onMouseDown={keepFocus}>
        <button
          type="button"
          class="mic-btn"
          classList={{ on: mic.active(), finishing: mic.status() === "finishing" }}
          style={{ width: `${size()}px`, height: `${size()}px`, "--mic-level": String(mic.level()) }}
          aria-label={local.label ?? "Dictate"}
          aria-pressed={mic.active()}
          title={tip()}
          data-mic
          onPointerDown={onDown}
          onPointerUp={onUp}
          onPointerCancel={onUp}
          onKeyDown={onKey}
          onClick={(e) => e.preventDefault()}
        >
          <MicIcon size={Math.round(size() * 0.56)} />
        </button>
        <Show when={local.showLang}>
          <button
            type="button"
            class="mic-lang"
            title="Dictation language: English / বাংলা"
            aria-label={`Dictation language: ${langShort()}. Switch`}
            onPointerDown={keepFocus}
            onClick={() => toggleDictationLang()}
          >
            {langShort()}
          </button>
        </Show>
      </span>
    </Show>
  );
}

export default MicButton;
