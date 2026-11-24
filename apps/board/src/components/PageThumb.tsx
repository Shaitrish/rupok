// Real page thumbnails: each page is drawn from `Board.page_preview_json`
// (decimated stroke polylines and box rects) onto a small canvas.
//
// Thumbnails render lazily: a page is drawn the first time it scrolls into
// view, the current page again (debounced) whenever the document changes,
// and any other page when its item count changes. Work goes through one
// queue that draws a single page per idle slice, so a long lesson never
// blocks the pen.

import { createEffect, on, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { pages, revision } from "../state";
import {
  type Branding, type PageBackground, DIM_ALPHA, bgPlacements, brandInk, brandLine, brandTextBox, cssColor, frameOf, hasBranding, logoRect,
} from "../design";

const MAX_POINTS = 48;
const CURRENT_DEBOUNCE_MS = 450;

type Rect = [number, number, number, number];
type PreviewItem =
  | { kind: "stroke"; color: string; width: number; opacity: number; pts: number[]; fill?: string; dash?: boolean; brush?: string }
  | { kind: "text"; rect: Rect; text: string; size: number; color: string; stamp?: string }
  | { kind: "image"; rect: Rect; asset: string }
  | { kind: "object"; rect: Rect; title: string };
interface Preview {
  frame: { w: number; h: number } | null;
  bg: number;
  pattern: string;
  background?: PageBackground;
  branding?: Branding;
  bounds: Rect | null;
  items: PreviewItem[];
}

// ------------------------------------------------------------------ queue

const queue = new Map<HTMLCanvasElement, () => void>();
let pumping = false;
const idle: (cb: () => void) => void =
  (window as any).requestIdleCallback ? (cb) => (window as any).requestIdleCallback(cb, { timeout: 200 }) : (cb) => setTimeout(cb, 16);

function enqueue(canvas: HTMLCanvasElement, job: () => void) {
  queue.set(canvas, job);
  if (pumping) return;
  pumping = true;
  const pump = () => {
    const next = queue.entries().next();
    if (next.done) {
      pumping = false;
      return;
    }
    const [key, run] = next.value;
    queue.delete(key);
    try {
      run();
    } catch (e) {
      console.warn("thumbnail failed", e);
    }
    idle(pump);
  };
  idle(pump);
}

// ----------------------------------------------------------------- images

const images = new Map<string, HTMLImageElement>();
const waiting = new Map<string, Set<() => void>>();

/** The decoded image for an asset, or null while it loads (`onLoad` then fires). */
function imageFor(asset: string, onLoad: () => void): HTMLImageElement | null {
  let img = images.get(asset);
  if (!img) {
    const url = controller.assetUrl(asset);
    if (!url) return null;
    img = new Image();
    img.decoding = "async";
    img.src = url;
    images.set(asset, img);
    img.onload = img.onerror = () => {
      const cbs = waiting.get(asset);
      waiting.delete(asset);
      cbs?.forEach((cb) => cb());
    };
  }
  if (img.complete && img.naturalWidth > 0) return img;
  if (!img.complete) {
    let set = waiting.get(asset);
    if (!set) waiting.set(asset, (set = new Set()));
    set.add(onLoad);
  }
  return null;
}

// ---------------------------------------------------------------- drawing

function draw(canvas: HTMLCanvasElement, p: Preview, redraw: () => void, index: number) {
  const cssW = canvas.clientWidth, cssH = canvas.clientHeight;
  if (!cssW || !cssH) return;
  const dpr = Math.min(3, window.devicePixelRatio || 1);
  const W = Math.round(cssW * dpr), H = Math.round(cssH * dpr);
  if (canvas.width !== W || canvas.height !== H) {
    canvas.width = W;
    canvas.height = H;
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, W, H);
  // World area shown: the slide frame, or the content of an infinite board.
  let area: Rect | null = p.frame ? [0, 0, p.frame.w, p.frame.h] : null;
  if (!area && p.bounds) {
    const [x0, y0, x1, y1] = p.bounds;
    const pad = Math.max(x1 - x0, y1 - y0) * 0.06 + 8;
    area = [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
  }
  if (!area) return;
  const aw = Math.max(1, area[2] - area[0]), ah = Math.max(1, area[3] - area[1]);
  const s = Math.min(cssW / aw, cssH / ah);
  ctx.setTransform(dpr * s, 0, 0, dpr * s, dpr * ((cssW - aw * s) / 2 - area[0] * s), dpr * ((cssH - ah * s) / 2 - area[1] * s));
  ctx.beginPath();
  ctx.rect(area[0], area[1], aw, ah);
  ctx.clip();
  // UI tints come from design tokens via CSS (see .thumb-canvas), never raw values.
  const css = getComputedStyle(canvas);
  const objectTint = css.color;
  const placeholder = css.outlineColor;
  const minLine = 1 / s; // one CSS pixel
  drawBackground(ctx, p, redraw);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const it of p.items) {
    ctx.globalAlpha = 1;
    if (it.kind === "stroke") {
      const pts = it.pts;
      if (pts.length < 2) continue;
      ctx.globalAlpha = Math.max(0, Math.min(1, it.opacity));
      ctx.strokeStyle = it.color;
      ctx.fillStyle = it.color;
      const lw = Math.max(it.width * 0.85, minLine);
      if (pts.length === 2) {
        ctx.beginPath();
        ctx.arc(pts[0], pts[1], lw / 2, 0, Math.PI * 2);
        ctx.fill();
        continue;
      }
      ctx.lineWidth = lw;
      ctx.beginPath();
      ctx.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1]);
      if (it.fill) {
        // A shape's soft fill, under its outline (catalog.rs fill).
        ctx.save();
        ctx.globalAlpha = 1;
        ctx.fillStyle = it.fill;
        ctx.fill("evenodd");
        ctx.restore();
      }
      brushLook(ctx, it, lw);
      // The dash pattern of catalog.rs `dash_pattern`.
      if (it.dash) ctx.setLineDash([2.5 * it.width + 8, 1.8 * it.width + 7]);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.shadowBlur = 0;
    } else if (it.kind === "image") {
      const [x, y, w, h] = it.rect;
      const img = imageFor(it.asset, redraw);
      if (img) {
        ctx.drawImage(img, x, y, w, h);
      } else {
        ctx.fillStyle = placeholder;
        ctx.globalAlpha = 0.25;
        ctx.fillRect(x, y, w, h);
      }
    } else if (it.kind === "object") {
      const [x, y, w, h] = it.rect;
      const r = Math.min(w, h) * 0.04;
      ctx.beginPath();
      ctx.roundRect(x, y, w, h, r);
      ctx.fillStyle = objectTint;
      ctx.globalAlpha = 0.12;
      ctx.fill();
      ctx.globalAlpha = 0.5;
      ctx.lineWidth = minLine;
      ctx.strokeStyle = objectTint;
      ctx.stroke();
      // A small "spark" in the middle marks a live object.
      const d = Math.min(w, h) * 0.18;
      ctx.globalAlpha = 0.55;
      ctx.beginPath();
      ctx.moveTo(x + w / 2, y + h / 2 - d);
      ctx.quadraticCurveTo(x + w / 2, y + h / 2, x + w / 2 + d, y + h / 2);
      ctx.quadraticCurveTo(x + w / 2, y + h / 2, x + w / 2, y + h / 2 + d);
      ctx.quadraticCurveTo(x + w / 2, y + h / 2, x + w / 2 - d, y + h / 2);
      ctx.quadraticCurveTo(x + w / 2, y + h / 2, x + w / 2, y + h / 2 - d);
      ctx.fillStyle = objectTint;
      ctx.fill();
    } else if (it.kind === "text" && it.stamp) {
      // A stamp: its emoji or label, centred in its box.
      const [x, y, w, h] = it.rect;
      ctx.fillStyle = it.color;
      ctx.font = `700 ${Math.min(h * 0.7, (w / Math.max(1, [...it.text].length)) * 1.6)}px ${css.fontFamily}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(it.text, x + w / 2, y + h / 2);
      ctx.textAlign = "start";
    } else if (it.kind === "text") {
      const [x, y, w] = it.rect;
      ctx.fillStyle = it.color;
      ctx.font = `500 ${it.size}px ${css.fontFamily}`;
      ctx.textBaseline = "top";
      const lh = it.size * 1.25;
      it.text.split("\n").forEach((line, i) => ctx.fillText(line, x, y + i * lh, Math.max(w, it.size)));
    }
  }
  ctx.globalAlpha = 1;
  drawBranding(ctx, p, redraw, css.fontFamily, index);
}

/** Under the items: the background image and the dim wash (design.ts). */
function drawBackground(ctx: CanvasRenderingContext2D, p: Preview, redraw: () => void) {
  const { w: fw, h: fh } = frameOf(p);
  const bg = p.background;
  if (bg) {
    const img = imageFor(bg.image, redraw);
    if (img) {
      ctx.save();
      ctx.beginPath();
      ctx.rect(0, 0, fw, fh);
      ctx.clip();
      ctx.globalAlpha = Math.max(0, Math.min(1, bg.opacity));
      for (const [x, y, w, h] of bgPlacements(bg.fit, fw, fh, img.naturalWidth, img.naturalHeight)) ctx.drawImage(img, x, y, w, h);
      ctx.restore();
    }
    if (bg.dim) {
      ctx.globalAlpha = DIM_ALPHA;
      ctx.fillStyle = cssColor(p.bg);
      ctx.fillRect(0, 0, fw, fh);
      ctx.globalAlpha = 1;
    }
  }
}

/** Above the items (as on the board): the branding logo and text line. */
function drawBranding(ctx: CanvasRenderingContext2D, p: Preview, redraw: () => void, font: string, index: number) {
  const { w: fw, h: fh } = frameOf(p);
  const b = p.branding;
  if (!hasBranding(b)) return;
  if (b.logo) {
    const img = imageFor(b.logo, redraw);
    if (img) {
      ctx.globalAlpha = Math.max(0, Math.min(1, b.logo_opacity));
      ctx.drawImage(img, ...logoRect(b, fw, fh, img.naturalWidth / img.naturalHeight));
      ctx.globalAlpha = 1;
    }
  }
  const line = brandLine(b, { page: index + 1, pages: pages().length });
  if (line) {
    const { rect, size } = brandTextBox(b, fw, fh);
    ctx.font = `600 ${size}px ${font}`;
    ctx.fillStyle = brandInk(p.bg);
    ctx.textBaseline = "middle";
    ctx.textAlign = b.text_align;
    const x = b.text_align === "left" ? rect[0] : b.text_align === "right" ? rect[0] + rect[2] : rect[0] + rect[2] / 2;
    ctx.fillText(line, x, rect[1] + rect[3] / 2, rect[2]);
    ctx.textAlign = "left";
  }
}

/** A thumbnail's sketch of a pen style (brushes.ts): dashes, dots, the
 * rainbow, a glow; textured styles a little lighter. */
function brushLook(ctx: CanvasRenderingContext2D, it: { pts: number[]; brush?: string; color: string }, lw: number) {
  switch (it.brush) {
    case "dashed": ctx.setLineDash([lw * 2, lw * 2]); break;
    case "dotted": ctx.setLineDash([0.01, lw * 2]); break;
    case "neon": ctx.shadowColor = it.color; ctx.shadowBlur = lw * 2; break;
    case "pencil": case "chalk": ctx.globalAlpha *= 0.8; break;
    case "rainbow": {
      const n = it.pts.length;
      const g = ctx.createLinearGradient(it.pts[0], it.pts[1], it.pts[n - 2], it.pts[n - 1]);
      for (let k = 0; k <= 6; k++) g.addColorStop(k / 6, `hsl(${k * 60}, 72%, 56%)`);
      ctx.strokeStyle = g;
      break;
    }
  }
}

/** Draw page `index` into `canvas` now. */
function render(canvas: HTMLCanvasElement, index: number) {
  if (!canvas.isConnected) return;
  const json = controller.board.page_preview_json(index, MAX_POINTS);
  const p = JSON.parse(json) as Preview | null;
  if (!p) return;
  draw(canvas, p, () => enqueue(canvas, () => render(canvas, index)), index);
}

/** A thumbnail canvas for one page. */
export function ThumbCanvas(props: { index: number; current: boolean; itemCount: number; design?: string }) {
  let canvas!: HTMLCanvasElement;
  let visible = false;
  let stale = true;
  let timer: number | undefined;

  const request = (delay = 0) => {
    stale = true;
    clearTimeout(timer);
    if (!visible) return;
    const go = () => {
      stale = false;
      enqueue(canvas, () => render(canvas, props.index));
    };
    if (delay) timer = window.setTimeout(go, delay);
    else go();
  };

  onMount(() => {
    const io = new IntersectionObserver(
      ([e]) => {
        visible = e.isIntersecting;
        if (visible && stale) request();
      },
      { root: canvas.closest(".pages-list"), rootMargin: "120px 0px" },
    );
    io.observe(canvas);
    onCleanup(() => {
      io.disconnect();
      clearTimeout(timer);
      queue.delete(canvas);
    });
  });

  // The current page follows every edit (debounced); other pages redraw when
  // their content changes size or they move to another index.
  createEffect(on(revision, () => props.current && request(CURRENT_DEBOUNCE_MS), { defer: true }));
  createEffect(on(() => [props.itemCount, props.index, props.design] as const, () => request(), { defer: true }));
  createEffect(on(() => props.current, (cur, was) => was && !cur && request(), { defer: true }));

  return <canvas class="thumb-canvas" ref={canvas} aria-hidden="true" />;
}
