// Settings rows for voice input that need their own UI: the privacy notice
// (which provider sends audio where), the offline model download, and
// "Test microphone" (provider, permission, input level, a 3-second
// transcript).

import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import type { SettingProps } from "../settings/registry";
import { isDesktop } from "../platform";
import {
  activeLang, closeHint, lastError, refreshWhisper, startDictation, stopDictation, voiceChoice, voiceEnv, voicePrefs, voiceStatus, whisper,
  whisperModel,
} from "./dictation";
import { type LevelProbe, type MicAdvice, type MicPermission, gatherMicFacts, mediaErrorCode, micAdvice, micLevelProbe, micPermission } from "./diagnose";
import { downloadVoiceModel, mb as MB, modelError, modelProgress, stopVoiceModelDownload } from "./model";
import { audioPathShort, audioPathText, detectPlatform } from "./select";
import { removeModel } from "./whisper";
const MODEL_INFO: Record<string, string> = {
  tiny: "Fastest, rough Bangla",
  base: "Recommended: good English, fair Bangla",
  small: "Best Bangla, needs a fast computer",
};

/** Which provider is used here and where its audio goes. */
export function VoicePrivacy(props: SettingProps) {
  const env = voiceEnv;
  const rows = () => {
    const e = env();
    const out: { name: string; where: string; here: boolean; cloud: boolean }[] = [];
    const engineName = e.edge ? "Edge" : e.engine === "chromium" ? "Chrome" : e.engine === "webkit" ? "Safari / macOS" : "This browser";
    out.push({
      name: `Browser speech (${engineName})`,
      where: e.speechApi && !(e.desktop && e.os !== "mac") ? audioPathText("webspeech", e) : "Not available here.",
      here: voiceChoice() === "webspeech",
      cloud: e.engine === "chromium",
    });
    if (e.desktop) {
      out.push({
        name: "Offline voice (Whisper)",
        where: !e.whisperBuilt ? "Not in this build of the desktop app."
          : e.whisperCpuOk === false ? whisper()?.cpu_note ?? "This processor cannot run it."
          : audioPathText("whisper", e) + (e.whisperModel ? "" : " Download the model below."),
        here: voiceChoice() === "whisper" || voiceChoice() === "setup",
        cloud: false,
      });
    }
    out.push({ name: "Your system's dictation", where: audioPathText("hint", e), here: voiceChoice() === "hint", cloud: false });
    return out;
  };
  return (
    <div class="st-embed voice-privacy-list" role="group" aria-labelledby={props.labelId} aria-describedby={props.descId}>
      <For each={rows()}>
        {(r) => (
          <div class="voice-privacy-row">
            <span class="st-dot" classList={{ live: r.here }} aria-hidden="true" />
            <span class="voice-privacy-name">{r.name}{r.here ? " · in use" : ""}</span>
            <span class="voice-privacy-where" classList={{ cloud: r.cloud && r.here }}>{r.where}</span>
          </div>
        )}
      </For>
      <p class="voice-privacy-note">Rupok never records or stores your voice. The microphone is only on while the mic button is lit.</p>
    </div>
  );
}

/** Download / remove the offline Whisper model (desktop app). */
export function VoiceModel(props: SettingProps) {
  const [asking, setAsking] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  onMount(() => void refreshWhisper());
  const progress = modelProgress;
  const busy = () => progress()?.model ?? null;
  const start = async (id: string) => {
    setAsking(null);
    setError(null);
    await downloadVoiceModel(id);
  };
  const cancelDownload = () => stopVoiceModelDownload();
  const remove = async (id: string) => {
    setError(null);
    try { await removeModel(id); } catch (e: any) { setError(String(e?.message ?? e)); }
    void refreshWhisper();
  };
  return (
    <div class="st-embed voice-models" role="group" aria-labelledby={props.labelId} aria-describedby={props.descId}>
      <Show when={whisper()} fallback={<div class="st-status">Checking…</div>}>
        {(s) => (
          <Show when={s().built} fallback={<div class="st-status">This build of the desktop app has no offline voice.</div>}>
            <Show when={s().cpu_ok === false}><div class="voice-model-error" role="alert">{s().cpu_note}</div></Show>
            <For each={s().models}>
              {(m) => (
                <div class="voice-model-row">
                  <div class="voice-model-text">
                    <span class="voice-model-name">
                      {m.id[0].toUpperCase() + m.id.slice(1)} · {MB(m.bytes)}
                      <Show when={m.installed && whisperModel() === m.id}><span class="voice-model-badge">In use</span></Show>
                    </span>
                    <span class="voice-model-sub">{MODEL_INFO[m.id] ?? ""}</span>
                  </div>
                  <div class="st-inline">
                    <Show
                      when={!m.installed}
                      fallback={
                        <>
                          <Show when={whisperModel() !== m.id}>
                            <button class="st-btn small" onClick={() => voicePrefs.write({ model: m.id })}>Use</button>
                          </Show>
                          <button class="st-btn small danger" onClick={() => void remove(m.id)}>Remove</button>
                        </>
                      }
                    >
                      <Show
                        when={busy() === m.id}
                        fallback={
                          <Show
                            when={asking() === m.id}
                            fallback={<button class="st-btn small" disabled={!!busy() || s().cpu_ok === false} onClick={() => setAsking(m.id)}>Download…</button>}
                          >
                            <span class="voice-consent">Download {MB(m.bytes)} from Hugging Face?</span>
                            <button class="st-btn small solid" onClick={() => void start(m.id)}>Download</button>
                            <button class="st-btn small ghost" onClick={() => setAsking(null)}>Cancel</button>
                          </Show>
                        }
                      >
                        <progress max={progress()?.total || m.bytes} value={progress()?.done ?? 0} aria-label={`Downloading the ${m.id} model`} />
                        <span class="voice-model-sub">{MB(progress()?.done ?? 0)} / {MB(m.bytes)}</span>
                        <button class="st-btn small ghost" onClick={() => void cancelDownload()}>Stop</button>
                      </Show>
                    </Show>
                  </div>
                </div>
              )}
            </For>
            <Show when={error() || modelError()}><div class="voice-model-error" role="alert">{error() || modelError()}</div></Show>
            <p class="voice-privacy-note">Downloaded once, checked against a fixed checksum, kept in the app's data folder. After that, voice input works offline and audio never leaves this computer.</p>
          </Show>
        )}
      </Show>
    </div>
  );
}

const TEST_MS = 3000;
const PERMISSION_TEXT: Record<MicPermission, string> = {
  granted: "Allowed",
  denied: "Blocked for this site",
  prompt: "Not decided yet (the browser will ask)",
  unknown: isDesktop ? "Answered by the app" : "The browser does not say",
};

/** Settings › Voice input › Test microphone: which provider runs, the
 * microphone permission, the input level and what 3 seconds of speech
 * became. Teacher only (the microphone). */
export function VoiceTest(props: SettingProps) {
  const [phase, setPhase] = createSignal<"idle" | "listening" | "finishing" | "done">("idle");
  const [perm, setPerm] = createSignal<MicPermission | null>(null);
  const [level, setLevel] = createSignal(0);
  const [peak, setPeak] = createSignal(0);
  const [text, setText] = createSignal("");
  const [live, setLive] = createSignal("");
  const [advice, setAdvice] = createSignal<MicAdvice | null>(null);
  const [note, setNote] = createSignal<string | null>(null);
  let box!: HTMLDivElement;
  let probe: LevelProbe | null = null;
  let timer: number | undefined;
  const plat = detectPlatform(navigator.userAgent, navigator.platform);
  const env = () => ({ ...plat, desktop: isDesktop });

  onCleanup(() => {
    clearTimeout(timer);
    probe?.stop();
    if (phase() === "listening") stopDictation();
  });

  const settle = async (started: boolean) => {
    setPerm(await micPermission());
    const err = started ? lastError() : null;
    // The fix shows here, not also in the popover by the mic.
    closeHint();
    if (err && !advice()) {
      if (err.code === "not-allowed" || err.code === "no-mic" || err.code === "busy" || err.code === "no-api") {
        setAdvice(micAdvice(await gatherMicFacts(env(), err.code, err.detail)));
      } else setNote(err.message);
    }
    setPhase("done");
  };

  /** Runs inside the click, so the browser may ask for the microphone. */
  const run = () => {
    if (phase() === "listening" || phase() === "finishing") return;
    clearTimeout(timer);
    setPhase("listening");
    setText(""); setLive(""); setAdvice(null); setNote(null); setPeak(0); setLevel(0);
    void micPermission().then(setPerm);
    const choice = voiceChoice();
    // The level meter has its own stream; the provider opens its own too.
    const probing = micLevelProbe((v) => { setLevel(v); setPeak((p) => Math.max(p, v)); })
      .then((p) => { probe = p; })
      .catch(async (e) => {
        const code = e?.name === "NoApi" ? "no-api" : mediaErrorCode(e);
        setAdvice(micAdvice(await gatherMicFacts(env(), code === "other" ? "busy" : code, [e?.name, e?.message].filter(Boolean).join(": "))));
      });
    let started = false;
    let ended: () => void = () => {};
    const endedP = new Promise<void>((r) => (ended = r));
    if (choice === "webspeech" || choice === "whisper") {
      started = startDictation(
        {
          insert: (t) => setText((s) => (s ? `${s} ${t}` : t).trim()),
          interim: (t) => setLive(t),
          element: box,
        },
        { lang: activeLang(), bubble: false, anchor: box, onEnd: () => ended() },
      );
    } else {
      setNote(choice === "setup"
        ? "Speech recognition: download the offline voice model below first. The microphone level is still tested."
        : "No speech recognition here, so only the microphone level is tested.");
    }
    timer = window.setTimeout(async () => {
      await probing;
      probe?.stop();
      probe = null;
      if (started && voiceStatus() !== "idle") {
        setPhase("finishing");
        stopDictation();
        // Offline voice transcribes the last piece now; give it time.
        await Promise.race([endedP, new Promise((r) => setTimeout(r, 15000))]);
      }
      setLive("");
      await settle(started);
    }, TEST_MS);
  };

  const levelWord = () => {
    const p = peak();
    if (advice()) return "No input";
    if (p >= 0.08) return "Good";
    if (p >= 0.02) return "Quiet: speak louder or move closer";
    return phase() === "done" ? "Silent: check the input device in the system's sound settings" : "…";
  };

  return (
    <div class="st-embed voice-test" ref={box} role="group" aria-labelledby={props.labelId} aria-describedby={props.descId}>
      <div class="voice-test-head">
        <button class="st-btn small solid" data-voice-test disabled={phase() === "listening" || phase() === "finishing"} onClick={run}>
          {phase() === "listening" ? "Speak now…" : phase() === "finishing" ? "Finishing…" : phase() === "done" ? "Test again" : "Test microphone"}
        </button>
        <div class="voice-test-meter" aria-hidden="true"><span style={{ width: `${Math.round(level() * 100)}%` }} /></div>
      </div>
      <Show when={phase() !== "idle"}>
        <dl class="voice-test-rows">
          <dt>Speech recognition</dt><dd>{audioPathShort(voiceChoice(), voiceEnv())} <span class="voice-model-sub">({voiceChoice() === "hint" ? "system dictation" : voiceChoice()})</span></dd>
          <dt>Microphone permission</dt><dd>{perm() ? PERMISSION_TEXT[perm()!] : "…"}</dd>
          <dt>Input level</dt><dd>{levelWord()}</dd>
          <dt>Heard</dt><dd class="voice-test-text">{text() || live() || (phase() === "done" ? "(nothing recognised)" : "…")}</dd>
        </dl>
      </Show>
      <Show when={note()}><p class="voice-privacy-note">{note()}</p></Show>
      <Show when={advice()}>
        {(a) => (
          <div class="voice-test-advice" role="alert">
            <strong>{a().title}</strong>
            <p>{a().text}</p>
            <Show when={a().steps.length}><ol><For each={a().steps}>{(st) => <li>{st}</li>}</For></ol></Show>
          </div>
        )}
      </Show>
    </div>
  );
}
