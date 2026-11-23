// Settings → AI & agents: Connect AI (live.ts, the teacher's own agent
// editing the board) and handwriting to text (handwriting.ts). The Claude
// key never enters prefs: see handwriting.ts.

import { Match, Show, Switch, createResource, createSignal } from "solid-js";
import { claudeKey, keyVersion, pickProvider, setClaudeKey } from "../../handwriting";
import { live, liveAgent, livePhase, liveWaiting } from "../../live";
import { isView, showToast } from "../../state";
import type { SettingProps } from "../../settings/registry";
import { Icon } from "../Icon";

const SETUP_CMD = "claude mcp add rupok -- rupok mcp --live";
const DOCS = "https://github.com/shaitrish/rupok/blob/main/docs/ai-agents.md";

export function ConnectAiSection(props: SettingProps) {
  const [copied, setCopied] = createSignal(false);
  const copy = () => navigator.clipboard?.writeText(SETUP_CMD).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1400); }, () => {});
  const phase = livePhase;
  return (
    <div class="st-connect">
      <div class="st-connect-top">
        <span class="st-dot" classList={{ live: phase() === "connected", wait: ["searching", "pairing", "reconnecting"].includes(phase()) }} aria-hidden="true" />
        <div class="st-row-text">
          <div class="st-label" id={props.labelId}>Connect AI</div>
          <div class="st-help" id={props.descId} role="status">
            <Switch fallback="Let your own AI assistant (Claude Code, Claude Desktop, Codex…) see and edit this lesson live.">
              <Match when={phase() === "connected"}>{liveAgent() ?? "Your AI"} is connected to this board.</Match>
              <Match when={phase() === "searching"}>Looking for an AI assistant on this computer…</Match>
              <Match when={phase() === "pairing"}>
                Tell your AI the code {liveWaiting()[0]?.code ?? ""} (the pairing card at the top of the board has it too).
              </Match>
              <Match when={phase() === "reconnecting"}>Reconnecting…</Match>
              <Match when={phase() === "unreachable"}>No AI assistant found. Run the setup command below once, then connect.</Match>
            </Switch>
          </div>
        </div>
        <Switch
          fallback={
            <button type="button" class="st-btn solid" disabled={isView()} onClick={() => live.connect()}>
              <Icon name="smartink" size={18} /><span>Connect</span>
            </button>
          }
        >
          <Match when={phase() === "connected" || phase() === "reconnecting"}>
            <button type="button" class="st-btn" onClick={() => live.disconnect()}>Disconnect</button>
          </Match>
          <Match when={phase() === "searching" || phase() === "pairing"}>
            <button type="button" class="st-btn" onClick={() => live.cancel()}>Cancel</button>
          </Match>
        </Switch>
      </div>
      <div class="st-connect-setup">
        <span class="st-help">One-time setup in a terminal:</span>
        <code class="st-code">{SETUP_CMD}</code>
        <button type="button" class="st-btn ghost small" onClick={copy}>{copied() ? "Copied" : "Copy"}</button>
        <a class="st-link" href={DOCS} target="_blank" rel="noreferrer">Setup guide</a>
      </div>
    </div>
  );
}

export function HandwritingStatus(props: SettingProps) {
  const [active] = createResource(keyVersion, () => pickProvider());
  return (
    <span class="st-status" aria-labelledby={props.labelId}>
      <Show when={active()} fallback={<><span class="st-dot" aria-hidden="true" />Off</>}>
        {(p) => <><span class="st-dot live" aria-hidden="true" />{p().label}</>}
      </Show>
    </span>
  );
}

export function ClaudeKeyField(props: SettingProps) {
  const [draft, setDraft] = createSignal("");
  const hasKey = () => (keyVersion(), claudeKey() !== "");
  const save = () => {
    const k = draft().trim();
    if (!k.startsWith("sk-ant-")) {
      showToast("That doesn't look like a Claude API key (they start with sk-ant-)");
      return;
    }
    setClaudeKey(k);
    setDraft("");
    showToast("Key saved on this device");
  };
  return (
    <Show
      when={!hasKey()}
      fallback={
        <div class="st-inline">
          <span class="st-status"><span class="st-dot live" aria-hidden="true" />Saved on this device</span>
          <button type="button" class="st-btn danger" onClick={() => { setClaudeKey(""); showToast("Key removed"); }}>Remove</button>
        </div>
      }
    >
      <div class="st-inline">
        <input
          class="st-input"
          type="password"
          autocomplete="off"
          spellcheck={false}
          placeholder="sk-ant-…"
          aria-labelledby={props.labelId}
          aria-describedby={props.descId}
          value={draft()}
          onInput={(e) => setDraft(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && save()}
        />
        <button type="button" class="st-btn solid" disabled={!draft().trim()} onClick={save}>Save</button>
      </div>
    </Show>
  );
}
