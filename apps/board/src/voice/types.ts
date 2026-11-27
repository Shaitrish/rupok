// Voice input (speech-to-text): the provider interface. Providers are chosen
// in select.ts; the session state and startDictation() live in dictation.ts.

/** Dictation languages (BCP 47). English and Bangla at least. */
export type VoiceLang = "en-US" | "en-GB" | "en-IN" | "bn-BD" | "bn-IN";

export const VOICE_LANGS: { value: VoiceLang; label: string; short: string }[] = [
  { value: "en-US", label: "English", short: "EN" },
  { value: "bn-BD", label: "বাংলা (Bangla)", short: "বাং" },
  { value: "en-GB", label: "English (UK)", short: "EN" },
  { value: "en-IN", label: "English (India)", short: "EN" },
  { value: "bn-IN", label: "বাংলা (India)", short: "বাং" },
];

/** The engines. "hint" is not an engine: it shows how to use the OS's own
 * dictation (Win+H, macOS Dictation), which types into any focused field.
 * "setup" is not one either: the desktop app can run offline voice but the
 * model is not downloaded yet, so the mic offers the download. */
export type ProviderId = "webspeech" | "whisper";
export type ProviderChoice = ProviderId | "hint" | "setup";
/** The Settings value: automatic or a forced choice. */
export type ProviderPref = "auto" | ProviderId | "os";

/** Where the audio goes, for the privacy line in Settings and on the mic. */
export type AudioPath = "cloud" | "device" | "device-or-cloud";

export type VoiceErrorCode =
  | "not-allowed" // the microphone was refused (by the browser, the site setting or the OS)
  | "no-mic"
  | "busy" // the microphone exists but could not be opened (another app, or the OS)
  | "no-api" // this web engine cannot capture audio at all
  | "network" // the cloud service could not be reached
  | "service" // the browser has the speech API but no service behind it (Chromium, Brave…)
  | "unsupported" // the API exists but does not work here (e.g. WebView2)
  | "no-model" // Whisper: the model is not downloaded
  | "aborted"
  | "other";

export interface VoiceError {
  code: VoiceErrorCode;
  message: string;
  /** The engine's own words ("NotAllowedError: Permission denied by
   * system"), so the diagnosis can tell the OS from the site setting. */
  detail?: string;
}

export interface SessionHandlers {
  /** Live text of the current utterance (replaced on each call). */
  onInterim(text: string): void;
  /** A finished piece of text, ready to insert. */
  onFinal(text: string): void;
  /** Microphone level 0–1 (providers that can measure it). */
  onLevel?(level: number): void;
  onError(err: VoiceError): void;
  /** The session ended (after stop, cancel, an error or silence). */
  onEnd(): void;
}

export interface SessionOptions {
  lang: VoiceLang;
}

export interface DictationSession {
  /** Stop listening and deliver what was heard. */
  stop(): void;
  /** Stop and throw away anything not yet delivered. */
  cancel(): void;
}

export interface VoiceProvider {
  id: ProviderId;
  label: string;
  /** True when the provider punctuates by itself. */
  punctuates: boolean;
  start(opts: SessionOptions, h: SessionHandlers): DictationSession;
}
