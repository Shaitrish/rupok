// The AI chat panel: chat with your own agent inside Rupok (chat.ts).
// Prepare: docked on the right (or left), the slide re-fits beside it
// (controller insets). Teach: a sheet over the slide that hides itself while
// you draw. Keyboard-first: Enter sends, Shift+Enter new line, ↑ recalls,
// @ mentions board context, / runs commands, Esc closes menus.

import { For, Match, Show, Switch, createEffect, createMemo, createSignal, on, onCleanup, onMount, type JSX } from "solid-js";
import { controller } from "../controller";
import { isView, mode, pageIndex } from "../state";
import { livePairError, livePhase, liveHasChat, liveWaiting, live, liveAgent } from "../live";
import {
  SUGGESTIONS, agentCommands, agentSessions, answerPermission, attachPage, autoApprove, chat, chatAgent, chatAgents,
  chatBusy, chatHistory, chatOpen, chatPrefs, chatStatus, chatUsage, chatWidth, closeAiPanel, composerDraft, currentMode,
  currentModel, denyAllAndStop, editMessage, editsAutoApproved, forgetChat, formatUsage, goToPage, initChat,
  listAgentSessions, modeAutoApproves, models, modes, newChat, pendingPermissions, pickAgent, recallSent, refreshAgents,
  replaying, resumeAgentSession, resumeChat, retryAgent, retryLast, sendPrompt, setAttachPage, setAutoApprove,
  setChatOpen, setChatWidth, setMode, setModel, startRelay, stopChat, unqueue, type ChatItem,
} from "../chat";
import {
  MAX_FILE_BYTES, RUPOK_COMMANDS, fileAttachment, mentionSuggestions,
  type Attachment, type SlashCommand, type Suggestion,
} from "../chatContext";
import { cloudProviders } from "../chatProviders";
import { renderMarkdown, onMathReady } from "../markdown";
import { setAiPanelOpen } from "./AiStatus";
import { Icon } from "./Icon";
import { MicButton } from "./MicButton";
import { SolveChipAction } from "../solve/SolveUI";
import { openSettings } from "../settings/open";
import "../chat.css";

const DOCS = "https://github.com/shaitrish/rupok/blob/main/docs/ai-agents.md#chat-in-rupok";
const desktop = () => !!(window as any).__TAURI_INTERNALS__;

function copyText(text: string, done?: (v: boolean) => void) {
  navigator.clipboard?.writeText(text).then(() => { done?.(true); setTimeout(() => done?.(false), 1300); }, () => {});
}

function Copy(props: { text: string }) {
  const [ok, setOk] = createSignal(false);
  return <button class="chat-copy" onClick={() => copyText(props.text, setOk)}>{ok() ? "Copied" : "Copy"}</button>;
}

function Cmd(props: { text: string }) {
  return <div class="chat-cmd"><code>{props.text}</code><Copy text={props.text} /></div>;
}

/** Close a popover on outside click or Esc. */
function usePopover(open: () => boolean, close: () => void, root: () => HTMLElement | undefined) {
  const down = (e: PointerEvent) => { const r = root(); if (open() && r && !r.contains(e.target as Node)) close(); };
  const key = (e: KeyboardEvent) => { if (open() && e.key === "Escape") { e.stopPropagation(); close(); } };
  window.addEventListener("pointerdown", down, true);
  window.addEventListener("keydown", key, true);
  onCleanup(() => { window.removeEventListener("pointerdown", down, true); window.removeEventListener("keydown", key, true); });
}

/** Six digits for the code `rupok agent` shows. */
function CodeEntry(props: { port: number }) {
  const [code, setCode] = createSignal("");
  let input!: HTMLInputElement;
  onMount(() => input.focus());
  const submit = (e?: Event) => {
    e?.preventDefault();
    if (code().length === 6) live.pairWith(props.port, code());
  };
  return (
    <form class="chat-pair" onSubmit={submit}>
      <label class="chat-pair-label" for="chat-code">Enter the code shown where <code>rupok agent</code> is running</label>
      <input
        id="chat-code" ref={input} class="chat-code-input" inputmode="numeric" autocomplete="one-time-code" maxlength={7} placeholder="000 000"
        value={code().length > 3 ? `${code().slice(0, 3)} ${code().slice(3)}` : code()}
        onInput={(e) => {
          const v = e.currentTarget.value.replace(/\D/g, "").slice(0, 6);
          setCode(v);
          if (v.length === 6) live.pairWith(props.port, v);
        }}
        aria-describedby="chat-code-err"
      />
      <Show when={livePairError()}><div id="chat-code-err" class="chat-err" role="alert">{livePairError()}</div></Show>
      <button class="btn solid chat-wide" type="submit" disabled={code().length !== 6}><span class="label">Connect</span></button>
    </form>
  );
}

/** Not connected to a chat relay yet. */
function Connect() {
  const boardPair = () => liveWaiting().find((w) => w.pair === "board");
  const agentOnly = () => livePhase() === "pairing" && !boardPair() && liveWaiting().length > 0;
  return (
    <div class="chat-center">
      <Switch>
        <Match when={livePhase() === "searching"}>
          <div class="chat-spinner" aria-hidden="true" />
          <p class="chat-lead">Looking for Rupok's AI relay on this computer…</p>
        </Match>
        <Match when={boardPair()}>{(w) => <CodeEntry port={w().port} />}</Match>
        <Match when={livePhase() === "reconnecting"}>
          <div class="chat-spinner" aria-hidden="true" />
          <p class="chat-lead">Connection lost. Reconnecting…</p>
          <button class="btn" onClick={() => live.disconnect()}>Stop</button>
        </Match>
        <Match when={agentOnly() || (livePhase() === "connected" && !liveHasChat())}>
          <p class="chat-lead"><b>{liveAgent() ?? "Your AI"}</b> can edit this board, but chat inside Rupok needs the relay.</p>
          <p class="chat-p">Run this in a terminal on this computer, then try again:</p>
          <Cmd text="rupok agent" />
          <div class="chat-row">
            <button class="btn solid" onClick={() => startRelay()}><span class="label">Try again</span></button>
            <button class="btn" onClick={() => { closeAiPanel(); setAiPanelOpen(true); }}>Live edit settings</button>
          </div>
        </Match>
        <Match when={true}>
          <span class="chat-badge"><Icon name="smartink" size={22} /></span>
          <p class="chat-lead"><b>Chat with your own AI</b> and watch it build on the board.</p>
          <p class="chat-p">Claude Code, Codex, OpenCode or Gemini CLI runs on this computer with your own subscription. Rupok never sees your account.</p>
          <Show when={desktop()} fallback={<><p class="chat-p">Start the relay in a terminal (it shows a code), then connect:</p><Cmd text="rupok agent" /></>}>
            <p class="chat-p">Rupok starts your assistant for you.</p>
          </Show>
          <div class="chat-row">
            <button class="btn solid" onClick={() => startRelay()}><Icon name="smartink" size={18} /><span class="label">Connect</span></button>
            <a class="chat-link" href={DOCS} target="_blank" rel="noopener">Setup guide</a>
          </div>
        </Match>
      </Switch>
    </div>
  );
}

/** The agent cannot run: not installed, not logged in, or it stopped. */
function AgentProblem() {
  const st = chatStatus;
  const current = () => chatAgents().find((a) => a.id === (st().agent ?? chatAgent()));
  return (
    <div class="chat-center">
      <Switch>
        <Match when={st().code === "login"}>
          <p class="chat-lead"><b>Log in to {st().label ?? "your assistant"} once</b></p>
          <p class="chat-p">{st().login || current()?.login || "Run it once in a terminal and log in. It uses your own account."}</p>
          <Show when={(st().agent ?? chatAgent()) === "claude"}>
            <Cmd text="claude" />
            <p class="chat-hint">Type <code>/login</code> if it does not ask. It uses your Claude subscription.</p>
          </Show>
          <button class="btn solid chat-wide" onClick={retryAgent}><span class="label">I've logged in, try again</span></button>
        </Match>
        <Match when={st().code === "not_installed"}>
          <p class="chat-lead"><b>{st().label ? `${st().label} is not installed` : "No AI assistant installed"}</b></p>
          <p class="chat-p">Install one, then press Refresh. Each runs with your own account.</p>
          <For each={chatAgents().filter((a) => a.id !== "custom")}>
            {(a) => (
              <div class="chat-agent-row">
                <div class="chat-agent-name">
                  {a.label}
                  <Show when={a.available} fallback={<span class="chat-tag">not installed</span>}><span class="chat-tag ok">installed</span></Show>
                </div>
                <Show when={!a.available}><For each={a.install}>{(c) => <Cmd text={c} />}</For></Show>
                <Show when={a.available}><button class="btn soft" onClick={() => pickAgent(a.id)}>Use {a.label}</button></Show>
              </div>
            )}
          </For>
          <button class="btn chat-wide" onClick={refreshAgents}><span class="label">Refresh</span></button>
        </Match>
        <Match when={true}>
          <p class="chat-lead"><b>{st().label ?? "The assistant"} stopped</b></p>
          <Show when={st().error}><p class="chat-p chat-err-text">{st().error}</p></Show>
          <button class="btn solid chat-wide" onClick={retryAgent}><span class="label">Restart</span></button>
        </Match>
      </Switch>
    </div>
  );
}

// ------------------------------------------------------------ header pickers

/** A small menu button in the header. */
function Menu(props: { label: JSX.Element; title: string; class?: string; children: (close: () => void) => JSX.Element; disabled?: boolean }) {
  const [open, setOpen] = createSignal(false);
  let root!: HTMLDivElement;
  usePopover(open, () => setOpen(false), () => root);
  return (
    <div class="chat-picker" ref={root}>
      <button class={`chat-picker-btn ${props.class ?? ""}`} disabled={props.disabled} onClick={() => setOpen(!open())} aria-haspopup="menu" aria-expanded={open()} title={props.title}>
        {props.label}
        <Icon name="down" size={14} />
      </button>
      <Show when={open()}>
        <div class="chat-menu surface" role="menu">{props.children(() => setOpen(false))}</div>
      </Show>
    </div>
  );
}

function MenuItem(props: { label: string; note?: string; desc?: string; selected?: boolean; disabled?: boolean; onClick?: () => void }) {
  return (
    <button class="chat-menu-item" role="menuitemradio" aria-checked={!!props.selected} disabled={props.disabled} onClick={props.onClick} title={props.desc ?? ""}>
      <span class="chat-menu-main">
        <span>{props.label}</span>
        <Show when={props.desc}><span class="chat-menu-desc">{props.desc}</span></Show>
      </span>
      <span class="chat-menu-note">{props.selected ? "✓" : props.note ?? ""}</span>
    </button>
  );
}

function AgentPicker() {
  const current = () => chatAgents().find((a) => a.id === chatAgent());
  return (
    <Menu
      title="Assistant"
      label={<><span class="chat-dot" classList={{ live: chatStatus().state === "ready" || chatStatus().state === "busy", busy: chatBusy() }} /><span class="chat-picker-label">{current()?.label ?? chatStatus().label ?? "Assistant"}</span></>}
    >
      {(close) => (
        <>
          <div class="chat-menu-sep first">On this computer</div>
          <For each={chatAgents()}>
            {(a) => (
              <MenuItem
                label={a.label} selected={a.id === chatAgent()} disabled={!a.available}
                note={a.available ? (a.logged_in === false ? "log in first" : "") : "not installed"}
                onClick={() => { close(); if (a.id !== chatAgent()) pickAgent(a.id); }}
              />
            )}
          </For>
          <Show when={cloudProviders().length}>
            <div class="chat-menu-sep">Rupok Cloud</div>
            <For each={cloudProviders()}>{(p) => <MenuItem label={p.label} desc={p.note()} disabled={!p.available()} note={p.comingSoon ? "soon" : ""} />}</For>
          </Show>
          <div class="chat-menu-sep" />
          <MenuItem label="Refresh list" onClick={() => { close(); refreshAgents(); }} />
          <MenuItem label="Chat settings…" onClick={() => { close(); openSettings("ai.chatAgent"); }} />
        </>
      )}
    </Menu>
  );
}

function ModelPicker() {
  const name = () => models().find((m) => m.id === currentModel())?.name ?? currentModel() ?? "Model";
  return (
    <Show when={models().length}>
      <Menu title="Model" class="chip" label={<span class="chat-picker-label">{name()}</span>}>
        {(close) => <For each={models()}>{(m) => <MenuItem label={m.name} desc={m.description} selected={m.id === currentModel()} onClick={() => { close(); setModel(m.id); }} />}</For>}
      </Menu>
    </Show>
  );
}

function ModePicker() {
  const name = () => modes().find((m) => m.id === currentMode())?.name ?? "Mode";
  return (
    <Show when={modes().length}>
      <Menu title="Mode: how much the assistant may do without asking" class="chip" label={<span class="chat-picker-label">{name()}</span>}>
        {(close) => <For each={modes()}>{(m) => <MenuItem label={m.name} desc={m.description} selected={m.id === currentMode()} onClick={() => { close(); setMode(m.id); }} />}</For>}
      </Menu>
    </Show>
  );
}

/** Auto-approve board edits: this chat's toggle, or on because of the mode. */
function AutoApproveChip() {
  const byMode = modeAutoApproves;
  return (
    <button
      class="chat-auto"
      classList={{ on: editsAutoApproved() }}
      aria-pressed={editsAutoApproved()}
      disabled={byMode()}
      onClick={() => setAutoApprove(!autoApprove())}
      title={byMode() ? "The assistant's mode already makes changes without asking" : editsAutoApproved() ? "Board edits are approved automatically in this chat. Click to ask each time." : "Ask before each board edit. Click to approve them automatically in this chat."}
    >
      <span class="chat-auto-dot" aria-hidden="true" />
      {editsAutoApproved() ? "Auto-approve" : "Ask first"}
    </button>
  );
}

function History() {
  const [open, setOpen] = createSignal(false);
  let root!: HTMLDivElement;
  usePopover(open, () => setOpen(false), () => root);
  const fmt = (t: number | string) => new Date(t).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  const toggle = () => { const o = !open(); setOpen(o); if (o && chatStatus().caps?.list) listAgentSessions(); };
  const local = () => chatHistory();
  const remote = () => (agentSessions() ?? []).filter((s) => !local().some((h) => h.session === s.sessionId));
  return (
    <div class="chat-picker" ref={root}>
      <button class="btn icon" onClick={toggle} title="Chats on this lesson" aria-label="Chat history" aria-expanded={open()}><Icon name="replay" size={18} /></button>
      <Show when={open()}>
        <div class="chat-menu surface chat-history" role="menu">
          <div class="chat-menu-sep first">Chats on this lesson</div>
          <Show when={local().length} fallback={<div class="chat-menu-empty">No earlier chats yet.</div>}>
            <For each={local()}>
              {(h) => (
                <div class="chat-hist-row">
                  <button class="chat-menu-item" onClick={() => { setOpen(false); resumeChat(h); }} disabled={h.session === chatStatus().session}>
                    <span class="chat-menu-main"><span class="chat-hist-title">{h.title}</span><span class="chat-menu-desc">{h.label} · {fmt(h.updated)}</span></span>
                    <span class="chat-menu-note">{h.session === chatStatus().session ? "open" : ""}</span>
                  </button>
                  <button class="chat-hist-x" onClick={() => forgetChat(h.session)} aria-label={`Forget "${h.title}"`} title="Forget">×</button>
                </div>
              )}
            </For>
          </Show>
          <Show when={chatStatus().caps?.list}>
            <div class="chat-menu-sep">Also in {chatStatus().label ?? "the assistant"}</div>
            <Show when={agentSessions()} fallback={<div class="chat-menu-empty">Loading…</div>}>
              <Show when={remote().length} fallback={<div class="chat-menu-empty">Nothing else.</div>}>
                <For each={remote().slice(0, 8)}>
                  {(s) => <MenuItem label={s.title || "Untitled chat"} desc={s.updatedAt ? fmt(s.updatedAt) : undefined} onClick={() => { setOpen(false); resumeAgentSession(s.sessionId); }} />}
                </For>
              </Show>
            </Show>
          </Show>
          <div class="chat-menu-sep" />
          <MenuItem label="New chat" note="Ctrl ⇧ O" onClick={() => { setOpen(false); newChat(); }} />
        </div>
      </Show>
    </div>
  );
}

// ------------------------------------------------------------ conversation

function statusIcon(s: string) {
  return s === "completed" ? "✓" : s === "failed" ? "!" : "";
}

/** Allow once / Always for this session / Deny, from the agent's options. */
function permButtons(opts: { optionId: string; name: string; kind: string }[]) {
  const by = (k: string) => opts.find((o) => o.kind === k);
  const out: { optionId: string | null; label: string; primary?: boolean; deny?: boolean }[] = [];
  const once = by("allow_once"), always = by("allow_always"), reject = by("reject_once") ?? by("reject_always");
  if (once) out.push({ optionId: once.optionId, label: "Allow once", primary: true });
  if (always) out.push({ optionId: always.optionId, label: "Always for this session" });
  out.push({ optionId: reject?.optionId ?? null, label: "Deny", deny: true });
  if (!once && !always) for (const o of opts) if (!o.kind.startsWith("reject")) out.unshift({ optionId: o.optionId, label: o.name, primary: true });
  return out;
}

/** "mcp__rupok__add_smart_object" → "Rupok · add smart object". */
function prettyTool(t: string) {
  const m = /^mcp__([a-z0-9-]+)__(.+)$/i.exec(t);
  if (m) return `${m[1] === "rupok" ? "Rupok" : m[1]} · ${m[2].replace(/_/g, " ")}`;
  return t;
}

function Item(props: { item: ChatItem; last: boolean }) {
  // Re-render once KaTeX has loaded (maths shows as code until then).
  const [mathTick, setMathTick] = createSignal(0);
  onCleanup(onMathReady(() => setMathTick((n) => n + 1)));
  const [copied, setCopied] = createSignal(false);
  const it = props.item;
  switch (it.kind) {
    case "user":
      return (
        <div class="chat-msg user" classList={{ queued: !!it.queued }}>
          <div class="chat-bubble">{it.text}</div>
          <div class="chat-msg-foot">
            <Show when={it.context.length}><span class="chat-ctx-sent">{it.context.join(" · ")}</span></Show>
            <Show when={it.queued} fallback={<button class="chat-act" onClick={() => editMessage(it.text)} title="Edit and send again">Edit</button>}>
              <span class="chat-queued">Queued</span>
              <button class="chat-act" onClick={() => unqueue(it.id)}>Cancel</button>
            </Show>
          </div>
        </div>
      );
    case "agent":
      return (
        <div class="chat-msg agent">
          {/* renderMarkdown escapes all HTML first. */}
          <div class="chat-md" innerHTML={(mathTick(), renderMarkdown(it.text))} />
          <Show when={it.done}>
            <div class="chat-msg-foot agent">
              <button class="chat-act" onClick={() => copyText(it.text, setCopied)}>{copied() ? "Copied" : "Copy"}</button>
              <Show when={props.last && !chatBusy()}><button class="chat-act" onClick={retryLast}>Retry</button></Show>
            </div>
          </Show>
        </div>
      );
    case "thought":
      return (
        <details class="chat-thought">
          <summary>Thinking</summary>
          <div class="chat-md" innerHTML={renderMarkdown(it.text)} />
        </details>
      );
    case "plan": {
      const done = () => it.entries.filter((e) => e.status === "completed").length;
      return (
        <div class="chat-plan" role="group" aria-label="Plan">
          <div class="chat-plan-title">Plan <span class="chat-plan-count">{done()} / {it.entries.length}</span></div>
          <ul>
            <For each={it.entries}>
              {(e) => (
                <li classList={{ done: e.status === "completed", now: e.status === "in_progress" }}>
                  <span class="chat-check" aria-hidden="true">{e.status === "completed" ? "✓" : ""}</span>
                  <span>{e.content}</span>
                </li>
              )}
            </For>
          </ul>
        </div>
      );
    }
    case "notice":
      return <div class="chat-notice" classList={{ error: it.tone === "error" }}>{it.text}</div>;
    case "permission":
      return (
        <div class="chat-perm" role="group" aria-label="Permission request" classList={{ done: !!it.answered }}>
          <div class="chat-perm-title"><Icon name="smartink" size={16} /><span>{it.title}</span></div>
          <Show when={!it.answered && it.preview}>
            {(p) => (
              <figure class="chat-perm-preview">
                <img
                  src={`data:image/png;base64,${p().png}`}
                  alt={`Page ${p().page} before the change`}
                  onLoad={(e) => e.currentTarget.closest(".chat-perm")?.scrollIntoView({ block: "end" })}
                />
                <figcaption>Page {p().page} now · the change lands here</figcaption>
              </figure>
            )}
          </Show>
          <Show when={!it.answered && it.detail}>
            <details class="chat-details"><summary>Details</summary><pre>{it.detail}</pre></details>
          </Show>
          <Show when={!it.answered} fallback={<div class="chat-perm-answer">{it.answered}</div>}>
            <div class="chat-perm-actions">
              <For each={permButtons(it.options)}>
                {(b) => (
                  <button class={`btn ${b.primary ? "solid" : b.deny ? "chat-deny" : "soft"}`} onClick={() => answerPermission(it.id, it.reqId, b.optionId, b.label)}>
                    <span class="label">{b.label}</span>
                  </button>
                )}
              </For>
              <button class="btn chat-deny chat-deny-all" onClick={denyAllAndStop} title="Refuse this and stop the assistant">Deny all & stop</button>
            </div>
          </Show>
        </div>
      );
    case "tool":
      return (
        <div class="chat-tool" classList={{ running: it.status === "pending" || it.status === "in_progress", failed: it.status === "failed" }}>
          <div class="chat-tool-head">
            <span class="chat-tool-state" aria-label={it.status}>{statusIcon(it.status)}</span>
            <span class="chat-tool-title">{prettyTool(it.title)}</span>
          </div>
          <For each={it.changes}>
            {(c, i) => (
              <div class="chat-change">
                <span>{c.label}</span>
                {/* One jump link per page in a card: on its last change there. */}
                <Show when={c.page && !it.changes.slice(i() + 1).some((n) => n.page === c.page)}>
                  <button class="chat-jump" onClick={() => goToPage(c.page, c.itemId)}>Go to page {c.page}</button>
                </Show>
              </div>
            )}
          </For>
          <Show when={it.input || it.detail}>
            <details class="chat-details" open={it.status === "failed"}>
              <summary>Details</summary>
              <Show when={it.input}><pre>{it.input}</pre></Show>
              <Show when={it.detail}><pre classList={{ err: it.status === "failed" }}>{it.detail}</pre></Show>
            </details>
          </Show>
        </div>
      );
  }
}

// ------------------------------------------------------------------ composer

/** The token being typed at the caret: "@pa…" or "/qu…" (slash only at the start). */
function tokenAt(text: string, caret: number): { kind: "@" | "/"; start: number; query: string } | null {
  const before = text.slice(0, caret);
  const at = /(^|\s)@([^\n@]{0,40})$/.exec(before);
  // A mention is a few words ("@object Ohm's law"); past that it is the sentence.
  if (at && at[2].trim().split(/\s+/).length <= 4 && !/[—,.;:?!]\s/.test(at[2])) return { kind: "@", start: before.length - at[2].length - 1, query: at[2] };
  const sl = /^\/([\w-]*)$/.exec(before);
  if (sl) return { kind: "/", start: 0, query: sl[1] };
  return null;
}

function Composer() {
  const [text, setText] = createSignal("");
  const [atts, setAtts] = createSignal<Attachment[]>([]);
  const [menu, setMenu] = createSignal<{ kind: "@" | "/"; start: number; items: (Suggestion | SlashCommand)[] } | null>(null);
  const [sel, setSel] = createSignal(0);
  const [drag, setDrag] = createSignal(false);
  let recallN = -1;
  let ta!: HTMLTextAreaElement;
  let fileInput!: HTMLInputElement;
  const grow = () => { ta.style.height = "auto"; ta.style.height = `${Math.min(180, ta.scrollHeight)}px`; };
  const setValue = (v: string, caret = v.length) => { setText(v); ta.value = v; ta.setSelectionRange(caret, caret); grow(); };

  createEffect(on(composerDraft, (d) => { if (d) { setValue(d.text); ta.focus(); } }, { defer: true }));

  let menuSeq = 0;
  const updateMenu = async () => {
    const tok = tokenAt(text(), ta.selectionStart ?? text().length);
    if (!tok) return setMenu(null);
    const n = ++menuSeq;
    if (tok.kind === "/") {
      const q = tok.query.toLowerCase();
      const agentNames = new Set(agentCommands().map((x) => x.name));
      const all = [...agentCommands(), ...RUPOK_COMMANDS.filter((r) => !agentNames.has(r.name))];
      const items = all.filter((x) => x.name.toLowerCase().startsWith(q));
      setMenu(items.length ? { kind: "/", start: 0, items } : null);
    } else {
      const items = await mentionSuggestions(controller, tok.query);
      if (n !== menuSeq) return;
      setMenu(items.length ? { kind: "@", start: tok.start, items } : null);
    }
    setSel(0);
  };

  const addFiles = async (files: FileList | File[]) => {
    for (const f of Array.from(files)) {
      if (f.size > MAX_FILE_BYTES) { alert(`${f.name} is larger than ${MAX_FILE_BYTES >> 20} MB.`); continue; }
      const a = await fileAttachment(f);
      setAtts((l) => [...l, a]);
    }
  };

  const accept = async (i = sel()) => {
    const m = menu();
    if (!m) return;
    const item = m.items[i];
    const caret = ta.selectionStart ?? text().length;
    if (m.kind === "/") {
      const c = item as SlashCommand;
      setValue(`/${c.name} `);
    } else {
      const s = item as Suggestion;
      const before = text().slice(0, m.start), after = text().slice(caret);
      if (s.make === "file") {
        setValue(before + after, before.length);
        fileInput.click();
      } else {
        const a = await s.make();
        setAtts((l) => [...l.filter((x) => x.label !== a.label), a]);
        const ins = `${s.insert} `;
        setValue(before + ins + after, (before + ins).length);
      }
    }
    setMenu(null);
    ta.focus();
  };

  const send = () => {
    const t = text();
    if (!t.trim()) return;
    // Mentions removed from the text drop their attachment.
    const keep = atts().filter((a) => a.kind === "file" || a.kind === "image" || t.includes(a.label) || /@(page|selection|object|lesson)/.test(t));
    void sendPrompt(t, keep);
    setValue("");
    setAtts([]);
    setMenu(null);
    recallN = -1;
  };

  const onKey = (e: KeyboardEvent) => {
    e.stopPropagation(); // keep board shortcuts out of the chat
    const m = menu();
    if (m) {
      if (e.key === "ArrowDown") { e.preventDefault(); setSel((sel() + 1) % m.items.length); return; }
      if (e.key === "ArrowUp") { e.preventDefault(); setSel((sel() - 1 + m.items.length) % m.items.length); return; }
      if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") { e.preventDefault(); void accept(); return; }
      if (e.key === "Escape") { e.preventDefault(); setMenu(null); return; }
    }
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); send(); return; }
    if (e.key === "ArrowUp" && (!text() || recallN >= 0) && ta.selectionStart === 0) {
      const t = recallSent(recallN + 1);
      if (t != null) { e.preventDefault(); recallN++; setValue(t); }
      return;
    }
    if (e.key === "ArrowDown" && recallN >= 0) {
      e.preventDefault();
      recallN--;
      setValue(recallN >= 0 ? recallSent(recallN) ?? "" : "");
      return;
    }
    if (e.key === "Escape" && chatBusy()) { e.preventDefault(); stopChat(); }
  };

  const page = () => pageIndex() + 1;
  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDrag(false);
    if (e.dataTransfer?.files.length) void addFiles(e.dataTransfer.files);
  };
  const onPaste = (e: ClipboardEvent) => {
    const files = Array.from(e.clipboardData?.files ?? []);
    if (files.length) { e.preventDefault(); void addFiles(files); }
  };

  return (
    <div
      class="chat-composer"
      classList={{ drag: drag() }}
      onDragOver={(e) => { if (e.dataTransfer?.types.includes("Files")) { e.preventDefault(); setDrag(true); } }}
      onDragLeave={(e) => { if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) setDrag(false); }}
      onDrop={onDrop}
    >
      <Show when={menu()}>
        {(m) => (
          <div class="chat-suggest-menu surface" role="listbox" aria-label={m().kind === "/" ? "Commands" : "Mention"}>
            <For each={m().items}>
              {(item, i) => {
                const isCmd = m().kind === "/";
                const c = item as SlashCommand, s = item as Suggestion;
                return (
                  <button
                    class="chat-sugg-item" role="option" aria-selected={i() === sel()} classList={{ on: i() === sel() }}
                    onPointerEnter={() => setSel(i())} onPointerDown={(e) => { e.preventDefault(); void accept(i()); }}
                  >
                    <span class="chat-sugg-main">{isCmd ? `/${c.name}` : `@${s.label}`}<Show when={isCmd && c.hint}><span class="chat-sugg-hint"> {c.hint}</span></Show></span>
                    <span class="chat-sugg-desc">{isCmd ? c.description : s.hint ?? ""}</span>
                    <Show when={isCmd}><span class="chat-sugg-src">{c.source === "agent" ? chatStatus().label ?? "agent" : "Rupok"}</span></Show>
                  </button>
                );
              }}
            </For>
          </div>
        )}
      </Show>
      <div class="chat-ctx">
        <button class="chat-chip" classList={{ on: attachPage() }} onClick={() => setAttachPage(!attachPage())} aria-pressed={attachPage()} title="Send the current page (or your selection) with the message">
          <Icon name="layers" size={14} />
          {attachPage() ? `Page ${page()}` : "Attach page"}
        </button>
        <For each={atts()}>
          {(a) => (
            <span class="chat-chip on att">
              <Show when={a.thumb} fallback={<span class="chat-att-kind">{a.kind === "lesson" ? "Lesson" : a.kind === "file" ? "File" : a.kind === "object" ? "Object" : ""}</span>}>
                <img class="chat-att-thumb" src={a.thumb} alt="" />
              </Show>
              <span class="chat-att-label">{a.label}</span>
              <button class="chat-att-x" onClick={() => setAtts((l) => l.filter((x) => x.id !== a.id))} aria-label={`Remove ${a.label}`}>×</button>
            </span>
          )}
        </For>
        <button class="chat-chip" onClick={() => fileInput.click()} title="Attach a picture, PDF or lesson file"><Icon name="plus" size={14} />File</button>
        <input ref={fileInput} type="file" multiple hidden accept="image/*,application/pdf,.rpk,.txt,.md,.csv,.json" onChange={(e) => { if (e.currentTarget.files) void addFiles(e.currentTarget.files); e.currentTarget.value = ""; }} />
      </div>
      <div class="chat-input-row">
        <textarea
          ref={ta} class="chat-input" rows={1}
          placeholder={chatBusy() ? "Add a follow-up…" : "Ask your assistant…"}
          value={text()}
          onInput={(e) => { setText(e.currentTarget.value); grow(); void updateMenu(); }}
          onClick={() => void updateMenu()}
          onKeyDown={onKey}
          onPaste={onPaste}
          aria-label="Message"
        />
        {/* Voice input (voice/): dictates at the caret; renders nothing when off in Settings. Alt+V too. */}
        <MicButton target={() => ta} size={32} showLang class="chat-mic" label="Dictate a message" />
        <Show when={chatBusy() && !text().trim()} fallback={
          <button class="btn icon solid chat-send" onClick={send} disabled={!text().trim()} aria-label={chatBusy() ? "Queue" : "Send"} title={chatBusy() ? "Queue · Enter" : "Send · Enter"}><Icon name="up" size={18} /></button>
        }>
          <button class="btn icon chat-stop" onClick={stopChat} aria-label="Stop" title="Stop · Esc"><span class="chat-stop-sq" /></button>
        </Show>
      </div>
      <div class="chat-foot">
        <span>{chatBusy() ? "Esc stops · Enter queues" : "@ mention · / commands · ↑ recall"}</span>
        <Show when={chatUsage()}>{(u) => <span class="chat-usage" title="Context used by this chat">{formatUsage(u())}</span>}</Show>
      </div>
      <Show when={drag()}><div class="chat-drop">Drop pictures, PDFs or lessons to attach</div></Show>
    </div>
  );
}

function Conversation() {
  let list!: HTMLDivElement;
  let stick = true;
  const lastAgent = createMemo(() => [...chat.items].reverse().find((x) => x.kind === "agent")?.id);
  createEffect(on(() => [chat.items.length, chat.items[chat.items.length - 1]], () => {
    if (stick) queueMicrotask(() => list && (list.scrollTop = list.scrollHeight));
  }));
  const obs = new MutationObserver(() => { if (stick) list.scrollTop = list.scrollHeight; });
  onMount(() => obs.observe(list, { childList: true, subtree: true, characterData: true }));
  onCleanup(() => obs.disconnect());
  return (
    <>
      <div class="chat-list" ref={list} onScroll={() => { stick = list.scrollHeight - list.scrollTop - list.clientHeight < 40; }} aria-live="polite">
        <Show when={chat.items.length === 0 && !replaying()}>
          <div class="chat-empty">
            <p class="chat-lead"><b>What shall we build?</b></p>
            <p class="chat-p">Your assistant sees this board and builds on it live. Every change is one undo step.</p>
            <div class="chat-suggest">
              <For each={SUGGESTIONS}>{(s) => <button class="chat-sugg" onClick={() => void sendPrompt(s)}>{s}</button>}</For>
            </div>
            <p class="chat-hint">Type <kbd>@</kbd> to point at a page, your selection, a Smart Object or a lesson, and <kbd>/</kbd> for commands.</p>
          </div>
        </Show>
        <Show when={replaying()}><div class="chat-notice">Reopening the chat…</div></Show>
        <For each={chat.items}>{(it) => <Item item={it} last={it.id === lastAgent()} />}</For>
        <Show when={chatStatus().state === "starting"}><div class="chat-notice">Starting {chatStatus().label ?? "the assistant"}…</div></Show>
        <Show when={chatStatus().state === "busy" && !pendingPermissions().length}>
          <div class="chat-typing" aria-label="Working"><span /><span /><span /></div>
        </Show>
      </div>
      <Show when={pendingPermissions().length > 1}>
        <div class="chat-perm-bar">
          {pendingPermissions().length} requests waiting
          <button class="btn chat-deny" onClick={denyAllAndStop}>Deny all & stop</button>
        </div>
      </Show>
      <Show when={chatStatus().warning}><div class="chat-notice error chat-warn">{chatStatus().warning}</div></Show>
      <Composer />
    </>
  );
}

export default function ChatPanel() {
  const [hidden, setHidden] = createSignal(false); // Teach: tucked away while drawing
  onMount(() => initChat(controller));
  const teach = () => mode() === "teach";
  const docked = () => chatOpen() && !teach() && !isView();

  // Prepare: give the panel's width to the slide (re-fit beside it).
  const side = () => chatPrefs.read().position;
  createEffect(on([docked, chatWidth, side], ([d, w, sd]) => {
    controller.chatSide = sd as "left" | "right";
    controller.chatInset = d ? (w as number) + 12 : 0;
    controller.updateInsets(true);
  }));
  // A relay that offers chat (or asks for its code) opens the chat panel.
  createEffect(on([liveHasChat, () => liveWaiting().some((w) => w.pair === "board")], ([has, ask]) => {
    if ((has || ask) && !isView()) { setAiPanelOpen(false); setChatOpen(true); }
  }, { defer: true }));
  // Teach: tuck the sheet away when the teacher draws on the slide.
  onMount(() => {
    const down = (e: PointerEvent) => {
      if (!teach() || !chatOpen()) return;
      const t = e.target as Element;
      if (t.closest?.(".chat-panel, .chat-peek, [data-ai-toggle], .topbar, .dock")) return;
      setHidden(true);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && chatOpen() && teach()) setHidden(true);
      // Ctrl+Shift+O: new chat (as in Claude's apps).
      if (chatOpen() && liveHasChat() && (e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "o") { e.preventDefault(); newChat(); }
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("keydown", key);
    onCleanup(() => { window.removeEventListener("pointerdown", down, true); window.removeEventListener("keydown", key); });
  });
  createEffect(on(chatOpen, (o) => { if (o) setHidden(false); }));

  // Resize from the inner edge.
  const startResize = (e: PointerEvent) => {
    e.preventDefault();
    const x0 = e.clientX, w0 = chatWidth();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const dir = side() === "left" ? -1 : 1;
    const move = (ev: PointerEvent) => setChatWidth(w0 + dir * (x0 - ev.clientX));
    const up = () => { el.removeEventListener("pointermove", move); el.removeEventListener("pointerup", up); };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  };

  const chatReady = () => liveHasChat() && chatStatus().state !== "error" && chatStatus().state !== "offline";
  return (
    <Show when={chatOpen()}>
      <Show
        when={!(teach() && hidden())}
        fallback={
          <div class="solve-peek-row">
            <SolveChipAction />
            <button class="chat-peek surface" onClick={() => setHidden(false)} aria-label="Show the AI chat">
              <span class="chat-dot live" classList={{ busy: chatBusy() }} />
              <span>{pendingPermissions().length ? "AI needs your OK" : chatBusy() ? `${chatStatus().label ?? "AI"} is working…` : "AI chat"}</span>
            </button>
          </div>
        }
      >
        <aside class="chat-panel surface" classList={{ overlay: teach(), left: side() === "left" }} style={{ width: `${chatWidth()}px` }} role="complementary" aria-label="AI chat">
          <div class="chat-resize" onPointerDown={startResize} role="separator" aria-orientation="vertical" aria-label="Resize the chat" />
          <header class="chat-head">
            <Show when={chatReady()} fallback={<div class="chat-title">AI chat</div>}><AgentPicker /></Show>
            <div class="chat-head-actions">
              <Show when={chatReady()}>
                <History />
                <button class="btn icon" onClick={newChat} title="New chat · Ctrl ⇧ O" aria-label="New chat"><Icon name="plus" size={18} /></button>
              </Show>
              <Show when={teach()}>
                <button class="btn icon" onClick={() => setHidden(true)} title="Hide (Esc)" aria-label="Hide the chat"><Icon name="down" size={18} /></button>
              </Show>
              <button class="btn icon" onClick={() => closeAiPanel()} title="Close" aria-label="Close the chat"><Icon name="close" size={18} /></button>
            </div>
          </header>
          <Show when={chatReady() && (models().length || modes().length || chatStatus().session)}>
            <div class="chat-subhead">
              <ModelPicker />
              <ModePicker />
              <span class="chat-subhead-gap" />
              <AutoApproveChip />
            </div>
          </Show>
          <Switch>
            <Match when={isView()}>
              <div class="chat-center"><p class="chat-lead">Chat is off in Student view.</p><p class="chat-p">Switch to Prepare or Teach to use your assistant.</p></div>
            </Match>
            <Match when={!liveHasChat()}><Connect /></Match>
            <Match when={chatStatus().state === "error" || chatStatus().state === "offline"}><AgentProblem /></Match>
            <Match when={true}><Conversation /></Match>
          </Switch>
        </aside>
      </Show>
    </Show>
  );
}
