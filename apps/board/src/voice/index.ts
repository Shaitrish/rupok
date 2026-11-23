// Voice input (speech-to-text): the public API for other features.
//
//   import { MicButton } from "../components/MicButton";
//   <MicButton target={() => textarea} />
//
//   import { startDictation } from "../voice";
//   startDictation(textarea);                                  // a field
//   startDictation({ insert: (t) => setPrompt((p) => p + t),   // or anything
//                    interim: (t) => setLive(t), element: box });
//
// See components/MicButton.tsx and voice/dictation.ts.

export {
  cancelDictation, isDictating, privacyText, refreshWhisper, setDictationLang, startDictation, stopDictation,
  toggleDictation, toggleDictationLang, voiceChoice, voicePrefs, interim as voiceInterim, voiceStatus,
  type CustomTarget, type DictationOptions, type DictationTarget, type VoicePrefs, type VoiceStatus,
} from "./dictation";
export { applySpokenPunctuation, joinDictation } from "./punctuation";
export { chooseProvider } from "./select";
export { VOICE_LANGS, type VoiceLang } from "./types";
