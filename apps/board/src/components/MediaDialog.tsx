// The media dialogs: "Video / YouTube…" and "Web embed…" (paste a link,
// see what Rupok makes of it, set start/end, mute, loop, autoplay), the
// settings of a placed media item, and the Media tab of the Smart Object
// library (MediaGallery). Links off the allowlist are never embedded: the
// dialog says so and offers to open them in the browser.

import { For, Show, createMemo, createSignal } from "solid-js";
import { type DomItem, type MediaInfo, showToast } from "../state";
import {
  MEDIA_ACCEPT, MODEL_ACCEPT, clock, insertLink, insertMediaFile, insertModelFile, mediaBusy, online, openInBrowser,
  openUrl, parseClock, parseLink, pickFile, replaceMedia, trustedSites, updateMedia,
} from "../media/media";
import { Dialog } from "./Dialogs";
import { Icon } from "./Icon";
import "./Media.css";

export interface MediaDialogOptions {
  kind: MediaInfo["type"];
  /** Prefilled link (pasted). */
  url?: string;
  /** Edit this placed item instead of adding one. */
  edit?: DomItem;
  /** Editing: change the link too. */
  replace?: boolean;
}

const [open, setOpen] = createSignal<MediaDialogOptions | null>(null);
export const openMediaDialog = (o: MediaDialogOptions) => setOpen(o);

/** Example links for the trusted sites, one tap to insert. */
const EXAMPLES: { site: string; label: string; url: string }[] = [
  { site: "PhET", label: "Projectile motion", url: "https://phet.colorado.edu/en/simulations/projectile-motion" },
  { site: "PhET", label: "Build an atom", url: "https://phet.colorado.edu/en/simulations/build-an-atom" },
  { site: "GeoGebra", label: "Graphing calculator", url: "https://www.geogebra.org/graphing" },
  { site: "Desmos", label: "Graphing calculator", url: "https://www.desmos.com/calculator" },
  { site: "Wikipedia", label: "Photosynthesis", url: "https://en.wikipedia.org/wiki/Photosynthesis" },
];

function Toggle(props: { label: string; on: boolean; onChange: (v: boolean) => void; hint?: string }) {
  return (
    <button class="media-toggle" role="switch" aria-checked={props.on} title={props.hint} onClick={() => props.onChange(!props.on)}>
      <span class="mt-knob" aria-hidden="true" />
      {props.label}
    </button>
  );
}

function MediaDialog(props: { opts: MediaDialogOptions; onClose: () => void }) {
  const edit = props.opts.edit;
  const old = edit?.media;
  const isLink = () => props.opts.kind === "youtube" || props.opts.kind === "web";
  const [url, setUrl] = createSignal(props.opts.url ?? (old && isLink() ? old.src : ""));
  const [title, setTitle] = createSignal(old?.title ?? "");
  const [start, setStart] = createSignal(old?.start != null ? clock(old.start) : "");
  const [end, setEnd] = createSignal(old?.end != null ? clock(old.end) : "");
  const [muted, setMuted] = createSignal(!!old?.muted);
  const [loop, setLoop] = createSignal(!!old?.loop);
  const [autoplay, setAutoplay] = createSignal(!!old?.autoplay);
  const [busy, setBusy] = createSignal(false);
  const link = createMemo(() => (url().trim() ? parseLink(url()) : null));
  const browserUrl = createMemo(() => (url().trim() && !link() ? openUrl(/^https?:/i.test(url().trim()) ? url().trim() : `https://${url().trim()}`) : null));
  const kind = () => link()?.kind ?? props.opts.kind;
  const playable = () => kind() !== "web";
  const startS = createMemo(() => parseClock(start()));
  const endS = createMemo(() => parseClock(end()));
  const timeError = () => {
    if (Number.isNaN(startS()) || Number.isNaN(endS())) return "Times look like 1:30 or 90.";
    if (startS() != null && endS() != null && endS()! <= startS()!) return "End must be after start.";
    return null;
  };
  const thumb = () => {
    const id = link()?.video_id;
    return id && online() ? `https://i.ytimg.com/vi/${id}/mqdefault.jpg` : null;
  };
  // When a pasted link carries a start time, show it.
  const onUrl = (v: string) => {
    setUrl(v);
    const l = parseLink(v);
    if (l?.start != null && !start()) setStart(clock(l.start));
    if (l?.end != null && !end()) setEnd(clock(l.end));
  };
  const opts = () => ({
    title: title().trim() || undefined,
    start: playable() ? startS() : undefined,
    end: playable() ? endS() : undefined,
    muted: muted(),
    loop: loop(),
    autoplay: autoplay(),
  });
  const canSave = () => !busy() && !timeError() && (!isLink() || !!link());
  const save = async () => {
    if (!canSave()) return;
    setBusy(true);
    try {
      if (!edit) {
        if (await insertLink(url(), opts())) props.onClose();
        return;
      }
      const o = opts();
      if (isLink() && link() && link()!.url !== old!.src) {
        if (await replaceMedia(edit, { url: url() }, o)) props.onClose();
        return;
      }
      const next: MediaInfo = { ...old!, title: o.title ?? old!.title, start: o.start, end: o.end, muted: o.muted, loop: o.loop, autoplay: o.autoplay };
      if (updateMedia(edit.id, next)) props.onClose();
    } finally {
      setBusy(false);
    }
  };
  const heading = () =>
    edit ? "Media settings" : props.opts.kind === "web" ? "Embed a web page" : "Add a video";
  const sub = () =>
    edit
      ? "Change the title, where it starts and stops, and how it plays."
      : props.opts.kind === "web"
        ? "Paste a link from a trusted education site. It runs live on the board (it needs the internet) with a picture for offline and PDF."
        : "Paste a YouTube link. It plays in class from youtube-nocookie.com; offline and in PDFs it shows a poster.";
  return (
    <Dialog icon={props.opts.kind === "web" ? "globe" : props.opts.kind === "audio" ? "audio" : "video"} title={heading()} sub={sub()} width={620} onClose={props.onClose}>
      <div class="media-form" onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && void save()}>
        <Show when={isLink()}>
          <div class="field">
            <label for="media-url">{props.opts.kind === "web" ? "Link" : "YouTube link"}</label>
            <input
              id="media-url"
              class="input"
              type="url"
              inputmode="url"
              autocomplete="off"
              spellcheck={false}
              placeholder={props.opts.kind === "web" ? "https://phet.colorado.edu/en/simulations/…" : "https://www.youtube.com/watch?v=…"}
              value={url()}
              onInput={(e) => onUrl(e.currentTarget.value)}
              ref={(el) => queueMicrotask(() => !edit && el.focus())}
            />
          </div>
          <Show when={url().trim()}>
            <Show
              when={link()}
              fallback={
                <div class="media-notice" role="status">
                  <Icon name="info" size={20} />
                  <div>
                    <b>This site can't be embedded.</b> Rupok embeds only YouTube and trusted education sites:{" "}
                    {trustedSites().filter((s) => s.id !== "youtube").map((s) => s.name).join(", ")}.
                    <Show when={browserUrl()}>
                      {(u) => (
                        <button class="btn soft" onClick={() => openInBrowser(u())}>
                          <Icon name="openLink" /><span class="label">Open in browser</span>
                        </button>
                      )}
                    </Show>
                  </div>
                </div>
              }
            >
              {(l) => (
                <div class="media-detected">
                  <Show when={thumb()} fallback={<span class="md-badge"><Icon name={l().kind === "youtube" ? "video" : "globe"} size={24} active /></span>}>
                    {(t) => <img class="md-thumb" src={t()} alt="" referrerpolicy="no-referrer" />}
                  </Show>
                  <div>
                    <div class="md-site">{l().site_name}</div>
                    <div class="md-title">{l().kind === "youtube" ? (title() || "Title from YouTube") : (title() || l().title)}</div>
                    <div class="md-url">{l().url}</div>
                  </div>
                </div>
              )}
            </Show>
          </Show>
        </Show>
        <div class="field">
          <label for="media-title">Title</label>
          <input id="media-title" class="input" value={title()} onInput={(e) => setTitle(e.currentTarget.value)} placeholder={isLink() ? "From the link" : "Title"} />
        </div>
        <Show when={playable()}>
          <div class="row">
            <div class="field">
              <label for="media-start">Start at</label>
              <input id="media-start" class="input tnum" inputmode="numeric" placeholder="0:00" value={start()} onInput={(e) => setStart(e.currentTarget.value)} />
            </div>
            <div class="field">
              <label for="media-end">Stop at</label>
              <input id="media-end" class="input tnum" inputmode="numeric" placeholder="End" value={end()} onInput={(e) => setEnd(e.currentTarget.value)} />
            </div>
          </div>
          <Show when={timeError()}>
            <div class="error-box">{timeError()}</div>
          </Show>
          <div class="media-toggles">
            <Toggle label="Muted" on={muted()} onChange={setMuted} />
            <Toggle label="Loop" on={loop()} onChange={setLoop} />
            <Toggle label="Autoplay" on={autoplay()} onChange={setAutoplay} hint="Starts when the page is shown in Teach or the Student view" />
          </div>
        </Show>
        <div class="actions">
          <div class="grow" />
          <button class="btn" onClick={props.onClose}>Cancel</button>
          <button class="btn solid" disabled={!canSave()} onClick={() => void save()}>
            <Show when={busy() || mediaBusy()} fallback={edit ? "Save" : "Add to page"}>
              <span class="media-spinner small" /> Adding…
            </Show>
          </button>
        </div>
      </div>
    </Dialog>
  );
}

export function MediaDialogHost() {
  return (
    <Show when={open()}>
      {(o) => <MediaDialog opts={o()} onClose={() => setOpen(null)} />}
    </Show>
  );
}

// ------------------------------------------------------- insert entry points

export const insertVideo = () => openMediaDialog({ kind: "youtube" });
export const insertWebEmbed = () => openMediaDialog({ kind: "web" });
export async function pickMedia(kind: "video" | "audio") {
  const f = await pickFile(kind === "audio" ? "audio/*,.mp3,.m4a,.ogg,.oga,.opus,.wav" : MEDIA_ACCEPT);
  if (f) await insertMediaFile(f);
}
export async function pickModel() {
  const f = await pickFile(MODEL_ACCEPT);
  if (f) await insertModelFile(f);
}

/** A pasted or dropped link: embed it when trusted, else explain and offer
 * the browser. Returns true when it was a link. */
export function handleLink(text: string): boolean {
  const t = text.trim();
  if (!/^(https?:\/\/|www\.|youtu\.be\/)\S+$/i.test(t) || t.length > 2048) return false;
  if (parseLink(t)) void insertLink(t);
  else openMediaDialog({ kind: "web", url: t });
  return true;
}

// ------------------------------------------------------- library tab

export function MediaGallery(props: { onInserted: () => void }) {
  const run = (f: () => unknown) => async () => {
    props.onInserted();
    await f();
  };
  const cards: { icon: "play" | "video" | "audio" | "cube" | "globe"; title: string; text: string; run: () => unknown }[] = [
    { icon: "play", title: "YouTube video", text: "Paste a link. Start and end times, mute, loop.", run: insertVideo },
    { icon: "video", title: "Video file", text: "mp4 or webm, saved in the lesson: plays offline.", run: () => pickMedia("video") },
    { icon: "audio", title: "Audio clip", text: "mp3, m4a, ogg or wav, saved in the lesson.", run: () => pickMedia("audio") },
    { icon: "cube", title: "3D model", text: "A .glb file: orbit, zoom, animate, saved views.", run: pickModel },
    { icon: "globe", title: "Web embed", text: "PhET, GeoGebra, Desmos, Google Slides, Wikipedia, Khan Academy.", run: insertWebEmbed },
  ];
  return (
    <div class="media-gallery">
      <div class="mg-grid">
        <For each={cards}>
          {(c) => (
            <button class="mg-card" onClick={run(c.run)}>
              <span class="mg-icon"><Icon name={c.icon} size={28} active /></span>
              <span class="mg-title">{c.title}</span>
              <span class="mg-text">{c.text}</span>
            </button>
          )}
        </For>
      </div>
      <div class="mg-head">Try a trusted site</div>
      <div class="mg-examples">
        <For each={EXAMPLES}>
          {(x) => (
            <button
              class="preset-chip"
              disabled={!online()}
              title={online() ? x.url : "Needs the internet"}
              onClick={run(() => insertLink(x.url).then((id) => id || showToast("Could not add it.")))}
            >
              {x.site}: {x.label}
            </button>
          )}
        </For>
      </div>
      <p class="mg-note">
        Drop a video, audio or .glb file on the board, or paste a YouTube link, to add it directly.
        Embeds need the internet; files and 3D models work offline.
      </p>
    </div>
  );
}
