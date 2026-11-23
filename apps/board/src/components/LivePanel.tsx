// "Connect AI": pair the board with the teacher's own AI agent and show what
// it is doing. See ../live.ts for the protocol and docs/ai-agents.md for setup.

import { For, Match, Show, Switch, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { camera, isView, items, pageIndex, revision } from "../state";
import {
  allowEdit, live, liveActivity, liveAgent, liveEditing, liveLna, livePhase, livePointer,
  liveWaiting, setLivePointer,
} from "../live";
import { Icon } from "./Icon";
import { aiPanelOpen, setAiPanelOpen } from "./AiStatus";
import { openAiPanel } from "../chat";
import "../live.css";

const DOCS = "https://github.com/shaitrish/rupok/blob/main/docs/ai-agents.md";
const SETUP_CMD = "claude mcp add rupok -- rupok mcp --live";

const fmtCode = (c: string) => `${c.slice(0, 3)} ${c.slice(3)}`;
const fmtTime = (t: number) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

function copy(text: string, done: (v: boolean) => void) {
  navigator.clipboard?.writeText(text).then(() => { done(true); setTimeout(() => done(false), 1400); }, () => {});
}

function CopyButton(props: { text: string; label?: string }) {
  const [ok, setOk] = createSignal(false);
  return (
    <button class="ai-copy" onClick={() => copy(props.text, setOk)} title="Copy">
      {ok() ? "Copied" : props.label ?? "Copy"}
    </button>
  );
}

function Toggle(props: { on: boolean; onChange: (v: boolean) => void; label: string; hint: string }) {
  return (
    <label class="ai-toggle">
      <span class="ai-toggle-text">
        <span class="ai-toggle-label">{props.label}</span>
        <span class="ai-toggle-hint">{props.hint}</span>
      </span>
      <input type="checkbox" role="switch" checked={props.on} onChange={(e) => props.onChange(e.currentTarget.checked)} />
      <span class="ai-switch" aria-hidden="true" />
    </label>
  );
}

/** Browser note for Chrome's Local Network Access prompt on https pages. */
function LnaNote() {
  const https = location.protocol === "https:";
  return (
    <Switch>
      <Match when={liveLna() === "denied"}>
        <div class="ai-note warn">
          Your browser is blocking this site from reaching apps on this computer. Click the icon at the left of the
          address bar, open <b>Site settings</b>, set <b>Local network access</b> to <b>Allow</b>, then try again.
        </div>
      </Match>
      <Match when={https && liveLna() === "prompt"}>
        <div class="ai-note">
          Your browser will ask to let Rupok connect to apps on this device. Choose <b>Allow</b>: Rupok only talks to
          the AI assistant running on this computer.
        </div>
      </Match>
    </Switch>
  );
}

function PairingCard(props: { agent: string; code: string }) {
  const say = () => `Connect to my Rupok board, code ${props.code}`;
  return (
    <div class="ai-card">
      <div class="ai-card-agent"><span class="ai-dot" />{props.agent}</div>
      <div class="ai-code" aria-label={`Pairing code ${props.code.split("").join(" ")}`}>{fmtCode(props.code)}</div>
      <div class="ai-say">Tell {props.agent}: <q>{say()}</q></div>
      <CopyButton text={say()} label="Copy message" />
    </div>
  );
}

/** A laser ring or highlight box the agent asked for, in slide coordinates. */
function PointerOverlay() {
  let timer: number | undefined;
  createEffect(on(livePointer, (p) => {
    clearTimeout(timer);
    if (p) timer = window.setTimeout(() => setLivePointer(null), Math.max(0, p.until - Date.now()));
  }));
  onCleanup(() => clearTimeout(timer));
  return (
    <Show when={livePointer() && livePointer()!.page === pageIndex() + 1 ? livePointer() : null}>
      {(p) => {
        const box = () => {
          const [cx, cy, z] = camera();
          const q = p();
          const w = q.w ?? 0, h = q.h ?? 0;
          return { x: (q.x - cx) * z, y: (q.y - cy) * z, w: w * z, h: h * z, ring: !(q.w && q.h) };
        };
        return (
          <div class="ai-pointer-layer" aria-live="polite">
            <Show
              when={!box().ring}
              fallback={<div class="ai-ring" style={{ left: `${box().x}px`, top: `${box().y}px` }} />}
            >
              <div class="ai-box" style={{ left: `${box().x}px`, top: `${box().y}px`, width: `${box().w}px`, height: `${box().h}px` }} />
            </Show>
            <Show when={p().label}>
              <div
                class="ai-pointer-label"
                style={{ left: `${box().ring ? box().x + 36 : box().x}px`, top: `${box().ring ? box().y - 14 : box().y + box().h + 8}px` }}
              >
                {p().label}
              </div>
            </Show>
          </div>
        );
      }}
    </Show>
  );
}

/** Small tags on the items an AI made on this page. */
function AiMarks() {
  const [who, setWho] = createSignal<Record<string, string>>({});
  let t: number | undefined;
  createEffect(on([revision, pageIndex], () => {
    clearTimeout(t);
    t = window.setTimeout(() => setWho(live.attribution()), 250);
  }));
  onCleanup(() => clearTimeout(t));
  return (
    <div class="ai-marks" aria-hidden="true">
      <For each={items.filter((i) => who()[i.id])}>
        {(it) => {
          const pos = () => {
            const [cx, cy, z] = camera();
            return { left: `${(it.x - cx) * z}px`, top: `${(it.y - cy) * z}px` };
          };
          return <div class="ai-mark" style={pos()} title={`Made by ${who()[it.id]}`}>AI · {who()[it.id]}</div>;
        }}
      </For>
    </div>
  );
}

export default function LivePanel() {
  // Opened from the AI status button in the top bar (AiStatus.tsx) or the ⋯ menu.
  const open = aiPanelOpen, setOpen = setAiPanelOpen;
  const [marks, setMarks] = createSignal(false);
  let root!: HTMLDivElement;

  onMount(() => {
    live.init(controller);
    const away = (e: PointerEvent) => {
      const t = e.target as Element;
      // The header's AI button toggles the panel itself.
      if (open() && !root.contains(t) && !t.closest?.("[data-ai-toggle]")) setOpen(false);
    };
    window.addEventListener("pointerdown", away, true);
    onCleanup(() => window.removeEventListener("pointerdown", away, true));
  });
  // Pairing codes and setup help open the panel by themselves.
  createEffect(on(livePhase, (p) => { if (p === "pairing" || p === "unreachable") setOpen(true); }, { defer: true }));
  const agent = () => liveAgent() ?? "AI";

  return (
    <Show when={!isView()}>
      <div class="ai-dock" ref={root}>
        <Show when={open()}>
          <div class="ai-panel surface" role="dialog" aria-label="AI assistant">
            <div class="ai-head">
              <span class="ai-badge"><Icon name="smartink" size={18} /></span>
              <div class="ai-head-text">
                <div class="ai-title">AI assistant</div>
                <div class="ai-sub">Your own AI, editing this board live</div>
              </div>
              <button class="btn icon" onClick={() => setOpen(false)} aria-label="Close"><Icon name="close" size={18} /></button>
            </div>

            <Switch>
              <Match when={livePhase() === "idle"}>
                <p class="ai-p">
                  Ask Claude, Codex or OpenCode to make a quiz, a simulation or a clean diagram, and watch it appear here.
                  It runs on this computer with your own subscription; Rupok never sees your account.
                </p>
                <LnaNote />
                <div class="ai-actions">
                  <button class="btn solid" onClick={() => live.connect()}><Icon name="smartink" size={18} /><span class="label">Connect</span></button>
                  <a class="ai-link" href={DOCS} target="_blank" rel="noopener">Set up your AI</a>
                </div>
              </Match>
              <Match when={livePhase() === "searching"}>
                <p class="ai-p">Looking for an AI assistant on this computer…</p>
                <LnaNote />
              </Match>
              <Match when={livePhase() === "pairing"}>
                <For each={liveWaiting()}>{(w) => <PairingCard agent={w.agent} code={w.code} />}</For>
                <p class="ai-hint">The code works once. You stay connected until you disconnect or close this tab.</p>
                <div class="ai-actions"><button class="btn" onClick={() => live.cancel()}>Cancel</button></div>
              </Match>
              <Match when={livePhase() === "unreachable"}>
                <p class="ai-p"><b>No AI assistant found on this computer.</b></p>
                <LnaNote />
                <ol class="ai-steps">
                  <li>Install the <code>rupok</code> command (see the setup guide).</li>
                  <li>
                    Add it to your AI. For Claude Code:
                    <div class="ai-cmd"><code>{SETUP_CMD}</code><CopyButton text={SETUP_CMD} /></div>
                  </li>
                  <li>Start (or restart) your AI, then try again.</li>
                </ol>
                <p class="ai-p">
                  Or chat with it inside Rupok: run <code>rupok agent</code> in a terminal and enter the code it shows.
                </p>
                <div class="ai-actions">
                  <button class="btn solid" onClick={() => live.connect()}><span class="label">Try again</span></button>
                  <button class="btn" onClick={() => { setOpen(false); openAiPanel(); }}><span class="label">Chat in Rupok</span></button>
                  <a class="ai-link" href={DOCS} target="_blank" rel="noopener">Setup guide</a>
                </div>
              </Match>
              <Match when={livePhase() === "reconnecting"}>
                <p class="ai-p">Lost the connection to {agent()}. Reconnecting…</p>
                <div class="ai-actions"><button class="btn" onClick={() => live.disconnect()}>Stop</button></div>
              </Match>
              <Match when={livePhase() === "connected"}>
                <div class="ai-agent">
                  <span class="ai-dot live" classList={{ pulse: !!liveEditing() }} />
                  <div>
                    <div class="ai-agent-name">{agent()}</div>
                    <div class="ai-sub">{liveEditing() ? `${liveEditing()!.label}…` : "Connected. Ask it anything about this board."}</div>
                  </div>
                </div>
                <Toggle
                  on={allowEdit()}
                  onChange={(v) => live.setAllowEdit(v)}
                  label="Allow AI to edit live"
                  hint={allowEdit() ? "Every change is one undo step (Ctrl Z)." : "Off: it can look at the board but not change it."}
                />
                <Toggle on={marks()} onChange={setMarks} label="Mark AI edits" hint="Tag what the AI made on this page." />
                <Show when={liveActivity().length}>
                  <div class="ai-section">Activity</div>
                  <ul class="ai-activity">
                    <For each={liveActivity().slice(0, 8)}>
                      {(a) => (
                        <li classList={{ failed: !a.ok }} title={a.error ?? ""}>
                          <span class="ai-time">{fmtTime(a.at)}</span>
                          <span class="ai-what">{a.ok ? `${a.agent} ${a.label}` : `Refused: ${a.label}`}</span>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>
                <div class="ai-actions"><button class="btn ai-danger" onClick={() => live.disconnect()}>Disconnect</button></div>
              </Match>
            </Switch>
          </div>
        </Show>
      </div>
      <PointerOverlay />
      <Show when={marks() && livePhase() === "connected"}><AiMarks /></Show>
    </Show>
  );
}
