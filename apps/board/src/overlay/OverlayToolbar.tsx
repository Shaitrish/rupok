// The overlay's dock, on the same shell as the board's Smart Dock
// (components/DockShell.tsx): grip, minimise to the 48px bubble, popovers.
// Three recent pen colours stay inline (the rest are one tap away), then
// highlighter, eraser, laser, undo, clear, click-through, "Save to lesson"
// and exit. In the desktop app it lives in its own small always-on-top
// window that is never click-through (the OS moves the window: `windowed`);
// in a browser it floats over the page and drags anywhere.

import { For, Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { Icon } from "../components/Icon";
import { DockPopover, DockShell, longPress } from "../components/DockShell";
import type { IconName } from "../icons.generated";
import { type DockEdge, type DockFrame, type DockPlace, isVertical } from "../dock";
import { APP_SLUG } from "../brand";
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
const ICON: Record<OverlayTool, IconName> = { pen: "pen", highlighter: "highlighter", eraser: "eraser", laser: "laser" };

export interface OverlayToolbarProps {
  send: (cmd: OverlayCommand) => void;
  clickThrough: boolean;
  onClickThrough: (on: boolean) => void;
  saving: boolean;
  onExit: () => void;
  /** Desktop toolbar window: the grip moves the window. */
  draggable?: boolean;
  /** Bumped when the ink window (re)starts, so the current tool is re-sent. */
  resync?: number;
}

// Where the browser overlay's dock sits, and whether it is minimised.
const KEY = `${APP_SLUG}.overlayDock`;
function loadState(): { place: DockPlace; min: boolean } {
  const def = { place: { edge: "bottom" as DockEdge, x: 0.5, y: 1 }, min: false };
  try {
    const r = JSON.parse(localStorage.getItem(KEY) || "{}");
    const ok = (n: unknown) => typeof n === "number" && n >= 0 && n <= 1;
    const edge = ["bottom", "top", "left", "right", "float"].includes(r.place?.edge) ? r.place.edge : null;
    return {
      place: edge && ok(r.place.x) && ok(r.place.y) ? { edge, x: r.place.x, y: r.place.y, ...(r.place.vertical === true ? { vertical: true } : {}) } : def.place,
      min: r.min === true,
    };
  } catch {
    return def;
  }
}

export default function OverlayToolbar(props: OverlayToolbarProps) {
  const [tool, setTool] = createSignal<OverlayTool>("pen");
  const [color, setColor] = createSignal(COLORS[0].rgba);
  /** Three inline colour slots; a colour from the palette replaces the least recently used. */
  const [slots, setSlots] = createSignal(COLORS.slice(0, 3).map((c) => c.rgba));
  const [used, setUsed] = createSignal(COLORS.slice(0, 3).map((c) => c.rgba));
  const [palette, setPalette] = createSignal<HTMLElement | null>(null);
  const saved = loadState();
  const [place, setPlace] = createSignal<DockPlace>(saved.place);
  const [min, setMin] = createSignal(saved.min);
  const [viewport, setViewport] = createSignal({ w: window.innerWidth, h: window.innerHeight });
  const windowed = () => !!props.draggable;
  const iconSize = () => (prefs().density === "compact" ? 20 : prefs().density === "large" ? 28 : 24);
  const vertical = () => !windowed() && isVertical(place(), "auto");
  const frame = (): DockFrame => ({ w: viewport().w, h: viewport().h, top: 12, left: 12, right: 12, bottom: 12 });
  let el: HTMLElement | undefined;

  onMount(() => {
    const r = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", r);
    onCleanup(() => window.removeEventListener("resize", r));
  });
  const persist = () => {
    if (windowed()) return;
    try {
      localStorage.setItem(KEY, JSON.stringify({ place: place(), min: min() }));
    } catch { /* storage unavailable */ }
  };

  const sendTool = () => {
    const t = tool();
    props.send({ type: "tool", tool: t, color: t === "highlighter" ? HL_COLORS[0] : color(), width: WIDTH[t] });
  };
  createEffect(on(() => props.resync, sendTool));
  const pick = (t: OverlayTool, c?: number) => {
    setTool(t);
    if (c !== undefined) {
      setColor(c);
      if (!slots().includes(c)) {
        const lru = [...slots()].sort((a, b) => used().indexOf(b) - used().indexOf(a))[0];
        setSlots(slots().map((s) => (s === lru ? c : s)));
      }
      setUsed([c, ...used().filter((u) => u !== c)].slice(0, 3));
    }
    sendTool();
    // Picking a tool means "draw now".
    if (props.clickThrough) props.onClickThrough(false);
  };
  const label = (c: number) => COLORS.find((x) => x.rgba === c)?.label ?? hex(c);

  const ToolBtn = (p: { id: Exclude<OverlayTool, "pen">; icon: IconName; label: string }) => (
    <button
      class="tool"
      classList={{ on: tool() === p.id }}
      title={p.label}
      aria-label={p.label}
      aria-pressed={tool() === p.id}
      onClick={() => pick(p.id)}
    >
      <Icon name={p.icon} size={iconSize()} active={tool() === p.id} />
    </button>
  );

  let moreBtn!: HTMLButtonElement;
  const sep = () => <div class="dock-sep" />;
  return (
    <>
      <Show when={windowed() && palette()}>
        <Palette inline />
      </Show>
      <DockShell
        place={place()}
        vertical={vertical()}
        orientation="auto"
        frame={() => frame()}
        onPlace={(p, done) => {
          setPlace(p);
          if (done) persist();
        }}
        minimized={min()}
        onMinimized={(m) => {
          setPalette(null);
          setMin(m);
          persist();
        }}
        windowed={windowed()}
        class="ov-bar"
        gripClass="ov-grip"
        label="Overlay tools"
        iconSize={iconSize()}
        bubbleLabel={`Open the overlay tools (${tool() === "pen" ? label(color()) : TOOLS.find((t) => t.id === tool())?.label})`}
        bubble={
          <>
            <Icon name={ICON[tool()]} size={24} active />
            <Show when={tool() === "pen" || tool() === "highlighter"}>
              <span class="dock-dot" style={{ background: hex(tool() === "pen" ? color() : HL_COLORS[0]) }} />
            </Show>
          </>
        }
        onDragStart={() => setPalette(null)}
        ref={(d) => (el = d)}
      >
        <For each={slots()}>
          {(c) => {
            let btn!: HTMLButtonElement;
            const lp = longPress(() => setPalette(btn));
            return (
              <button
                ref={btn}
                class="tool"
                classList={{ on: tool() === "pen" && color() === c }}
                title={`${label(c)}  ·  hold for all colours`}
                aria-label={label(c)}
                aria-pressed={tool() === "pen" && color() === c}
                onPointerDown={lp.onPointerDown}
                onClick={() => !lp.consumed() && pick("pen", c)}
              >
                <span class="swatch-dot" style={{ background: hex(c) }} />
              </button>
            );
          }}
        </For>
        <button
          ref={moreBtn}
          class="tool"
          classList={{ on: !!palette() }}
          title="All colours"
          aria-label="All colours"
          aria-haspopup="dialog"
          aria-expanded={!!palette()}
          onClick={() => setPalette(palette() ? null : moreBtn)}
        >
          <Icon name="palette" size={iconSize()} />
        </button>
        {sep()}
        <For each={TOOLS}>{(t) => <ToolBtn {...t} />}</For>
        {sep()}
        <button class="tool" title="Undo  ·  Ctrl+Z" aria-label="Undo" onClick={() => props.send({ type: "undo" })}>
          <Icon name="undo" size={iconSize()} />
        </button>
        <button class="tool" title="Clear the ink" aria-label="Clear the ink" onClick={() => props.send({ type: "clear" })}>
          <Icon name="trash" size={iconSize()} />
        </button>
        {sep()}
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
      </DockShell>
      <Show when={!windowed() && palette()}>
        <Palette />
      </Show>
    </>
  );

  function Palette(p: { inline?: boolean }) {
    return (
      <DockPopover
        anchor={palette()!}
        dock={() => el}
        vertical={vertical()}
        hand={prefs().hand}
        label="All colours"
        inline={p.inline}
        onClose={() => setPalette(null)}
      >
        <div class="dock-pop-title">Pen</div>
        <div class="dock-palette" role="group" aria-label="Colours">
          <For each={COLORS}>
            {(c) => (
              <button
                class="tool"
                classList={{ on: tool() === "pen" && color() === c.rgba }}
                title={c.label}
                aria-label={`${c.label} (palette)`}
                onClick={() => {
                  pick("pen", c.rgba);
                  setPalette(null);
                }}
              >
                <span class="swatch-dot" style={{ background: hex(c.rgba) }} />
              </button>
            )}
          </For>
        </div>
      </DockPopover>
    );
  }
}
