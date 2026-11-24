// Settings → AI & agents → AI in action: class mode, working cards, the
// landing sparkle and chime, live Solve this, students asking AI, and the
// "pre-approve Rupok tools" helper (aiAction/).

import { For, createSignal } from "solid-js";
import { aiPrefs } from "../../aiAction/state";
import type { SettingProps } from "../registry";
import { registerSettings } from "../registry";

const d = aiPrefs.defaults;
const GROUP = "AI in action";
const DOCS = "https://github.com/shaitrish/rupok/blob/main/docs/ai-agents.md#make-ai-smoother-pre-approve-rupok-tools";

/** One-time setups that pre-approve Rupok's tools in each agent. */
export const PREAPPROVE: { agent: string; file: string; snippet: string }[] = [
  {
    agent: "Claude Code",
    file: "~/.claude/settings.json (or .claude/settings.json in your lessons folder)",
    snippet: `{ "permissions": { "allow": ["mcp__rupok__*"] } }`,
  },
  {
    agent: "Codex",
    file: "~/.codex/config.toml",
    snippet: `[mcp_servers.rupok]\ndefault_tools_approval_mode = "approve"`,
  },
  {
    agent: "OpenCode",
    file: "opencode.json",
    snippet: `{ "permission": { "rupok_*": "allow" } }`,
  },
];

function Preapprove(props: SettingProps) {
  const [tab, setTab] = createSignal(0);
  const [copied, setCopied] = createSignal(false);
  const cur = () => PREAPPROVE[tab()];
  const copy = () => navigator.clipboard?.writeText(cur().snippet).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1400); }, () => {});
  return (
    <div class="st-connect" aria-labelledby={props.labelId} aria-describedby={props.descId}>
      <div class="st-seg" role="tablist">
        <For each={PREAPPROVE}>
          {(p, i) => <button role="tab" aria-selected={tab() === i()} classList={{ on: tab() === i() }} onClick={() => setTab(i())}>{p.agent}</button>}
        </For>
      </div>
      <div class="st-connect-setup">
        <span class="st-help">Add to <b>{cur().file}</b>:</span>
        <code class="st-code" style={{ "white-space": "pre-wrap" }}>{cur().snippet}</code>
        <button type="button" class="st-btn ghost small" onClick={copy}>{copied() ? "Copied" : "Copy"}</button>
        <a class="st-link" href={DOCS} target="_blank" rel="noreferrer">Guide</a>
      </div>
      <span class="st-help">Rupok's tools only touch the open board (each change is one undo step). Reads never ask; deleting still asks in Rupok's own chat.</span>
    </div>
  );
}

registerSettings([
  {
    id: "ai.classMode", category: "ai", group: GROUP, order: 30, type: "segmented", label: "Class mode",
    description: "The AI's board edits apply at once, with an Undo button. Only deleting, clearing, replacing your ink or changing settings and modes still ask.",
    keywords: ["permission", "prompt", "approve", "allow", "teach", "undo", "class"],
    options: [
      { value: "teach", label: "In Teach" },
      { value: "always", label: "Always" },
      { value: "off", label: "Off" },
    ],
    get: () => aiPrefs.read().classMode,
    set: (v) => aiPrefs.write({ classMode: v as "teach" | "always" | "off" }),
    default: d.classMode,
  },
  {
    id: "ai.preapprove", category: "ai", group: GROUP, order: 31, type: "section", label: "Make AI smoother: pre-approve Rupok tools",
    description: "So Claude Code, Codex or OpenCode stop asking before each board change. One line, once.",
    keywords: ["permissions", "allow", "mcp__rupok", "settings.json", "codex", "opencode", "prompts"],
    component: Preapprove,
  },
  {
    id: "ai.progress", category: "ai", group: GROUP, order: 32, type: "toggle", label: "Show the AI working",
    description: "A card on the board where the result will appear, with the AI's current step and plan. It turns into the result when it is ready.",
    keywords: ["progress", "working", "status", "plan", "placeholder"],
    get: () => aiPrefs.read().progress,
    set: (v) => aiPrefs.write({ progress: v }),
    default: d.progress,
  },
  {
    id: "ai.celebrate", category: "ai", group: GROUP, order: 33, type: "toggle", label: "Sparkle when a result lands",
    description: "A small, quick sparkle where the AI's result appears.",
    keywords: ["celebrate", "animation", "confetti"],
    get: () => aiPrefs.read().celebrate,
    set: (v) => aiPrefs.write({ celebrate: v }),
    default: d.celebrate,
  },
  {
    id: "ai.sound", category: "ai", group: GROUP, order: 34, type: "toggle", label: "Chime when a result lands",
    description: "A soft two-note chime. Off by default.",
    keywords: ["sound", "audio", "chime"],
    get: () => aiPrefs.read().sound,
    set: (v) => aiPrefs.write({ sound: v }),
    default: d.sound,
  },
  {
    id: "ai.streamSolve", category: "ai", group: GROUP, order: 35, type: "toggle", label: "Write solutions line by line",
    description: "Solve this writes each step on the board as the AI works it out (when you don't check solutions first, or in class mode).",
    keywords: ["solve", "stream", "live", "handwriting"],
    get: () => aiPrefs.read().streamSolve,
    set: (v) => aiPrefs.write({ streamSolve: v }),
    default: d.streamSolve,
  },
  {
    id: "ai.studentsAsk", category: "ai", group: GROUP, order: 36, type: "toggle", label: "Students can ask AI",
    description: "In Student view, students can ask the connected AI to show them something. It appears on their own layer only, never in your lesson.",
    keywords: ["students", "student view", "ask", "visualise", "learn by doing"],
    get: () => aiPrefs.read().studentsAsk,
    set: (v) => aiPrefs.write({ studentsAsk: v }),
    default: d.studentsAsk,
  },
  {
    id: "ai.studentLimit", category: "ai", group: GROUP, order: 37, type: "slider", label: "Asks per student",
    description: "How many times a student may ask in 10 minutes.",
    keywords: ["limit", "rate", "students"],
    min: 1, max: 10, step: 1, format: (v) => `${v} / 10 min`,
    get: () => aiPrefs.read().studentLimit,
    set: (v) => aiPrefs.write({ studentLimit: v }),
    default: d.studentLimit,
  },
]);
