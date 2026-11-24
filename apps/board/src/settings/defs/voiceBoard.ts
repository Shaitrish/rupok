// Settings → General → Voice on the board (the dock's Voice text tool;
// src/voice/board.ts, VoiceBoard.tsx), and its keys under Keyboard.

import { voiceBoardPrefs } from "../../voice/board";
import { voicePrefs } from "../../voice/dictation";
import { registerSettings } from "../registry";

const d = voiceBoardPrefs.defaults;
const GROUP = "Voice on the board";
const on = () => voicePrefs.read().enabled;

registerSettings([
  {
    id: "general.voiceBoardMode", category: "general", group: GROUP, order: 320, type: "segmented", label: "A new voice box",
    description: "Write what you say on the board, or send it to your AI assistant as a question. Switch on the box any time.",
    keywords: ["voice text", "speech", "dictation", "ask AI", "prompt", "board", "ভয়েস", "বোর্ডে লিখুন"],
    options: [
      { value: "text", label: "Text on the board" },
      { value: "ask", label: "Ask AI" },
    ],
    get: () => voiceBoardPrefs.read().mode,
    set: (v) => voiceBoardPrefs.write({ mode: v as typeof d.mode }),
    default: d.mode,
    enabled: on,
  },
  {
    id: "general.voiceBoardTalk", category: "general", group: GROUP, order: 321, type: "segmented", label: "Talking",
    description: "Tap the page and talk until you finish (holding the dock button is push-to-talk too), or listen only while the dock button, the mic or Space is held.",
    keywords: ["push to talk", "hold", "toggle", "walkie-talkie", "clicker", "space"],
    options: [
      { value: "toggle", label: "Tap to talk" },
      { value: "hold", label: "Hold to talk" },
    ],
    get: () => voiceBoardPrefs.read().talk,
    set: (v) => voiceBoardPrefs.write({ talk: v as typeof d.talk }),
    default: d.talk,
    enabled: on,
  },
  {
    id: "general.voiceBoardStyle", category: "general", group: GROUP, order: 322, type: "toggle", label: "Write it by hand",
    description: "New voice text is written in handwriting, like Solve this (English). Off: typed text you can edit.",
    keywords: ["handwriting", "hand-written", "style", "typed", "হাতের লেখা"],
    get: () => voiceBoardPrefs.read().style === "handwriting",
    set: (v) => voiceBoardPrefs.write({ style: v ? "handwriting" : "typed" }),
    default: d.style === "handwriting",
    enabled: on,
  },
  {
    id: "general.voiceBoardPause", category: "general", group: GROUP, order: 323, type: "slider", label: "New line after a pause",
    description: "Stop talking this long and the next words start a new line. Saying \"new line\" (নতুন লাইন) always works.",
    keywords: ["pause", "silence", "line break", "new line", "বিরতি"],
    min: 0, max: 5000, step: 250,
    format: (v) => (v === 0 ? "Off" : `${(v / 1000).toFixed(v % 1000 ? 2 : 0).replace(/0$/, "")} s`),
    get: () => voiceBoardPrefs.read().pauseMs,
    set: (v) => voiceBoardPrefs.write({ pauseMs: v }),
    default: d.pauseMs,
    enabled: on,
  },
  {
    id: "general.voiceBoardStudent", category: "general", group: GROUP, order: 324, type: "toggle", label: "Students may ask the AI by voice",
    description: "In Student view, the Voice text tool (when added in Teaching modes) can ask the AI. Students never write on the board.",
    keywords: ["student", "student view", "allow", "ছাত্র"],
    get: () => voiceBoardPrefs.read().student,
    set: (v) => voiceBoardPrefs.write({ student: v }),
    default: d.student,
    enabled: on,
  },
  {
    id: "keyboard.voiceBoard", category: "keyboard", group: "Editing", order: 392, type: "key", label: "Voice text: listen or pause (with the tool on; hold with Hold to talk)",
    keys: ["Space", "B", "."],
    keywords: ["shortcut", "voice", "clicker", "presenter", "space", "push to talk"],
  },
  {
    id: "keyboard.voiceBoardDone", category: "keyboard", group: "Editing", order: 393, type: "key", label: "Voice text: finish the box",
    keys: ["Esc"],
    keywords: ["shortcut", "voice", "finish", "done"],
  },
]);
