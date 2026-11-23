// Settings rows for voice input that need their own UI: the privacy notice
// (which provider sends audio where) and the offline model download.

import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import type { SettingProps } from "../settings/registry";
import { refreshWhisper, voiceChoice, voiceEnv, voicePrefs, whisper, whisperModel } from "./dictation";
import { audioPathText } from "./select";
import { cancelDownload, downloadModel, onDownloadProgress, removeModel } from "./whisper";

const MB = (b: number) => `${Math.round(b / 1_000_000)} MB`;
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
        where: e.whisperBuilt ? audioPathText("whisper", e) : "Not in this build of the desktop app.",
        here: voiceChoice() === "whisper",
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
  const [progress, setProgress] = createSignal<{ model: string; done: number; total: number } | null>(null);
  const [busy, setBusy] = createSignal<string | null>(null);
  const [asking, setAsking] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  onMount(() => {
    void refreshWhisper();
    let off: (() => void) | undefined;
    void onDownloadProgress((p) => setProgress(p)).then((u) => (off = u)).catch(() => {});
    onCleanup(() => off?.());
  });
  const start = async (id: string) => {
    setAsking(null);
    setError(null);
    setBusy(id);
    setProgress({ model: id, done: 0, total: whisper()?.models.find((m) => m.id === id)?.bytes ?? 0 });
    try {
      await downloadModel(id);
      voicePrefs.write({ model: id });
    } catch (e: any) {
      const msg = String(e?.message ?? e);
      if (!/cancel/i.test(msg)) setError(msg);
    } finally {
      setBusy(null);
      setProgress(null);
      void refreshWhisper();
    }
  };
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
                            fallback={<button class="st-btn small" disabled={!!busy()} onClick={() => setAsking(m.id)}>Download…</button>}
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
            <Show when={error()}><div class="voice-model-error" role="alert">{error()}</div></Show>
            <p class="voice-privacy-note">Downloaded once, checked against a fixed checksum, kept in the app's data folder. After that, voice input works offline and audio never leaves this computer.</p>
          </Show>
        )}
      </Show>
    </div>
  );
}
