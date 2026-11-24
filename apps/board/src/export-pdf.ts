// Export as PDF. Loaded (with pdf-lib) the first time someone exports, so
// the board's first load does not grow.
//
// Every board page becomes one PDF form XObject drawn in world units, in the
// board's layer order: page colour and pattern, then images, text and Smart
// Objects in stacking order (the DOM layer), then ink and shapes in stacking
// order (the ink canvas sits above the DOM layer). "Slides" puts one form on
// a page at the frame's aspect ratio; "Handout" scales 2 or 3 forms onto A4
// portrait pages next to ruled note lines.
//
// How each element is exported:
// - Ink and shapes: filled vector outlines from `Board.page_export_json`
//   (the renderer's capsule chains, filled with the non-zero rule), each
//   stroke painted once at its opacity, so the highlighter stays translucent.
// - Images: JPEG bytes are embedded as they are (no re-encoding), PNG goes
//   through pdf-lib, other formats are drawn to a canvas first. Imported PDF
//   slides are stored as JPEG images (pdf.ts keeps no copy of the PDF), so
//   they embed losslessly at the resolution they were imported at.
// - Text: laid out by the browser with the board's own CSS and drawn to a
//   canvas at 3×, so Bangla (and any script) is shaped correctly; embedded
//   as a soft mask painted in the text colour.
// - Smart Objects: a `snapshot` (spec/smart-object.md) from a hidden copy of
//   the object with its saved step and state, else a placeholder card.
//
// Exporting only reads the document: it never changes the lesson or its
// undo history.

import { renderTex } from "./katex";
import { PDFDocument, PDFHexString, type PDFRef, StandardFonts, rgb } from "pdf-lib";
import type { Controller } from "./controller";
import { theme as objectTheme } from "./objects";
import { BOARD_COLORS } from "./state";
import { APP_NAME, FILE_EXT, SITE_URL } from "./brand";
import {
  type Branding, type PageBackground, DIM_ALPHA, bgPlacements, brandInkRgba, brandLine, brandTextBox, frameOf, hasBranding, imageSize, logoRect,
} from "./design";

export type Layout = "slides" | "handout3" | "handout2";

export interface ExportOptions {
  /** Board page indexes (0-based), in output order. */
  pages: number[];
  layout: Layout;
  /** Dark pages become white paper and light ink turns dark. */
  printFriendly: boolean;
  /** Pen and highlighter strokes (shapes are always kept). */
  includeInk: boolean;
  title: string;
}

export interface ExportProgress {
  done: number;
  total: number;
  label: string;
}

export class ExportCancelled extends Error {
  constructor() {
    super("Export cancelled");
  }
}

// ------------------------------------------------------------------- data

type Rect = [number, number, number, number];
interface StrokeItem { kind: "stroke"; color: number; opacity: number; tool: string; shape: string | null; outline: number[][] }
interface TextItem { kind: "text"; rect: Rect; text: string; size: number; color: number; latex?: boolean }
interface ImageItem { kind: "image"; rect: Rect; asset: string }
interface ObjectItem { kind: "object"; id: string; rect: Rect; title: string; bundle: string; step: number; steps: number; state: string | null }
type Item = StrokeItem | TextItem | ImageItem | ObjectItem;
interface PageData {
  title: string;
  frame: { w: number; h: number } | null;
  bg: number;
  pattern: "plain" | "grid" | "dots" | "lines";
  /** Page design (design.ts): drawn like the board. */
  background?: PageBackground;
  branding?: Branding;
  bounds: Rect | null;
  items: Item[];
}

/** World units per PDF point on slide pages: a 1920 × 1080 slide is 960 × 540 pt. */
const SLIDE_PT_PER_UNIT = 0.5;
/** PDF viewers refuse pages larger than this (points). */
const MAX_PAGE_PT = 14400;
const A4: [number, number] = [595.28, 841.89];
/** Text is rasterised at this many pixels per world unit (≈ 430 dpi on a slide page). */
const TEXT_SCALE = 3;
const OBJECT_RADIUS = 8;
const SNAPSHOT_SETTLE_MS = 1000;
const SNAPSHOT_REPLY_MS = 1500;
const SNAPSHOT_IDLE_MS = 3000;
const SNAPSHOT_LOAD_MS = 8000;
/** Snapshot size: 1.5× (≈ 220 dpi for an object filling half a slide page),
 * capped at 2.2 MP so the object can encode its PNG within the reply time. */
const SNAPSHOT_SCALE = 1.5;
const SNAPSHOT_MAX_PIXELS = 2_200_000;
const SNAPSHOT_MAX_CHARS = 16 * 1024 * 1024;
/** One object at a time: object frames share the board's main thread, so a
 * heavy object loading would starve another one's snapshot. */
const SNAPSHOT_CONCURRENCY = 1;

const yieldToUi = () => new Promise<void>((r) => setTimeout(r, 0));

/** Resolve in the next idle period of the main thread (at most `max` ms). */
const whenIdle = (max: number) =>
  new Promise<void>((r) => {
    const ric = (window as any).requestIdleCallback as ((cb: () => void, o: { timeout: number }) => number) | undefined;
    if (ric) ric(() => r(), { timeout: max });
    else setTimeout(r, 50);
  });

// ----------------------------------------------------------------- colour

const red = (c: number) => (c >>> 24) & 255;
const green = (c: number) => (c >>> 16) & 255;
const blue = (c: number) => (c >>> 8) & 255;
const alphaOf = (c: number) => (c & 255) / 255;
const hex6 = (c: number) => (c >>> 8).toString(16).padStart(6, "0");
const css = (c: number) => `#${hex6(c)}`;

function luminance(c: number): number {
  const lin = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(red(c)) + 0.7152 * lin(green(c)) + 0.0722 * lin(blue(c));
}

/** Same threshold as Smart Object theming (objects.ts). */
const isDark = (c: number) => luminance(c) < 0.25;

/** Light ink on paper: keep the hue, make it dark enough to read on white. */
function darkInk(c: number): number {
  if (luminance(c) <= 0.45) return c;
  const [r, g, b] = [red(c) / 255, green(c) / 255, blue(c) / 255];
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  let h = 0;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  if (d) {
    if (max === r) h = ((g - b) / d + 6) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
  }
  const l2 = Math.max(0.12, Math.min(1 - l, 0.32));
  const c2 = (1 - Math.abs(2 * l2 - 1)) * s;
  const x = c2 * (1 - Math.abs((h % 2) - 1));
  const m = l2 - c2 / 2;
  const [r1, g1, b1] = h < 1 ? [c2, x, 0] : h < 2 ? [x, c2, 0] : h < 3 ? [0, c2, x] : h < 4 ? [0, x, c2] : h < 5 ? [x, 0, c2] : [c2, 0, x];
  const to = (v: number) => Math.round((v + m) * 255) & 255;
  return ((to(r1) << 24) | (to(g1) << 16) | (to(b1) << 8) | (c & 255)) >>> 0;
}

/** `r g b` operands in 0..1. */
const rgbOps = (c: number) => `${(red(c) / 255).toFixed(3)} ${(green(c) / 255).toFixed(3)} ${(blue(c) / 255).toFixed(3)}`;

/** A number for a content stream (no exponents, at most `d` decimals). */
function num(v: number, d = 2): string {
  if (!Number.isFinite(v)) return "0";
  const s = v.toFixed(d);
  return s.includes(".") ? s.replace(/\.?0+$/, "") || "0" : s;
}

function parseRgba(s: string): [number, number, number, number] {
  const m = s.match(/rgba?\(([^)]+)\)/);
  const p = m ? m[1].split(",").map((x) => Number(x.trim())) : [30, 60, 120, 0.1];
  return [p[0] ?? 0, p[1] ?? 0, p[2] ?? 0, p[3] ?? 1];
}

// ------------------------------------------------------------ PNG predictor

/** PNG Paeth prediction per row (PDF `Predictor 15`), for flate image streams. */
function paeth(src: Uint8Array, w: number, h: number, colors: number): Uint8Array {
  const row = w * colors;
  const out = new Uint8Array((row + 1) * h);
  for (let y = 0; y < h; y++) {
    const o = y * (row + 1);
    const s = y * row;
    out[o] = 4;
    for (let i = 0; i < row; i++) {
      const a = i >= colors ? src[s + i - colors] : 0;
      const b = y > 0 ? src[s - row + i] : 0;
      const c = y > 0 && i >= colors ? src[s - row + i - colors] : 0;
      const p = a + b - c;
      const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
      const pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      out[o + 1 + i] = (src[s + i] - pred) & 255;
    }
  }
  return out;
}

// ------------------------------------------------------------ compression

const encoder = new TextEncoder();

/** zlib-deflate with the browser's native CompressionStream (fast, off the
 * main thread), or null where it is missing (pdf-lib's pako then). */
async function nativeDeflate(data: Uint8Array): Promise<Uint8Array | null> {
  if (typeof CompressionStream === "undefined") return null;
  try {
    const stream = new Blob([data as BlobPart]).stream().pipeThrough(new CompressionStream("deflate"));
    return new Uint8Array(await new Response(stream).arrayBuffer());
  } catch {
    return null;
  }
}

// --------------------------------------------------------------- exporter

interface Resources {
  gs: Map<string, PDFRef>;
  xo: Map<string, PDFRef>;
}

interface PageForm {
  ref: PDFRef;
  /** Size in world units. */
  w: number;
  h: number;
}

class Exporter {
  private assets = new Map<string, Promise<{ ref: PDFRef } | null>>();
  private alphaStates = new Map<number, PDFRef>();
  private snapshots = new Map<string, Promise<HTMLImageElement | null>>();
  private snapQueue: (() => void)[] = [];
  private snapRunning = 0;
  private seq = 0;

  constructor(
    readonly pdf: PDFDocument,
    readonly c: Controller,
    readonly o: ExportOptions,
    readonly signal?: AbortSignal,
  ) {}

  check() {
    if (this.signal?.aborted) throw new ExportCancelled();
  }

  page(index: number, strokes: boolean): PageData | null {
    try {
      return JSON.parse(this.c.board.page_export_json(index, strokes)) as PageData | null;
    } catch {
      return null;
    }
  }

  /** True when this page prints as white paper. */
  paper(p: PageData) {
    return this.o.printFriendly && isDark(p.bg);
  }

  private alpha(a: number, res: Resources): string {
    const key = Math.round(a * 100);
    let ref = this.alphaStates.get(key);
    if (!ref) {
      ref = this.pdf.context.register(this.pdf.context.obj({ Type: "ExtGState", ca: key / 100, CA: key / 100 }));
      this.alphaStates.set(key, ref);
    }
    const name = `G${key}`;
    res.gs.set(name, ref);
    return `/${name} gs`;
  }

  /** A FlateDecode stream object (content streams are ASCII). */
  private async flate(data: string | Uint8Array, dict: Record<string, unknown> = {}): Promise<PDFRef> {
    const ctx = this.pdf.context;
    const bytes = typeof data === "string" ? encoder.encode(data) : data;
    const z = await nativeDeflate(bytes);
    return ctx.register(z ? ctx.stream(z, { ...dict, Filter: "FlateDecode" } as any) : ctx.flateStream(bytes, dict as any));
  }

  private xobject(ref: PDFRef, res: Resources): string {
    const name = `X${ref.objectNumber}`;
    res.xo.set(name, ref);
    return name;
  }

  // ------------------------------------------------------------ images

  /** Image XObject from canvas pixels: RGB, plus a soft mask when not opaque. */
  private async embedPixels(canvas: HTMLCanvasElement, opaque: boolean): Promise<PDFRef> {
    const { width: w, height: h } = canvas;
    const data = canvas.getContext("2d")!.getImageData(0, 0, w, h).data;
    const rgbBytes = new Uint8Array(w * h * 3);
    const alpha = opaque ? null : new Uint8Array(w * h);
    for (let i = 0, j = 0, k = 0; i < data.length; i += 4, j += 3, k++) {
      rgbBytes[j] = data[i];
      rgbBytes[j + 1] = data[i + 1];
      rgbBytes[j + 2] = data[i + 2];
      if (alpha) alpha[k] = data[i + 3];
    }
    const parms = (colors: number) => ({ Predictor: 15, Colors: colors, BitsPerComponent: 8, Columns: w });
    const dict: Record<string, unknown> = {
      Type: "XObject", Subtype: "Image", Width: w, Height: h, ColorSpace: "DeviceRGB", BitsPerComponent: 8, DecodeParms: parms(3),
    };
    if (alpha) {
      dict.SMask = await this.flate(paeth(alpha, w, h, 1), {
        Type: "XObject", Subtype: "Image", Width: w, Height: h, ColorSpace: "DeviceGray", BitsPerComponent: 8, DecodeParms: parms(1),
      });
    }
    return this.flate(paeth(rgbBytes, w, h, 3), dict);
  }

  /** A one-colour image: the canvas alpha as soft mask over a 1-bit image of `color`. */
  private async embedMask(canvas: HTMLCanvasElement, color: number): Promise<PDFRef> {
    const { width: w, height: h } = canvas;
    const data = canvas.getContext("2d")!.getImageData(0, 0, w, h).data;
    const alpha = new Uint8Array(w * h);
    for (let i = 3, k = 0; i < data.length; i += 4, k++) alpha[k] = data[i];
    const smask = await this.flate(paeth(alpha, w, h, 1), {
      Type: "XObject", Subtype: "Image", Width: w, Height: h, ColorSpace: "DeviceGray", BitsPerComponent: 8,
      DecodeParms: { Predictor: 15, Colors: 1, BitsPerComponent: 8, Columns: w },
    });
    const base = new Uint8Array(Math.ceil(w / 8) * h); // every pixel is palette entry 0
    return this.flate(base, {
      Type: "XObject", Subtype: "Image", Width: w, Height: h, BitsPerComponent: 1,
      ColorSpace: ["Indexed", "DeviceRGB", 0, PDFHexString.of(hex6(color))], SMask: smask,
    });
  }

  private asset(name: string): Promise<{ ref: PDFRef } | null> {
    let p = this.assets.get(name);
    if (!p) {
      p = this.loadAsset(name).catch((e) => {
        console.warn("export: image skipped", name, e);
        return null;
      });
      this.assets.set(name, p);
    }
    return p;
  }

  private async loadAsset(name: string): Promise<{ ref: PDFRef } | null> {
    const bytes = this.c.board.asset(name);
    if (!bytes) return null;
    const ext = name.split(".").pop()?.toLowerCase() ?? "";
    if (ext === "jpg" || ext === "jpeg") {
      try {
        const img = await this.pdf.embedJpg(bytes);
        await img.embed();
        return { ref: img.ref };
      } catch { /* odd JPEG: draw it instead */ }
    } else if (ext === "png") {
      try {
        const img = await this.pdf.embedPng(bytes);
        await img.embed();
        return { ref: img.ref };
      } catch { /* interlaced or odd PNG: draw it instead */ }
    }
    // GIF, WebP, SVG, …: draw to a canvas (SVG at a generous size).
    const url = this.c.assetUrl(name);
    if (!url) return null;
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    await img.decode();
    let w = img.naturalWidth || 1024, h = img.naturalHeight || 768;
    if (ext === "svg") {
      const k = 2400 / Math.max(w, h);
      w *= k;
      h *= k;
    }
    const k = Math.min(1, 4096 / Math.max(w, h));
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(w * k));
    canvas.height = Math.max(1, Math.round(h * k));
    const g = canvas.getContext("2d")!;
    g.drawImage(img, 0, 0, canvas.width, canvas.height);
    const corners = g.getImageData(0, 0, canvas.width, canvas.height).data;
    let opaque = true;
    for (let i = 3; i < corners.length; i += 4 * 61) if (corners[i] < 255) { opaque = false; break; }
    if (opaque) {
      const blob: Blob | null = await new Promise((r) => canvas.toBlob(r, "image/jpeg", 0.92));
      if (blob) {
        const jpg = await this.pdf.embedJpg(new Uint8Array(await blob.arrayBuffer()));
        await jpg.embed();
        return { ref: jpg.ref };
      }
    }
    return { ref: await this.embedPixels(canvas, false) };
  }

  /** `q … cm /X Do Q` placing an image over the world rect (y down). */
  private placeImage(ops: string[], name: string, x: number, y: number, w: number, h: number) {
    ops.push(`q ${num(w)} 0 0 ${num(-h)} ${num(x)} ${num(y + h)} cm /${name} Do Q`);
  }

  // -------------------------------------------------------------- text

  private async drawText(ops: string[], res: Resources, t: TextItem, paper: boolean) {
    const color = paper ? darkInk(t.color) : t.color;
    const r = t.latex ? await rasterLatex(t) : await rasterText(t);
    if (!r) return;
    const ref = await this.embedMask(r.canvas, color);
    ops.push("q");
    if (alphaOf(color) < 0.995) ops.push(this.alpha(alphaOf(color), res));
    this.placeImage(ops, this.xobject(ref, res), ...r.rect);
    ops.push("Q");
  }

  // ------------------------------------------------------ Smart Objects

  /**
   * Snapshot the objects of every exported page, a few at a time, before any
   * page is drawn: object frames share the main thread with the board, so
   * they answer fastest while the exporter waits instead of working.
   */
  async snapshotObjects(onDone: (done: number, total: number) => void) {
    const jobs: Promise<unknown>[] = [];
    for (const [k, index] of this.o.pages.entries()) {
      if (k % 10 === 9) await yieldToUi();
      const p = this.page(index, false);
      if (!p) continue;
      const bg = this.paper(p) ? 0xffffffff : p.bg;
      for (const it of p.items) if (it.kind === "object") jobs.push(this.snapshot(index, it, bg));
    }
    let done = 0;
    if (jobs.length) onDone(0, jobs.length);
    await Promise.all(jobs.map((j) => j.then(() => onDone(++done, jobs.length))));
    return jobs.length;
  }

  private snapshot(index: number, o: ObjectItem, bg: number): Promise<HTMLImageElement | null> {
    const key = `${index}:${o.id}`;
    let p = this.snapshots.get(key);
    if (!p) {
      p = new Promise((resolve) => {
        const run = () => {
          this.snapRunning++;
          snapshotObject(this.c, o, css(bg), `export-${++this.seq}`, this.signal).then((img) => {
            this.snapRunning--;
            this.snapQueue.shift()?.();
            resolve(img);
          });
        };
        if (this.snapRunning < SNAPSHOT_CONCURRENCY) run();
        else this.snapQueue.push(run);
      });
      this.snapshots.set(key, p);
    }
    return p;
  }

  private async drawObject(ops: string[], res: Resources, index: number, o: ObjectItem, bg: number, paper: boolean) {
    const [x, y, w, h] = o.rect;
    if (!(w > 0 && h > 0)) return;
    const img = await this.snapshot(index, o, bg);
    this.check();
    let canvas: HTMLCanvasElement;
    if (img && img.naturalWidth > 0) {
      canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      const g = canvas.getContext("2d")!;
      g.fillStyle = css(bg); // objects sit on the page colour on the board
      g.fillRect(0, 0, canvas.width, canvas.height);
      g.drawImage(img, 0, 0);
    } else {
      canvas = placeholderCard(o, bg, paper);
    }
    const ref = await this.embedPixels(canvas, true);
    const r = Math.min(OBJECT_RADIUS, w / 2, h / 2);
    ops.push(`q ${roundRect(x, y, w, h, r)} W n`);
    this.placeImage(ops, this.xobject(ref, res), x, y, w, h);
    ops.push("Q");
  }

  // ------------------------------------------------------------- strokes

  private drawStrokes(ops: string[], res: Resources, items: Item[], paper: boolean) {
    let lastColor = -1;
    let lastAlpha = -1;
    for (const it of items) {
      if (it.kind !== "stroke" || !it.outline.length) continue;
      if (!this.o.includeInk && !it.shape) continue;
      const translucent = it.opacity < 0.6 || it.tool === "highlighter";
      const color = paper && !translucent ? darkInk(it.color) : it.color;
      const a = Math.round(Math.max(0, Math.min(1, alphaOf(color) * it.opacity)) * 100) / 100;
      if (a <= 0) continue;
      if (a !== lastAlpha) {
        ops.push(this.alpha(a, res));
        lastAlpha = a;
      }
      if (color >>> 8 !== lastColor) {
        ops.push(`${rgbOps(color)} rg`);
        lastColor = color >>> 8;
      }
      // One fill per stroke: its polygons wind the same way, so the non-zero
      // rule paints their union once (a highlighter is not darker where the
      // stroke crosses itself, as on the board).
      const parts: string[] = [];
      for (const poly of it.outline) {
        if (poly.length < 6) continue;
        parts.push(`${poly[0]} ${poly[1]} m`);
        for (let i = 2; i < poly.length; i += 2) parts.push(`${poly[i]} ${poly[i + 1]} l`);
        parts.push("h");
      }
      if (parts.length) ops.push(parts.join("\n"), "f");
    }
  }

  // ------------------------------------------------------------- a page

  async pageForm(index: number): Promise<PageForm | null> {
    const p = this.page(index, true);
    if (!p) return null;
    const paper = this.paper(p);
    const bg = paper ? 0xffffffff : p.bg;
    // The slide frame, or the content of an infinite board.
    let area: Rect = [0, 0, 1920, 1080];
    if (p.frame && p.frame.w > 0 && p.frame.h > 0) area = [0, 0, p.frame.w, p.frame.h];
    else if (p.bounds && p.bounds.every(Number.isFinite)) {
      const [x0, y0, x1, y1] = p.bounds;
      const pad = Math.max(x1 - x0, y1 - y0) * 0.04 + 24;
      area = [x0 - pad, y0 - pad, Math.max(1, x1 - x0 + 2 * pad), Math.max(1, y1 - y0 + 2 * pad)];
    }
    const [ax, ay, aw, ah] = area;
    const res: Resources = { gs: new Map(), xo: new Map() };
    const ops: string[] = [];
    // World coordinates (y down) inside the form (y up).
    ops.push(`1 0 0 -1 ${num(-ax)} ${num(ah + ay)} cm`);
    ops.push(`${rgbOps(bg)} rg ${num(ax)} ${num(ay)} ${num(aw)} ${num(ah)} re f`);
    // A dark page printed as paper leaves out its background image (a chalkboard texture).
    if (p.background && !paper) await this.drawBackground(ops, res, p.background, frameOf(p), bg);
    this.drawPattern(ops, res, p.pattern, area, paper ? BOARD_COLORS[0].pattern : BOARD_COLORS.find((b) => b.rgba === p.bg)?.pattern);
    for (const it of p.items) {
      if (it.kind === "image") {
        const img = await this.asset(it.asset);
        if (img) this.placeImage(ops, this.xobject(img.ref, res), ...it.rect);
      } else if (it.kind === "text") {
        await this.drawText(ops, res, it, paper);
      } else if (it.kind === "object") {
        await this.drawObject(ops, res, index, it, bg, paper);
      }
    }
    // Branding sits above the items and under the ink, as on the board.
    if (hasBranding(p.branding)) await this.drawBranding(ops, res, p.branding, frameOf(p), bg, index);
    this.drawStrokes(ops, res, p.items, paper);
    const ctx = this.pdf.context;
    const resources = ctx.obj({
      ExtGState: ctx.obj(Object.fromEntries(res.gs)),
      XObject: ctx.obj(Object.fromEntries(res.xo)),
    });
    const content = ops.join("\n");
    await yieldToUi();
    const ref = await this.flate(content, { Type: "XObject", Subtype: "Form", BBox: [0, 0, aw, ah], Resources: resources });
    return { ref, w: aw, h: ah };
  }

  /** The background image (each asset embedded once per PDF), clipped to the page, and the dim wash. */
  private async drawBackground(ops: string[], res: Resources, b: PageBackground, f: { w: number; h: number }, bg: number) {
    const img = await this.asset(b.image);
    const url = this.c.assetUrl(b.image);
    if (!img || !url) return;
    const size = await imageSize(url);
    const name = this.xobject(img.ref, res);
    ops.push(`q 0 0 ${num(f.w)} ${num(f.h)} re W n`);
    if (b.opacity < 0.995) ops.push(this.alpha(Math.max(0, b.opacity), res));
    for (const [x, y, w, h] of bgPlacements(b.fit, f.w, f.h, size.w, size.h)) this.placeImage(ops, name, x, y, w, h);
    ops.push("Q");
    if (b.dim) ops.push(`q ${this.alpha(DIM_ALPHA, res)} ${rgbOps(bg)} rg 0 0 ${num(f.w)} ${num(f.h)} re f Q`);
  }

  /** The branding logo and text line (the text drawn as an image, so any script works). */
  private pageCount = 0;

  private async drawBranding(ops: string[], res: Resources, b: Branding, f: { w: number; h: number }, bg: number, index: number) {
    if (b.logo) {
      const img = await this.asset(b.logo);
      const url = this.c.assetUrl(b.logo);
      if (img && url) {
        const size = await imageSize(url);
        ops.push("q");
        if (b.logo_opacity < 0.995) ops.push(this.alpha(Math.max(0, b.logo_opacity), res));
        this.placeImage(ops, this.xobject(img.ref, res), ...logoRect(b, f.w, f.h, size.h ? size.w / size.h : 1));
        ops.push("Q");
      }
    }
    // `{page}` is the page's number in the lesson, as on the board.
    this.pageCount ||= (JSON.parse(this.c.board.pages_json()) as unknown[]).length;
    const line = brandLine(b, { page: index + 1, pages: this.pageCount });
    if (!line) return;
    const { rect, size } = brandTextBox(b, f.w, f.h);
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(rect[2] * TEXT_SCALE));
    canvas.height = Math.max(1, Math.ceil(rect[3] * TEXT_SCALE));
    const g = canvas.getContext("2d")!;
    g.scale(TEXT_SCALE, TEXT_SCALE);
    g.font = `600 ${size}px ${getComputedStyle(document.body).fontFamily}`;
    g.textBaseline = "middle";
    g.textAlign = b.text_align;
    const x = b.text_align === "left" ? 0 : b.text_align === "right" ? rect[2] : rect[2] / 2;
    g.fillText(line, x, rect[3] / 2, rect[2]);
    const [color, a] = brandInkRgba(bg);
    const ref = await this.embedMask(canvas, color);
    ops.push(`q ${this.alpha(a, res)}`);
    this.placeImage(ops, this.xobject(ref, res), ...rect);
    ops.push("Q");
  }

  private drawPattern(ops: string[], res: Resources, pattern: string, area: Rect, color: string | undefined) {
    if (pattern !== "grid" && pattern !== "lines" && pattern !== "dots") return;
    const [r, g, b, a] = parseRgba(color ?? "rgba(30, 60, 120, 0.10)");
    const [ax, ay, aw, ah] = area;
    const c = (((r & 255) << 24) | ((g & 255) << 16) | ((b & 255) << 8) | 255) >>> 0;
    const out: string[] = ["q", this.alpha(a, res)];
    const from = (v: number, step: number) => Math.floor(v / step) * step;
    // Mirrors the CSS backgrounds of .page-frame (styles.css).
    if (pattern === "grid") {
      out.push(`${rgbOps(c)} rg`);
      for (let y = from(ay, 48); y < ay + ah; y += 48) out.push(`${num(ax)} ${num(y)} ${num(aw)} 1.5 re`);
      for (let x = from(ax, 48); x < ax + aw; x += 48) out.push(`${num(x)} ${num(ay)} 1.5 ${num(ah)} re`);
      out.push("f");
    } else if (pattern === "lines") {
      out.push(`${rgbOps(c)} rg`);
      for (let y = from(ay, 48) + 46; y < ay + ah; y += 48) out.push(`${num(ax)} ${num(y)} ${num(aw)} 2 re`);
      out.push("f");
    } else {
      out.push(`${rgbOps(c)} RG 1 J 4.4 w`);
      for (let y = from(ay, 40); y <= ay + ah; y += 40) {
        for (let x = from(ax, 40); x <= ax + aw; x += 40) out.push(`${x} ${y} m ${x} ${y} l`);
      }
      out.push("S");
    }
    out.push("Q");
    ops.push(out.join("\n"));
  }

  // ------------------------------------------------------------ layouts

  async slidePage(form: PageForm) {
    const k = Math.min(SLIDE_PT_PER_UNIT, MAX_PAGE_PT / Math.max(form.w, form.h));
    const [pw, ph] = [form.w * k, form.h * k];
    const page = this.pdf.addPage([pw, ph]);
    const name = page.node.newXObject("Pg", form.ref).asString();
    page.node.addContentStream(await this.flate(`q ${num(k, 5)} 0 0 ${num(k, 5)} 0 0 cm ${name} Do Q`));
  }

  private handoutFont?: Awaited<ReturnType<PDFDocument["embedFont"]>>;
  private titleMask?: { ref: PDFRef; w: number; h: number } | null;

  async handoutPage(forms: { form: PageForm; number: number }[], sheet: number, sheets: number, perPage: 2 | 3) {
    const [W, H] = A4;
    const page = this.pdf.addPage([W, H]);
    const font = (this.handoutFont ??= await this.pdf.embedFont(StandardFonts.Helvetica));
    const mx = 40, top = 44, bottom = 40, header = 26;
    const cw = W - 2 * mx;
    const y0 = H - top - header; // top of the first slot (PDF y up)
    const gap = perPage === 3 ? 18 : 24;
    const slotH = (y0 - bottom - gap * (perPage - 1)) / perPage;
    const maxW = cw * (perPage === 3 ? 0.56 : 0.62);
    const lineGap = 22;
    const ops: string[] = [];
    // Header: the lesson title (drawn as an image so any script works).
    if (this.titleMask === undefined) this.titleMask = await this.embedTitle();
    if (this.titleMask) {
      const th = 11;
      const tw = Math.min(cw * 0.75, (this.titleMask.w / this.titleMask.h) * th);
      const name = page.node.newXObject("Tt", this.titleMask.ref).asString();
      ops.push(`q ${num(tw)} 0 0 ${num(tw * (this.titleMask.h / this.titleMask.w))} ${mx} ${num(H - top - 4)} cm ${name} Do Q`);
    }
    ops.push(`0.79 0.8 0.82 RG 0.6 w ${mx} ${num(H - top - header + 8)} m ${num(W - mx)} ${num(H - top - header + 8)} l S`);
    forms.forEach(({ form }, i) => {
      const slotTop = y0 - i * (slotH + gap);
      const k = Math.min(maxW / form.w, (slotH - 4) / form.h);
      const [sw, sh] = [form.w * k, form.h * k];
      const sx = mx, sy = slotTop - sh;
      const name = page.node.newXObject("Pg", form.ref).asString();
      ops.push(`q ${num(k, 5)} 0 0 ${num(k, 5)} ${num(sx)} ${num(sy)} cm ${name} Do Q`);
      ops.push(`0.82 0.83 0.86 RG 0.6 w ${num(sx)} ${num(sy)} ${num(sw)} ${num(sh)} re S`);
      // Ruled note lines: beside the slide, then across below it.
      const lines: string[] = [];
      const lx = sx + sw + 16;
      for (let y = slotTop - lineGap + 4; y > slotTop - slotH + 2; y -= lineGap) {
        const besideSlide = y > sy - 2;
        if (besideSlide && lx < W - mx - 40) lines.push(`${num(lx)} ${num(y)} m ${num(W - mx)} ${num(y)} l`);
        else if (!besideSlide && y < sy - 12) lines.push(`${mx} ${num(y)} m ${num(W - mx)} ${num(y)} l`);
      }
      if (lines.length) ops.push(`0.78 0.8 0.83 RG 0.5 w ${lines.join(" ")} S`);
    });
    page.node.addContentStream(await this.flate(ops.join("\n")));
    const grey = rgb(0.45, 0.47, 0.5);
    const label = `${sheet} / ${sheets}`;
    page.drawText(label, { x: W - mx - font.widthOfTextAtSize(label, 8.5), y: H - top - 2, size: 8.5, font, color: grey });
    forms.forEach(({ form, number }, i) => {
      const slotTop = y0 - i * (slotH + gap);
      const t = String(number);
      page.drawText(t, { x: mx - 8 - font.widthOfTextAtSize(t, 8), y: slotTop - 8, size: 8, font, color: grey });
      void form;
    });
  }

  private async embedTitle(): Promise<{ ref: PDFRef; w: number; h: number } | null> {
    const title = this.o.title.trim();
    if (!title) return null;
    const px = 44;
    const probe = document.createElement("canvas").getContext("2d")!;
    const family = getComputedStyle(document.body).fontFamily;
    probe.font = `600 ${px}px ${family}`;
    const w = Math.ceil(probe.measureText(title).width) + 8;
    const canvas = document.createElement("canvas");
    canvas.width = Math.min(8000, w);
    canvas.height = Math.ceil(px * 1.5);
    const g = canvas.getContext("2d")!;
    g.font = probe.font;
    g.textBaseline = "alphabetic";
    g.fillText(title, 2, px * 1.1);
    return { ref: await this.embedMask(canvas, 0x1c2024ff), w: canvas.width, h: canvas.height };
  }
}

// ---------------------------------------------------------------- helpers

/** A closed rounded-rectangle path (world units). */
function roundRect(x: number, y: number, w: number, h: number, r: number): string {
  const k = 0.5523 * r;
  const [x1, y1] = [x + w, y + h];
  return [
    `${num(x + r)} ${num(y)} m`,
    `${num(x1 - r)} ${num(y)} l`,
    `${num(x1 - r + k)} ${num(y)} ${num(x1)} ${num(y + r - k)} ${num(x1)} ${num(y + r)} c`,
    `${num(x1)} ${num(y1 - r)} l`,
    `${num(x1)} ${num(y1 - r + k)} ${num(x1 - r + k)} ${num(y1)} ${num(x1 - r)} ${num(y1)} c`,
    `${num(x + r)} ${num(y1)} l`,
    `${num(x + r - k)} ${num(y1)} ${num(x)} ${num(y1 - r + k)} ${num(x)} ${num(y1 - r)} c`,
    `${num(x)} ${num(y + r)} l`,
    `${num(x)} ${num(y + r - k)} ${num(x + r - k)} ${num(y)} ${num(x + r)} ${num(y)} c h`,
  ].join(" ");
}

let measureRoot: HTMLDivElement | null = null;

function segments(text: string): { index: number; segment: string }[] {
  const Seg = (Intl as any).Segmenter;
  if (Seg) return Array.from(new Seg(undefined, { granularity: "word" }).segment(text) as Iterable<{ index: number; segment: string }>);
  const out: { index: number; segment: string }[] = [];
  const re = /\s+|\S+/g;
  for (let m = re.exec(text); m; m = re.exec(text)) out.push({ index: m.index, segment: m[0] });
  return out;
}

/**
 * Draw a text item the way the board shows it: the browser lays it out with
 * the board's CSS (`.item.text`, so wrapping matches exactly) and the canvas
 * draws each line with full text shaping (Bangla conjuncts, vowel signs).
 */
async function rasterText(t: TextItem): Promise<{ canvas: HTMLCanvasElement; rect: Rect } | null> {
  if (!t.text.trim()) return null;
  const [x, y, w] = t.rect;
  const size = t.size > 0 ? t.size : 40;
  if (!measureRoot) {
    // Like `.world`: an absolutely placed box with no size, so text items
    // wrap here exactly as they do on the board.
    measureRoot = document.createElement("div");
    measureRoot.setAttribute("aria-hidden", "true");
    measureRoot.style.cssText = "position:absolute;left:0;top:0;visibility:hidden;pointer-events:none";
    document.body.appendChild(measureRoot);
  }
  const el = document.createElement("div");
  el.className = "item text";
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  el.style.minWidth = `${w}px`;
  el.style.fontSize = `${size}px`;
  el.textContent = t.text;
  measureRoot.appendChild(el);
  try {
    const family = getComputedStyle(el).fontFamily;
    const font = `500 ${size}px ${family}`;
    await document.fonts.load(font, t.text).catch(() => []);
    const box = el.getBoundingClientRect();
    const lh = size * 1.25;
    const lines: string[] = [];
    const node = el.firstChild as Text;
    const range = document.createRange();
    for (const { index, segment } of segments(t.text)) {
      const clean = segment.replace(/[\r\n]/g, "");
      if (!clean) continue;
      range.setStart(node, index);
      range.setEnd(node, index + segment.length);
      const r = range.getClientRects()[0] ?? range.getBoundingClientRect();
      if (!r || (!r.width && !r.height)) continue;
      const li = Math.max(0, Math.floor(((r.top + r.bottom) / 2 - box.top) / lh));
      lines[li] = (lines[li] ?? "") + clean;
    }
    const pad = Math.ceil(size * 0.6);
    const W = box.width + 2 * pad, H = Math.max(box.height, lines.length * lh) + 2 * pad;
    let s = TEXT_SCALE;
    while (W * H * s * s > 48e6 && s > 1) s -= 0.5;
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(W * s));
    canvas.height = Math.max(1, Math.ceil(H * s));
    const g = canvas.getContext("2d")!;
    g.scale(s, s);
    g.translate(pad, pad);
    g.font = font;
    g.fillStyle = "#000";
    g.textBaseline = "alphabetic";
    // The board centres the font's ascent + descent in each 1.25 line box.
    const m = g.measureText("Hg");
    const asc = m.fontBoundingBoxAscent || size * 0.97;
    const desc = m.fontBoundingBoxDescent || size * 0.24;
    const base = (lh - (asc + desc)) / 2 + asc;
    lines.forEach((line, i) => line && g.fillText(line, 0, i * lh + base));
    return { canvas, rect: [x - pad, y - pad, W, H] };
  } finally {
    el.remove();
  }
}

/**
 * Draw a typeset maths item: KaTeX lays it out with the board's CSS, then
 * each glyph run is drawn with its own font at its own place, rules (a
 * fraction bar) as rectangles and KaTeX's SVG parts (a root sign) as
 * images. Returns a black-on-transparent mask like `rasterText`.
 */
async function rasterLatex(t: TextItem): Promise<{ canvas: HTMLCanvasElement; rect: Rect } | null> {
  if (!t.text.trim()) return null;
  const [x, y, w] = t.rect;
  const size = t.size > 0 ? t.size : 40;
  if (!measureRoot) {
    measureRoot = document.createElement("div");
    measureRoot.setAttribute("aria-hidden", "true");
    measureRoot.style.cssText = "position:absolute;left:0;top:0;visibility:hidden;pointer-events:none";
    document.body.appendChild(measureRoot);
  }
  const el = document.createElement("div");
  el.className = "item text math";
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  el.style.minWidth = `${w}px`;
  el.style.fontSize = `${size}px`;
  el.style.color = "#000";
  measureRoot.appendChild(el);
  try {
    await renderTex(el, t.text);
    await document.fonts.ready;
    const box = el.getBoundingClientRect();
    const pad = Math.ceil(size * 0.4);
    const W = box.width + 2 * pad, H = box.height + 2 * pad;
    let s = TEXT_SCALE;
    while (W * H * s * s > 48e6 && s > 1) s -= 0.5;
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.ceil(W * s));
    canvas.height = Math.max(1, Math.ceil(H * s));
    const g = canvas.getContext("2d")!;
    g.scale(s, s);
    g.translate(pad - box.left, pad - box.top);
    g.fillStyle = "#000";
    g.textBaseline = "alphabetic";
    const range = document.createRange();
    const walk = document.createTreeWalker(el, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
    const svgs: Promise<void>[] = [];
    for (let n = walk.nextNode(); n; n = walk.nextNode()) {
      if (n.nodeType === Node.TEXT_NODE) {
        const text = n.textContent ?? "";
        const parent = n.parentElement;
        if (!text.trim() || !parent || parent.closest(".katex-mathml")) continue;
        const cs = getComputedStyle(parent);
        g.font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
        range.selectNodeContents(n);
        const r = range.getBoundingClientRect();
        if (!r.width) continue;
        const m = g.measureText(text);
        const asc = m.fontBoundingBoxAscent || parseFloat(cs.fontSize) * 0.8;
        const desc = m.fontBoundingBoxDescent || parseFloat(cs.fontSize) * 0.2;
        // The glyph run's box is its font's ascent + descent, centred.
        const base = r.top + (r.height - (asc + desc)) / 2 + asc;
        g.fillText(text, r.left, base);
      } else {
        const e = n as Element;
        if (e.closest(".katex-mathml")) continue;
        if (e instanceof SVGSVGElement) {
          const r = e.getBoundingClientRect();
          if (!r.width || !r.height) continue;
          const copy = e.cloneNode(true) as SVGSVGElement;
          copy.setAttribute("width", `${r.width}`);
          copy.setAttribute("height", `${r.height}`);
          copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
          copy.setAttribute("fill", "#000");
          const img = new Image();
          svgs.push(new Promise<void>((done) => {
            img.onload = () => {
              g.drawImage(img, r.left, r.top, r.width, r.height);
              done();
            };
            img.onerror = () => done();
          }));
          img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(copy.outerHTML)}`;
          continue;
        }
        const cs = getComputedStyle(e);
        const bw = parseFloat(cs.borderBottomWidth);
        if (bw > 0 && cs.borderBottomStyle !== "none") {
          const r = e.getBoundingClientRect();
          g.fillRect(r.left, r.bottom - bw, r.width, Math.max(bw, 0.04 * size));
        }
      }
    }
    await Promise.all(svgs);
    return { canvas, rect: [x - pad, y - pad, W, H] };
  } finally {
    el.remove();
  }
}

/** Card drawn for a Smart Object that gave no snapshot. */
function placeholderCard(o: ObjectItem, bg: number, paper: boolean): HTMLCanvasElement {
  const [, , w, h] = o.rect;
  const s = Math.min(2, Math.sqrt(3_000_000 / (w * h)));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(w * s));
  canvas.height = Math.max(1, Math.round(h * s));
  const g = canvas.getContext("2d")!;
  g.scale(s, s);
  const dark = !paper && isDark(bg);
  g.fillStyle = css(bg);
  g.fillRect(0, 0, w, h);
  const inset = Math.min(w, h) * 0.02 + 2;
  g.beginPath();
  g.roundRect(inset, inset, w - 2 * inset, h - 2 * inset, Math.min(24, Math.min(w, h) * 0.06));
  g.fillStyle = dark ? "rgba(255, 255, 255, 0.06)" : "rgba(17, 20, 24, 0.035)";
  g.fill();
  g.lineWidth = Math.max(1.5, Math.min(w, h) * 0.004);
  g.setLineDash([g.lineWidth * 5, g.lineWidth * 4]);
  g.strokeStyle = dark ? "rgba(255, 255, 255, 0.28)" : "rgba(17, 20, 24, 0.22)";
  g.stroke();
  g.setLineDash([]);
  const family = getComputedStyle(document.body).fontFamily;
  const unit = Math.min(w / 16, h / 9);
  const titleSize = Math.max(14, Math.min(56, unit * 0.62));
  const subSize = Math.max(10, titleSize * 0.5);
  // The live-object spark, in the accent.
  const cx = w / 2, cy = h / 2 - titleSize * 1.4, d = titleSize * 0.85;
  g.beginPath();
  g.moveTo(cx, cy - d);
  g.quadraticCurveTo(cx, cy, cx + d, cy);
  g.quadraticCurveTo(cx, cy, cx, cy + d);
  g.quadraticCurveTo(cx, cy, cx - d, cy);
  g.quadraticCurveTo(cx, cy, cx, cy - d);
  g.fillStyle = dark ? "#3ee6b0" : "#12b886";
  g.fill();
  g.textAlign = "center";
  g.textBaseline = "alphabetic";
  g.fillStyle = dark ? "#eef0f2" : "#1c2024";
  g.font = `650 ${titleSize}px ${family}`;
  let title = o.title.trim() || "Smart Object";
  const maxW = w - 2 * inset - titleSize * 2;
  while (title.length > 1 && g.measureText(title).width > maxW) title = title.slice(0, -2) + "…";
  g.fillText(title, cx, cy + d + titleSize * 1.5);
  g.font = `500 ${subSize}px ${family}`;
  g.fillStyle = dark ? "#a6adb7" : "#5f6670";
  const host = SITE_URL.replace(/^https?:\/\//, "");
  g.fillText(`Interactive — open the .${FILE_EXT} file in ${host}`, cx, cy + d + titleSize * 1.5 + subSize * 1.9, maxW);
  return canvas;
}

/**
 * Picture a Smart Object for the PDF: load its bundle in a hidden sandboxed
 * iframe (same CSP and SDK as on the board), give it the saved step and
 * state, let it settle, then ask for a `snapshot`. Messages that would
 * change the lesson (state, goto, resize) are ignored. Resolves to null when
 * the object does not answer in time.
 */
function snapshotObject(
  c: Controller,
  o: ObjectItem,
  bg: string,
  requestId: string,
  signal?: AbortSignal,
): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const [, , w, h] = o.rect;
    const iframe = document.createElement("iframe");
    iframe.setAttribute("sandbox", "allow-scripts");
    iframe.setAttribute("referrerpolicy", "no-referrer");
    iframe.setAttribute("aria-hidden", "true");
    iframe.title = o.title;
    iframe.tabIndex = -1;
    // Behind the opaque board: laid out and painted (so animation frames
    // run), but never seen.
    iframe.style.cssText =
      `position:fixed;left:0;top:0;width:${Math.round(w)}px;height:${Math.round(h)}px;border:0;` +
      `z-index:-1;pointer-events:none;background:${bg};color-scheme:normal`;
    let done = false;
    let timer = 0;
    let loaded = false;
    let ready = false;
    let asked = false;
    const post = (m: Record<string, unknown>) => iframe.contentWindow?.postMessage({ rupok: 1, ...m }, "*");
    // Ask once the frame has loaded (scripts, modules) and is initialised,
    // after a moment for first paint and intro animations, and once the main
    // thread (shared with a srcdoc frame) is idle, so a heavy object has
    // finished starting up and the reply time measures only the snapshot.
    const schedule = () => {
      if (!loaded || !ready || asked || done) return;
      asked = true;
      clearTimeout(timer);
      timer = window.setTimeout(async () => {
        await whenIdle(SNAPSHOT_IDLE_MS);
        if (done) return;
        const scale = Math.max(0.5, Math.min(SNAPSHOT_SCALE, Math.sqrt(SNAPSHOT_MAX_PIXELS / (w * h))));
        post({ type: "snapshot", id: requestId, scale });
        // About 1.5 s for the reply. A first snapshot in a fresh frame pays a
        // one-off warm-up (its first SVG picture and PNG encode), so a slow
        // but working object gets one more 1.5 s (a late reply to either
        // request counts) before the placeholder.
        timer = window.setTimeout(() => {
          post({ type: "snapshot", id: requestId, scale });
          timer = window.setTimeout(() => finish(null), SNAPSHOT_REPLY_MS);
        }, SNAPSHOT_REPLY_MS);
      }, SNAPSHOT_SETTLE_MS);
    };
    const finish = (img: HTMLImageElement | null) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      window.removeEventListener("message", onMessage);
      signal?.removeEventListener("abort", onAbort);
      iframe.remove();
      resolve(img);
    };
    const onAbort = () => finish(null);
    const onMessage = (e: MessageEvent) => {
      if (e.source !== iframe.contentWindow) return;
      const m = e.data;
      if (!m || m.rupok !== 1 || typeof m.type !== "string") return;
      if (m.type === "hello") {
        let state: unknown = null;
        try {
          state = o.state ? JSON.parse(o.state) : null;
        } catch { state = null; }
        post({
          type: "init", id: o.id, step: o.step, steps: Math.max(o.steps, o.step), state, theme: objectTheme(bg), mode: "present",
        });
        ready = true;
        schedule();
      } else if (m.type === "snapshot" && m.id === requestId) {
        const url = m.dataUrl;
        if (typeof url !== "string" || url.length > SNAPSHOT_MAX_CHARS || !/^data:image\/png;base64,/.test(url)) return finish(null);
        clearTimeout(timer);
        const img = new Image();
        img.onload = () => finish(img);
        img.onerror = () => finish(null);
        img.src = url;
      }
    };
    if (signal?.aborted) return resolve(null);
    signal?.addEventListener("abort", onAbort);
    window.addEventListener("message", onMessage);
    iframe.addEventListener("load", () => {
      loaded = true;
      schedule();
    });
    timer = window.setTimeout(() => finish(null), SNAPSHOT_LOAD_MS);
    iframe.srcdoc = c.objects.srcdoc(o.bundle);
    document.body.appendChild(iframe);
  });
}

// --------------------------------------------------------------- entry

/** Build the PDF. Yields to the UI between pages; throws ExportCancelled on abort. */
export async function exportPdf(
  c: Controller,
  o: ExportOptions,
  onProgress: (p: ExportProgress) => void,
  signal?: AbortSignal,
): Promise<Uint8Array> {
  const pdf = await PDFDocument.create();
  pdf.setTitle(o.title || "Lesson", { showInWindowTitleBar: true });
  pdf.setCreator(APP_NAME);
  pdf.setProducer(`${APP_NAME} · pdf-lib`);
  pdf.setCreationDate(new Date());
  pdf.setModificationDate(new Date());
  const ex = new Exporter(pdf, c, o, signal);
  const total = o.pages.length;
  onProgress({ done: 0, total, label: "Preparing…" });
  await document.fonts.ready;
  await ex.snapshotObjects((done, n) =>
    onProgress({ done: 0, total, label: `Picturing Smart Objects: ${Math.min(done + 1, n)} of ${n}` }),
  );
  const perPage = o.layout === "handout2" ? 2 : o.layout === "handout3" ? 3 : 0;
  const sheets = perPage ? Math.ceil(total / perPage) : total;
  let pending: { form: PageForm; number: number }[] = [];
  for (let i = 0; i < total; i++) {
    ex.check();
    onProgress({ done: i, total, label: `Page ${i + 1} of ${total}` });
    await yieldToUi();
    const form = await ex.pageForm(o.pages[i]);
    if (!form) continue;
    if (!perPage) await ex.slidePage(form);
    else {
      pending.push({ form, number: o.pages[i] + 1 });
      if (pending.length === perPage || i === total - 1) {
        await ex.handoutPage(pending, Math.ceil((i + 1) / perPage), sheets, perPage as 2 | 3);
        pending = [];
      }
    }
  }
  if (pending.length) await ex.handoutPage(pending, sheets, sheets, perPage as 2 | 3);
  ex.check();
  onProgress({ done: total, total, label: "Finishing…" });
  await yieldToUi();
  return pdf.save({ useObjectStreams: true, objectsPerTick: 40 });
}
