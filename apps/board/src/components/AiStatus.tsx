// The AI assistant's place in the top bar: one icon button with a status dot.
// Not connected: the plain icon; a click starts the Connect flow (live.ts)
// and opens the AI panel (LivePanel.tsx). Connected: a mint dot. While the
// agent edits: the dot pulses. Right after an AI edit, a small undo button
// sits next to it for a few seconds.
//
// The panel's open state lives here so the header, the ⋯ menu and the panel
// share it.

import { Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { chatOpen, closeAiPanel, openAiPanel } from "../chat";
import { allowEdit, live, liveHasChat, liveAgent, liveEditing, liveLastEdit, livePhase, liveWaiting } from "../live";
import { Icon } from "./Icon";
import "./Header.css";

export const [aiPanelOpen, setAiPanelOpen] = createSignal(false);

/** What the header button (or the ⋯ item) does: close the open panel, else
 * open it, starting the Connect flow when no AI is connected. */
export function toggleAi() {
  // The in-app chat (ChatPanel.tsx) takes the button when a relay offers it,
  // and always in the desktop app (which starts its own relay).
  if (chatOpen()) return closeAiPanel();
  if (liveHasChat() || (window as any).__TAURI_INTERNALS__) {
    setAiPanelOpen(false);
    return openAiPanel();
  }
  if (aiPanelOpen()) return setAiPanelOpen(false);
  const p = livePhase();
  if (p === "idle" || p === "unreachable") live.connect();
  setAiPanelOpen(true);
}

const fmtCode = (c: string) => `${c.slice(0, 3)} ${c.slice(3)}`;

/** One line describing the AI state: tooltip, aria-label and the ⋯ item. */
export function aiStatusText(): string {
  const agent = liveAgent() ?? "AI";
  switch (livePhase()) {
    case "searching": return "Looking for your AI…";
    case "pairing": return liveWaiting().length === 1 ? `Pair your AI: code ${fmtCode(liveWaiting()[0].code)}` : "Pair your AI";
    case "reconnecting": return `Reconnecting to ${agent}…`;
    case "unreachable": return "No AI found on this computer: set it up";
    case "connected": {
      const e = liveEditing();
      if (e) return `${e.agent} is editing…`;
      return allowEdit() ? `${agent} connected` : `${agent} connected (view only)`;
    }
    default: return "Connect AI: your own assistant edits this board live";
  }
}
export const aiConnected = () => livePhase() === "connected";

export function AiStatus() {
  const [recent, setRecent] = createSignal(false);
  let timer: number | undefined;
  createEffect(on(liveLastEdit, (e) => {
    clearTimeout(timer);
    setRecent(!!e);
    if (e) timer = window.setTimeout(() => setRecent(false), 8000);
  }));
  onCleanup(() => clearTimeout(timer));

  const phase = livePhase;
  const busy = () => phase() === "connected" && !!liveEditing();
  const waiting = () => phase() === "searching" || phase() === "reconnecting" || phase() === "pairing";
  return (
    <div class="ai-status" classList={{ on: aiConnected(), busy: busy() }}>
      <Show when={aiConnected() && recent() && liveLastEdit() && !liveEditing()}>
        <button
          class="btn icon ai-status-undo"
          onClick={() => live.undoLast()}
          title={`Undo ${liveLastEdit()!.agent}'s last change  ·  Ctrl Z`}
          aria-label={`Undo ${liveLastEdit()!.agent}'s last change`}
        >
          <Icon name="undo" size={18} />
        </button>
      </Show>
      <button
        class="btn icon ai-status-btn"
        data-ai-toggle
        onClick={toggleAi}
        title={aiStatusText()}
        aria-label={aiStatusText()}
        aria-expanded={aiPanelOpen()}
        aria-haspopup="dialog"
      >
        <Icon name="smartink" active={aiConnected()} />
        <Show when={phase() !== "idle" && phase() !== "unreachable"}>
          <span class="ai-status-dot" classList={{ live: aiConnected(), wait: waiting(), pulse: busy(), off: aiConnected() && !allowEdit() }} aria-hidden="true" />
        </Show>
      </button>
    </div>
  );
}
