// Page design: background images, branding (a logo and a line of text on
// every page), the built-in looks and the teacher's default for new lessons.
//
// The document stores it (spec/rupok-format.md, "Page design"): a page's
// `background` and `branding` keys and the lesson's `meta.branding`. Every
// change is one engine call, so one undo step.
//
// The same geometry is drawn four ways and must agree: the board
// (components/PageBackdrop.tsx, DOM layers under the ink, rasterised once by
// the compositor, never per pen frame), thumbnails (PageThumb.tsx, canvas),
// the PDF export (export-pdf.ts) and the Page design preview.

import { createSignal } from "solid-js";
import type { PageInfo } from "./state";

export type Pattern = "plain" | "grid" | "dots" | "lines";
export type BgFit = "fit" | "fill" | "stretch" | "tile" | "center";
export type Rect = [number, number, number, number];

export interface PageBackground {
  /** Asset name `<blake3>.<ext>`. */
  image: string;
  fit: BgFit;
  /** 0..1 */
  opacity: number;
  /** A wash of the page colour at 50 % over the image. */
  dim: boolean;
}

export interface Branding {
  logo?: string | null;
  /** Logo box: top-left and width as fractions of the page frame. */
  logo_x: number;
  logo_y: number;
  logo_w: number;
  logo_opacity: number;
  school: string;
  teacher: string;
  class: string;
  /** Show the day's date (the day the page is shown or exported). */
  date: boolean;
  text_pos: "top" | "bottom";
  text_align: "left" | "center" | "right";
  /** Page branding only: hide branding on this page. */
  off?: boolean;
}

export interface PageStyle {
  pattern: Pattern;
  bg_color: number;
  background: PageBackground | null;
}

export const DEFAULT_BRANDING: Branding = {
  logo: null, logo_x: 0.025, logo_y: 0.035, logo_w: 0.1, logo_opacity: 1,
  school: "", teacher: "", class: "", date: false, text_pos: "bottom", text_align: "center",
};
export const MIN_LOGO_W = 0.02;
export const MAX_LOGO_W = 0.6;
export const MAX_BRAND_TEXT = 120;
/** Opacity of the page-colour wash of "Dim for readability". */
export const DIM_ALPHA = 0.5;
/** World size of a page without a frame (an infinite board shows its design here). */
export const DEFAULT_FRAME = { w: 1920, h: 1080 };

const sameJson = <T,>(a: T, b: T) => JSON.stringify(a) === JSON.stringify(b);

/** The lesson's branding (`meta.branding`), refreshed by `controller.sync`. */
export const [lessonBranding, setLessonBranding] = createSignal<Branding | null>(null, { equals: sameJson });

/**
 * A change being dragged (a slider, the logo): shown on the current page at
 * once and written to the document on release, so a drag is one undo step.
 */
export const [designPreview, setDesignPreview] = createSignal<{ background?: PageBackground | null; branding?: Branding | null } | null>(null);

/** The branding a page shows: its own, else the lesson's; null when hidden. */
export function brandingFor(page: PageInfo | undefined, lesson: Branding | null): Branding | null {
  const own = page?.branding;
  if (own) return own.off ? null : own;
  return lesson;
}

export const styleOf = (p: PageInfo | undefined): PageStyle => ({
  pattern: p?.pattern ?? "plain",
  bg_color: p?.bg_color ?? 0xffffffff,
  background: p?.background ?? null,
});

export const hasBranding = (b: Branding | null | undefined): b is Branding =>
  !!b && !b.off && (!!b.logo || !!b.school || !!b.teacher || !!b.class || b.date);

export const frameOf = (p: { frame: { w: number; h: number } | null } | undefined) =>
  p?.frame && p.frame.w > 0 && p.frame.h > 0 ? p.frame : DEFAULT_FRAME;

// ------------------------------------------------------------- geometry

/** Most tiles a tiled background draws; a tinier image is drawn larger. */
export const MAX_TILES = 1024;

/** Where a background image goes on a `fw` × `fh` page: one rect, or the
 * tiles of `tile` (at most about `MAX_TILES`: tiny images are drawn larger).
 * Callers clip to the page. */
export function bgPlacements(fit: BgFit, fw: number, fh: number, iw: number, ih: number): Rect[] {
  if (!(iw > 0 && ih > 0)) return [[0, 0, fw, fh]];
  switch (fit) {
    case "stretch":
      return [[0, 0, fw, fh]];
    case "fill":
    case "fit": {
      const s = fit === "fill" ? Math.max(fw / iw, fh / ih) : Math.min(fw / iw, fh / ih);
      const w = iw * s, h = ih * s;
      return [[(fw - w) / 2, (fh - h) / 2, w, h]];
    }
    case "center":
      return [[(fw - iw) / 2, (fh - ih) / 2, iw, ih]];
    case "tile": {
      const k = Math.max(1, Math.sqrt((fw / iw) * (fh / ih) / MAX_TILES));
      const tw = iw * k, th = ih * k;
      const out: Rect[] = [];
      for (let y = 0; y < fh; y += th) for (let x = 0; x < fw; x += tw) out.push([x, y, tw, th]);
      return out;
    }
  }
}

/** The CSS for a background image layer, matching `bgPlacements`. */
export function bgCss(fit: BgFit, iw = 0, ih = 0, fw = DEFAULT_FRAME.w, fh = DEFAULT_FRAME.h): Record<string, string> {
  switch (fit) {
    case "stretch": return { "background-size": "100% 100%", "background-repeat": "no-repeat" };
    case "fill": return { "background-size": "cover", "background-position": "center", "background-repeat": "no-repeat" };
    case "center": return { "background-size": iw && ih ? `${iw}px ${ih}px` : "auto", "background-position": "center", "background-repeat": "no-repeat" };
    case "tile": {
      const [, , tw, th] = bgPlacements("tile", fw, fh, iw || 1, ih || 1)[0];
      return { "background-size": iw && ih ? `${tw}px ${th}px` : "auto", "background-position": "0 0", "background-repeat": "repeat" };
    }
    default: return { "background-size": "contain", "background-position": "center", "background-repeat": "no-repeat" };
  }
}

/** The logo's rect on a `fw` × `fh` page; `aspect` = image width / height. */
export function logoRect(b: Branding, fw: number, fh: number, aspect: number): Rect {
  const w = b.logo_w * fw;
  const h = w / (aspect > 0 && Number.isFinite(aspect) ? aspect : 1);
  return [b.logo_x * fw, b.logo_y * fh, w, h];
}

export type Corner = "top-left" | "top-right" | "bottom-left" | "bottom-right";
export const CORNERS: { id: Corner; label: string }[] = [
  { id: "top-left", label: "Top left" },
  { id: "top-right", label: "Top right" },
  { id: "bottom-left", label: "Bottom left" },
  { id: "bottom-right", label: "Bottom right" },
];
const MARGIN_X = 0.025, MARGIN_Y = 0.035;

/** Logo position for a corner preset. */
export function cornerPos(c: Corner, logoW: number, fw: number, fh: number, aspect: number): { logo_x: number; logo_y: number } {
  const hFrac = (logoW * fw) / (aspect > 0 ? aspect : 1) / fh;
  return {
    logo_x: c.endsWith("left") ? MARGIN_X : Math.max(0, 1 - MARGIN_X - logoW),
    logo_y: c.startsWith("top") ? MARGIN_Y : Math.max(0, 1 - MARGIN_Y - hFrac),
  };
}

/** The corner the logo is in (by its centre), or null when it is elsewhere. */
export function cornerOf(b: Branding, fw: number, fh: number, aspect: number): Corner | null {
  for (const c of CORNERS) {
    const p = cornerPos(c.id, b.logo_w, fw, fh, aspect);
    if (Math.abs(p.logo_x - b.logo_x) < 0.004 && Math.abs(p.logo_y - b.logo_y) < 0.004) return c.id;
  }
  return null;
}

/** Resize the logo, keeping the edges nearest the page corner where they are. */
export function resizeLogo(b: Branding, w: number, fw: number, fh: number, aspect: number): Branding {
  w = clamp(w, MIN_LOGO_W, MAX_LOGO_W);
  const a = aspect > 0 ? aspect : 1;
  const hOld = (b.logo_w * fw) / a / fh, hNew = (w * fw) / a / fh;
  const right = b.logo_x + b.logo_w / 2 > 0.5, bottom = b.logo_y + hOld / 2 > 0.5;
  return {
    ...b,
    logo_w: w,
    logo_x: clamp(right ? b.logo_x + b.logo_w - w : b.logo_x, 0, 1),
    logo_y: clamp(bottom ? b.logo_y + hOld - hNew : b.logo_y, 0, 1),
  };
}

export const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** The branding text line: school · teacher · class · date. */
export function brandLine(b: Branding, when = new Date()): string {
  const parts = [b.school, b.teacher, b.class].map((s) => s.trim()).filter(Boolean);
  if (b.date) parts.push(new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "long", year: "numeric" }).format(when));
  return parts.join("  ·  ");
}

/** The text line's box on a `fw` × `fh` page and its font size (world units).
 * 28 units on a 1080-high slide: readable from the back of a classroom. */
export function brandTextBox(b: Branding, fw: number, fh: number): { rect: Rect; size: number } {
  const size = Math.round(fh * 0.026);
  const h = size * 1.6;
  const mx = fw * MARGIN_X;
  const y = b.text_pos === "top" ? fh * 0.018 : fh - fh * 0.018 - h;
  return { rect: [mx, y, fw - 2 * mx, h], size };
}

/** Text colour for branding on a page colour (0xRRGGBBAA): colour and alpha. */
export function brandInkRgba(bg: number): [number, number] {
  return isDarkColor(bg) ? [0xffffffff, 0.78] : [0x1c2024ff, 0.72];
}

export function brandInk(bg: number): string {
  const [c, a] = brandInkRgba(bg);
  return `rgba(${(c >>> 24) & 255}, ${(c >>> 16) & 255}, ${(c >>> 8) & 255}, ${a})`;
}

export function isDarkColor(rgba: number): boolean {
  const r = (rgba >>> 24) & 255, g = (rgba >>> 16) & 255, b = (rgba >>> 8) & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b < 128;
}

export const cssColor = (rgba: number) => "#" + (rgba >>> 8).toString(16).padStart(6, "0");

// --------------------------------------------------------------- images

const sizes = new Map<string, Promise<{ w: number; h: number }>>();

/** The natural size of an image URL (cached by URL). */
export function imageSize(url: string): Promise<{ w: number; h: number }> {
  let p = sizes.get(url);
  if (!p) {
    p = new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
      img.onerror = () => resolve({ w: 0, h: 0 });
      img.src = url;
    });
    sizes.set(url, p);
  }
  return p;
}

const LIMITS = { logo: 1024, background: 2560 };
const MAX_UPLOAD = 20 * 1024 * 1024;
const MAX_SVG = 2 * 1024 * 1024;

export interface PreparedImage { bytes: Uint8Array; ext: string; w: number; h: number }

/**
 * Read an uploaded logo or background image. Big photos are scaled down
 * (logos to 1024 px, backgrounds to 2560 px) so a lesson stays small enough
 * to share; SVG is kept as it is, with a width and height added when it only
 * has a viewBox (so every renderer agrees on its size).
 */
export async function prepareImage(file: File | Blob, kind: "logo" | "background"): Promise<PreparedImage> {
  if (file.size > MAX_UPLOAD) throw new Error("That image is larger than 20 MB");
  const type = file.type || "";
  if (type === "image/svg+xml" || ("name" in file && /\.svg$/i.test(file.name))) {
    if (file.size > MAX_SVG) throw new Error("That SVG is larger than 2 MB");
    const svg = normalizeSvg(await file.text());
    if (!svg) throw new Error("That SVG could not be read");
    return { bytes: new TextEncoder().encode(svg.text), ext: "svg", w: svg.w, h: svg.h };
  }
  if (!/^image\/(png|jpeg|webp|gif|bmp)$/.test(type)) throw new Error("Use a PNG, JPEG, WebP or SVG image");
  const bmp = await createImageBitmap(file);
  const [w0, h0] = [bmp.width, bmp.height];
  const max = LIMITS[kind];
  const k = Math.min(1, max / Math.max(w0, h0));
  const keep = k === 1 && file.size <= (kind === "logo" ? 1_000_000 : 3_000_000) && /png|jpeg|webp/.test(type);
  if (keep) {
    bmp.close();
    const ext = type === "image/jpeg" ? "jpg" : type.split("/")[1];
    return { bytes: new Uint8Array(await file.arrayBuffer()), ext, w: w0, h: h0 };
  }
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(w0 * k));
  canvas.height = Math.max(1, Math.round(h0 * k));
  const g = canvas.getContext("2d")!;
  g.drawImage(bmp, 0, 0, canvas.width, canvas.height);
  bmp.close();
  // Logos keep their transparency (PNG); opaque backgrounds become JPEG.
  let opaque = kind === "background";
  if (opaque) {
    const d = g.getImageData(0, 0, canvas.width, canvas.height).data;
    for (let i = 3; i < d.length; i += 4 * 97) if (d[i] < 255) { opaque = false; break; }
  }
  const mime = opaque ? "image/jpeg" : "image/png";
  const blob: Blob | null = await new Promise((r) => canvas.toBlob(r, mime, 0.86));
  if (!blob) throw new Error("That image could not be converted");
  return { bytes: new Uint8Array(await blob.arrayBuffer()), ext: opaque ? "jpg" : "png", w: canvas.width, h: canvas.height };
}

/** Give an SVG an explicit width and height (from its viewBox when missing). */
export function normalizeSvg(text: string): { text: string; w: number; h: number } | null {
  const doc = new DOMParser().parseFromString(text, "image/svg+xml");
  const svg = doc.documentElement;
  if (!svg || svg.nodeName.toLowerCase() !== "svg" || doc.querySelector("parsererror")) return null;
  const num = (v: string | null) => {
    const n = v && /^\s*[\d.]+\s*(px)?\s*$/.test(v) ? parseFloat(v) : NaN;
    return Number.isFinite(n) && n > 0 ? n : NaN;
  };
  let w = num(svg.getAttribute("width")), h = num(svg.getAttribute("height"));
  const vb = (svg.getAttribute("viewBox") ?? "").trim().split(/[\s,]+/).map(Number);
  const [vw, vh] = vb.length === 4 && vb[2] > 0 && vb[3] > 0 ? [vb[2], vb[3]] : [NaN, NaN];
  if (!Number.isFinite(w) || !Number.isFinite(h)) {
    if (Number.isFinite(vw)) {
      // Keep the viewBox's aspect at a useful size (512 on the long side).
      const s = 512 / Math.max(vw, vh);
      w = Number.isFinite(w) ? w : Number.isFinite(h) ? (h * vw) / vh : vw * s;
      h = Number.isFinite(h) ? h : (w * vh) / vw;
    } else {
      w = Number.isFinite(w) ? w : 512;
      h = Number.isFinite(h) ? h : 512;
    }
    svg.setAttribute("width", String(Math.round(w)));
    svg.setAttribute("height", String(Math.round(h)));
    if (!Number.isFinite(vw)) svg.setAttribute("viewBox", `0 0 ${Math.round(w)} ${Math.round(h)}`);
  }
  return { text: new XMLSerializer().serializeToString(doc), w: Math.round(w), h: Math.round(h) };
}

// --------------------------------------------------------------- presets

/** Exam paper: a header with Name, Class, Roll and Date lines, a margin and ruled lines. */
export function examPaperSvg(): string {
  const lines: string[] = [];
  for (let y = 232; y <= 1032; y += 64) lines.push(`<line x1="96" y1="${y}" x2="1824" y2="${y}"/>`);
  const field = (label: string, x: number, end: number) =>
    `<text x="${x}" y="100">${label}</text><line x1="${x + label.length * 15 + 16}" y1="104" x2="${end}" y2="104" stroke="#5b6470" stroke-width="2"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 1920 1080">` +
    `<g font-family="Figtree, 'Noto Sans Bengali', Arial, sans-serif" font-size="28" font-weight="600" fill="#5b6470">` +
    field("Name", 96, 900) + field("Class", 960, 1270) + field("Roll", 1320, 1560) + field("Date", 1600, 1824) +
    `</g><rect x="96" y="148" width="1728" height="3" fill="#3e63dd" fill-opacity="0.5"/>` +
    `<line x1="176" y1="164" x2="176" y2="1048" stroke="#e5484d" stroke-opacity="0.45" stroke-width="2"/>` +
    `<g stroke="#3e63dd" stroke-opacity="0.18" stroke-width="2">${lines.join("")}</g></svg>`;
}

export interface Preset {
  id: string;
  label: string;
  hint: string;
  bg: number;
  pattern: Pattern;
  /** A generated background image (SVG). */
  image?: () => string;
}

export const PRESETS: Preset[] = [
  { id: "whiteboard", label: "Whiteboard", hint: "White, nothing else: best on a projector in a bright room", bg: 0xffffffff, pattern: "plain" },
  { id: "chalkboard", label: "Chalkboard", hint: "Classic green board; ink turns white", bg: 0x1f3b30ff, pattern: "plain" },
  { id: "dark", label: "Dark board", hint: "Near black; easy on the eyes in a dim room", bg: 0x17191cff, pattern: "plain" },
  { id: "notebook", label: "Lined notebook", hint: "Cream paper with ruled lines", bg: 0xfbf7edff, pattern: "lines" },
  { id: "graph", label: "Graph paper", hint: "Squares for maths and graphs", bg: 0xffffffff, pattern: "grid" },
  { id: "exam", label: "Exam paper", hint: "Name, class, roll and date lines, a margin and ruled lines", bg: 0xffffffff, pattern: "plain", image: examPaperSvg },
];

/** The look of a preset, adding its generated image to the lesson. */
export function presetStyle(p: Preset, board: { add_asset(bytes: Uint8Array, ext: string): string | undefined }): PageStyle {
  let background: PageBackground | null = null;
  if (p.image) {
    const image = board.add_asset(new TextEncoder().encode(p.image()), "svg");
    if (image) background = { image, fit: "stretch", opacity: 1, dim: false };
  }
  return { pattern: p.pattern, bg_color: p.bg, background };
}

// ------------------------------------------------- default for new lessons

/** The teacher's own look for new lessons, kept per device (IndexedDB). */
export interface DesignDefault {
  version: 1;
  style: PageStyle;
  branding: Branding | null;
  /** The images it uses, by asset name. */
  assets: Record<string, Uint8Array>;
}

export const [designDefault, setDesignDefault] = createSignal<DesignDefault | null>(null);

/** The parts of the board that applying a design needs. */
export interface DesignTarget {
  add_asset(bytes: Uint8Array, ext: string): string | undefined;
  set_page_style_json(json: string, all: boolean): void;
  set_branding_json(json: string, pageOnly: boolean): void;
  forget_history(): void;
}

/**
 * Give a just-created lesson the teacher's default design: the branding on
 * every page and, unless the lesson was made from a template with its own
 * look (`style` false), the page look. Not an undo step.
 */
export function applyDesignDefault(board: DesignTarget, opts: { style: boolean }): boolean {
  const d = designDefault();
  if (!d) return false;
  try {
    const names = new Map<string, string>();
    for (const [name, bytes] of Object.entries(d.assets)) {
      const got = board.add_asset(bytes, name.split(".").pop() || "bin");
      if (got) names.set(name, got);
    }
    const rename = (n: string | null | undefined) => (n ? names.get(n) ?? null : null);
    if (opts.style) {
      const bg = d.style.background;
      const image = bg ? rename(bg.image) : null;
      board.set_page_style_json(JSON.stringify({ ...d.style, background: bg && image ? { ...bg, image } : null }), true);
    }
    if (d.branding) board.set_branding_json(JSON.stringify({ ...d.branding, logo: rename(d.branding.logo) }), false);
    board.forget_history();
    return true;
  } catch (e) {
    console.warn("default page design not applied", e);
    return false;
  }
}
