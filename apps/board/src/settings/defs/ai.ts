// Settings → AI & agents: Connect AI (live.ts) and handwriting to text.

import { allowEdit, live } from "../../live";
import { ClaudeKeyField, ConnectAiSection, HandwritingStatus } from "../../components/settings/AiSettings";
import { registerSettings } from "../registry";

registerSettings([
  {
    id: "ai.connect", category: "ai", order: 10, type: "section", label: "Connect AI",
    description: "Let your own AI assistant see and edit this lesson live.",
    keywords: ["live", "agent", "MCP", "Claude Code", "Claude Desktop", "Codex", "OpenCode", "pair", "pairing code", "এআই", "সংযোগ"],
    component: ConnectAiSection,
  },
  {
    id: "ai.allowEdit", category: "ai", order: 20, type: "toggle", label: "Let the AI edit the board",
    description: "Off: a connected AI can look at the lesson and point, but not change it.",
    keywords: ["view only", "read only", "permission", "agent"],
    get: allowEdit,
    set: (v) => live.setAllowEdit(v),
    default: true,
  },
  {
    id: "ai.handwriting", category: "ai", group: "Handwriting to text", order: 100, type: "custom", label: "Handwriting to text",
    description: "Lasso some writing and tap To text. It becomes a text box in the same place and colour.",
    keywords: ["recognition", "OCR", "convert", "to text", "handwriting", "হাতের লেখা", "লেখা থেকে টেক্সট"],
    component: HandwritingStatus,
  },
  {
    id: "ai.claudeKey", category: "ai", group: "Handwriting to text", order: 110, type: "custom", label: "Claude API key",
    description: "Optional. Reads English, বাংলা and maths with Claude on your own Anthropic account. Kept only on this device, never in lessons or exported settings.",
    keywords: ["api key", "anthropic", "sk-ant", "Bangla", "Bengali", "বাংলা", "math", "handwriting provider"],
    component: ClaudeKeyField,
  },
]);
