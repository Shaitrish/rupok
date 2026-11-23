// Which speech-to-text provider to use. Pure (no DOM): the caller passes in
// what it detected, so this is unit-tested (apps/board/test/voice.test.ts).
//
// Automatic order:
//   1. Whisper (desktop, model downloaded): on-device, offline, Bangla.
//   2. Web Speech: Chrome/Edge (cloud) and Safari/WKWebView on macOS
//      (on-device on recent macOS). Not in WebView2 (Windows desktop: the API
//      exists but has no service behind it) nor WebKitGTK (Linux: none).
//   3. The OS dictation hint (Win+H, macOS Dictation).

import type { AudioPath, ProviderChoice, ProviderPref } from "./types";

export type OsName = "windows" | "mac" | "linux" | "chromeos" | "android" | "ios" | "other";
export type Engine = "chromium" | "webkit" | "gecko" | "other";

export interface VoiceEnv {
  os: OsName;
  engine: Engine;
  /** Edge rather than Chrome (both Chromium; Edge sends audio to Microsoft). */
  edge: boolean;
  /** Inside the Tauri desktop app. */
  desktop: boolean;
  /** `SpeechRecognition` or `webkitSpeechRecognition` exists. */
  speechApi: boolean;
  /** The desktop app was built with offline voice (feature `whisper`). */
  whisperBuilt: boolean;
  /** The downloaded Whisper model in use, or null. */
  whisperModel: string | null;
  /** Web Speech failed this session with "network"/"service-not-allowed". */
  speechBroken?: boolean;
}

/** Detect the OS and browser engine from the user agent. */
export function detectPlatform(ua: string, platform = ""): { os: OsName; engine: Engine; edge: boolean } {
  const os: OsName =
    /Android/i.test(ua) ? "android"
    : /iPhone|iPad|iPod/i.test(ua) || (/Macintosh/.test(ua) && /Mobile\//.test(ua)) ? "ios"
    : /CrOS/.test(ua) ? "chromeos"
    : /Windows/i.test(ua) || /^Win/i.test(platform) ? "windows"
    : /Mac OS X|Macintosh/i.test(ua) || /^Mac/i.test(platform) ? "mac"
    : /Linux|X11/i.test(ua) ? "linux"
    : "other";
  const engine: Engine =
    /Firefox\//.test(ua) && !/Seamonkey/i.test(ua) ? "gecko"
    : /Chrome\/|Chromium\/|Edg\//.test(ua) ? "chromium"
    : /AppleWebKit\//.test(ua) ? "webkit"
    : "other";
  return { os, engine, edge: /Edg\//.test(ua) };
}

/** Web Speech actually works here (not just exists). */
export function webSpeechUsable(env: VoiceEnv): boolean {
  if (!env.speechApi || env.speechBroken) return false;
  // Desktop app: WebView2 exposes the API without a working service, and
  // WebKitGTK has none. WKWebView on macOS works.
  if (env.desktop) return env.os === "mac";
  return env.engine === "chromium" || env.engine === "webkit";
}

export function whisperUsable(env: VoiceEnv): boolean {
  return env.desktop && env.whisperBuilt && !!env.whisperModel;
}

/** The provider for a Settings preference. A forced provider that is not
 * available falls back to the automatic order, then to the hint. */
export function chooseProvider(env: VoiceEnv, pref: ProviderPref = "auto"): ProviderChoice {
  if (pref === "os") return "hint";
  if (pref === "whisper" && whisperUsable(env)) return "whisper";
  if (pref === "webspeech" && webSpeechUsable(env)) return "webspeech";
  if (whisperUsable(env)) return "whisper";
  if (webSpeechUsable(env)) return "webspeech";
  return "hint";
}

/** Where the audio goes with this provider, for the privacy notice. */
export function audioPath(choice: ProviderChoice, env: VoiceEnv): AudioPath {
  if (choice === "whisper" || choice === "hint") return choice === "whisper" ? "device" : "device-or-cloud";
  // Safari / WKWebView: on-device on recent macOS (Apple's servers on older
  // versions or when the on-device model for the language is missing).
  if (env.engine === "webkit") return "device-or-cloud";
  return "cloud"; // Chrome → Google, Edge → Microsoft
}

/** Who receives the audio, in words. */
export function audioPathText(choice: ProviderChoice, env: VoiceEnv): string {
  if (choice === "whisper") return "On this computer. Audio never leaves it.";
  if (choice === "hint") return "Your system's dictation decides (Windows and macOS can work on-device).";
  if (env.engine === "webkit") return "On this device on recent macOS; older versions send audio to Apple.";
  return env.edge ? "Sent to Microsoft's speech service while you speak." : "Sent to Google's speech service while you speak.";
}

/** Where the audio goes, in two or three words (the live-text bubble). */
export function audioPathShort(choice: ProviderChoice, env: VoiceEnv): string {
  if (choice === "whisper") return "On this computer";
  if (choice === "hint") return "System dictation";
  if (env.engine === "webkit") return "Apple speech";
  return env.edge ? "Cloud · Microsoft" : "Cloud · Google";
}

/** How to dictate with the operating system when no provider exists. */
export function osDictationHint(os: OsName): { keys: string[]; text: string } {
  switch (os) {
    case "windows":
      return { keys: ["Win", "H"], text: "Windows voice typing: click in the field, press Win + H and speak." };
    case "mac":
      return { keys: ["Fn", "Fn"], text: "macOS Dictation: click in the field, press Fn twice (or the mic key) and speak. Turn it on in System Settings › Keyboard." };
    case "chromeos":
      return { keys: ["Search", "D"], text: "ChromeOS dictation: press Search + D and speak. Turn it on in Settings › Accessibility." };
    case "android":
    case "ios":
      return { keys: [], text: "Tap the microphone on your on-screen keyboard and speak." };
    case "linux":
      return { keys: [], text: "This browser has no speech recognition. Use Chrome or Edge, or the Rupok desktop app with the offline voice model (Settings › Voice input)." };
    default:
      return { keys: [], text: "This browser has no speech recognition. Try Chrome, Edge or Safari." };
  }
}
