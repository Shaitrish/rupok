// The offline voice model download (desktop app), shared by Settings ›
// Voice input › Offline voice model and the mic's first-use offer, so both
// show the same progress. The teacher starts it (consent, with the size);
// the shell downloads from a fixed URL and checks a pinned SHA-256
// (apps/desktop/src/voice.rs).

import { createSignal } from "solid-js";
import { refreshWhisper, voicePrefs, whisper } from "./dictation";
import { cancelDownload, downloadModel, onDownloadProgress } from "./whisper";

export interface DownloadProgress {
  model: string;
  done: number;
  total: number;
}

const [progress, setProgress] = createSignal<DownloadProgress | null>(null);
const [error, setError] = createSignal<string | null>(null);
export { error as modelError, progress as modelProgress };

/** The model offered on first use: multilingual, so Bangla works. */
export const FIRST_MODEL = "base";

export const mb = (b: number) => `${Math.round(b / 1_000_000)} MB`;

/** The size of a model, from the shell's catalogue. */
export const modelBytes = (id: string) => whisper()?.models.find((m) => m.id === id)?.bytes ?? 0;

let listening: Promise<unknown> | null = null;
function listen() {
  listening ??= onDownloadProgress((p) => setProgress(p)).catch(() => { listening = null; });
}

/** Download a model (after the teacher agreed); true when it is ready. */
export async function downloadVoiceModel(id: string): Promise<boolean> {
  if (progress()) return false;
  listen();
  setError(null);
  setProgress({ model: id, done: 0, total: modelBytes(id) });
  try {
    await downloadModel(id);
    voicePrefs.write({ model: id });
    return true;
  } catch (e: any) {
    const msg = String(e?.message ?? e);
    if (!/cancel/i.test(msg)) {
      setError(/dns|resolve|connect|timed out|network|io:/i.test(msg)
        ? `The download failed: ${msg}. Check the internet connection and try again.`
        : msg);
    }
    return false;
  } finally {
    setProgress(null);
    await refreshWhisper();
  }
}

export const stopVoiceModelDownload = () => void cancelDownload().catch(() => {});
