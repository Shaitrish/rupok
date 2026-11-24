// Classroom stamps: a star for a good answer, a tick, a cross, "Excellent",
// a numbered badge, any emoji.
//
// A stamp is a text item with the `stamp` key (spec/rupok-format.md, Stamps):
// `text` holds what a reader without stamps shows (the emoji or the label),
// so older readers still see "⭐" or "Excellent!". The board draws the stamp
// from its kind, colour and box (`stampSvg`), the PDF export rasterises the
// same SVG (`stampCanvas`), and replay shows it at the time it was placed.

import { createSignal } from "solid-js";
import { type DomItem, items, prefs, setPrefs } from "./state";
import { customSize, resolveStamp, svgDataUrl } from "./stampLib";

export type StampRender = "star" | "tick" | "cross" | "heart" | "circle" | "label" | "emoji" | "arrow";

export interface StampDef {
  /** Stored as the item's `stamp` key. */
  id: string;
  label: string;
  /** The fallback text stored in the item. */
  text: string;
  /** Default colour, 0xRRGGBBAA (emoji ignore it). */
  color: number;
  render: StampRender;
  /** Arrow direction in degrees (0 = right). */
  angle?: number;
}

const GOLD = 0xf5b301ff, GREEN = 0x30a46cff, RED = 0xe5484dff, BLUE = 0x3e63ddff, ORANGE = 0xf76b15ff, PURPLE = 0x8e4ec6ff;

export const STAMPS: StampDef[] = [
  { id: "star", label: "Star", text: "⭐", color: GOLD, render: "star" },
  { id: "tick", label: "Tick", text: "✅", color: GREEN, render: "tick" },
  { id: "cross", label: "Cross", text: "❌", color: RED, render: "cross" },
  { id: "thumbs-up", label: "Thumbs up", text: "👍", color: GOLD, render: "emoji" },
  { id: "hundred", label: "100", text: "💯", color: RED, render: "emoji" },
  { id: "heart", label: "Heart", text: "❤️", color: RED, render: "heart" },
  { id: "trophy", label: "Trophy", text: "🏆", color: GOLD, render: "emoji" },
  { id: "excellent", label: "Excellent", text: "Excellent!", color: GREEN, render: "label" },
  { id: "good-job", label: "Good job", text: "Good job!", color: BLUE, render: "label" },
  { id: "try-again", label: "Try again", text: "Try again", color: ORANGE, render: "label" },
  { id: "important", label: "Important", text: "⚡ Important", color: GOLD, render: "label" },
  { id: "question", label: "Question", text: "?", color: PURPLE, render: "circle" },
  { id: "badge", label: "Number", text: "1", color: BLUE, render: "circle" },
  { id: "arrow-right", label: "Arrow right", text: "→", color: RED, render: "arrow", angle: 0 },
  { id: "arrow-down", label: "Arrow down", text: "↓", color: RED, render: "arrow", angle: 90 },
  { id: "arrow-left", label: "Arrow left", text: "←", color: RED, render: "arrow", angle: 180 },
  { id: "arrow-up", label: "Arrow up", text: "↑", color: RED, render: "arrow", angle: 270 },
];

/** Emoji for the picker: a classroom set first, then the system font does
 * the rest (any emoji typed or pasted into the picker's field works). */
export const EMOJI = [
  "😀", "😃", "😄", "😊", "🙂", "😉", "😍", "🤩", "😎", "🤔", "😮", "😢", "😡", "🥳", "😴", "🤯",
  "👍", "👎", "👏", "🙌", "👌", "✌️", "🤞", "💪", "👀", "🙋", "✋", "👉", "👈", "👆", "👇", "🫶",
  "⭐", "🌟", "✨", "💯", "🔥", "⚡", "💡", "❗", "❓", "✅", "❌", "⚠️", "📌", "🎯", "🏆", "🥇",
  "🥈", "🥉", "🎉", "🎈", "🎁", "❤️", "💙", "💚", "💛", "🧡", "💜", "📚", "📝", "✏️", "📐", "🧪",
  "🔬", "🔭", "🧮", "💻", "🖥️", "⌨️", "🧠", "🌍", "🌱", "🍎", "⏰", "🔔", "🚀", "🐞", "🦉", "🐝",
];

export const stampDef = (id: string): StampDef | undefined => STAMPS.find((s) => s.id === id);

/** Default box of a stamp, world units: squares for symbols, wider labels. */
export function stampSize(def: StampDef, text: string): { w: number; h: number } {
  if (def.render === "label") {
    const h = 76;
    const chars = [...text].length;
    return { w: Math.round(Math.max(h * 1.8, h * 0.42 * chars + h * 0.9)), h };
  }
  return { w: 96, h: 96 };
}

// ---------------------------------------------------------------- drawing

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
const css = (c: number) => `#${(c >>> 8).toString(16).padStart(6, "0")}`;
/** Light colours (gold, yellow) get dark text, the rest white. */
export function onColor(c: number): string {
  const r = (c >>> 24) & 255, g = (c >>> 16) & 255, b = (c >>> 8) & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b > 168 ? "#1c2024" : "#ffffff";
}
const FONT = "Figtree Variable, Figtree, system-ui, -apple-system, Segoe UI, Roboto, sans-serif";
const EMOJI_FONT = "Apple Color Emoji, Segoe UI Emoji, Noto Color Emoji, Twemoji Mozilla, sans-serif";

function starPath(cx: number, cy: number, R: number, r: number): string {
  let d = "";
  for (let i = 0; i < 10; i++) {
    const a = -Math.PI / 2 + (i * Math.PI) / 5;
    const rad = i % 2 ? r : R;
    d += `${i ? "L" : "M"}${(cx + rad * Math.cos(a)).toFixed(2)} ${(cy + rad * Math.sin(a)).toFixed(2)}`;
  }
  return d + "Z";
}

/**
 * The stamp as SVG markup filling a `w`×`h` box (world units): flat shapes
 * in the stamp's colour (FigJam-like stickers), labels as pills, emoji in the
 * system emoji font.
 */
export function stampSvg(kind: string, text: string, color: number, w: number, h: number): string {
  const def = stampDef(kind);
  const custom = def ? undefined : resolveStamp(kind);
  if (custom?.art === "svg" && custom.svg) {
    // An agent's (or the teacher's) SVG, already sanitised in Rust, drawn
    // as an image so it can never run code in the page.
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}"><image href="${esc(svgDataUrl(custom.svg, css(color)))}" x="0" y="0" width="${w}" height="${h}" preserveAspectRatio="xMidYMid meet"/></svg>`;
  }
  const render: StampRender = def?.render ?? (custom?.art === "text" ? "label" : "emoji");
  const c = css(color);
  const s = Math.min(w, h);
  const cx = w / 2, cy = h / 2;
  let body = "";
  switch (render) {
    case "star": {
      const R = s * 0.47;
      body = `<path d="${starPath(cx, cy + s * 0.03, R, R * 0.48)}" fill="${c}" stroke="${c}" stroke-width="${(s * 0.08).toFixed(2)}" stroke-linejoin="round"/>`
        + `<path d="${starPath(cx, cy + s * 0.03, R * 0.62, R * 0.3)}" fill="#fff" fill-opacity="0.22"/>`;
      break;
    }
    case "tick":
      body = `<path d="M${cx - s * 0.32} ${cy + s * 0.02} L${cx - s * 0.08} ${cy + s * 0.25} L${cx + s * 0.34} ${cy - s * 0.26}" fill="none" stroke="${c}" stroke-width="${(s * 0.16).toFixed(2)}" stroke-linecap="round" stroke-linejoin="round"/>`;
      break;
    case "cross": {
      const k = s * 0.28;
      body = `<path d="M${cx - k} ${cy - k} L${cx + k} ${cy + k} M${cx + k} ${cy - k} L${cx - k} ${cy + k}" fill="none" stroke="${c}" stroke-width="${(s * 0.16).toFixed(2)}" stroke-linecap="round"/>`;
      break;
    }
    case "heart": {
      const k = s / 100;
      const t = (x: number, y: number) => `${(cx + (x - 50) * k).toFixed(2)} ${(cy + (y - 52) * k).toFixed(2)}`;
      body = `<path d="M${t(50, 86)} C${t(14, 62)} ${t(4, 40)} ${t(18, 24)} C${t(30, 11)} ${t(46, 16)} ${t(50, 30)} C${t(54, 16)} ${t(70, 11)} ${t(82, 24)} C${t(96, 40)} ${t(86, 62)} ${t(50, 86)}Z" fill="${c}"/>`;
      break;
    }
    case "circle": {
      const label = [...text].slice(0, 3).join("");
      const fs = s * (label.length > 2 ? 0.38 : label.length > 1 ? 0.48 : 0.58);
      body = `<circle cx="${cx}" cy="${cy}" r="${s * 0.46}" fill="${c}"/>`
        + `<text x="${cx}" y="${cy}" dy="0.35em" text-anchor="middle" font-family="${FONT}" font-weight="800" font-size="${fs.toFixed(2)}" fill="${onColor(color)}">${esc(label)}</text>`;
      break;
    }
    case "label": {
      const fs = h * 0.42;
      body = `<rect x="${h * 0.04}" y="${h * 0.06}" width="${w - h * 0.08}" height="${h * 0.88}" rx="${h * 0.44}" fill="${c}"/>`
        + `<text x="${cx}" y="${cy}" dy="0.35em" text-anchor="middle" font-family="${FONT}" font-weight="800" font-size="${fs.toFixed(2)}" fill="${onColor(color)}">${esc(text)}</text>`;
      break;
    }
    case "arrow": {
      const a = def?.angle ?? 0;
      const k = s / 100;
      const pts = [[8, 40], [58, 40], [58, 18], [94, 50], [58, 82], [58, 60], [8, 60]]
        .map(([x, y]) => `${(cx + (x - 50) * k).toFixed(2)},${(cy + (y - 50) * k).toFixed(2)}`)
        .join(" ");
      body = `<polygon points="${pts}" fill="${c}" stroke="${c}" stroke-width="${(s * 0.06).toFixed(2)}" stroke-linejoin="round" transform="rotate(${a} ${cx} ${cy})"/>`;
      break;
    }
    default:
      body = `<text x="${cx}" y="${cy}" dy="0.35em" text-anchor="middle" font-family="${EMOJI_FONT}" font-size="${(s * 0.82).toFixed(2)}">${esc(text)}</text>`;
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">${body}</svg>`;
}

/** The stamp drawn on a canvas at `scale` device px per world unit (PDF
 * export, thumbnails). */
export async function stampCanvas(kind: string, text: string, color: number, w: number, h: number, scale: number): Promise<HTMLCanvasElement | null> {
  if (!(w > 0 && h > 0)) return null;
  let s = scale;
  while (w * h * s * s > 16e6 && s > 1) s -= 0.5;
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.ceil(w * s));
  canvas.height = Math.max(1, Math.ceil(h * s));
  const g = canvas.getContext("2d");
  if (!g) return null;
  const def = stampDef(kind);
  const custom = def ? undefined : resolveStamp(kind);
  if (custom?.art === "svg" && custom.svg) {
    const img = new Image();
    img.src = svgDataUrl(custom.svg, css(color));
    try {
      await img.decode();
    } catch {
      return null;
    }
    const k = Math.min(canvas.width / (img.naturalWidth || 1), canvas.height / (img.naturalHeight || 1));
    const dw = (img.naturalWidth || canvas.width) * k, dh = (img.naturalHeight || canvas.height) * k;
    g.drawImage(img, (canvas.width - dw) / 2, (canvas.height - dh) / 2, dw, dh);
    return canvas;
  }
  if ((!def && custom?.art !== "text") || def?.render === "emoji") {
    // Emoji straight from the system font (an SVG image may not reach it).
    const size = Math.min(w, h) * 0.82 * s;
    g.font = `${size}px ${EMOJI_FONT}`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(text, canvas.width / 2, canvas.height / 2 + size * 0.04);
    return canvas;
  }
  const svg = stampSvg(kind, text, color, w, h);
  const img = new Image();
  img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  try {
    await img.decode();
  } catch {
    return null;
  }
  g.drawImage(img, 0, 0, canvas.width, canvas.height);
  return canvas;
}

// -------------------------------------------------------------- placement

/** The stamp the Stamps tool places, and the emoji when it is "emoji". */
export const [currentStamp, setCurrentStamp] = createSignal<{ id: string; emoji?: string }>({ id: "star" });

/** The next number for a badge: one more than the highest on the page. */
export function nextBadge(list: readonly DomItem[] = items): number {
  let max = 0;
  for (const it of list) if (it.stamp === "badge") max = Math.max(max, parseInt(it.text ?? "", 10) || 0);
  return max + 1;
}

/** What placing `id` stores: kind, fallback text, size and colour. */
export function stampToPlace(id: string, emoji?: string): { kind: string; text: string; w: number; h: number; color: number } {
  const def = stampDef(id);
  const custom = def ? undefined : resolveStamp(id);
  if (custom) {
    const { w, h } = customSize(custom);
    const c = custom.color ? (parseInt(custom.color.slice(1, 7), 16) * 256 + 255) >>> 0 : 0x1c2024ff;
    return { kind: id, text: custom.art === "emoji" ? custom.emoji ?? custom.name : custom.art === "text" ? custom.text ?? custom.name : custom.name, w, h, color: c };
  }
  if (!def) {
    const text = emoji || id;
    return { kind: "emoji", text, w: 96, h: 96, color: 0x1c2024ff };
  }
  const text = id === "badge" ? String(nextBadge()) : def.text;
  return { kind: def.id, text, ...stampSize(def, text), color: def.color };
}

/** Ids of stamps placed in this session, so only they pop (not on load). */
export const freshStamps = new Set<string>();

export function toggleFavorite(id: string): boolean {
  const fav = prefs().stampFavorites;
  const on = !fav.includes(id);
  setPrefs({ stampFavorites: on ? [...fav, id].slice(-8) : fav.filter((f) => f !== id) });
  return on;
}

/** The long-press quick menu, open at a board point (CSS px). */
export const [quickMenu, setQuickMenu] = createSignal<{ x: number; y: number } | null>(null);

// ------------------------------------------------------------ celebration

/** A short burst of stars and confetti across the board (Celebrate.tsx). */
export type CelebrateKind = "stars" | "confetti" | "hearts";
export const [celebration, setCelebration] = createSignal(0);
/** What the next burst throws (Celebrate.tsx reads it). */
export let celebrateKind: CelebrateKind = "stars";
export function celebrate(kind: CelebrateKind = "stars") {
  celebrateKind = kind;
  setCelebration((n) => n + 1);
}
