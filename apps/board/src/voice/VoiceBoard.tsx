// Voice on the board: the UI of the Voice text tool (logic in voice/board.ts).
//
//   - With the tool on, a tap on the page places a voice box there (a drag
//     sets its width) and it starts listening.
//   - The box shows a pulsing mint mic and the words as they are recognised
//     (lighter), final words in the ink colour, wrapped inside the box and the
//     page. Drag it to move it, drag its right edge to change the width.
//   - Its bar: mic (listen / pause), EN / বাং, Text ↔ Ask AI, typed ↔
//     handwriting, where the audio goes, Done, discard.
//   - Tap the box, press Esc or Done to finish: one text item, one undo step.
//   - Space (or a clicker's "blank" key, B or .) toggles listening; with
//     "Hold to talk" it listens while held.
//
// Mounted inside the board (App.tsx), above the ink, under the chrome.

import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { Icon } from "../components/Icon";
import { MicIcon } from "../components/MicButton";
import { controller } from "../controller";
import { camera, hex, isView, mode, pageIndex, pages, tool } from "../state";
import {
  BOX_PAD, finishVoiceBox, listen, onToolChange, openVoiceBox, pauseListening, setVoiceBoxEl, setVoiceLang,
  toggleListening, updateVoiceBox, voiceBoardPrefs, voiceBox, voiceLang, voiceLayout, voiceListening, voiceSent,
} from "./board";
import { privacyShort, privacyText, voiceStatus } from "./dictation";
import "./voice-board.css";

const DRAG_PX = 6;
const BAR_H = 52;

const editableTarget = (e: Event) => {
  const t = e.target as HTMLElement | null;
  return !!t && (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT");
};

/** Screen (board-relative) px → world units, and back. */
function toWorld(sx: number, sy: number): [number, number] {
  const [cx, cy, z] = camera();
  return [cx + sx / z, cy + sy / z];
}
function toScreen(wx: number, wy: number): [number, number] {
  const [cx, cy, z] = camera();
  return [(wx - cx) * z, (wy - cy) * z];
}
function boardXY(e: PointerEvent): [number, number] {
  const r = controller.root.getBoundingClientRect();
  return [e.clientX - r.left, e.clientY - r.top];
}

function GripIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="currentColor">
      <circle cx="9" cy="6" r="1.6" /><circle cx="15" cy="6" r="1.6" />
      <circle cx="9" cy="12" r="1.6" /><circle cx="15" cy="12" r="1.6" />
      <circle cx="9" cy="18" r="1.6" /><circle cx="15" cy="18" r="1.6" />
    </svg>
  );
}
function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M5 12.5l4.5 4.5L19 7.5" />
    </svg>
  );
}
/** "Hand-written" toggle: a pen nib over a wavy line. */
function ScriptIcon() {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
      <path d="M3 18c2-3 3.5-3 4.5-1s2.5 2 4-1 3-3 4-1" />
      <path d="M14.5 4.5l5 5L13 16l-4 1 1-4z" />
    </svg>
  );
}

/** Where a tap or a drag on the page will put a box (while dragging). */
function PlaceLayer() {
  const [drag, setDrag] = createSignal<{ x0: number; y0: number; x1: number; y1: number } | null>(null);
  let start: { id: number; x: number; y: number } | null = null;
  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const [x, y] = boardXY(e);
    start = { id: e.pointerId, x, y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const move = (e: PointerEvent) => {
    if (!start || e.pointerId !== start.id) return;
    const [x, y] = boardXY(e);
    if (drag() || Math.hypot(x - start.x, y - start.y) > DRAG_PX * 2) setDrag({ x0: start.x, y0: start.y, x1: x, y1: y });
  };
  const up = (e: PointerEvent) => {
    if (!start || e.pointerId !== start.id) return;
    const d = drag();
    const s = start;
    start = null;
    setDrag(null);
    if (e.type === "pointercancel") return;
    const [wx, wy] = toWorld(d ? Math.min(d.x0, d.x1) : s.x, s.y);
    const z = camera()[2];
    const w = d && Math.abs(d.x1 - d.x0) / z > 120 ? Math.abs(d.x1 - d.x0) / z : undefined;
    void openVoiceBox({ x: wx, y: wy, w });
  };
  return (
    <div
      class="vb-place"
      data-voice-place
      aria-label="Tap the page where the words should go"
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
    >
      <Show when={drag()}>
        {(d) => (
          <div
            class="vb-drag"
            style={{ left: `${Math.min(d().x0, d().x1)}px`, top: `${d().y0}px`, width: `${Math.abs(d().x1 - d().x0)}px` }}
          />
        )}
      </Show>
    </div>
  );
}

/** The voice box on the page (world units) and its bar (screen px). */
function Box() {
  let el!: HTMLDivElement;
  onMount(() => setVoiceBoxEl(el));
  onCleanup(() => setVoiceBoxEl(null));
  const b = () => voiceBox()!;
  const L = () => voiceLayout();
  const ask = () => b().mode === "ask";
  const bn = () => voiceLang().startsWith("bn");
  const empty = () => !L()?.lines.some((l) => l.done || l.live);
  /** The box sits on the page: its colours follow the page, not the app theme. */
  const pageBg = () => pages()[pageIndex()]?.bg_color ?? 0xffffffff;
  const darkBoard = () => (pageBg() >>> 8) < 0x808080;

  // Drag the body to move the box; a tap finishes it.
  let press: { id: number; x: number; y: number; bx: number; by: number; moved: boolean } | null = null;
  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    const [x, y] = boardXY(e);
    const l = L();
    press = { id: e.pointerId, x, y, bx: l?.x ?? b().x, by: l?.y ?? b().y, moved: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const move = (e: PointerEvent) => {
    if (!press || e.pointerId !== press.id) return;
    const [x, y] = boardXY(e);
    if (!press.moved && Math.hypot(x - press.x, y - press.y) < DRAG_PX) return;
    press.moved = true;
    const z = camera()[2];
    updateVoiceBox({ x: press.bx + (x - press.x) / z, y: press.by + (y - press.y) / z });
  };
  const up = (e: PointerEvent) => {
    if (!press || e.pointerId !== press.id) return;
    const p = press;
    press = null;
    if (e.type === "pointercancel") return;
    if (p.moved) {
      const l = L();
      if (l) updateVoiceBox({ x: l.x, y: l.y }); // settle where it is shown (on the page)
    } else void finishVoiceBox();
  };

  // The right edge sets the width (the wrap width).
  let resize: { id: number; x: number; w: number } | null = null;
  const rdown = (e: PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const [x] = boardXY(e);
    resize = { id: e.pointerId, x, w: L()?.w ?? b().w };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const rmove = (e: PointerEvent) => {
    if (!resize || e.pointerId !== resize.id) return;
    const [x] = boardXY(e);
    updateVoiceBox({ w: Math.max(120, resize.w + (x - resize.x) / camera()[2]) });
  };
  const rup = (e: PointerEvent) => {
    if (!resize || e.pointerId !== resize.id) return;
    resize = null;
    const l = L();
    if (l) updateVoiceBox({ w: l.w });
  };

  // The bar sits above the box (below it near the top of the screen), on screen.
  const bar = () => {
    const l = L();
    if (!l) return { left: "0px", top: "0px" };
    const [sx, sy] = toScreen(l.x, l.y);
    const [, sy2] = toScreen(l.x, l.y + l.h);
    const r = controller.root.getBoundingClientRect();
    const top = sy - BAR_H - 6 < 8 ? sy2 + 6 : sy - BAR_H - 6;
    return { left: `${Math.max(8, Math.min(r.width - 8 - 420, sx))}px`, top: `${Math.max(8, Math.min(r.height - BAR_H - 8, top))}px` };
  };
  const [cx, cy, z] = [() => camera()[0], () => camera()[1], () => camera()[2]];
  const holdMode = () => voiceBoardPrefs.read().talk === "hold";
  const micDown = (e: PointerEvent) => {
    e.preventDefault();
    if (holdMode()) { listen(); (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId); }
  };
  const micUp = () => { if (holdMode()) void finishVoiceBox(); };

  return (
    <>
      <div class="vb-world" style={{ transform: `scale(${z()}) translate(${-cx()}px, ${-cy()}px)`, "--vbz": String(z()) }}>
        <Show when={L()}>
          {(l) => (
            <div
              ref={el}
              class="vb-box"
              classList={{ ask: ask(), listening: voiceListening(), empty: empty(), "on-dark": darkBoard() }}
              data-voice-box
              role="status"
              aria-live="polite"
              aria-label={ask() ? "Ask AI: your question" : "Voice text"}
              style={{
                left: `${l().x}px`, top: `${l().y}px`, width: `${l().w}px`, "min-height": `${l().h}px`,
                "font-size": `${l().size}px`, padding: `${BOX_PAD}px`, "--vb-ink": hex(b().color), "--vb-page": hex(pageBg()),
              }}
              onPointerDown={down}
              onPointerMove={move}
              onPointerUp={up}
              onPointerCancel={up}
            >
              <span class="vb-badge" aria-hidden="true"><MicIcon size={16} /></span>
              <Show
                when={!empty()}
                fallback={
                  <div class="vb-line vb-placeholder">
                    {ask() ? (bn() ? "AI-কে প্রশ্ন করুন…" : "Ask the AI…") : voiceListening() ? (bn() ? "বলুন…" : "Speak…") : bn() ? "মাইকে চাপ দিন" : "Tap the mic to talk"}
                  </div>
                }
              >
                <For each={l().lines}>
                  {(line) => (
                    <div class="vb-line">
                      <span class="vb-done">{line.done}</span>
                      <span class="vb-live">{line.live}</span>
                      {!line.done && !line.live ? "​" : ""}
                    </div>
                  )}
                </For>
              </Show>
              <div class="vb-resize" aria-hidden="true" onPointerDown={rdown} onPointerMove={rmove} onPointerUp={rup} onPointerCancel={rup} />
            </div>
          )}
        </Show>
      </div>
      <div
        class="vb-bar surface"
        role="toolbar"
        aria-label="Voice text"
        style={bar()}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <button type="button" class="vb-btn vb-grip" aria-label="Move the voice box" title="Drag to move"
          onPointerDown={down} onPointerMove={move} onPointerUp={(e) => { if (press?.moved) up(e); else press = null; }} onPointerCancel={up}>
          <GripIcon />
        </button>
        <button
          type="button"
          class="vb-btn vb-mic"
          classList={{ on: voiceListening(), finishing: voiceStatus() === "finishing" }}
          aria-pressed={voiceListening()}
          aria-label={voiceListening() ? "Pause listening" : holdMode() ? "Hold to talk" : "Listen"}
          title={`${voiceListening() ? "Pause" : holdMode() ? "Hold to talk" : "Listen"}  ·  Space`}
          data-voice-mic
          onPointerDown={micDown}
          onPointerUp={micUp}
          onClick={() => { if (!holdMode()) toggleListening(); }}
        >
          <MicIcon size={22} />
        </button>
        <div class="vb-seg" role="radiogroup" aria-label="Language">
          <button type="button" role="radio" aria-checked={!bn()} classList={{ on: !bn() }} onClick={() => setVoiceLang("en-US")}>EN</button>
          <button type="button" role="radio" aria-checked={bn()} classList={{ on: bn() }} onClick={() => setVoiceLang("bn-BD")}>বাং</button>
        </div>
        <div class="vb-seg" role="radiogroup" aria-label="Where the words go">
          <button type="button" role="radio" aria-checked={!ask()} classList={{ on: !ask() }} disabled={isView()} onClick={() => updateVoiceBox({ mode: "text" })}>Text</button>
          <button type="button" role="radio" aria-checked={ask()} classList={{ on: ask() }} data-voice-ask onClick={() => updateVoiceBox({ mode: "ask" })}>
            <Icon name="object" size={16} />Ask AI
          </button>
        </div>
        <Show when={!ask()}>
          <button
            type="button"
            class="vb-btn"
            classList={{ on: b().style === "handwriting" }}
            aria-pressed={b().style === "handwriting"}
            aria-label="Write it by hand"
            title="Hand-written (English), like Solve this"
            data-voice-script
            onClick={() => updateVoiceBox({ style: b().style === "handwriting" ? "typed" : "handwriting" })}
          >
            <ScriptIcon />
          </button>
        </Show>
        <span class="vb-privacy" title={privacyText()}>{privacyShort()}</span>
        <button type="button" class="vb-btn vb-finish" aria-label={ask() ? "Send to the AI" : "Done: put it on the board"} title="Done  ·  Esc" data-voice-done onClick={() => void finishVoiceBox()}>
          <CheckIcon />
        </button>
        <button type="button" class="vb-btn" aria-label="Discard" title="Discard" data-voice-discard onClick={() => void finishVoiceBox({ cancel: true })}>
          <Icon name="close" size={20} />
        </button>
      </div>
    </>
  );
}

/** "Sent to AI" for a moment after an Ask AI box. */
function SentBubble() {
  return (
    <Show when={voiceSent()}>
      {(s) => (
        <div class="vb-sent surface" role="status" data-voice-sent>
          <span class="vb-sent-icon"><Icon name="object" size={16} /></span>
          <span class="vb-sent-text">
            <b>{s().ok ? (mode() === "teach" ? "Sent to the AI" : "Asked the AI") : "Waiting in the chat box"}</b>
            <span>{s().text}</span>
          </span>
        </div>
      )}
    </Show>
  );
}

export default function VoiceBoard() {
  const allowed = () => !isView() || voiceBoardPrefs.read().student;
  createEffect(on(tool, (t) => onToolChange(t), { defer: true }));
  onMount(() => {
    let spaceDown = false;
    const active = () => tool() === "voice" || !!voiceBox();
    const down = (e: KeyboardEvent) => {
      if (!allowed()) return;
      if (e.key === "Escape" && voiceBox()) {
        e.preventDefault();
        e.stopImmediatePropagation();
        void finishVoiceBox();
        return;
      }
      if (!active() || editableTarget(e) || e.ctrlKey || e.metaKey || e.altKey) return;
      const clicker = e.key === "b" || e.key === "B" || e.key === ".";
      if (e.key !== " " && !clicker) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.repeat || spaceDown) return;
      spaceDown = true;
      if (voiceBoardPrefs.read().talk === "hold") {
        if (!voiceBox()) void openVoiceBox({ ptt: true });
        else listen();
      } else if (!voiceBox()) void openVoiceBox();
      else toggleListening();
    };
    const up = (e: KeyboardEvent) => {
      const clicker = e.key === "b" || e.key === "B" || e.key === ".";
      if ((e.key !== " " && !clicker) || !spaceDown) return;
      spaceDown = false;
      e.stopImmediatePropagation();
      if (voiceBoardPrefs.read().talk === "hold" && voiceBox()) {
        if (voiceBox()?.ptt) void finishVoiceBox();
        else pauseListening();
      }
    };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    onCleanup(() => {
      window.removeEventListener("keydown", down, true);
      window.removeEventListener("keyup", up, true);
    });
  });
  return (
    <Show when={allowed()}>
      <div class="vb-layer" classList={{ placing: tool() === "voice" }}>
        <Show when={tool() === "voice"}>
          <PlaceLayer />
        </Show>
        <Show when={voiceBox()}>
          <Box />
        </Show>
        <SentBubble />
      </div>
    </Show>
  );
}
