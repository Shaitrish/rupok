// The mode switcher: a segmented pill at the top left (Prepare · Teach ·
// Student, plus Overlay in the desktop app). Arrow keys move between modes;
// F5 enters Teach and Alt+1/2/3 pick a mode from anywhere (controller.ts).

import { For, Show } from "solid-js";
import { controller } from "../controller";
import { MODES, type Mode } from "../modes";
import * as platform from "../platform";
import { mode } from "../state";
import { Icon } from "./Icon";

export function ModeSwitcher() {
  let group!: HTMLDivElement;
  const ids = MODES.map((m) => m.id);
  const onKey = (e: KeyboardEvent) => {
    const i = ids.indexOf(mode());
    const step = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    e.stopPropagation();
    const next = ids[(i + step + ids.length) % ids.length] as Mode;
    controller.setMode(next);
    queueMicrotask(() => group.querySelector<HTMLButtonElement>(`[data-mode="${next}"]`)?.focus());
  };
  return (
    <div class="mode-seg" role="radiogroup" aria-label="Mode" ref={group} onKeyDown={onKey}>
      <For each={MODES}>
        {(m) => (
          <button
            role="radio"
            data-mode={m.id}
            aria-checked={mode() === m.id}
            tabIndex={mode() === m.id ? 0 : -1}
            classList={{ on: mode() === m.id }}
            title={`${m.hint}  ·  ${m.key}`}
            onClick={() => controller.setMode(m.id)}
          >
            <Icon name={m.icon} size={20} active={mode() === m.id} />
            <span class="mode-label">{m.label}</span>
          </button>
        )}
      </For>
      <Show when={platform.isDesktop}>
        <button
          data-mode="overlay"
          tabIndex={-1}
          title="Overlay: draw over any app on the screen"
          onClick={() => void platform.openOverlay?.()}
        >
          <Icon name="overlay" size={20} />
          <span class="mode-label">Overlay</span>
        </button>
      </Show>
    </div>
  );
}
