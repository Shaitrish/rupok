// The toolbar. What it holds, and in which order, comes from the current
// mode's config (modes.ts): drawing tools, undo/redo, a page pager, replay and
// an exit button. Ink colours and sizes follow the active ink tool.

import { For, Match, Show, Switch, createSignal, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { TOOLBAR_ITEMS, type ToolbarItem } from "../modes";
import {
  HL_COLORS, PEN_COLORS, WIDTHS, canRedo, canUndo, color, hex, hlColor, isView, lessonOpened, mode, modeConfig, pageIndex, pages, prefs,
  setColor, setHlColor, setSizes, setWidth, sizeKey, sizes, studentEntry, tool, width, type Tool,
} from "../state";
import { Icon } from "./Icon";
import { FILE_EXT } from "../brand";

const INK_TOOLS: Tool[] = ["pen", "highlighter", "rect", "ellipse", "line", "arrow"];
const META = new Map(TOOLBAR_ITEMS.map((t) => [t.id, t]));
const kindOf = (id: ToolbarItem) => META.get(id)?.kind ?? "tool";

/** Icon size per density; Teach's floating toolbar is one step more compact. */
const iconSizeFor = () => {
  const d = prefs().density;
  if (mode() === "teach") return d === "large" ? 24 : 20;
  return d === "compact" ? 20 : d === "large" ? 28 : 24;
};

/** Teach: the toolbar floats over the slide and fades after a few idle
 * seconds; a hover, a key or touching the toolbar brings it back. */
const TEACH_IDLE_MS = 3000;

function pick(t: Tool) {
  controller.setTool(t);
  if (INK_TOOLS.includes(t)) {
    controller.board.set_color(t === "highlighter" ? hlColor() : color());
    controller.board.set_width(sizes()[sizeKey(t)] ?? width());
  } else if (t === "eraser") {
    controller.board.set_width(sizes().eraser);
  }
}

function Entry(props: { id: ToolbarItem }) {
  const meta = () => META.get(props.id)!;
  const tip = () => (meta().key ? `${meta().label}  ·  ${meta().key}` : meta().label);
  return (
    <Switch
      fallback={
        <button
          class="tool"
          classList={{ on: tool() === props.id }}
          title={tip()}
          aria-label={meta().label}
          aria-pressed={tool() === props.id}
          onClick={() => pick(props.id as Tool)}
        >
          <Icon name={meta().icon} size={iconSizeFor()} active={tool() === props.id} />
        </button>
      }
    >
      <Match when={props.id === "smartink"}>
        <button
          class="tool"
          classList={{ on: prefs().smartShapes !== "off" }}
          title={`${prefs().smartShapes === "off" ? "Smart shapes: off" : prefs().smartShapes === "hold" ? "Smart shapes: hold to snap" : "Smart shapes: snap on lift"}  ·  M`}
          aria-label="Smart shapes"
          aria-pressed={prefs().smartShapes !== "off"}
          onClick={() => controller.toggleSmartInk()}
        >
          <Icon name="smartink" size={iconSizeFor()} active={prefs().smartShapes !== "off"} />
        </button>
      </Match>
      <Match when={props.id === "undo"}>
        <button class="tool" disabled={!canUndo()} title={tip()} aria-label="Undo" onClick={() => controller.board.undo()}>
          <Icon name="undo" size={iconSizeFor()} />
        </button>
      </Match>
      <Match when={props.id === "redo"}>
        <button class="tool" disabled={!canRedo()} title={tip()} aria-label="Redo" onClick={() => controller.board.redo()}>
          <Icon name="redo" size={iconSizeFor()} />
        </button>
      </Match>
      <Match when={props.id === "pager"}>
        <button class="tool" disabled={pageIndex() === 0} onClick={() => controller.prev()} title="Back  ·  ←" aria-label="Previous page">
          <Icon name="prev" size={iconSizeFor()} />
        </button>
        <span class="viewer-count" aria-live="polite">{pageIndex() + 1} / {pages().length}</span>
        <button class="tool" disabled={pageIndex() >= pages().length - 1 && mode() !== "teach"} onClick={() => controller.next()} title="Next  ·  →" aria-label="Next page">
          <Icon name="next" size={iconSizeFor()} />
        </button>
      </Match>
      <Match when={props.id === "replay"}>
        <Show
          when={isView()}
          fallback={
            <button class="tool" title="Replay this page stroke by stroke" aria-label="Replay" onClick={() => controller.board.replay_start()}>
              <Icon name="replay" size={iconSizeFor()} />
            </button>
          }
        >
          <button class="btn solid viewer-replay" onClick={() => controller.board.replay_start()} title="Replay this page stroke by stroke">
            <Icon name="replay" size={iconSizeFor()} /><span>Replay</span>
          </button>
        </Show>
      </Match>
      <Match when={props.id === "exit"}>
        <button class="tool" title={`Exit ${mode() === "teach" ? "Teach" : "mode"}  ·  Esc`} aria-label="Exit mode" onClick={() => controller.exitMode()}>
          <Icon name="exit" size={iconSizeFor()} />
        </button>
      </Match>
    </Switch>
  );
}

export default function Toolbar() {
  const [idle, setIdle] = createSignal(false);
  let idleTimer: number | undefined;
  const wake = () => {
    setIdle(false);
    clearTimeout(idleTimer);
    if (mode() === "teach") idleTimer = window.setTimeout(() => setIdle(true), TEACH_IDLE_MS);
  };
  onMount(() => {
    // Hovering (a mouse or a pen with no button down) counts as activity;
    // drawing does not, so the toolbar stays out of the way of the ink.
    const onMove = (e: PointerEvent) => e.buttons === 0 && e.pointerType !== "touch" && wake();
    window.addEventListener("pointermove", onMove);
    window.addEventListener("keydown", wake);
    wake();
    onCleanup(() => {
      clearTimeout(idleTimer);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("keydown", wake);
    });
  });
  const cfg = () => modeConfig();
  const side = () => (isView() ? "bottom" : prefs().toolbar);
  const inkActive = () => INK_TOOLS.includes(tool()) || tool() === "eraser";
  const showColors = () => cfg().chrome.colors && INK_TOOLS.includes(tool());
  const showSizes = () => cfg().chrome.sizes && inkActive();
  const colors = () => (tool() === "highlighter" ? HL_COLORS : PEN_COLORS);
  const sizeOptions = () => (tool() === "eraser" ? [16, 32, 64] : tool() === "highlighter" ? [14, 22, 34] : WIDTHS);
  const activeColor = () => (tool() === "highlighter" ? hlColor() : color());
  const sizeOn = (w: number) => sizes()[sizeKey(tool())] === w;
  return (
    <>
      <Show when={cfg().tools.length > 0}>
        <div
          class={`toolbar surface ${side()}`}
          classList={{ viewer: isView(), teach: mode() === "teach", idle: mode() === "teach" && idle() }}
          onPointerDown={wake}
          onPointerEnter={wake}
          role="toolbar"
          aria-label={isView() ? "Lesson viewer" : mode() === "teach" ? "Teaching tools" : "Drawing tools"}
        >
          <For each={cfg().tools}>
            {(id, i) => (
              <>
                {/* A separator wherever the kind of entry changes (tools → actions → navigation). */}
                <Show when={i() > 0 && kindOf(cfg().tools[i() - 1]) !== kindOf(id)}>
                  <div class="tsep" />
                </Show>
                <Entry id={id} />
              </>
            )}
          </For>
          <Show when={showColors()}>
            <div class="tsep" />
            <For each={colors()}>
              {(c) => (
                <button
                  class="tool"
                  classList={{ on: (activeColor() >>> 8) === (c >>> 8) }}
                  title={hex(c)}
                  aria-label={`Colour ${hex(c)}`}
                  onClick={() => {
                    if (tool() === "highlighter") setHlColor(c);
                    else setColor(c);
                    controller.board.set_color(c);
                  }}
                >
                  <span class="swatch-dot" style={{ background: hex(c) }} />
                </button>
              )}
            </For>
          </Show>
          <Show when={showSizes()}>
            <div class="tsep" />
            <For each={sizeOptions()}>
              {(w, i) => (
                <button
                  class="tool"
                  classList={{ on: sizeOn(w) }}
                  title={`Size ${w}`}
                  aria-label={`Size ${w}`}
                  onClick={() => {
                    if (tool() !== "eraser" && tool() !== "highlighter") setWidth(w);
                    setSizes({ ...sizes(), [sizeKey(tool())]: w });
                    controller.board.set_width(w);
                  }}
                >
                  <span class="size-dot" style={{ width: `${4 + i() * 4}px`, height: `${4 + i() * 4}px` }} />
                </button>
              )}
            </For>
          </Show>
        </div>
      </Show>
      <Show when={isView() && studentEntry && !lessonOpened()}>
        <div class="view-empty" role="dialog" aria-label="Open a lesson">
          <div class="badge"><Icon name="open" size={24} active /></div>
          <h2>Open a lesson</h2>
          <p>Got a <code>.{FILE_EXT}</code> file from your teacher? Open it to flip through the pages and replay the class stroke by stroke.</p>
          <button class="btn solid" onClick={() => controller.openFile()}>
            <Icon name="open" /><span>Open lesson file</span>
          </button>
          <p class="hint">You can also drop the file anywhere on this page.</p>
        </div>
      </Show>
    </>
  );
}
