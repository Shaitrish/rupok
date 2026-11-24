// Screen-space controls for media items: a small pill at the bottom-left
// of each video, audio clip or web embed (play/pause, restart, mute, time,
// open link; replace and settings while preparing). It sits above the ink
// layer so it works with any tool, and covers only itself: the rest of the
// item stays free for the pen. In Teach the buttons are bigger.

import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { camera, type DomItem, isView, items, mode, tool } from "../state";
import { clock, focusMedia, online, openInBrowser, pb, pickFile, MEDIA_ACCEPT, replaceMedia, restart, stopPlayer, toggle, toggleMute } from "../media/media";
import { openMediaDialog } from "./MediaDialog";
import { Icon } from "./Icon";
import "./Media.css";

function Pill(props: { item: DomItem; rect: { x: number; y: number; w: number; h: number } }) {
  const id = props.item.id;
  const m = () => props.item.media!;
  const s = () => pb(id);
  const big = () => mode() === "teach";
  const link = () => m().type === "youtube" || m().type === "web";
  const web = () => m().type === "web";
  const prepare = () => mode() === "prepare" && !isView();
  const roomy = () => props.rect.w >= (big() ? 460 : 380);
  /** Small on screen (a phone, zoomed out): play, mute and settings only. */
  const compact = () => props.rect.w < (big() ? 340 : 300);
  const muted = () => (s().active ? s().muted : !!m().muted);
  let el!: HTMLDivElement;
  const [width, setWidth] = createSignal(0);
  onMount(() => {
    const ro = new ResizeObserver(() => setWidth(el.offsetWidth));
    ro.observe(el);
    onCleanup(() => ro.disconnect());
  });
  const pos = createMemo(() => {
    const inset = big() ? 16 : 12;
    const h = big() ? 64 : 52;
    const w = width();
    // Inside the bottom-left corner when it fits; else just below the item.
    const inside = props.rect.h >= h * 2.2 && w + 2 * inset <= props.rect.w;
    const x = inside ? props.rect.x + inset : props.rect.x;
    const left = Math.max(8, Math.min(x, innerWidth - w - 8));
    return {
      left: `${Math.round(left)}px`,
      top: `${Math.round(inside ? props.rect.y + props.rect.h - inset - h : props.rect.y + props.rect.h + 8)}px`,
    };
  });
  const playLabel = () => (web() ? (s().active ? "Close page" : "Open page") : s().playing ? "Pause" : "Play");
  const onPlay = () => {
    focusMedia(id);
    if (web() && s().active) stopPlayer(id);
    else toggle(id);
  };
  const replace = async () => {
    if (link()) return openMediaDialog({ kind: m().type === "youtube" ? "youtube" : "web", edit: props.item, replace: true });
    const f = await pickFile(MEDIA_ACCEPT);
    if (f) await replaceMedia(props.item, { file: f });
  };
  return (
    <div
      ref={el}
      class="media-pill"
      classList={{ big: big() }}
      style={pos()}
      role="toolbar"
      aria-label={`${m().title || "Media"} controls`}
      onPointerDown={(e) => e.stopPropagation()}
    >
      <button
        class="mp-btn mp-play"
        classList={{ wide: web() }}
        aria-label={playLabel()}
        title={playLabel() + (web() ? "" : " (Space or K)")}
        disabled={link() && !online() && !s().active}
        onClick={onPlay}
      >
        <Show when={s().active && s().loading} fallback={<Icon name={web() ? (s().active ? "close" : "globe") : s().playing ? "pause" : "play"} size={big() ? 28 : 24} active />}>
          <span class="media-spinner small" />
        </Show>
        <Show when={web()}>
          <span class="mp-text">{s().active ? "Close" : online() ? "Open" : "Offline"}</span>
        </Show>
      </button>
      <Show when={!web() && !compact()}>
        <button class="mp-btn" aria-label="Play from the start" title="Play from the start" onClick={() => restart(id)}>
          <Icon name="restart" size={big() ? 24 : 20} />
        </button>
      </Show>
      <Show when={!web()}>
        <button class="mp-btn" aria-label={muted() ? "Unmute" : "Mute"} aria-pressed={muted()} title={muted() ? "Unmute" : "Mute"} onClick={() => toggleMute(id)}>
          <Icon name={muted() ? "mute" : "volume"} size={big() ? 24 : 20} />
        </button>
      </Show>
      <Show when={!link() && roomy() && s().duration > 0}>
        <span class="mp-time">{clock(s().time)} / {clock(s().duration)}</span>
      </Show>
      <Show when={link() && roomy()}>
        <button class="mp-btn" aria-label="Open in browser" title="Open in browser" onClick={() => openInBrowser(m().src)}>
          <Icon name="openLink" size={big() ? 24 : 20} />
        </button>
      </Show>
      <Show when={prepare()}>
        <span class="mp-sep" />
        <Show when={!compact()}>
          <button class="mp-btn" aria-label="Replace" title={link() ? "Change the link" : "Replace the file"} onClick={replace}>
            <Icon name="replace" size={20} />
          </button>
        </Show>
        <button class="mp-btn" aria-label="Media settings" title="Title, start and end, mute, loop, autoplay" onClick={() => openMediaDialog({ kind: m().type, edit: props.item })}>
          <Icon name="settings" size={20} />
        </button>
      </Show>
    </div>
  );
}

export function MediaControls() {
  const list = createMemo(() => [...items].filter((i) => i.kind === "image" && i.media));
  const rectOf = (it: DomItem) => {
    const [cx, cy, z] = camera();
    return { x: (it.x - cx) * z, y: (it.y - cy) * z, w: it.w * z, h: it.h * z };
  };
  const visible = (r: { x: number; y: number; w: number; h: number }) =>
    r.w >= 96 && r.x + r.w > 0 && r.y + r.h > 0 && r.x < innerWidth && r.y < innerHeight;
  return (
    <div class="media-layer">
      <For each={list()}>
        {(it) => {
          const r = createMemo(() => rectOf(it));
          // While the teacher works inside a loaded YouTube player (Use
          // objects), its own controls are in the same corner.
          const hidden = () => tool() === "interact" && it.media?.type === "youtube" && pb(it.id).active;
          return (
            <Show when={visible(r()) && !hidden()}>
              <Pill item={it} rect={r()} />
            </Show>
          );
        }}
      </For>
    </div>
  );
}
