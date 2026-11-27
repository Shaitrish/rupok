// Web Speech API provider (SpeechRecognition / webkitSpeechRecognition).
// Chrome and Edge stream the audio to Google's / Microsoft's service; Safari
// and WKWebView on macOS recognise on-device on recent versions. Interim
// results give the live text. The browser asks for the microphone itself on
// first use: start() runs inside the tap on the mic (MicButton's
// pointerdown, the voice box's click), so the question appears at once.
// When the microphone is refused, dictation.ts asks diagnose.ts why (the
// site setting, the OS, no device) and shows the fix. Nothing is recorded
// or stored.

import type { DictationSession, SessionHandlers, SessionOptions, VoiceErrorCode, VoiceProvider } from "./types";

type Ctor = new () => any;

export function speechCtor(): Ctor | null {
  const w = window as any;
  return (w.SpeechRecognition || w.webkitSpeechRecognition || null) as Ctor | null;
}

const ERRORS: Record<string, [VoiceErrorCode, string]> = {
  "not-allowed": ["not-allowed", "The microphone was blocked."],
  "service-not-allowed": ["unsupported", "Speech recognition is not available in this browser."],
  "audio-capture": ["no-mic", "No microphone was found, or it could not be opened."],
  network: ["network", "The speech service could not be reached. Check the internet connection."],
  "language-not-supported": ["unsupported", "This language is not supported by the browser's speech service."],
  aborted: ["aborted", "Stopped."],
};

export const webSpeechProvider: VoiceProvider = {
  id: "webspeech",
  label: "Browser speech recognition",
  // Chrome does not add punctuation; Safari sometimes does. Spoken
  // punctuation is converted either way (nothing left to convert is fine).
  punctuates: false,
  start(opts: SessionOptions, h: SessionHandlers): DictationSession {
    const Ctor = speechCtor();
    if (!Ctor) {
      queueMicrotask(() => {
        h.onError({ code: "unsupported", message: "Speech recognition is not available in this browser." });
        h.onEnd();
      });
      return { stop() {}, cancel() {} };
    }
    const rec = new Ctor();
    rec.lang = opts.lang;
    rec.interimResults = true;
    rec.continuous = true;
    rec.maxAlternatives = 1;
    let cancelled = false;
    let ended = false;
    let pendingInterim = "";
    rec.onresult = (e: any) => {
      if (cancelled) return;
      let interim = "";
      for (let i = e.resultIndex; i < e.results.length; i++) {
        const r = e.results[i];
        const text = r[0]?.transcript ?? "";
        if (r.isFinal) {
          if (text.trim()) h.onFinal(text);
        } else interim += text;
      }
      pendingInterim = interim;
      h.onInterim(interim);
    };
    rec.onerror = (e: any) => {
      if (cancelled || e.error === "no-speech") return;
      const [code, message] = ERRORS[e.error] ?? ["other", `Speech recognition error: ${e.error}`];
      if (code !== "aborted") h.onError({ code, message, detail: [e.error, e.message].filter(Boolean).join(": ") });
    };
    rec.onend = () => {
      if (ended) return;
      ended = true;
      // Some engines end without a final result for the last words.
      if (!cancelled && pendingInterim.trim()) h.onFinal(pendingInterim);
      h.onInterim("");
      h.onEnd();
    };
    try {
      rec.start();
    } catch (err) {
      queueMicrotask(() => {
        h.onError({ code: "other", message: String((err as Error)?.message ?? err) });
        rec.onend();
      });
    }
    return {
      stop() {
        try { rec.stop(); } catch { /* already stopped */ }
      },
      cancel() {
        cancelled = true;
        try { rec.abort(); } catch { /* already stopped */ }
      },
    };
  },
};
