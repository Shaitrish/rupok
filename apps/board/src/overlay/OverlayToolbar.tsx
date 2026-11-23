// The compact floating toolbar of the overlay: pen colours, highlighter,
// eraser, laser, undo, clear, click-through, "Save to lesson" and exit.
// In the desktop app it lives in its own small always-on-top window that is
// never click-through; in a browser it floats at the bottom of the page.

import { For, createEffect, createSignal, on } from "solid-js";
import { Icon } from "../components/Icon";
import type { IconName } from "../icons.generated";
import { HL_COLORS, hex, prefs } from "../state";
import type { OverlayCommand, OverlayTool } from "./ink";

/** Ink colours that read over any screen content (values from PEN_COLORS). */
const COLORS: { rgba: number; label: string }[] = [
  { rgba: 0xe5484dff, label: "Red pen" },
  { rgba: 0x3e63ddff, label: "Blue pen" },
  { rgba: 0x30a46cff, label: "Green pen" },
  { rgba: 0x1c2024ff, label: "Black pen" },
  { rgba: 0xffffffff, label: "White pen" },
];
const WIDTH: Record<OverlayTool, number> = { pen: 4, highlighter: 22, eraser: 32, laser: 9 };
const TOOLS: { id: Exclude<OverlayTool, "pen">; icon: IconName; label: string }[] = [
  { id: "highlighter", icon: "highlighter", label: "Highlighter" },
  { id: "eraser", icon: "eraser", label: "Eraser" },
  { id: "laser", icon: "laser", label: "Laser pointer" },
];

export interface OverlayToolbarProps {
  send: (cmd: OverlayCommand) => void;
  clickThrough: boolean;
  onClickThrough: (on: boolean) => void;
  saving: boolean;
  onExit: () => void;
  /** Desktop toolbar window: show a grip that moves the window. */
  draggable?: boolean;
  /** Bumped when the ink window (re)starts, so the current tool is re-sent. */
  resync?: number;
}

export default function OverlayToolbar(props: OverlayToolbarProps) {
  const [tool, setTool] = createSignal<OverlayTool>("pen");
  const [color, setColor] = createSignal(COLORS[0].rgba);
  const iconSize = () => (prefs().density === "compact" ? 20 : prefs().density === "large" ? 28 : 24);

  const sendTool = () => {
    const t = tool();
    props.send({ type: "tool", tool: t, color: t === "highlighter" ? HL_COLORS[0] : color(), width: WIDTH[t] });
  };
  createEffect(on(() => props.resync, sendTool));
  const pick = (t: OverlayTool, c?: number) => {
    setTool(t);
    if (c !== undefined) setColor(c);
    sendTool();
    // Picking a tool means "draw now".
    if (props.clickThrough) props.onClickThrough(false);
  };

  return (
    <div class="ov-bar surface" role="toolbar" aria-label="Overlay tools">
      {props.draggable && (
        <div class="tool ov-grip" data-tauri-drag-region title="Move the toolbar" aria-hidden="true">
          <Icon name="more" size={iconSize()} />
        </div>
      )}
      <For each={COLORS}>
        {(c) => (
          <button
            class="tool"
            classList={{ on: tool() === "pen" && color() === c.rgba }}
            title={c.label}
            aria-label={c.label}
            aria-pressed={tool() === "pen" && color() === c.rgba}
            onClick={() => pick("pen", c.rgba)}
          >
            <span class="swatch-dot" style={{ background: hex(c.rgba) }} />
          </button>
        )}
      </For>
      <div class="tsep" />
      <For each={TOOLS}>
        {(t) => (
          <button
            class="tool"
            classList={{ on: tool() === t.id }}
            title={t.label}
            aria-label={t.label}
            aria-pressed={tool() === t.id}
            onClick={() => pick(t.id)}
          >
            <Icon name={t.icon} size={iconSize()} active={tool() === t.id} />
          </button>
        )}
      </For>
      <div class="tsep" />
      <button class="tool" title="Undo  ·  Ctrl+Z" aria-label="Undo" onClick={() => props.send({ type: "undo" })}>
        <Icon name="undo" size={iconSize()} />
      </button>
      <button class="tool" title="Clear the ink" aria-label="Clear the ink" onClick={() => props.send({ type: "clear" })}>
        <Icon name="trash" size={iconSize()} />
      </button>
      <div class="tsep" />
      <button
        class="tool"
        classList={{ on: props.clickThrough }}
        title={props.clickThrough ? "Clicks go to the apps below. Click to draw again" : "Click through: use the apps below, keep the ink"}
        aria-label="Click through to the apps below"
        aria-pressed={props.clickThrough}
        onClick={() => props.onClickThrough(!props.clickThrough)}
      >
        <Icon name="interact" size={iconSize()} active={props.clickThrough} />
      </button>
      <button
        class="tool"
        title="Save to lesson: the screen and your ink as a new page"
        aria-label="Save to lesson"
        disabled={props.saving}
        onClick={() => props.send({ type: "save" })}
      >
        <Icon name="save" size={iconSize()} />
      </button>
      <button class="tool ov-exit" title="Exit the overlay  ·  Esc" aria-label="Exit the overlay" onClick={() => props.onExit()}>
        <Icon name="close" size={iconSize()} />
      </button>
    </div>
  );
}
