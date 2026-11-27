// Voice input session state and the public API: startDictation(target),
// stopDictation(), cancelDictation(), toggleDictation(target).
//
// One dictation runs at a time. The target is an <input>, <textarea> or
// contentEditable element (text goes in at the caret, as if typed), or a
// custom target `{ insert(text), interim?(text) }` for features that are not
// a text field (e.g. Solve this). The UI (MicButton, the live-text bubble,
// the OS dictation hint, Alt+V) is in MicButton.tsx and VoiceLayer.tsx.

import { createMemo, createSignal } from "solid-js";
import { isDesktop } from "../platform";
import { jsonStore } from "../settings/bind";
import { showToast } from "../state";
import { caretContext, commitField, type EditableEl, fieldValue, focusForDictation, focusedEditable, insertText, isEditable } from "./insert";
import { type MicAdvice, gatherMicFacts, micAdvice } from "./diagnose";
import { joinDictation } from "./punctuation";
import { type VoiceEnv, audioPathShort, audioPathText, chooseProvider, detectPlatform, lacksSpeechService } from "./select";
import type { DictationSession, ProviderChoice, ProviderPref, VoiceError, VoiceLang, VoiceProvider } from "./types";
import { speechCtor, webSpeechProvider } from "./webspeech";
import { type WhisperStatus, makeWhisperProvider, whisperStatus } from "./whisper";

// ---------------------------------------------------------------- settings

export interface VoicePrefs {
  enabled: boolean;
  provider: ProviderPref;
  lang: VoiceLang;
  autoPunctuation: boolean;
  /** "toggle": click to start and again to stop (a long press also works as
   * push-to-talk). "hold": listen only while the mic or Alt+V is held. */
  mode: "toggle" | "hold";
  /** Preferred offline model ("tiny" | "base" | "small"). */
  model: string;
}

export const voicePrefs = jsonStore<VoicePrefs>("rupok.voice", {
  enabled: true,
  provider: "auto",
  lang: "en-US",
  autoPunctuation: true,
  mode: "toggle",
  model: "base",
});

// ------------------------------------------------------------- environment

const [whisper, setWhisper] = createSignal<WhisperStatus | null>(null);
const [speechBroken, setSpeechBroken] = createSignal(false);
export { whisper };

/** Ask the desktop shell which offline models are installed. */
export async function refreshWhisper(): Promise<WhisperStatus | null> {
  if (!isDesktop) return null;
  try {
    const s = await whisperStatus();
    setWhisper(s);
    return s;
  } catch {
    setWhisper(null); // an older shell without the voice commands
    return null;
  }
}
if (isDesktop) void refreshWhisper();

/** The installed model to use: the preferred one, else any installed. */
export const whisperModel = createMemo(() => {
  const s = whisper();
  if (!s?.built) return null;
  const inst = s.models.filter((m) => m.installed).map((m) => m.id);
  return inst.includes(voicePrefs.read().model) ? voicePrefs.read().model : inst[0] ?? null;
});

const plat = typeof navigator !== "undefined" ? detectPlatform(navigator.userAgent, navigator.platform) : { os: "other" as const, engine: "other" as const, edge: false };
const noSpeechService = typeof navigator !== "undefined" && (
  !!(navigator as any).brave ||
  lacksSpeechService(navigator.userAgent, (navigator as any).userAgentData?.brands?.map((b: { brand: string }) => b.brand) ?? null, plat.os)
);

export const voiceEnv = createMemo<VoiceEnv>(() => ({
  ...plat,
  desktop: isDesktop,
  speechApi: typeof window !== "undefined" && !!speechCtor(),
  whisperBuilt: !!whisper()?.built,
  whisperModel: whisperModel(),
  whisperCpuOk: whisper()?.cpu_ok !== false,
  speechBroken: speechBroken(),
  noSpeechService,
}));

/** The provider dictation will use now. */
export const voiceChoice = createMemo<ProviderChoice>(() => chooseProvider(voiceEnv(), voicePrefs.read().provider));
export const privacyText = () => audioPathText(voiceChoice(), voiceEnv());
export const privacyShort = () => audioPathShort(voiceChoice(), voiceEnv());

let whisperProvider: VoiceProvider | null = null;
function provider(choice: ProviderChoice): VoiceProvider | null {
  if (choice === "webspeech") return webSpeechProvider;
  if (choice === "whisper") return (whisperProvider ??= makeWhisperProvider(() => whisperModel()));
  return null;
}

// ------------------------------------------------------------------ session

/** A custom target for features that are not a text field. */
export interface CustomTarget {
  insert(text: string): void;
  /** Live text while speaking ("" clears it). */
  interim?(text: string): void;
  /** Where the live-text bubble and the hint are shown. */
  element?: HTMLElement | null;
  /** The text already there, so new words get a space and sentence
   * capitals the way they do in a field (default: none). */
  before?(): string;
}
export type DictationTarget = EditableEl | CustomTarget;

export interface DictationOptions {
  lang?: VoiceLang;
  /** The element to anchor the bubble/hint to (default: the target). */
  anchor?: HTMLElement | null;
  /** Called with each piece of text as it is inserted. */
  onFinal?(text: string): void;
  /** Called when the session ends (stopped, cancelled or failed). */
  onEnd?(reason: "stopped" | "cancelled" | "error"): void;
  /** Show the live-text bubble (default true). Voice text on the board
   * shows the words in its own box. */
  bubble?: boolean;
}

export type VoiceStatus = "idle" | "listening" | "finishing";

const [status, setStatus] = createSignal<VoiceStatus>("idle");
const [interim, setInterim] = createSignal("");
const [level, setLevel] = createSignal(0);
const [activeTarget, setActiveTarget] = createSignal<DictationTarget | null>(null);
const [activeAnchor, setActiveAnchor] = createSignal<HTMLElement | null>(null);
const [activeLang, setActiveLang] = createSignal<VoiceLang>(voicePrefs.read().lang);
const [lastError, setLastError] = createSignal<VoiceError | null>(null);
/** The OS-dictation hint popover, anchored to an element. It also offers
 * the offline voice download ("setup") and explains a microphone failure
 * (`problem`). */
const [hint, setHint] = createSignal<HTMLElement | null>(null);
/** Why the microphone failed and how to fix it, shown in the hint popover. */
const [problem, setProblem] = createSignal<MicAdvice | null>(null);
export { problem as voiceProblem };

/** The live-text bubble is wanted for the running session. */
const [bubbleOn, setBubbleOn] = createSignal(true);

export { activeAnchor, activeLang, activeTarget, bubbleOn, hint, interim, lastError, level, setHint, status as voiceStatus };

let session: DictationSession | null = null;
let sessionOpts: DictationOptions = {};
/** The last start, for "Try again" in the hint popover. */
let lastAttempt: { target: DictationTarget; opts: DictationOptions } | null = null;
let endReason: "stopped" | "cancelled" | "error" = "stopped";
let valueAtStart = "";
let unbindBlur: (() => void) | null = null;

const isCustom = (t: DictationTarget): t is CustomTarget => typeof (t as CustomTarget).insert === "function" && !(t instanceof HTMLElement);
const targetEl = (t: DictationTarget | null): HTMLElement | null => (!t ? null : isCustom(t) ? t.element ?? null : t);

/** True while dictating (into `target`, when given). */
export function isDictating(target?: DictationTarget | null): boolean {
  if (status() === "idle") return false;
  return target === undefined || activeTarget() === target;
}

/** Resolve "the field the teacher means": the given target, the focused
 * field, or the board text box being edited. */
export function resolveTarget(target?: DictationTarget | null): DictationTarget | null {
  if (target) return target;
  const f = focusedEditable();
  if (f) return f;
  const editing = document.querySelector<HTMLElement>(".item.text[contenteditable='true']");
  return editing && isEditable(editing) ? editing : null;
}

function deliver(text: string) {
  const t = activeTarget();
  if (!t) return;
  const p = voicePrefs.read();
  if (isCustom(t)) {
    const out = joinDictation(t.before?.() ?? "", text, "", { lang: activeLang(), autoPunctuation: p.autoPunctuation });
    if (out) { t.insert(out); sessionOpts.onFinal?.(out); }
    return;
  }
  if (!t.isConnected) return;
  const { before, after } = caretContext(t);
  const out = joinDictation(before, text, after, { lang: activeLang(), autoPunctuation: p.autoPunctuation });
  if (!out) return;
  insertText(t, out);
  sessionOpts.onFinal?.(out);
}

/**
 * Start dictating into `target` (default: the focused field or the text box
 * being edited). Returns false when nothing started: voice input is off, no
 * target, or no provider (then the OS dictation hint is shown instead).
 */
export function startDictation(target?: DictationTarget | null, opts: DictationOptions = {}): boolean {
  const p = voicePrefs.read();
  if (!p.enabled) return false;
  const t = resolveTarget(target);
  if (!t) {
    showToast("Click in a text field first, then press the mic or Alt+V");
    return false;
  }
  if (session) cancelDictation();
  const el = targetEl(t);
  if (!isCustom(t)) focusForDictation(t);
  lastAttempt = { target: t, opts };
  const choice = voiceChoice();
  const prov = provider(choice);
  setProblem(null);
  if (!prov) {
    // No engine: the OS's own dictation types into the focused field, or
    // (desktop) the offline voice download is offered.
    setHint(opts.anchor ?? el ?? document.body);
    return false;
  }
  setHint(null);
  sessionOpts = opts;
  endReason = "stopped";
  const lang = opts.lang ?? activeLang();
  setActiveLang(lang);
  setActiveTarget(() => t);
  setActiveAnchor(opts.anchor ?? el);
  setBubbleOn(opts.bubble !== false);
  setLastError(null);
  setInterim("");
  setLevel(0);
  setStatus("listening");
  valueAtStart = !isCustom(t) ? fieldValue(t) : "";
  if (!isCustom(t)) {
    const onBlur = () => queueMicrotask(() => {
      // Focus moved away (e.g. a press on the board ended text editing).
      if (activeTarget() === t && document.activeElement !== t && !t.contains(document.activeElement)) stopDictation();
    });
    t.addEventListener("blur", onBlur);
    unbindBlur = () => t.removeEventListener("blur", onBlur);
  }
  const mine = prov.start({ lang }, {
    onInterim: (s) => {
      if (session !== mine) return;
      setInterim(s);
      const ct = activeTarget();
      if (ct && isCustom(ct)) ct.interim?.(s);
    },
    onFinal: (s) => { if (session === mine || status() === "finishing") deliver(s); },
    onLevel: (v) => setLevel(v),
    onError: (err) => {
      endReason = "error";
      setLastError(err);
      const anchor = opts.anchor ?? el ?? document.body;
      if (err.code === "unsupported" && choice === "webspeech") {
        setSpeechBroken(true);
        setHint(anchor);
      } else if (err.code === "not-allowed" || err.code === "no-mic" || err.code === "busy" || err.code === "no-api") {
        // Say exactly why and how to fix it (the site setting, the OS
        // privacy switch, no device…), next to the mic.
        void explainFailure(err, anchor);
      } else if (err.code === "network" && choice === "webspeech") {
        showToast(navigator.onLine === false
          ? "No internet: the browser's speech recognition needs it. Reconnect, or use the Rupok desktop app (offline voice)."
          : "The speech service did not answer. Check the internet connection and tap the mic again.", 5000);
      } else showToast(err.message, 4000);
    },
    onEnd: () => finish(mine),
  });
  session = mine;
  return true;
}

/** Find out why the microphone failed and show the fix by the mic. */
async function explainFailure(err: VoiceError, anchor: HTMLElement) {
  const facts = await gatherMicFacts({ ...plat, desktop: isDesktop }, err.code, err.detail);
  setProblem(micAdvice(facts));
  setHint(anchor.isConnected ? anchor : document.body);
}

/** "Try again" from the hint popover: runs inside that click, so the
 * browser may ask for the microphone again. */
export function retryDictation(): boolean {
  const a = lastAttempt;
  setHint(null);
  setProblem(null);
  if (!a) return false;
  const el = targetEl(a.target);
  if (el && !el.isConnected) return false;
  return startDictation(a.target, a.opts);
}

/** Close the hint popover (and any problem it explains). */
export function closeHint() {
  setHint(null);
  setProblem(null);
}

function finish(s: DictationSession) {
  if (session !== s) return; // an older session ending late
  const t = activeTarget();
  session = null;
  unbindBlur?.();
  unbindBlur = null;
  if (t && !isCustom(t)) {
    if (endReason !== "cancelled") commitField(t, valueAtStart);
  } else if (t && isCustom(t)) t.interim?.("");
  setStatus("idle");
  setInterim("");
  setLevel(0);
  setActiveTarget(null);
  setActiveAnchor(null);
  const cb = sessionOpts.onEnd;
  sessionOpts = {};
  cb?.(endReason);
}

/** Stop listening and insert what was heard. */
export function stopDictation() {
  if (!session || status() !== "listening") return;
  setStatus("finishing");
  session.stop();
}

/** Stop and discard (Esc). Text already inserted stays (undo removes it). */
export function cancelDictation() {
  if (!session) return;
  endReason = "cancelled";
  const s = session;
  s.cancel();
  finish(s);
}

/** Start, or stop when already dictating into this target. */
export function toggleDictation(target?: DictationTarget | null, opts?: DictationOptions): boolean {
  const t = resolveTarget(target);
  if (status() !== "idle" && (!t || activeTarget() === t)) {
    stopDictation();
    return false;
  }
  return startDictation(t, opts);
}

/** Switch the language (the quick toggle). Restarts a running session. */
export function setDictationLang(lang: VoiceLang) {
  setActiveLang(lang);
  const t = activeTarget();
  if (t && session) {
    const opts = sessionOpts;
    const anchor = activeAnchor();
    cancelDictation();
    startDictation(t, { ...opts, anchor, lang });
  }
}

/** Flip between English and Bangla. */
export function toggleDictationLang() {
  setDictationLang(activeLang().startsWith("bn") ? (voicePrefs.read().lang.startsWith("en") ? voicePrefs.read().lang : "en-US") : "bn-BD");
}

/** Follow the default language from Settings. */
export function setDefaultLang(lang: VoiceLang) {
  voicePrefs.write({ lang });
  if (status() === "idle") setActiveLang(lang);
}
