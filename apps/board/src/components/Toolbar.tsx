import { For, Show } from "solid-js";
import { controller } from "../controller";
import type { IconName } from "../icons.generated";
import {
  HL_COLORS, PEN_COLORS, WIDTHS, color, hex, hlColor, prefs, setColor, setHlColor, setSizes, setWidth, sizeKey, sizes, tool, width,
  type Tool,
} from "../state";
import { Icon } from "./Icon";

const TOOLS: { id: Tool; icon: IconName; key: string; label: string }[] = [
  { id: "pen", icon: "pen", key: "P", label: "Pen" },
  { id: "highlighter", icon: "highlighter", key: "H", label: "Highlighter" },
  { id: "eraser", icon: "eraser", key: "E", label: "Eraser" },
  { id: "lasso", icon: "select", key: "V", label: "Select & move" },
  { id: "laser", icon: "laser", key: "L", label: "Laser pointer" },
  { id: "text", icon: "text", key: "T", label: "Text" },
  { id: "rect", icon: "rect", key: "R", label: "Rectangle" },
  { id: "ellipse", icon: "ellipse", key: "O", label: "Ellipse" },
  { id: "line", icon: "line", key: "N", label: "Line" },
  { id: "arrow", icon: "arrow", key: "A", label: "Arrow" },
  { id: "hand", icon: "hand", key: "Space", label: "Pan" },
  { id: "interact", icon: "interact", key: "I", label: "Use Smart Objects" },
];

const INK_TOOLS: Tool[] = ["pen", "highlighter", "rect", "ellipse", "line", "arrow"];

export default function Toolbar() {
  const iconSize = () => (prefs().density === "compact" ? 20 : prefs().density === "large" ? 28 : 24);
  const pick = (t: Tool) => {
    controller.setTool(t);
    if (INK_TOOLS.includes(t)) {
      controller.board.set_color(t === "highlighter" ? hlColor() : color());
      controller.board.set_width(sizes()[sizeKey(t)] ?? width());
    } else if (t === "eraser") {
      controller.board.set_width(sizes().eraser);
    }
  };
  const colors = () => (tool() === "highlighter" ? HL_COLORS : PEN_COLORS);
  const sizeOptions = () => (tool() === "eraser" ? [16, 32, 64] : tool() === "highlighter" ? [14, 22, 34] : WIDTHS);
  const activeColor = () => (tool() === "highlighter" ? hlColor() : color());
  const sizeOn = (w: number) => sizes()[sizeKey(tool())] === w;
  return (
    <div class={`toolbar surface ${prefs().toolbar}`} role="toolbar" aria-label="Drawing tools">
      <For each={TOOLS}>
        {(t) => (
          <button
            class="tool"
            classList={{ on: tool() === t.id }}
            title={`${t.label}  ·  ${t.key}`}
            aria-label={t.label}
            aria-pressed={tool() === t.id}
            onClick={() => pick(t.id)}
          >
            <Icon name={t.icon} size={iconSize()} active={tool() === t.id} />
          </button>
        )}
      </For>
      <Show when={INK_TOOLS.includes(tool()) || tool() === "eraser"}>
        <div class="tsep" />
        <Show when={tool() !== "eraser"}>
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
          <div class="tsep" />
        </Show>
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
  );
}
