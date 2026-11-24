// A media item on the page (World.tsx): the poster until it plays, then the
// player. YouTube and web embeds load only when played (click-to-load), so
// a page full of videos costs nothing and contacts no one until the teacher
// presses Play. Video and audio files play from the lesson, offline.
//
// The item sits under the ink canvas and the input layer like a Smart
// Object, so the pen always draws on top and nothing here steals pen
// input; the screen-space controls are in MediaControls.tsx.

import { Match, Show, Switch, createEffect, createMemo, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { type DomItem, mode, tool } from "../state";
import {
  embedUrl, online, patchPlayback, pb, registerPlayer, unregisterPlayer, siteName, play, clock,
} from "../media/media";
import { Icon } from "./Icon";
import "./Media.css";

const YT_ORIGIN = "https://www.youtube-nocookie.com";

/** What the iframe may do. Embeds are cross-origin pages on allowlisted
 * hosts (never srcdoc, never our origin), so `allow-same-origin` keeps
 * *their* origin for their own storage; it cannot reach the board. No
 * popups, top navigation, downloads or modals. */
const SANDBOX = {
  youtube: "allow-scripts allow-same-origin allow-presentation",
  web: "allow-scripts allow-same-origin allow-forms allow-presentation",
};

function YouTubeFrame(props: { item: DomItem; src: string }) {
  let frame!: HTMLIFrameElement;
  const id = props.item.id;
  const send = (func: string, args: unknown[] = []) =>
    frame.contentWindow?.postMessage(JSON.stringify({ event: "command", func, args }), YT_ORIGIN);
  const onMessage = (e: MessageEvent) => {
    if (e.origin !== YT_ORIGIN || e.source !== frame.contentWindow) return;
    let d: any;
    try {
      d = typeof e.data === "string" ? JSON.parse(e.data) : e.data;
    } catch {
      return;
    }
    if (!d || typeof d !== "object") return;
    if (d.event === "onReady") patchPlayback(id, { loading: false });
    const state = d.event === "onStateChange" ? d.info : d.event === "infoDelivery" ? d.info?.playerState : undefined;
    if (typeof state === "number") {
      // 1 playing, 2 paused, 0 ended, 3 buffering, 5 cued.
      patchPlayback(id, { playing: state === 1 || state === 3, loading: state === 3 });
    }
    const info = d.event === "infoDelivery" ? d.info : null;
    if (info) {
      if (typeof info.muted === "boolean") patchPlayback(id, { muted: info.muted });
      if (typeof info.currentTime === "number") patchPlayback(id, { time: info.currentTime });
      if (typeof info.duration === "number" && info.duration > 0) patchPlayback(id, { duration: info.duration });
    }
  };
  onMount(() => {
    window.addEventListener("message", onMessage);
    registerPlayer(id, {
      play: () => send("playVideo"),
      pause: () => send("pauseVideo"),
      restart: () => {
        send("seekTo", [props.item.media?.start ?? 0, true]);
        send("playVideo");
      },
      setMuted: (m) => send(m ? "mute" : "unMute"),
    });
  });
  onCleanup(() => window.removeEventListener("message", onMessage));
  return (
    <iframe
      ref={frame}
      class="media-frame"
      src={props.src}
      title={props.item.media?.title || "YouTube video"}
      sandbox={SANDBOX.youtube}
      allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
      referrerpolicy="strict-origin-when-cross-origin"
      onLoad={() => {
        // Ask the player to report its state (YouTube's iframe API protocol).
        frame.contentWindow?.postMessage(JSON.stringify({ event: "listening", id, channel: "widget" }), YT_ORIGIN);
        setTimeout(() => patchPlayback(id, { loading: false }), 4000);
      }}
    />
  );
}

function WebFrame(props: { item: DomItem; src: string }) {
  const id = props.item.id;
  onMount(() => {
    registerPlayer(id, {
      play: () => {},
      pause: () => {},
      restart: () => {
        const f = document.querySelector<HTMLIFrameElement>(`[data-item="${id}"] iframe`);
        if (f) f.src = props.src;
      },
      setMuted: () => {},
    });
  });
  return (
    <iframe
      class="media-frame"
      src={props.src}
      title={props.item.media?.title || "Web page"}
      sandbox={SANDBOX.web}
      allow="fullscreen"
      referrerpolicy="strict-origin-when-cross-origin"
      onLoad={() => patchPlayback(id, { loading: false })}
    />
  );
}

/** <video> or <audio> from the lesson's own asset: works offline. */
function FilePlayer(props: { item: DomItem }) {
  let el!: HTMLVideoElement & HTMLAudioElement;
  const id = props.item.id;
  const m = () => props.item.media!;
  const url = createMemo(() => controller.assetUrl(m().src));
  const start = () => m().start ?? 0;
  onMount(() => {
    el.muted = !!m().muted;
    patchPlayback(id, { active: true, muted: el.muted });
    registerPlayer(id, {
      play: () => {
        if (m().end && el.currentTime >= m().end! - 0.05) el.currentTime = start();
        el.play().catch(() => patchPlayback(id, { playing: false }));
      },
      pause: () => el.pause(),
      restart: () => {
        el.currentTime = start();
        el.play().catch(() => {});
      },
      setMuted: (v) => (el.muted = v),
    });
    if (m().autoplay && mode() !== "prepare") queueMicrotask(() => play(id));
  });
  onCleanup(() => {
    el.pause();
    unregisterPlayer(id);
  });
  const h = {
    onLoadedMetadata: () => {
      if (start() > 0) el.currentTime = start();
      patchPlayback(id, { duration: el.duration || 0 });
    },
    onPlay: () => patchPlayback(id, { playing: true, loading: false }),
    onPause: () => patchPlayback(id, { playing: false }),
    onWaiting: () => patchPlayback(id, { loading: true }),
    onPlaying: () => patchPlayback(id, { loading: false }),
    onVolumeChange: () => patchPlayback(id, { muted: el.muted }),
    onTimeUpdate: () => {
      const end = m().end;
      if (end && el.currentTime >= end) {
        if (m().loop) el.currentTime = start();
        else el.pause();
      }
      patchPlayback(id, { time: el.currentTime });
    },
    onError: () => patchPlayback(id, { error: "This file cannot be played here.", playing: false }),
  };
  // Native controls only while the teacher uses objects (I); otherwise the
  // pen draws over it and our controls drive it.
  const native = () => tool() === "interact";
  const loop = () => !!m().loop && !m().end;
  return (
    <Switch>
      <Match when={m().type === "video"}>
        <video
          ref={(e) => (el = e as any)}
          class="media-video"
          src={url()}
          poster={controller.assetUrl(props.item.asset!)}
          preload="metadata"
          playsinline
          loop={loop()}
          controls={native()}
          {...h}
        />
      </Match>
      <Match when={m().type === "audio"}>
        <audio ref={(e) => (el = e as any)} class="media-audio" src={url()} preload="metadata" loop={loop()} controls={native()} {...h} />
      </Match>
    </Switch>
  );
}

export function MediaItem(props: { item: DomItem }) {
  const id = props.item.id;
  const m = () => props.item.media!;
  const state = () => pb(id);
  const link = () => m().type === "youtube" || m().type === "web";
  const src = createMemo(() => {
    if (!link() || !state().active) return null;
    const base = embedUrl(m());
    if (!base) return null;
    // Played from the poster: start right away.
    return m().type === "youtube" && !base.includes("autoplay=1") ? `${base}&autoplay=1` : base;
  });
  onMount(() => {
    if (link() && m().autoplay && mode() !== "prepare" && online()) play(id);
  });
  onCleanup(() => {
    if (link()) unregisterPlayer(id);
  });
  // A link that fell off the allowlist (or was edited by hand) never loads.
  createEffect(() => {
    if (link() && state().active && !embedUrl(m())) {
      patchPlayback(id, { active: false, loading: false, error: "This link can't be embedded." });
    }
  });
  const posterUrl = () => {
    const a = props.item.asset ?? "";
    // An agent-added YouTube item has a plain SVG card: online, show the
    // real thumbnail instead (display only, nothing is saved).
    if (m().type === "youtube" && a.endsWith(".svg") && online()) {
      const vid = /[?&]v=([\w-]{11})/.exec(m().src)?.[1];
      if (vid) return `https://i.ytimg.com/vi/${vid}/hqdefault.jpg`;
    }
    return controller.assetUrl(a);
  };
  /** The poster picture: links before they load, and audio (its card). */
  const showPoster = () => (link() ? !state().active : m().type === "audio");
  /** The label and play glyph: also on a video file that has not started. */
  const showScrim = () => showPoster() || (m().type === "video" && !state().playing && state().time <= (m().start ?? 0) + 0.05);
  const caption = () =>
    m().type === "youtube" ? "YouTube" : m().type === "web" ? siteName(m().site) : m().type === "audio" ? "Audio" : "Video";
  return (
    <div
      class={`item media media-${m().type}`}
      classList={{ playing: state().playing, active: state().active }}
      data-item={id}
      style={{
        left: `${props.item.x}px`,
        top: `${props.item.y}px`,
        width: `${props.item.w}px`,
        height: `${props.item.h}px`,
        "pointer-events": tool() === "interact" ? "auto" : "none",
      }}
    >
      <Show when={link() && src()}>
        {(s) => (
          <Show when={m().type === "youtube"} fallback={<WebFrame item={props.item} src={s()} />}>
            <YouTubeFrame item={props.item} src={s()} />
          </Show>
        )}
      </Show>
      <Show when={!link()}>
        <FilePlayer item={props.item} />
      </Show>
      <Show when={showPoster()}>
        <img class="media-poster" src={posterUrl()} alt="" draggable={false} referrerpolicy="no-referrer" />
      </Show>
      <Show when={showScrim()}>
        <div class="media-scrim" aria-hidden="true">
          <Show when={m().type === "audio"}>
            <div class="media-audio-bar">
              <span class="media-audio-fill" style={{ width: `${state().duration ? (100 * state().time) / state().duration : 0}%` }} />
            </div>
            <div class="media-audio-time">{clock(state().time)}{state().duration ? ` / ${clock(state().duration)}` : ""}</div>
          </Show>
          <Show when={m().type !== "audio"}>
            <span class="media-glyph">
              <Icon name={m().type === "web" ? "globe" : "play"} size={28} active />
            </span>
          </Show>
          <span class="media-label">
            <span class="media-kind">{caption()}</span>
            <Show when={m().title}>
              <span class="media-title">{m().title}</span>
            </Show>
          </span>
        </div>
      </Show>
      <Show when={showPoster()}>
        <Show when={link() && !online()}>
          <div class="media-offline" role="status">
            <Icon name="globe" size={20} />
            {m().type === "youtube" ? "Online video, needs internet" : "Online page, needs internet"}
          </div>
        </Show>
      </Show>
      <Show when={state().active && state().loading}>
        <div class="media-loading" role="status" aria-label="Loading">
          <span class="media-spinner" />
        </div>
      </Show>
      <Show when={state().error}>
        <div class="media-offline" role="alert">{state().error}</div>
      </Show>
    </div>
  );
}
