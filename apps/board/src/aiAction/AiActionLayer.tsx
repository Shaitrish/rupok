// The "AI in action" layer over the board: working cards where results will
// land, the sparkle when they do, the class-mode Undo toast and destructive
// confirmations, and the students' "Ask AI" box and own layer.

import { For, Show, createEffect, createSignal, on, onCleanup } from "solid-js";
import { camera, isView, mode, pageIndex } from "../state";
import { controller } from "../controller";
import { liveHasChat } from "../live";
import { renderMarkdown } from "../markdown";
import { Icon } from "../components/Icon";
import { cancelCard, removeCard, retryCard } from "./progress";
import { aiToast, cards, confirmReq, setAiToast, sparkle, type Card } from "./state";
import { askOpen, asksLeft, clearStudentLayer, setAskOpen, studentAsk, studentLayer, studentsMayAsk } from "./student";
import "./aiAction.css";

/** Slide px → screen px on the board. */
function screen(r: { x: number; y: number; w: number; h: number }) {
  const [cx, cy, z] = camera();
  return { left: (r.x - cx) * z, top: (r.y - cy) * z, width: r.w * z, height: r.h * z };
}

const KIND_ICON: Record<Card["kind"], string> = { object: "smartink", handwriting: "pen", text: "text", shape: "rect", other: "smartink" };

function Preview(props: { html: string }) {
  let frame!: HTMLIFrameElement;
  // Re-render when the agent sends a new partial object (debounced a little).
  let t: number | undefined;
  createEffect(on(() => props.html, (h) => {
    clearTimeout(t);
    t = window.setTimeout(() => { if (frame) frame.srcdoc = controller.objects.wrap(h); }, 150);
  }));
  onCleanup(() => clearTimeout(t));
  return (
    <div class="aia-preview" aria-label="Live preview">
      <iframe ref={frame} title="Live preview" sandbox="allow-scripts" referrerpolicy="no-referrer" tabindex={-1} />
      <span class="aia-preview-tag">Live preview</span>
    </div>
  );
}

function ProgressCard(props: { card: Card }) {
  const c = () => props.card;
  const box = () => {
    const s = screen(c().rect);
    // Readable even when the target is small (a line of handwriting).
    const minW = mode() === "teach" ? 460 : 340, minH = mode() === "teach" ? 150 : 112;
    return { ...s, width: Math.max(s.width, minW), height: Math.max(s.height, minH) };
  };
  const big = () => box().height > 280 && box().width > 480;
  return (
    <div
      class="aia-card"
      classList={{ big: big(), landing: c().state === "landing", failed: c().state === "failed", done: c().state === "done", [`k-${c().kind}`]: true }}
      style={{ left: `${box().left}px`, top: `${box().top}px`, width: `${box().width}px`, height: `${box().height}px` }}
      role="status"
      aria-live="polite"
      data-card={c().id}
    >
      <svg class="aia-outline" aria-hidden="true"><rect x="1.5" y="1.5" rx="18" ry="18" /></svg>
      <div class="aia-body">
        <div class="aia-head">
          <span class="aia-agent"><Icon name={KIND_ICON[c().kind] as any} size={16} />{c().agent}</span>
          <Show when={c().title}><span class="aia-title">{c().title}</span></Show>
          <button class="aia-x" onClick={() => cancelCard(c().id)} aria-label="Cancel" title="Stop"><Icon name="close" size={16} /></button>
        </div>
        <div class="aia-step">
          <Show when={c().state === "working"}><span class="aia-dots" aria-hidden="true"><i /><i /><i /></span></Show>
          <span>{c().state === "failed" ? "Not quite yet" : c().step}</span>
        </div>
        <Show when={c().state === "failed"}>
          <p class="aia-error">{c().error ?? "It did not work this time."}</p>
          <div class="aia-row">
            <button class="aia-btn solid" onClick={() => retryCard(c().id)}>Try again</button>
            <button class="aia-btn" onClick={() => removeCard(c().id)}>Dismiss</button>
          </div>
        </Show>
        <Show when={c().state === "working" && c().error}>
          <p class="aia-note">{c().error}</p>
        </Show>
        <Show when={big() && c().plan.length}>
          <ul class="aia-plan">
            <For each={c().plan}>
              {(e) => <li classList={{ done: e.status === "completed", now: e.status === "in_progress" }}><span class="aia-tick" aria-hidden="true" />{e.content}</li>}
            </For>
          </ul>
        </Show>
        <Show when={big() && c().preview}><Preview html={c().preview!} /></Show>
        <Show when={c().percent != null}>
          <div class="aia-bar" role="progressbar" aria-valuenow={c().percent} aria-valuemin={0} aria-valuemax={100}><span style={{ width: `${c().percent}%` }} /></div>
        </Show>
      </div>
    </div>
  );
}

/** Cards working on another page: a chip that shows it. */
function Elsewhere(props: { card: Card }) {
  return (
    <button class="aia-elsewhere" onClick={() => { controller.board.goto_page(props.card.page - 1); controller.sync(true); }}>
      <span class="aia-dots" aria-hidden="true"><i /><i /><i /></span>
      {props.card.agent}: {props.card.step} <b>Page {props.card.page} →</b>
    </button>
  );
}

function Sparkle() {
  const [shown, setShown] = createSignal<{ left: number; top: number; n: number } | null>(null);
  let t: number | undefined;
  createEffect(on(sparkle, (s) => {
    if (!s || s.page !== pageIndex() + 1) return;
    const p = screen({ x: s.x, y: s.y, w: 0, h: 0 });
    setShown({ left: p.left, top: p.top, n: s.n });
    clearTimeout(t);
    t = window.setTimeout(() => setShown(null), 1100);
  }, { defer: true }));
  onCleanup(() => clearTimeout(t));
  return (
    <Show when={shown()} keyed>
      {(s) => (
        <div class="aia-sparkle" style={{ left: `${s.left}px`, top: `${s.top}px` }} aria-hidden="true">
          <span class="ring" />
          <For each={[0, 1, 2, 3, 4, 5, 6, 7]}>{(i) => <span class="star" style={{ "--a": `${i * 45 + 20}deg` } as any} />}</For>
        </div>
      )}
    </Show>
  );
}

function Toast() {
  return (
    <Show when={aiToast()} keyed>
      {(t) => (
        <div class="aia-toast" role="status">
          <span class="aia-toast-dot" aria-hidden="true" />
          <span><b>{t.agent}</b> {t.label}</span>
          <Show when={t.undo}><button class="aia-btn solid" onClick={() => t.undo?.()}>Undo</button></Show>
          <button class="aia-x" onClick={() => setAiToast(null)} aria-label="Dismiss"><Icon name="close" size={16} /></button>
        </div>
      )}
    </Show>
  );
}

function Confirm() {
  return (
    <Show when={confirmReq()} keyed>
      {(r) => (
        <div class="aia-confirm surface" role="alertdialog" aria-label="The AI asks to make a change">
          <p class="aia-confirm-q"><b>{r.agent}</b> wants to {r.label}.</p>
          <Show when={r.detail}><p class="aia-confirm-d">{r.detail}</p></Show>
          <div class="aia-row">
            <button class="aia-btn solid" onClick={() => r.answer(true)}>Allow</button>
            <button class="aia-btn" onClick={() => r.answer(false)}>Don't allow</button>
          </div>
        </div>
      )}
    </Show>
  );
}

function StudentLayer() {
  return (
    <For each={studentLayer().filter((i) => i.page === pageIndex() + 1)}>
      {(it) => {
        const box = () => screen(it.rect);
        return (
          <div class="aia-mine" style={{ left: `${box().left}px`, top: `${box().top}px`, width: `${box().width}px`, height: `${box().height}px` }}>
            <span class="aia-mine-tag">Only you see this</span>
            <Show when={it.kind === "object"} fallback={<div class="aia-mine-text" innerHTML={renderMarkdown((it.lines ?? []).join("\n\n"))} />}>
              <iframe title={it.title} sandbox="allow-scripts" referrerpolicy="no-referrer" srcdoc={controller.objects.wrap(it.html ?? "")} />
            </Show>
          </div>
        );
      }}
    </For>
  );
}

function StudentAsk() {
  const [q, setQ] = createSignal("");
  const [msg, setMsg] = createSignal<string | null>(null);
  const left = () => asksLeft();
  const send = (e?: Event) => {
    e?.preventDefault();
    const r = studentAsk(q());
    if (r === "sent") { setQ(""); setMsg("Asked! Watch the board."); setAskOpen(false); }
    else if (r === "limit") setMsg(`That's all your asks for now. More in ${left().resetMin} min.`);
    else if (r === "noagent") setMsg(null);
  };
  return (
    <Show when={studentsMayAsk()}>
      <div class="aia-ask">
        <Show when={askOpen()} fallback={
          <button class="aia-ask-chip surface" onClick={() => { setMsg(null); setAskOpen(true); }}>
            <Icon name="smartink" size={18} /> Ask AI to show me
            <span class="aia-ask-left" title="Asks left in the next 10 minutes">{left().left}/{left().limit}</span>
          </button>
        }>
          <form class="aia-ask-box surface" onSubmit={send}>
            <div class="aia-ask-head">
              <b>What should the AI show you?</b>
              <button type="button" class="aia-x" onClick={() => setAskOpen(false)} aria-label="Close"><Icon name="close" size={16} /></button>
            </div>
            <Show when={liveHasChat()} fallback={
              <p class="aia-ask-help">
                No AI is connected to this board. Ask your teacher to connect Rupok's AI, or open Rupok on your own
                computer with your own AI assistant and ask it there.
              </p>
            }>
              <input class="aia-ask-input" value={q()} onInput={(e) => setQ(e.currentTarget.value)} maxlength={300}
                placeholder="e.g. a 3D model of a water molecule" aria-label="Your question" autofocus />
              <div class="aia-row">
                <button class="aia-btn solid" type="submit" disabled={!q().trim() || left().left <= 0}>Show me</button>
                <span class="aia-ask-left">{left().left} of {left().limit} asks left</span>
                <Show when={studentLayer().length}><button type="button" class="aia-btn" onClick={clearStudentLayer}>Clear mine</button></Show>
              </div>
            </Show>
            <Show when={msg()}><p class="aia-ask-help" role="status">{msg()}</p></Show>
          </form>
        </Show>
      </div>
    </Show>
  );
}

export default function AiActionLayer() {
  const here = () => cards.list.filter((c) => c.page === pageIndex() + 1);
  const away = () => cards.list.filter((c) => c.page !== pageIndex() + 1 && c.state === "working").slice(-1);
  return (
    <>
      <div class="aia-layer" classList={{ teach: mode() === "teach" }}>
        <For each={here()}>{(c) => <ProgressCard card={c} />}</For>
        <Show when={isView()}><StudentLayer /></Show>
        <Sparkle />
      </div>
      <div class="aia-float" classList={{ teach: mode() === "teach" }}>
        <For each={away()}>{(c) => <Elsewhere card={c} />}</For>
        <Toast />
      </div>
      <Confirm />
      <StudentAsk />
    </>
  );
}
