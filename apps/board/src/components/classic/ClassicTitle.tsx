// Classic has no top bar. The lesson title shows for a few seconds when the
// lesson or the page changes, and while the pointer rests at the top edge,
// then fades: the slide keeps the whole screen.

import { Show, createEffect, createSignal, on, onCleanup, onMount } from "solid-js";
import { mode, pageIndex, pages, revision, savedRevision, title } from "../../state";
import { current } from "../../library/session";

const SHOW_MS = 2600;
const TOP_ZONE = 40;

export function ClassicTitle() {
  const [shown, setShown] = createSignal(true);
  let timer: number | undefined;
  const flash = () => {
    setShown(true);
    clearTimeout(timer);
    timer = window.setTimeout(() => setShown(false), SHOW_MS);
  };
  createEffect(on([title, pageIndex, mode], flash));
  onMount(() => {
    const move = (e: PointerEvent) => {
      if (e.pointerType === "mouse" && e.clientY < TOP_ZONE && e.buttons === 0) flash();
    };
    window.addEventListener("pointermove", move, { passive: true });
    onCleanup(() => {
      window.removeEventListener("pointermove", move);
      clearTimeout(timer);
    });
  });
  const dirty = () => revision() !== savedRevision() && !current();
  return (
    <div class="cl-titlebar" classList={{ shown: shown() }}>
      <span class="cl-titletext">{title()}</span>
      <span class="cl-titlepage">{pageIndex() + 1} / {pages().length}</span>
      <Show when={dirty()}>
        <span class="cl-unsaved" title="Changes are kept in this browser; ☰ → File → Save writes a file">Unsaved</span>
      </Show>
    </div>
  );
}
