// Rich media on the board: YouTube videos, trusted web embeds, video and
// audio files (image items with a `media` map, spec/rupok-format.md "Media
// items") and 3D models (a built-in Smart Object, crates/rupok-format/src/
// model3d.rs).
//
// Security (docs/roadmap/media.md): the allowlist lives in Rust
// (crates/rupok-format/src/media.rs). A link becomes an iframe only through
// `Board.media_embed_url`, which re-parses the stored link and rebuilds the
// embed URL on an allowlisted https host. Embeds are not Smart Objects:
// they show a trusted site in its own origin; Smart Objects keep their
// sandbox (opaque origin, no network) untouched.
//
// This module: link helpers, online state, posters, inserting, and the
// playback state the players and the controls share (session only, never
// saved).

import { createSignal } from "solid-js";
import { createStore, produce } from "solid-js/store";
import { Board } from "../wasm/rupok.js";
import type { Controller } from "../controller";
import { type DomItem, type MediaInfo, items, mode, pageIndex, pages, showToast } from "../state";

// ------------------------------------------------------------------ links

export interface MediaLink {
  kind: "youtube" | "web";
  site: string;
  site_name: string;
  url: string;
  embed: string;
  title: string;
  poster?: string;
  video_id?: string;
  start?: number;
  end?: number;
}

export interface Site {
  id: string;
  name: string;
  frame_origins: string[];
}

/** A link the board may embed, or null (not on the allowlist). */
export function parseLink(url: string): MediaLink | null {
  try {
    return JSON.parse(Board.media_link_json(url));
  } catch {
    return null;
  }
}

/** The iframe src for a stored item, rebuilt from the allowlist. */
export function embedUrl(m: MediaInfo): string | null {
  return Board.media_embed_url(JSON.stringify(m), location.origin) ?? null;
}

/** A safe http(s) link for "Open in browser", or null. */
export function openUrl(url: string): string | null {
  return Board.open_url(url) ?? null;
}

let sites: Site[] | null = null;
export function trustedSites(): Site[] {
  return (sites ??= JSON.parse(Board.media_sites_json()));
}

export function siteName(id?: string): string {
  return trustedSites().find((s) => s.id === id)?.name ?? "Web";
}

/** The first http(s) URL when `text` is just a link (pasted or dropped). */
export function linkIn(text: string): string | null {
  const t = text.trim();
  if (!t || t.length > 2048 || /\s/.test(t)) return null;
  return /^(https?:\/\/|www\.|youtu\.be\/)/i.test(t) ? t : null;
}

export function openInBrowser(url: string) {
  const safe = openUrl(url);
  if (safe) window.open(safe, "_blank", "noopener,noreferrer");
}

/** `youtube` or `web` media items need the internet. */
export const isOnline = (m: MediaInfo) => m.type === "youtube" || m.type === "web";

export function clock(t: number): string {
  if (!Number.isFinite(t) || t < 0) t = 0;
  const s = Math.floor(t);
  const h = Math.floor(s / 3600);
  const mm = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(mm).padStart(2, "0")}:${ss}` : `${mm}:${ss}`;
}

/** "1:30", "90", "1m30s" → seconds; "" → undefined; NaN when invalid. */
export function parseClock(s: string): number | undefined {
  const t = s.trim();
  if (!t) return undefined;
  if (/^\d+(\.\d+)?$/.test(t)) return Number(t);
  if (/^\d+(:\d{1,2}){1,2}$/.test(t)) return t.split(":").reduce((a, v) => a * 60 + Number(v), 0);
  const m = t.match(/^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s)?$/);
  if (m && m[0]) return Number(m[1] ?? 0) * 3600 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0);
  return NaN;
}

// ------------------------------------------------------------- online state

export const [online, setOnline] = createSignal(typeof navigator === "undefined" ? true : navigator.onLine);
if (typeof window !== "undefined") {
  window.addEventListener("online", () => setOnline(true));
  window.addEventListener("offline", () => setOnline(false));
}

// ------------------------------------------------------------ files

export const MAX_MEDIA_BYTES = 200 << 20;
export const WARN_MEDIA_BYTES = 50 << 20;
export const MAX_MODEL_BYTES = 18 << 20;

const VIDEO_EXT = /\.(mp4|m4v|webm|mov)$/i;
const AUDIO_EXT = /\.(mp3|m4a|aac|ogg|oga|opus|wav)$/i;
const MODEL_EXT = /\.(glb|gltf)$/i;
export const MEDIA_ACCEPT = "video/mp4,video/webm,audio/*,.mp4,.m4v,.webm,.mp3,.m4a,.ogg,.oga,.opus,.wav";
export const MODEL_ACCEPT = ".glb,.gltf,model/gltf-binary,model/gltf+json";

/** What a dropped or picked file is, by name and type (the engine sniffs
 * the bytes again before storing anything). */
export function fileKind(f: File): "video" | "audio" | "model" | null {
  const n = f.name.toLowerCase();
  if (MODEL_EXT.test(n) || f.type.startsWith("model/gltf")) return "model";
  if (f.type.startsWith("video/") || VIDEO_EXT.test(n)) return "video";
  if (f.type.startsWith("audio/") || AUDIO_EXT.test(n)) return "audio";
  return null;
}

export function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = accept;
    i.onchange = () => resolve(i.files?.[0] ?? null);
    i.click();
  });
}

const mb = (n: number) => `${(n / 1048576).toFixed(n < 10 * 1048576 ? 1 : 0)} MB`;

// ------------------------------------------------------------ posters

const W = 1280, H = 720;

function canvas(w = W, h = H): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return [c, c.getContext("2d")!];
}

function blobOf(c: HTMLCanvasElement, type = "image/jpeg", q = 0.86): Promise<Blob> {
  return new Promise((ok, fail) => c.toBlob((b) => (b ? ok(b) : fail(new Error("poster failed"))), type, q));
}

async function fetchWithTimeout(url: string, ms = 6000): Promise<Response | null> {
  if (!online()) return null;
  const ctl = new AbortController();
  const t = setTimeout(() => ctl.abort(), ms);
  try {
    const r = await fetch(url, { signal: ctl.signal, mode: "cors", credentials: "omit", referrerPolicy: "no-referrer" });
    return r.ok ? r : null;
  } catch {
    return null;
  } finally {
    clearTimeout(t);
  }
}

async function loadImage(blob: Blob): Promise<HTMLImageElement | null> {
  const url = URL.createObjectURL(blob);
  try {
    const img = new Image();
    img.src = url;
    await img.decode();
    return img;
  } catch {
    return null;
  } finally {
    URL.revokeObjectURL(url);
  }
}

function wrap(g: CanvasRenderingContext2D, text: string, maxW: number, maxLines: number): string[] {
  const words = text.split(/\s+/).filter(Boolean);
  const lines: string[] = [];
  let line = "";
  for (const w of words) {
    const next = line ? `${line} ${w}` : w;
    if (g.measureText(next).width > maxW && line) {
      lines.push(line);
      line = w;
      if (lines.length === maxLines) break;
    } else line = next;
  }
  if (lines.length < maxLines && line) lines.push(line);
  if (lines.length === maxLines && words.join(" ") !== lines.join(" ")) {
    let last = lines[maxLines - 1];
    while (last.length > 1 && g.measureText(`${last}…`).width > maxW) last = last.slice(0, -1);
    lines[maxLines - 1] = `${last}…`;
  }
  return lines;
}

const FONT = '"Figtree Variable", Figtree, "Noto Sans Bengali Variable", system-ui, sans-serif';

/** A flat poster card in the board's style: mint glyph disc, title, site. */
export async function cardPoster(kind: MediaInfo["type"] | "model", title: string, caption: string): Promise<Blob> {
  const [c, g] = canvas();
  g.fillStyle = "#f1f3f5";
  g.fillRect(0, 0, W, H);
  // Soft mint field and a dotted frame, FigJam-like.
  g.fillStyle = "#e6fcf5";
  g.beginPath();
  g.roundRect(40, 40, W - 80, H - 80, 36);
  g.fill();
  g.setLineDash([2, 14]);
  g.lineCap = "round";
  g.lineWidth = 4;
  g.strokeStyle = "#63e6be";
  g.stroke();
  g.setLineDash([]);
  // Glyph disc.
  const cx = W / 2, cy = 290, r = 92;
  g.fillStyle = "#3ee6b0";
  g.beginPath();
  g.arc(cx, cy, r, 0, Math.PI * 2);
  g.fill();
  g.fillStyle = "#04241b";
  g.strokeStyle = "#04241b";
  g.lineWidth = 12;
  g.lineJoin = "round";
  g.beginPath();
  if (kind === "audio") {
    for (const [i, hgt] of [36, 70, 104, 64, 30].entries()) {
      const x = cx - 56 + i * 28;
      g.moveTo(x, cy - hgt / 2);
      g.lineTo(x, cy + hgt / 2);
    }
    g.stroke();
  } else if (kind === "web") {
    g.lineWidth = 9;
    g.arc(cx, cy, 52, 0, Math.PI * 2);
    g.moveTo(cx - 52, cy);
    g.lineTo(cx + 52, cy);
    g.ellipse(cx, cy, 22, 52, 0, 0, Math.PI * 2);
    g.stroke();
  } else {
    g.moveTo(cx - 26, cy - 40);
    g.lineTo(cx + 44, cy);
    g.lineTo(cx - 26, cy + 40);
    g.closePath();
    g.fill();
  }
  g.textAlign = "center";
  g.fillStyle = "#111418";
  g.font = `700 52px ${FONT}`;
  const lines = wrap(g, title || "Untitled", W - 260, 2);
  lines.forEach((l, i) => g.fillText(l, cx, 470 + i * 62));
  g.font = `600 30px ${FONT}`;
  g.fillStyle = "#087f5b";
  g.fillText(caption, cx, 470 + lines.length * 62 + 18);
  return blobOf(c);
}

/** Fit `img` into a 16:9 poster, cropping letterbox bars (YouTube's
 * hqdefault is 4:3 with black bands). */
async function posterFromImage(img: HTMLImageElement): Promise<Blob> {
  const [c, g] = canvas();
  g.fillStyle = "#111418";
  g.fillRect(0, 0, W, H);
  const iw = img.naturalWidth, ih = img.naturalHeight;
  let sx = 0, sy = 0, sw = iw, sh = ih;
  if (iw / ih < 16 / 9 - 0.01) {
    sh = iw * 9 / 16;
    sy = (ih - sh) / 2;
  } else if (iw / ih > 16 / 9 + 0.01) {
    sw = ih * 16 / 9;
    sx = (iw - sw) / 2;
  }
  g.drawImage(img, sx, sy, sw, sh, 0, 0, W, H);
  return blobOf(c);
}

/** YouTube: the real thumbnail and title when online, else a card. */
async function youtubePoster(link: MediaLink, title?: string): Promise<{ blob: Blob; title: string }> {
  const id = link.video_id!;
  let name = title?.trim() || "";
  if (!name) {
    const r = await fetchWithTimeout(`https://www.youtube.com/oembed?format=json&url=${encodeURIComponent(link.url)}`, 5000);
    try {
      const j = r ? await r.json() : null;
      if (j && typeof j.title === "string") name = j.title.slice(0, 200);
    } catch { /* keep the default */ }
  }
  name ||= "YouTube video";
  for (const size of ["maxresdefault", "hqdefault"]) {
    const r = await fetchWithTimeout(`https://i.ytimg.com/vi/${id}/${size}.jpg`);
    const img = r ? await loadImage(await r.blob()) : null;
    // A missing maxres answers with a 120×90 placeholder.
    if (img && img.naturalWidth > 200) return { blob: await posterFromImage(img), title: name };
  }
  return { blob: await cardPoster("youtube", name, "YouTube · needs internet"), title: name };
}

async function webPoster(link: MediaLink, title: string): Promise<Blob> {
  if (link.poster) {
    const r = await fetchWithTimeout(link.poster);
    const img = r ? await loadImage(await r.blob()) : null;
    if (img && img.naturalWidth > 100) return posterFromImage(img);
  }
  return cardPoster("web", title, `${link.site_name} · needs internet`);
}

/** A frame of a video file (at `at` seconds) as the poster. */
async function videoPoster(file: Blob, at = 1): Promise<{ blob: Blob; w: number; h: number } | null> {
  const url = URL.createObjectURL(file);
  const v = document.createElement("video");
  v.muted = true;
  v.preload = "auto";
  v.playsInline = true;
  v.src = url;
  try {
    await new Promise<void>((ok, fail) => {
      v.onloadeddata = () => ok();
      v.onerror = () => fail(new Error("cannot decode"));
      setTimeout(() => fail(new Error("timeout")), 8000);
    });
    const t = Math.min(at, Math.max(0, (v.duration || 0) - 0.1));
    if (t > 0) {
      await new Promise<void>((ok) => {
        v.onseeked = () => ok();
        v.currentTime = t;
        setTimeout(ok, 3000);
      });
    }
    const vw = v.videoWidth, vh = v.videoHeight;
    if (!vw || !vh) return null;
    const s = Math.min(1, 1280 / vw, 1280 / vh);
    const [c, g] = canvas(Math.round(vw * s), Math.round(vh * s));
    g.drawImage(v, 0, 0, c.width, c.height);
    return { blob: await blobOf(c), w: vw, h: vh };
  } catch {
    return null;
  } finally {
    v.removeAttribute("src");
    v.load();
    URL.revokeObjectURL(url);
  }
}

const bytes = async (b: Blob) => new Uint8Array(await b.arrayBuffer());
const extOf = (b: Blob) => (b.type === "image/png" ? "png" : b.type === "image/svg+xml" ? "svg" : "jpg");

// ------------------------------------------------------------ inserting

let ctl: Controller | null = null;
/** Called once by the controller (avoids an import cycle). */
export function attachMedia(c: Controller) {
  ctl = c;
}
const board = () => ctl!.board;

/** Run an insert so the new item lands centred on the slide frame. */
function onFrame<T>(f: () => T): T {
  const frame = pages()[pageIndex()]?.frame;
  if (!frame) return f();
  const [x, y, z] = board().camera();
  board().fit_page();
  try {
    return f();
  } finally {
    board().set_camera(x, y, z);
  }
}

/** Size within `maxW × maxH` keeping the aspect ratio. */
function fitSize(w: number, h: number, maxW = 1280, maxH = 720): [number, number] {
  const s = Math.min(maxW / w, maxH / h);
  return [Math.round(w * s), Math.round(h * s)];
}

export interface LinkOptions {
  title?: string;
  start?: number;
  end?: number;
  muted?: boolean;
  loop?: boolean;
  autoplay?: boolean;
}

const [busy, setBusy] = createSignal<string | null>(null);
/** A media insert in progress ("Getting the video…"), for the loading toast. */
export const mediaBusy = busy;

async function withBusy<T>(label: string, f: () => Promise<T>): Promise<T> {
  setBusy(label);
  try {
    return await f();
  } finally {
    setBusy(null);
  }
}

function afterInsert(id: string) {
  if (!ctl) return;
  if (!ctl.selecting()) ctl.setTool("lasso");
  ctl.board.select_json(JSON.stringify([id]));
  focusMedia(id);
}

/** Insert a YouTube video or a trusted web embed. Returns the item id, or
 * null (and a toast saying why). */
export async function insertLink(url: string, opts: LinkOptions = {}): Promise<string | null> {
  const link = parseLink(url);
  if (!link) {
    showToast("Rupok embeds only YouTube and trusted education sites. Use Insert ▸ Web embed to open other links.", 5000);
    return null;
  }
  return withBusy(link.kind === "youtube" ? "Getting the video…" : `Adding ${link.site_name}…`, async () => {
    let title = opts.title?.trim() || link.title;
    let poster: Blob;
    if (link.kind === "youtube") {
      const p = await youtubePoster(link, opts.title);
      poster = p.blob;
      title = p.title;
    } else {
      poster = await webPoster(link, title);
    }
    const media: MediaInfo = {
      type: link.kind,
      src: link.url,
      title,
      start: opts.start ?? link.start,
      end: opts.end ?? link.end,
      muted: !!opts.muted,
      loop: !!opts.loop,
      autoplay: !!opts.autoplay,
    };
    try {
      const pb = await bytes(poster);
      const id = onFrame(() => board().add_media(JSON.stringify(media), pb, extOf(poster), undefined, "", 1280, 720));
      afterInsert(id);
      if (!online()) showToast("Added. It needs the internet to play; offline it shows the poster.", 4000);
      return id;
    } catch (e: any) {
      showToast(`Could not add the link: ${e?.message ?? e}`, 5000);
      return null;
    }
  });
}

/** Insert a video or audio file, stored in the lesson. */
export async function insertMediaFile(f: File): Promise<string | null> {
  if (f.size > MAX_MEDIA_BYTES) {
    showToast(`${f.name} is ${mb(f.size)}. Videos can be at most ${mb(MAX_MEDIA_BYTES)} in a lesson; trim or compress it first.`, 6000);
    return null;
  }
  return withBusy(`Adding ${f.name}…`, async () => {
    const data = new Uint8Array(await f.arrayBuffer());
    const sniff = Board.sniff_media(data);
    if (!sniff.length) {
      showToast(`${f.name} is not a video or audio file Rupok can play (mp4, webm, mp3, m4a, ogg, wav).`, 5000);
      return null;
    }
    const [kind, ext] = sniff as ["video" | "audio", string];
    const title = f.name.replace(/\.[^.]+$/, "").replace(/[_-]+/g, " ").trim() || (kind === "audio" ? "Audio" : "Video");
    let poster: Blob;
    let size: [number, number] = kind === "audio" ? [800, 450] : [1280, 720];
    const frame = kind === "video" ? await videoPoster(f) : null;
    if (frame) {
      poster = frame.blob;
      size = fitSize(frame.w, frame.h);
    } else {
      poster = await cardPoster(kind, title, kind === "audio" ? "Audio" : "Video");
    }
    const media: MediaInfo = { type: kind, src: "", title };
    try {
      const pb = await bytes(poster);
      const id = onFrame(() => board().add_media(JSON.stringify(media), pb, extOf(poster), data, ext, size[0], size[1]));
      afterInsert(id);
      if (f.size > WARN_MEDIA_BYTES) {
        showToast(`Added. ${f.name} is ${mb(f.size)}: the lesson file grows by that much, so it takes longer to save and share.`, 6000);
      }
      return id;
    } catch (e: any) {
      showToast(`Could not add ${f.name}: ${e?.message ?? e}`, 5000);
      return null;
    }
  });
}

/** Insert a 3D model as the built-in viewer object. */
export async function insertModelFile(f: File): Promise<string | null> {
  if (f.size > MAX_MODEL_BYTES) {
    showToast(`${f.name} is ${mb(f.size)}; 3D models can be at most ${mb(MAX_MODEL_BYTES)}. Try a smaller or compressed export.`, 6000);
    return null;
  }
  return withBusy(`Adding ${f.name}…`, async () => {
    try {
      const data = new Uint8Array(await f.arrayBuffer());
      const id = onFrame(() => board().add_model(data, f.name, "", 1280, 720));
      afterInsert(id);
      showToast("3D model added. Press I (Use objects) to turn it; Save view adds clicker steps.", 4500);
      return id;
    } catch (e: any) {
      showToast(`Could not add ${f.name}: ${e?.message ?? e}`, 6000);
      return null;
    }
  });
}

/** Any dropped or picked media or model file. */
export async function insertFile(f: File): Promise<string | null> {
  const k = fileKind(f);
  if (k === "model") return insertModelFile(f);
  if (k) return insertMediaFile(f);
  return null;
}

/** Change a media item's settings (title, times, flags); one undo step. */
export function updateMedia(id: string, m: MediaInfo): boolean {
  try {
    board().set_media(id, JSON.stringify(m), undefined, "", undefined, "");
    return true;
  } catch (e: any) {
    showToast(`Could not change it: ${e?.message ?? e}`, 4000);
    return false;
  }
}

/** Swap what a media item plays (a new link or file), keeping its place. */
export async function replaceMedia(item: DomItem, source: { url: string } | { file: File }, opts: LinkOptions = {}): Promise<boolean> {
  const old = item.media!;
  if ("url" in source) {
    const link = parseLink(source.url);
    if (!link) {
      showToast("That link is not on the list of sites Rupok can embed.", 4000);
      return false;
    }
    return withBusy("Updating…", async () => {
      const p = link.kind === "youtube" ? await youtubePoster(link, opts.title) : { blob: await webPoster(link, opts.title || link.title), title: opts.title || link.title };
      const m: MediaInfo = { ...old, ...opts, type: link.kind, src: link.url, title: p.title, start: opts.start ?? link.start, end: opts.end ?? link.end };
      try {
        board().set_media(item.id, JSON.stringify(m), await bytes(p.blob), extOf(p.blob), undefined, "");
        stopPlayer(item.id);
        return true;
      } catch (e: any) {
        showToast(`Could not change it: ${e?.message ?? e}`, 4000);
        return false;
      }
    });
  }
  const f = source.file;
  if (f.size > MAX_MEDIA_BYTES) {
    showToast(`${f.name} is ${mb(f.size)}; the limit is ${mb(MAX_MEDIA_BYTES)}.`, 5000);
    return false;
  }
  return withBusy(`Replacing with ${f.name}…`, async () => {
    const data = new Uint8Array(await f.arrayBuffer());
    const sniff = Board.sniff_media(data);
    if (!sniff.length) {
      showToast(`${f.name} is not a video or audio file.`, 4000);
      return false;
    }
    const [kind, ext] = sniff as ["video" | "audio", string];
    const title = f.name.replace(/\.[^.]+$/, "").replace(/[_-]+/g, " ").trim();
    const frame = kind === "video" ? await videoPoster(f) : null;
    const poster = frame?.blob ?? (await cardPoster(kind, title, kind === "audio" ? "Audio" : "Video"));
    const m: MediaInfo = { type: kind, src: "", title, muted: old.muted, loop: old.loop, autoplay: old.autoplay };
    try {
      board().set_media(item.id, JSON.stringify(m), await bytes(poster), extOf(poster), data, ext);
      stopPlayer(item.id);
      return true;
    } catch (e: any) {
      showToast(`Could not replace it: ${e?.message ?? e}`, 4000);
      return false;
    }
  });
}

// ------------------------------------------------------------ playback

/** Session-only playback state per item id (never saved). */
export interface Playback {
  /** The player is loaded (an iframe for links; always for files). */
  active: boolean;
  playing: boolean;
  muted: boolean;
  loading: boolean;
  time: number;
  duration: number;
  error?: string;
}

const blank = (): Playback => ({ active: false, playing: false, muted: false, loading: false, time: 0, duration: 0 });
export const [playback, setPlayback] = createStore<Record<string, Playback>>({});

export function pb(id: string): Playback {
  return playback[id] ?? blank();
}

export function patchPlayback(id: string, p: Partial<Playback>) {
  setPlayback(produce((s) => {
    s[id] = { ...(s[id] ?? blank()), ...p };
  }));
}

/** A player the controls drive: an <iframe> (YouTube, web) or <video>/<audio>. */
export interface Player {
  play(): void;
  pause(): void;
  restart(): void;
  setMuted(m: boolean): void;
}

const players = new Map<string, Player>();
export function registerPlayer(id: string, p: Player) {
  players.set(id, p);
}
export function unregisterPlayer(id: string) {
  players.delete(id);
  setPlayback(produce((s) => {
    delete s[id];
  }));
}

function itemById(id: string): DomItem | undefined {
  return [...items].find((i) => i.id === id);
}

/** Play (loading the embed first for links). */
export function play(id: string) {
  const it = itemById(id);
  if (!it?.media) return;
  focusMedia(id);
  if (isOnline(it.media) && !online()) {
    showToast(it.media.type === "youtube" ? "This video needs the internet. Connect, then press Play." : "This page needs the internet.", 3500);
    return;
  }
  const p = players.get(id);
  if (p && pb(id).active) p.play();
  else patchPlayback(id, { active: true, loading: true, playing: it.media.type !== "web" });
}

export function pause(id: string) {
  players.get(id)?.pause();
  patchPlayback(id, { playing: false });
}

export function toggle(id: string) {
  if (pb(id).playing) pause(id);
  else play(id);
}

export function restart(id: string) {
  focusMedia(id);
  const p = players.get(id);
  if (p && pb(id).active) p.restart();
  else play(id);
}

export function toggleMute(id: string) {
  const it = itemById(id);
  const m = !(pb(id).active ? pb(id).muted : !!it?.media?.muted);
  players.get(id)?.setMuted(m);
  patchPlayback(id, { muted: m });
}

/** Back to the poster (unloads an embed). */
export function stopPlayer(id: string) {
  players.get(id)?.pause();
  patchPlayback(id, { active: false, playing: false, loading: false, time: 0 });
}

// ------------------------------------------------------------ focus and keys

const [focused, setFocused] = createSignal<string | null>(null);
export function focusMedia(id: string | null) {
  setFocused(id);
}

/** The media item keys act on: the last one used on this page, else the
 * only playable one on the page. */
export function focusedMedia(): string | null {
  const list = [...items].filter((i) => i.media && i.media.type !== "web");
  const f = focused();
  if (f && list.some((i) => i.id === f)) return f;
  return list.length === 1 ? list[0].id : null;
}

/** Space, K, "." (a clicker's blank key) and the media key play and pause
 * the focused video. Space only in Teach and Student view, where it does
 * not mean "hold to pan". Returns true when it handled the key. */
export function mediaKey(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey || e.repeat) return false;
  const k = e.key;
  const presenting = mode() !== "prepare";
  const isPlayKey = k === "MediaPlayPause" || k === "k" || k === "K" || k === "." || (k === " " && presenting);
  if (!isPlayKey) return false;
  const id = focusedMedia();
  if (!id) return false;
  e.preventDefault();
  toggle(id);
  return true;
}
