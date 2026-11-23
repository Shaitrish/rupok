// Settings → AI & agents → Chat: the in-app chat with your own agent
// (chat.ts, components/ChatPanel.tsx, `rupok agent`).

import { chatPrefs, openAiPanel, setAttachPage, setAutoApprove } from "../../chat";
import { registerSettings } from "../registry";

const d = chatPrefs.defaults;
const GROUP = "Chat";

registerSettings([
  {
    id: "ai.chatOpen", category: "ai", group: GROUP, order: 40, type: "action", label: "AI chat",
    description: "Chat with Claude Code, Codex, OpenCode or Gemini CLI inside Rupok and watch it build on the board.",
    keywords: ["chat", "ACP", "rupok agent", "assistant", "prompt", "চ্যাট"],
    button: "Open chat",
    icon: "smartink",
    run: () => openAiPanel(),
  },
  {
    id: "ai.chatAgent", category: "ai", group: GROUP, order: 41, type: "select", label: "Assistant",
    description: "Which installed agent the chat starts. You can switch any time from the chat's header.",
    keywords: ["agent", "model", "Claude Code", "Codex", "OpenCode", "Gemini"],
    options: [
      { value: "auto", label: "First installed" },
      { value: "claude", label: "Claude Code" },
      { value: "codex", label: "Codex" },
      { value: "opencode", label: "OpenCode" },
      { value: "gemini", label: "Gemini CLI" },
    ],
    get: () => chatPrefs.read().agent,
    set: (v) => chatPrefs.write({ agent: v }),
    default: d.agent,
  },
  {
    id: "ai.chatPermissions", category: "ai", group: GROUP, order: 42, type: "segmented", label: "Board edits in a new chat",
    description: "Ask before each change to the board, or start each chat with \"Auto-approve\" on (the chip under the chat header). Other tools always ask.",
    keywords: ["permission", "allow", "approve", "always"],
    options: [
      { value: "ask", label: "Ask each time" },
      { value: "session", label: "Auto-approve" },
    ],
    get: () => chatPrefs.read().permissions,
    set: (v) => { chatPrefs.write({ permissions: v as "ask" | "session" }); setAutoApprove(v === "session"); },
    default: d.permissions,
  },
  {
    id: "ai.chatAllowNoPrompt", category: "ai", group: GROUP, order: 49, type: "toggle", label: "Allow no-prompt modes",
    description: "Show agent modes that never ask before acting (like Claude Code's bypass mode). Off by default: in class the AI should always ask first.",
    keywords: ["bypass", "never ask", "dangerous", "permissions", "full access"],
    get: () => { try { return localStorage.getItem("rupok.chat.allowNoPrompt") === "1"; } catch { return false; } },
    set: (v) => { try { localStorage.setItem("rupok.chat.allowNoPrompt", v ? "1" : "0"); } catch { /* storage off */ } },
    default: false,
  },
  {
    id: "ai.chatAttach", category: "ai", group: GROUP, order: 43, type: "toggle", label: "Attach the current page",
    description: "Send the page you are on (or your selection) with each message, so the assistant knows what you mean by \"this\".",
    keywords: ["context", "selection", "snapshot"],
    get: () => chatPrefs.read().attach,
    set: (v) => { chatPrefs.write({ attach: v }); setAttachPage(v); },
    default: d.attach,
  },
  {
    id: "ai.chatPosition", category: "ai", group: GROUP, order: 44, type: "segmented", label: "Chat panel side",
    description: "Where the chat docks in Prepare. The slide moves over to make room. In Teach it floats and hides while you draw.",
    keywords: ["dock", "left", "right", "sidebar"],
    options: [
      { value: "right", label: "Right" },
      { value: "left", label: "Left" },
    ],
    get: () => chatPrefs.read().position,
    set: (v) => chatPrefs.write({ position: v as "right" | "left" }),
    default: d.position,
  },
]);
