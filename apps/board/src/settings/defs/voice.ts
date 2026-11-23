// Settings → General → Voice input (speech-to-text in every text field;
// src/voice/*, components/MicButton.tsx), and its shortcut under Keyboard.

import { isDesktop } from "../../platform";
import { setDefaultLang, voicePrefs } from "../../voice/dictation";
import { VOICE_LANGS, type VoiceLang } from "../../voice/types";
import { VoiceModel, VoicePrivacy } from "../../voice/VoiceSettings";
import "../../voice/voice.css";
import { registerSettings } from "../registry";

const d = voicePrefs.defaults;
const GROUP = "Voice input";
const on = () => voicePrefs.read().enabled;

registerSettings([
  {
    id: "general.voiceInput", category: "general", group: GROUP, order: 300, type: "toggle", label: "Voice input",
    description: "A mic in text boxes, the title, search and AI prompts: speak instead of typing. Alt V in any field.",
    keywords: ["voice", "speech", "dictation", "dictate", "microphone", "mic", "speech to text", "ভয়েস", "কথা", "মাইক", "বলে লিখুন"],
    get: () => voicePrefs.read().enabled,
    set: (v) => voicePrefs.write({ enabled: v }),
    default: d.enabled,
  },
  {
    id: "general.voiceLang", category: "general", group: GROUP, order: 301, type: "select", label: "Language",
    description: "The language you usually speak. Switch any time with the EN / বাং chip on the mic.",
    keywords: ["language", "Bangla", "Bengali", "English", "ভাষা", "বাংলা"],
    options: VOICE_LANGS.map((l) => ({ value: l.value, label: l.label })),
    get: () => voicePrefs.read().lang,
    set: (v) => setDefaultLang(v as VoiceLang),
    default: d.lang,
    enabled: on,
  },
  {
    id: "general.voiceProvider", category: "general", group: GROUP, order: 302, type: "select", label: "Speech recognition",
    description: "Automatic uses offline voice when downloaded, else the browser's, else your system's dictation.",
    keywords: ["provider", "engine", "Whisper", "Web Speech", "Google", "offline", "Win+H", "Dictation"],
    options: [
      { value: "auto", label: "Automatic" },
      { value: "webspeech", label: "Browser (Chrome, Edge, Safari)" },
      ...(isDesktop ? [{ value: "whisper", label: "Offline (Whisper, on this computer)" }] : []),
      { value: "os", label: "System dictation (Win+H, macOS)" },
    ],
    get: () => voicePrefs.read().provider,
    set: (v) => voicePrefs.write({ provider: v as typeof d.provider }),
    default: d.provider,
    enabled: on,
  },
  {
    id: "general.voicePrivacy", category: "general", group: GROUP, order: 303, type: "custom", layout: "block",
    label: "Where your voice goes",
    description: "Chrome and Edge send audio to Google or Microsoft while you speak. Offline voice and recent macOS stay on the device.",
    keywords: ["privacy", "cloud", "on-device", "offline", "Google", "Microsoft", "Apple", "গোপনীয়তা"],
    component: VoicePrivacy,
  },
  {
    id: "general.voiceModel", category: "general", group: GROUP, order: 304, type: "custom", layout: "block",
    label: "Offline voice model",
    description: "Download once to dictate without internet, in English and Bangla. Needed on Linux, where the system has no speech recognition.",
    keywords: ["Whisper", "offline", "download", "model", "on-device", "remove", "অফলাইন"],
    component: VoiceModel,
    when: () => isDesktop,
  },
  {
    id: "general.voicePunctuation", category: "general", group: GROUP, order: 305, type: "toggle", label: "Spoken punctuation",
    description: "Say \"comma\", \"full stop\", \"question mark\", \"new line\" (কমা, দাঁড়ি, প্রশ্নবোধক চিহ্ন, নতুন লাইন). Also capitalises sentences.",
    keywords: ["punctuation", "comma", "full stop", "capital", "দাঁড়ি", "কমা"],
    get: () => voicePrefs.read().autoPunctuation,
    set: (v) => voicePrefs.write({ autoPunctuation: v }),
    default: d.autoPunctuation,
    enabled: on,
  },
  {
    id: "general.voiceMode", category: "general", group: GROUP, order: 306, type: "segmented", label: "Mic button",
    description: "Tap to start and tap to stop (holding also works), or listen only while held.",
    keywords: ["push to talk", "hold", "toggle", "walkie-talkie"],
    options: [
      { value: "toggle", label: "Tap to talk" },
      { value: "hold", label: "Hold to talk" },
    ],
    get: () => voicePrefs.read().mode,
    set: (v) => voicePrefs.write({ mode: v as "toggle" | "hold" }),
    default: d.mode,
    enabled: on,
  },
  {
    id: "keyboard.voice", category: "keyboard", group: "Editing", order: 390, type: "key", label: "Dictate into the focused field (tap, or hold to talk)",
    keys: ["Alt V"],
    keywords: ["shortcut", "key", "hotkey", "voice", "dictation", "speech", "mic", "push to talk"],
  },
  {
    id: "keyboard.voiceCancel", category: "keyboard", group: "Editing", order: 391, type: "key", label: "Cancel dictation",
    keys: ["Esc"],
    keywords: ["shortcut", "voice", "dictation", "stop"],
  },
]);
