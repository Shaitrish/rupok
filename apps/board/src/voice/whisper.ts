// Offline Whisper provider (desktop app, feature `whisper`). The webview
// captures the microphone with getUserMedia (WebView2 and WKWebView ask the
// user; on Linux the shell answers WebKitGTK's permission request for audio
// only), downsamples to 16 kHz mono and sends 16-bit PCM to the Rust command
// `voice_transcribe` (apps/desktop/src/voice_cmd.rs), which runs whisper.cpp
// on this computer. Audio stays in memory and is dropped after each pass.
//
// Live text: while you speak, the clip so far is re-transcribed every couple
// of seconds (when the previous pass is done); on stop the last pass is final.
// Long dictation is cut into ~20 s pieces so each pass stays fast.

import { desktopInvoke, desktopListen } from "../platform";
import type { DictationSession, SessionHandlers, SessionOptions, VoiceProvider } from "./types";

export const TARGET_RATE = 16000;
const INTERIM_EVERY_MS = 2200;
const PIECE_SECONDS = 20;

export interface ModelStatus {
  id: string;
  bytes: number;
  installed: boolean;
}
export interface WhisperStatus {
  built: boolean;
  downloading: boolean;
  models: ModelStatus[];
}

export const whisperStatus = () => desktopInvoke<WhisperStatus>("voice_status");
export const downloadModel = (model: string) => desktopInvoke<void>("voice_model_download", { model });
export const cancelDownload = () => desktopInvoke<void>("voice_model_cancel");
export const removeModel = (model: string) => desktopInvoke<void>("voice_model_remove", { model });
export const onDownloadProgress = (cb: (p: { model: string; done: number; total: number }) => void) =>
  desktopListen("rupok://voice-model-progress", cb);

/** Average-and-pick downsampling to 16 kHz (speech only needs < 8 kHz). */
export function downsample(input: Float32Array, fromRate: number): Float32Array {
  if (fromRate === TARGET_RATE) return input.slice();
  const ratio = fromRate / TARGET_RATE;
  const n = Math.floor(input.length / ratio);
  const out = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const a = Math.floor(i * ratio);
    const b = Math.min(input.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    for (let j = a; j < b; j++) sum += input[j];
    out[i] = b > a ? sum / (b - a) : input[a] ?? 0;
  }
  return out;
}

/** Float samples to little-endian 16-bit PCM bytes. */
export function toPcm16(chunks: Float32Array[]): Uint8Array {
  const total = chunks.reduce((n, c) => n + c.length, 0);
  const view = new DataView(new ArrayBuffer(total * 2));
  let o = 0;
  for (const c of chunks) {
    for (let i = 0; i < c.length; i++, o += 2) {
      const s = Math.max(-1, Math.min(1, c[i]));
      view.setInt16(o, s < 0 ? s * 0x8000 : s * 0x7fff, true);
    }
  }
  return new Uint8Array(view.buffer);
}

export function makeWhisperProvider(model: () => string | null): VoiceProvider {
  return {
    id: "whisper",
    label: "Offline (Whisper)",
    punctuates: true,
    start(opts: SessionOptions, h: SessionHandlers): DictationSession {
      let stopped = false;
      let cancelled = false;
      let busy = false;
      let chunks: Float32Array[] = [];
      let samples = 0;
      let stream: MediaStream | null = null;
      let ctx: AudioContext | null = null;
      let timer: number | undefined;
      let finished = false;

      const transcribe = async (pcm: Float32Array[]): Promise<string> => {
        const m = model();
        if (!m) throw Object.assign(new Error("Download the offline voice model in Settings first."), { code: "no-model" });
        return desktopInvoke<string>("voice_transcribe", toPcm16(pcm), { "x-voice-model": m, "x-voice-lang": opts.lang });
      };
      const fail = (e: any) => {
        const msg = String(e?.message ?? e);
        h.onError({ code: e?.code === "no-model" || /not downloaded/.test(msg) ? "no-model" : "other", message: msg });
      };
      const release = () => {
        clearInterval(timer);
        stream?.getTracks().forEach((t) => t.stop());
        void ctx?.close().catch(() => {});
        stream = null;
        ctx = null;
      };
      const end = () => {
        if (finished) return;
        finished = true;
        release();
        h.onInterim("");
        h.onEnd();
      };
      /** Transcribe the whole clip so far as live text, or cut a piece. */
      const tick = async () => {
        if (busy || stopped || !chunks.length) return;
        busy = true;
        const piece = samples >= PIECE_SECONDS * TARGET_RATE;
        const pcm = chunks;
        if (piece) { chunks = []; samples = 0; }
        try {
          const text = await transcribe(pcm);
          if (cancelled) return;
          if (piece) { if (text.trim()) h.onFinal(text); h.onInterim(""); }
          else if (!stopped) h.onInterim(text);
        } catch (e) {
          if (!cancelled) { fail(e); stopped = true; end(); }
        } finally {
          busy = false;
        }
      };

      (async () => {
        try {
          stream = await navigator.mediaDevices.getUserMedia({
            audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
          });
        } catch (e: any) {
          const denied = e?.name === "NotAllowedError" || e?.name === "SecurityError";
          h.onError({
            code: denied ? "not-allowed" : "no-mic",
            message: denied ? "Microphone access was refused. Allow it in your system's privacy settings." : "No microphone was found.",
          });
          end();
          return;
        }
        if (cancelled || stopped) { end(); return; }
        const AC: typeof AudioContext = (window as any).AudioContext || (window as any).webkitAudioContext;
        ctx = new AC();
        const src = ctx.createMediaStreamSource(stream);
        // ScriptProcessor is deprecated but works in every webview Tauri uses
        // (an AudioWorklet module would need a blob: URL in the CSP).
        const proc = ctx.createScriptProcessor(4096, 1, 1);
        const rate = ctx.sampleRate;
        proc.onaudioprocess = (ev) => {
          if (stopped) return;
          const input = ev.inputBuffer.getChannelData(0);
          let peak = 0;
          for (let i = 0; i < input.length; i += 4) peak = Math.max(peak, Math.abs(input[i]));
          h.onLevel?.(Math.min(1, peak * 2.5));
          const d = downsample(input, rate);
          chunks.push(d);
          samples += d.length;
        };
        src.connect(proc);
        // Some engines only run the processor when it reaches the output; a
        // muted gain keeps the microphone from playing back.
        const mute = ctx.createGain();
        mute.gain.value = 0;
        proc.connect(mute);
        mute.connect(ctx.destination);
        timer = window.setInterval(() => void tick(), INTERIM_EVERY_MS);
      })();

      return {
        stop() {
          if (stopped) return;
          stopped = true;
          clearInterval(timer);
          const pcm = chunks;
          chunks = [];
          release();
          h.onLevel?.(0);
          const finish = async () => {
            // Wait for a pass already running, then transcribe the rest.
            while (busy) await new Promise((r) => setTimeout(r, 50));
            if (!cancelled && pcm.length) {
              try {
                const text = await transcribe(pcm);
                if (!cancelled && text.trim()) h.onFinal(text);
              } catch (e) {
                if (!cancelled) fail(e);
              }
            }
            end();
          };
          void finish();
        },
        cancel() {
          cancelled = true;
          stopped = true;
          chunks = [];
          end();
        },
      };
    },
  };
}
