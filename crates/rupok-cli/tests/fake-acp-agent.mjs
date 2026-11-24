#!/usr/bin/env node
// A fake ACP agent for tests: speaks ACP v1 (JSON-RPC on stdio) like Claude
// Code's adapter would, and builds on the board by calling the Rupok MCP
// tools over the streamable HTTP endpoint it gets in `session/new`. No login
// or network needed.
//
// Prompts it understands (anything else gets a short echo):
//   "...Ohm's law..."   streams text, a plan, asks permission, then adds a
//                       page, a Smart Object and a text on the board.
//   "...context..."     says what board context came with the prompt.
//   "...slow..."        streams slowly until cancelled.
//   "Solve this problem..." (the board's Solve this prompt) reads the
//                       selection, points at it, writes a step-by-step
//                       solution with write_handwriting, then explains.
//   "/review ..."       one of its slash commands: answers "Reviewing: …".
//   "/stamp ..."        makes two custom stamps (an SVG star with a sparkle
//                       and a counter, a Smart dice that rolls on tap),
//                       places them, draws and restyles ink, and reorders
//                       pages, through the agent-control MCP tools.
// It also offers Claude-style modes, a model picker (session config option
// with category "model", mirrored in the unstable `models` field), slash
// commands, usage updates, and session list/load (replaying the chat).
// FAKE_ACP_AUTH=1: session/new fails with "Authentication required".

import { createInterface } from "node:readline";

let nextId = 1000;
const waiting = new Map();
let mcp = null; // { url, headers }
let cancelled = false;
let alwaysAllowed = false;
let mode = "default";
let model = "sonnet";
/** Sessions this process has seen: id → { title, updatedAt, log: [update…] }. */
const sessions = new Map();
const MODES = [
  { id: "default", name: "Default", description: "Ask before edits" },
  { id: "acceptEdits", name: "Accept edits", description: "Edit without asking" },
  { id: "plan", name: "Plan mode", description: "Plan only, no changes" },
  { id: "bypassPermissions", name: "Bypass permissions", description: "Never ask" },
];
const MODELS = [
  { value: "sonnet", name: "Sonnet 4.5", description: "Fast, everyday" },
  { value: "opus", name: "Opus 4.5", description: "Most capable" },
];
const modelState = () => ({
  configOptions: [{ id: "model", name: "Model", category: "model", type: "select", currentValue: model, options: MODELS }],
  models: { currentModelId: model, availableModels: MODELS.map((m) => ({ modelId: m.value, name: m.name, description: m.description })) },
});

const send = (msg) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...msg }) + "\n");
const notify = (method, params) => send({ method, params });
const request = (method, params) =>
  new Promise((resolve) => {
    const id = nextId++;
    waiting.set(id, resolve);
    send({ id, method, params });
  });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const update = (sessionId, u) => {
  const s = sessions.get(sessionId);
  if (s && u.sessionUpdate !== "available_commands_update" && u.sessionUpdate !== "usage_update") s.log.push(u);
  notify("session/update", { sessionId, update: u });
};
let used = 1200;
const say = async (sid, text, chunk = 12, ms = 15) => {
  for (let i = 0; i < text.length && !cancelled; i += chunk) {
    update(sid, { sessionUpdate: "agent_message_chunk", content: { type: "text", text: text.slice(i, i + chunk) } });
    await sleep(ms);
  }
};

// ------------------------------------------------------------ MCP over HTTP

let mcpSession = null;
let rpc = 1;
async function mcpPost(body) {
  const headers = { "content-type": "application/json", accept: "application/json, text/event-stream", "mcp-protocol-version": "2025-06-18" };
  for (const h of mcp.headers) headers[h.name.toLowerCase()] = h.value;
  if (mcpSession) headers["mcp-session-id"] = mcpSession;
  const res = await fetch(mcp.url, { method: "POST", headers, body: JSON.stringify(body) });
  const sid = res.headers.get("mcp-session-id");
  if (sid) mcpSession = sid;
  if (!("id" in body)) return null;
  const text = await res.text();
  if (!res.ok) throw new Error(`MCP HTTP ${res.status}: ${text}`);
  if ((res.headers.get("content-type") || "").includes("text/event-stream")) {
    for (const line of text.split("\n")) {
      if (line.startsWith("data:")) {
        const d = line.slice(5).trim();
        if (!d) continue;
        const m = JSON.parse(d);
        if (m.id === body.id) return m;
      }
    }
    throw new Error("no MCP response in the event stream");
  }
  return JSON.parse(text);
}
async function mcpInit() {
  if (mcpSession) return;
  await mcpPost({ jsonrpc: "2.0", id: rpc++, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "fake-acp-agent", version: "1" } } });
  await mcpPost({ jsonrpc: "2.0", method: "notifications/initialized" });
}
async function tool(name, args) {
  await mcpInit();
  const r = await mcpPost({ jsonrpc: "2.0", id: rpc++, method: "tools/call", params: { name, arguments: args } });
  if (r.error) throw new Error(r.error.message);
  const res = r.result;
  const out = res.structuredContent ?? JSON.parse(res.content?.find((c) => c.type === "text")?.text ?? "{}");
  if (res.isError) throw new Error(out.error ?? "tool failed");
  return out;
}

const OHM_HTML = `<!doctype html><html><head><title>Ohm's law</title><style>
html,body{margin:0;height:100%;font-family:var(--rupok-font-body,system-ui);background:var(--rupok-color-bg,#fff);color:var(--rupok-color-fg,#1d2433)}
body{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:28px}
h1{margin:0;font-size:48px}.row{display:flex;gap:40px;align-items:center;font-size:32px}
input{width:320px;height:48px;accent-color:var(--rupok-color-accent,#20c997)}
.out{font-size:64px;font-weight:700;color:var(--rupok-color-accent-text,#087f5b)}
</style></head><body><h1>Ohm's law: $I = V / R$</h1>
<div class="row"><label>Voltage <b id="vv">6</b> V</label><input id="v" type="range" min="1" max="12" value="6"></div>
<div class="row"><label>Resistance <b id="rv">3</b> Ω</label><input id="r" type="range" min="1" max="10" value="3"></div>
<div class="out" id="i">2.00 A</div>
<script>
var v=document.getElementById("v"),r=document.getElementById("r");
function upd(){document.getElementById("vv").textContent=v.value;document.getElementById("rv").textContent=r.value;
document.getElementById("i").textContent=(v.value/r.value).toFixed(2)+" A";}
v.oninput=upd;r.oninput=upd;upd();
</script></body></html>`;

async function ohm(sid) {
  await say(sid, "I'll build a short class on **Ohm's law**, where $V = IR$:\n\n");
  update(sid, { sessionUpdate: "plan", entries: [
    { content: "Add a page for Ohm's law", priority: "high", status: "in_progress" },
    { content: "Place a circuit simulation", priority: "high", status: "pending" },
    { content: "Add a practice question", priority: "medium", status: "pending" },
  ] });
  update(sid, { sessionUpdate: "tool_call", toolCallId: "t1", title: "mcp__rupok__add_page_after_current", kind: "edit", status: "pending", rawInput: { title: "Ohm's law" } });
  if (!alwaysAllowed) {
    const r = await request("session/request_permission", {
      sessionId: sid,
      toolCall: { toolCallId: "t1", title: "Add a page to your Rupok board", kind: "edit", rawInput: { title: "Ohm's law" } },
      options: [
        { optionId: "allow", name: "Allow", kind: "allow_once" },
        { optionId: "always", name: "Always allow", kind: "allow_always" },
        { optionId: "reject", name: "Reject", kind: "reject_once" },
      ],
    });
    const o = r.result?.outcome;
    if (!o || o.outcome !== "selected" || o.optionId === "reject") {
      update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t1", status: "failed" });
      await say(sid, "OK, I won't change the board.");
      return o?.outcome === "cancelled" ? "cancelled" : "end_turn";
    }
    if (o.optionId === "always") alwaysAllowed = true;
  }
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t1", status: "in_progress" });
  const p = await tool("add_page_after_current", { title: "Ohm's law" });
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t1", status: "completed" });

  update(sid, { sessionUpdate: "tool_call", toolCallId: "t2", title: "mcp__rupok__add_smart_object", kind: "edit", status: "in_progress", rawInput: { path: "live", page: p.page, title: "Ohm's law", width: 1600, height: 640 } });
  const o = await tool("add_smart_object", { path: "live", page: p.page, html: OHM_HTML, title: "Ohm's law", x: 160, y: 140, width: 1600, height: 640 });
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t2", status: "completed" });

  update(sid, { sessionUpdate: "tool_call", toolCallId: "t3", title: "mcp__rupok__add_text", kind: "edit", status: "in_progress" });
  await tool("add_text", { path: "live", page: p.page, text: "Practice: a 9 V battery drives 3 A. What is R?", x: 160, y: 860, size: 48 });
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "t3", status: "completed" });
  await tool("go_to_page", { page: p.page });
  update(sid, { sessionUpdate: "plan", entries: [
    { content: "Add a page for Ohm's law", priority: "high", status: "completed" },
    { content: "Place a circuit simulation", priority: "high", status: "completed" },
    { content: "Add a practice question", priority: "medium", status: "completed" },
  ] });
  await say(sid, `Done. Page ${p.page} has:\n\n- a live simulation: drag **V** and **R** and watch $I = V/R$\n- a practice question for the class (answer: $R = 3\\,\\Omega$)\n\nPress Ctrl+Z to undo any step.`);
  return "end_turn";
}

// The board's Solve this prompt: "... (item ids: a, b, c) ..." plus the
// recogniser's readings. Solves 2x + 3 = 7 (or says it would need to).
async function solveThis(sid, text) {
  const ids = (/item ids: ([^)]*)\)/.exec(text)?.[1] ?? "").split(",").map((s) => s.trim()).filter((s) => s && s !== "…");
  await say(sid, "Let me read the problem. ");
  update(sid, { sessionUpdate: "tool_call", toolCallId: "s1", title: "mcp__rupok__get_selection", kind: "read", status: "in_progress" });
  const sel = await tool("get_selection", {});
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "s1", status: "completed" });
  const items = sel.items ?? [];
  if (items.length) {
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const i of items) { const g = i.geometry; x0 = Math.min(x0, g.x); y0 = Math.min(y0, g.y); x1 = Math.max(x1, g.x + g.w); y1 = Math.max(y1, g.y + g.h); }
    update(sid, { sessionUpdate: "tool_call", toolCallId: "s2", title: "mcp__rupok__point_at", kind: "other", status: "in_progress" });
    await tool("point_at", { x: x0 - 10, y: y0 - 10, w: x1 - x0 + 20, h: y1 - y0 + 20, label: "the equation", seconds: 3 });
    update(sid, { sessionUpdate: "tool_call_update", toolCallId: "s2", status: "completed" });
  }
  const lines = [
    "Subtract 3 from both sides:",
    "$2x + 3 - 3 = 7 - 3$",
    "$2x = 4$",
    "Divide both sides by 2:",
    "$x = \\frac{4}{2}$",
    "$x = 2$",
  ];
  update(sid, { sessionUpdate: "tool_call", toolCallId: "s3", title: "mcp__rupok__write_handwriting", kind: "edit", status: "in_progress" });
  const r = await tool("write_handwriting", { lines, anchor: ids.length ? { item_ids: ids } : undefined });
  update(sid, { sessionUpdate: "tool_call_update", toolCallId: "s3", status: "completed" });
  const shown = r.status === "preview" ? "It's waiting for you to check it before the class sees it." : "It's on the board now.";
  await say(sid, `I solved $2x + 3 = 7$: subtract 3 from both sides to get $2x = 4$, then divide by 2, so $x = 2$. Check: $2 \\cdot 2 + 3 = 7$. ${shown}`);
  return "end_turn";
}

const STAR_SVG = '<svg viewBox="0 0 100 100"><path d="M50 6 61 38 95 38 67 58 78 92 50 72 22 92 33 58 5 38 39 38Z" fill="currentColor"/></svg>';
const DICE_HTML = `<!doctype html><html><head><title>Dice</title><style>html,body{margin:0;height:100%}
body{display:grid;place-items:center;font:120px system-ui;cursor:pointer;user-select:none}</style></head>
<body><div id="d">⚀</div><script>
var faces=["⚀","⚁","⚂","⚃","⚄","⚅"];
document.body.addEventListener("click",function(){document.getElementById("d").textContent=faces[Math.floor(Math.random()*6)];});
</script></body></html>`;

async function stamps(sid) {
  await say(sid, "Making your stamps…\n\n");
  const star = await tool("create_stamp", { name: "Super star", keywords: ["praise"], art: "svg", svg: STAR_SVG, color: "#f5b301",
    behavior: { animation: "sparkle", burst: "stars", counter: true, sound: "chime" } });
  const dice = await tool("create_stamp", { id: "dice", name: "Dice", art: "smart", html: DICE_HTML, width: 160, height: 160 });
  const a = await tool("add_stamp", { stamp: star.id, x: 200, y: 200 });
  await tool("add_stamp", { stamp: dice.id, x: 500, y: 200 });
  const s = await tool("draw_stroke", { points: [[100, 600], [400, 650], [700, 600]], brush: "marker" });
  await tool("restyle_items", { item_ids: [s.item_ids[0], a.item_id], color: "#e5484d" });
  await tool("list_stamps", {});
  await tool("duplicate_pages", { pages: [1] });
  await tool("move_pages", { pages: [2], to: 1 });
  await say(sid, `Done: **${star.stamp.name}** sparkles and counts, the **Dice** rolls when tapped. Both are in My stamps.`);
  return "end_turn";
}

async function prompt(params) {
  const sid = params.sessionId;
  cancelled = false;
  const text = (params.prompt || []).filter((b) => b.type === "text").map((b) => b.text).join("\n");
  const s = sessions.get(sid);
  if (s) {
    if (s.title === "New chat") s.title = text.slice(0, 40);
    s.updatedAt = new Date().toISOString();
    s.log.push({ sessionUpdate: "user_message_chunk", content: { type: "text", text } });
  }
  const res = (params.prompt || []).filter((b) => b.type === "resource");
  const imgs = (params.prompt || []).filter((b) => b.type === "image");
  try {
    if (/^Solve this problem/i.test(text)) return { stopReason: await solveThis(sid, text) };
    if (/context/i.test(text)) {
      const r = res[0]?.resource;
      let n = "no";
      try { n = String(JSON.parse(r?.text ?? "{}").items?.length ?? "no"); } catch { /* not JSON */ }
      const all = res.map((b) => `${b.resource.uri}${b.resource.blob ? " (file)" : ""}`);
      await say(sid, r ? `I got ${r.uri} with ${n} item(s)${imgs.length ? " and a snapshot" : ""}.` : "I got no board context.");
      if (all.length > 1 || imgs.length > 1) await say(sid, `\n\nAll context: ${all.join(", ")}; ${imgs.length} picture(s).`);
      return { stopReason: "end_turn" };
    }
    if (/^\/stamp/i.test(text)) return { stopReason: await stamps(sid) };
    if (/ohm/i.test(text)) return { stopReason: await ohm(sid) };
    if (/^\/review\b/.test(text)) {
      await say(sid, `Reviewing: ${text.slice(7).trim() || "the board"} (model ${model}, mode ${mode}).`);
      return { stopReason: "end_turn" };
    }
    if (/slow/i.test(text)) {
      for (let i = 0; i < 200 && !cancelled; i++) await say(sid, "thinking… ", 20, 100);
      return { stopReason: cancelled ? "cancelled" : "end_turn" };
    }
    await say(sid, `You said: ${text}`);
    return { stopReason: cancelled ? "cancelled" : "end_turn" };
  } catch (e) {
    await say(sid, `\n\nSomething failed: ${e.message}`);
    return { stopReason: "end_turn" };
  }
}

const rl = createInterface({ input: process.stdin });
rl.on("line", async (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  if (msg.method === undefined && msg.id !== undefined) {
    const w = waiting.get(msg.id);
    if (w) { waiting.delete(msg.id); w(msg); }
    return;
  }
  const reply = (result) => send({ id: msg.id, result });
  const fail = (code, message) => send({ id: msg.id, error: { code, message } });
  switch (msg.method) {
    case "initialize":
      return reply({
        protocolVersion: 1,
        agentCapabilities: { loadSession: true, sessionCapabilities: { list: {} }, promptCapabilities: { image: true, embeddedContext: true }, mcpCapabilities: { http: true, sse: false } },
        agentInfo: { name: "fake-acp-agent", title: "Fake Agent", version: "1.0.0" },
        authMethods: [],
      });
    case "session/new": {
      if (process.env.FAKE_ACP_AUTH === "1") return fail(-32000, "Authentication required");
      mcp = (msg.params.mcpServers || []).find((s) => s.type === "http" && s.name === "rupok") ?? null;
      mcpSession = null;
      const sid = `s${Date.now()}${Math.floor(Math.random() * 1000)}`;
      sessions.set(sid, { title: "New chat", updatedAt: new Date().toISOString(), log: [] });
      reply({ sessionId: sid, modes: { currentModeId: mode, availableModes: MODES }, ...modelState() });
      return setTimeout(() => notify("session/update", { sessionId: sid, update: { sessionUpdate: "available_commands_update", availableCommands: [
        { name: "review", description: "Review the board for mistakes", input: { hint: "what to review" } },
        { name: "compact", description: "Summarise the chat to save room" },
        { name: "init", description: "Write notes about this lesson folder" },
      ] } }), 20);
    }
    case "session/set_mode":
      mode = msg.params.modeId;
      alwaysAllowed = mode === "acceptEdits" || mode === "bypassPermissions";
      reply({});
      return update(msg.params.sessionId, { sessionUpdate: "current_mode_update", currentModeId: mode });
    case "session/prompt": {
      const r = await prompt(msg.params);
      used += 900;
      update(msg.params.sessionId, { sessionUpdate: "usage_update", used, size: 200000, cost: { amount: used / 1e6, currency: "USD" } });
      return reply(r);
    }
    case "session/list":
      return reply({ sessions: [...sessions.entries()].map(([id, x]) => ({ sessionId: id, cwd: msg.params.cwd, title: x.title, updatedAt: x.updatedAt })) });
    case "session/load": {
      const sid = msg.params.sessionId;
      const s = sessions.get(sid);
      if (!s) return fail(-32602, "unknown session");
      mcp = (msg.params.mcpServers || []).find((x) => x.type === "http" && x.name === "rupok") ?? mcp;
      mcpSession = null;
      for (const u of s.log) notify("session/update", { sessionId: sid, update: u });
      return reply({ modes: { currentModeId: mode, availableModes: MODES }, ...modelState() });
    }
    case "session/set_config_option":
      if (msg.params.configId === "model") model = msg.params.value;
      reply({ configOptions: modelState().configOptions });
      return update(msg.params.sessionId, { sessionUpdate: "config_option_update", configOptions: modelState().configOptions });
    case "session/set_model":
      model = msg.params.modelId;
      return reply({});
    case "session/cancel":
      cancelled = true;
      return;
    default:
      if (msg.id !== undefined) fail(-32601, `method not found: ${msg.method}`);
  }
});
rl.on("close", () => process.exit(0));
