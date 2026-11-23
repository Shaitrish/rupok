// Page snapshots for the live agent bridge: draw a page (or a region of it)
// to a PNG the agent can look at, so it can read handwriting.
//
// Drawn from the board's vector export data (`Board.page_export_json`: ink as
// filled outline polygons, exactly the board's capsule shapes), falling back
// to the thumbnail data (`page_preview_json`). Smart Objects run in sandboxed
// frames we cannot read, so they appear as labelled cards.

import type { Controller } from "./controller";

type Rect4 = [number, number, number, number];
interface Area { x: number; y: number; w: number; h: number }

const DEFAULT_WIDTH = 1280;
const MAX_SIDE = 1920;
const FONT = '"Inter Variable", Inter, "Hind Siliguri", system-ui, sans-serif';

function rgba(c: number, alpha = 1): string {
  const r = (c >>> 24) & 255, g = (c >>> 16) & 255, b = (c >>> 8) & 255;
  const a = ((c & 255) / 255) * alpha;
  return `rgba(${r}, ${g}, ${b}, ${a.toFixed(3)})`;
}

function cssColor(c: number | string, alpha = 1): string {
  return typeof c === "number" ? rgba(c, alpha) : c;
}

function isDark(c: number): boolean {
  const ch = (v: number) => {
    const x = v / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch((c >>> 24) & 255) + 0.7152 * ch((c >>> 16) & 255) + 0.0722 * ch((c >>> 8) & 255) < 0.25;
}

function wrap(ctx: CanvasRenderingContext2D, text: string, width: number): string[] {
  const out: string[] = [];
  for (const para of text.split("\n")) {
    const words = para.split(/(\s+)/);
    let line = "";
    for (const w of words) {
      const next = line + w;
      if (line && ctx.measureText(next).width > width) {
        out.push(line.trimEnd());
        line = w.trimStart();
      } else line = next;
    }
    out.push(line);
  }
  return out;
}

async function image(c: Controller, asset: string): Promise<ImageBitmap | null> {
  const bytes = c.board.asset(asset);
  if (!bytes) return null;
  const ext = asset.split(".").pop() || "png";
  const type = ext === "svg" ? "image/svg+xml" : ext === "jpg" ? "image/jpeg" : `image/${ext}`;
  try {
    return await createImageBitmap(new Blob([bytes as BlobPart], { type }));
  } catch {
    return null;
  }
}

function pattern(ctx: CanvasRenderingContext2D, kind: string, a: Area, dark: boolean, s: number) {
  if (kind === "plain") return;
  ctx.save();
  ctx.fillStyle = ctx.strokeStyle = dark ? "rgba(255, 255, 255, 0.10)" : "rgba(30, 60, 120, 0.10)";
  ctx.lineWidth = 1.5;
  const x0 = Math.floor(a.x / 48) * 48, y0 = Math.floor(a.y / 48) * 48;
  if (kind === "grid") {
    ctx.beginPath();
    for (let x = x0; x <= a.x + a.w; x += 48) { ctx.moveTo(x, a.y); ctx.lineTo(x, a.y + a.h); }
    for (let y = y0; y <= a.y + a.h; y += 48) { ctx.moveTo(a.x, y); ctx.lineTo(a.x + a.w, y); }
    ctx.stroke();
  } else if (kind === "lines") {
    ctx.beginPath();
    for (let y = y0 + 47; y <= a.y + a.h; y += 48) { ctx.moveTo(a.x, y); ctx.lineTo(a.x + a.w, y); }
    ctx.stroke();
  } else if (kind === "dots") {
    const d0x = Math.floor(a.x / 40) * 40 + 20, d0y = Math.floor(a.y / 40) * 40 + 20;
    for (let x = d0x; x <= a.x + a.w; x += 40)
      for (let y = d0y; y <= a.y + a.h; y += 40) { ctx.beginPath(); ctx.arc(x, y, Math.max(2, 1 / s), 0, Math.PI * 2); ctx.fill(); }
  }
  ctx.restore();
}

/** The page's items with ids and geometry (from the live outline). */
function outline(c: Controller, page: number, agent: string): any {
  return JSON.parse(c.board.live_apply(JSON.stringify({ op: "get_page", page }), agent));
}

/** Render a snapshot for a `snapshot` op: `{png, width, height, page, region}`. */
export async function renderSnapshot(c: Controller, op: any, agent: string) {
  const b = c.board;
  const pageCount = (JSON.parse(b.pages_json()) as unknown[]).length;
  const page = typeof op.page === "number" ? op.page : b.page_index() + 1;
  if (!(page >= 1 && page <= pageCount)) throw new Error(`page ${op.page} does not exist; the board has ${pageCount} page(s)`);
  const index = page - 1;

  // Vector data: the export (filled ink outlines) if available, else the thumbnail list.
  let data: any = null;
  let vector = true;
  try {
    data = JSON.parse((b as any).page_export_json(index, true));
  } catch {
    data = null;
  }
  if (!data) {
    vector = false;
    data = JSON.parse(b.page_preview_json(index, 100000));
  }
  if (!data) throw new Error(`page ${page} could not be read`);

  // The area to show: the slide, the content of an infinite page, the given
  // region, or the given items with a margin.
  let area: Area = data.frame ? { x: 0, y: 0, w: data.frame.w, h: data.frame.h } : { x: 0, y: 0, w: 1920, h: 1080 };
  if (!data.frame && data.bounds) {
    const [x0, y0, x1, y1] = data.bounds as Rect4;
    const pad = Math.max(x1 - x0, y1 - y0) * 0.06 + 16;
    area = { x: x0 - pad, y: y0 - pad, w: x1 - x0 + 2 * pad, h: y1 - y0 + 2 * pad };
  }
  if (Array.isArray(op.item_ids) && op.item_ids.length) {
    const items: any[] = outline(c, page, agent).items ?? [];
    const pick = items.filter((i) => op.item_ids.includes(i.id));
    if (!pick.length) throw new Error("none of item_ids are on that page; call get_current_page for ids");
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const i of pick) {
      const g = i.geometry;
      x0 = Math.min(x0, g.x); y0 = Math.min(y0, g.y); x1 = Math.max(x1, g.x + g.w); y1 = Math.max(y1, g.y + g.h);
    }
    const pad = Math.max(24, Math.max(x1 - x0, y1 - y0) * 0.12);
    area = { x: x0 - pad, y: y0 - pad, w: x1 - x0 + 2 * pad, h: y1 - y0 + 2 * pad };
  } else if (op.region && [op.region.x, op.region.y, op.region.w, op.region.h].every(Number.isFinite) && op.region.w > 0 && op.region.h > 0) {
    area = { x: op.region.x, y: op.region.y, w: op.region.w, h: op.region.h };
  }
  // Keep tiny regions legible: at least 240 × 135 slide pixels.
  if (area.w < 240) { area.x -= (240 - area.w) / 2; area.w = 240; }
  if (area.h < 135) { area.y -= (135 - area.h) / 2; area.h = 135; }

  const want = Math.max(64, Math.min(MAX_SIDE, Math.round(op.width ?? DEFAULT_WIDTH)));
  let W = want, H = Math.round((want * area.h) / area.w);
  if (H > MAX_SIDE) { H = MAX_SIDE; W = Math.max(64, Math.round((MAX_SIDE * area.w) / area.h)); }
  const s = W / area.w;

  await (document as any).fonts?.ready;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("this browser cannot draw a snapshot");
  const bg: number = data.bg ?? 0xffffffff;
  const dark = isDark(bg);
  // Outside the slide (zoomed regions at the edge): the board's canvas grey.
  ctx.fillStyle = dark ? "#0d0e10" : "#eceef2";
  ctx.fillRect(0, 0, W, H);
  ctx.setTransform(s, 0, 0, s, -area.x * s, -area.y * s);
  const slide: Area = data.frame ? { x: 0, y: 0, w: data.frame.w, h: data.frame.h } : area;
  ctx.fillStyle = rgba(bg);
  ctx.fillRect(slide.x, slide.y, slide.w, slide.h);
  pattern(ctx, data.pattern ?? "plain", { x: Math.max(area.x, slide.x), y: Math.max(area.y, slide.y), w: Math.min(area.w, slide.w), h: Math.min(area.h, slide.h) }, dark, s);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";

  for (const it of data.items ?? []) {
    ctx.globalAlpha = 1;
    if (it.kind === "stroke") {
      if (vector && Array.isArray(it.outline)) {
        ctx.fillStyle = cssColor(it.color, Math.max(0, Math.min(1, it.opacity ?? 1)));
        ctx.beginPath();
        for (const poly of it.outline as number[][]) {
          if (poly.length < 6) continue;
          ctx.moveTo(poly[0], poly[1]);
          for (let i = 2; i < poly.length; i += 2) ctx.lineTo(poly[i], poly[i + 1]);
          ctx.closePath();
        }
        ctx.fill("nonzero");
      } else if (Array.isArray(it.pts) && it.pts.length >= 2) {
        ctx.strokeStyle = cssColor(it.color);
        ctx.globalAlpha = Math.max(0, Math.min(1, it.opacity ?? 1));
        ctx.lineWidth = Math.max(it.width ?? 3, 1 / s);
        ctx.beginPath();
        ctx.moveTo(it.pts[0], it.pts[1]);
        for (let i = 2; i < it.pts.length; i += 2) ctx.lineTo(it.pts[i], it.pts[i + 1]);
        ctx.stroke();
      }
    } else if (it.kind === "text") {
      const [x, y, w] = it.rect as Rect4;
      ctx.fillStyle = cssColor(it.color);
      ctx.font = `500 ${it.size}px ${FONT}`;
      ctx.textBaseline = "top";
      const lh = it.size * 1.25;
      wrap(ctx, it.text, Math.max(w, it.size)).forEach((line, i) => ctx.fillText(line, x, y + i * lh));
    } else if (it.kind === "image") {
      const [x, y, w, h] = it.rect as Rect4;
      const img = await image(c, it.asset);
      if (img) ctx.drawImage(img, x, y, w, h);
      else { ctx.fillStyle = "rgba(128, 128, 128, 0.2)"; ctx.fillRect(x, y, w, h); }
    } else if (it.kind === "object") {
      const [x, y, w, h] = it.rect as Rect4;
      const r = Math.min(w, h) * 0.04;
      ctx.beginPath();
      ctx.roundRect(x, y, w, h, r);
      ctx.fillStyle = dark ? "rgba(255, 255, 255, 0.06)" : "rgba(17, 20, 24, 0.04)";
      ctx.fill();
      ctx.lineWidth = Math.max(2, 1.5 / s);
      ctx.strokeStyle = dark ? "rgba(255, 255, 255, 0.28)" : "rgba(17, 20, 24, 0.24)";
      ctx.stroke();
      const fs = Math.max(18, Math.min(56, h * 0.08));
      ctx.fillStyle = dark ? "#eef0f2" : "#111418";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.font = `650 ${fs}px ${FONT}`;
      ctx.fillText(it.title || "Smart Object", x + w / 2, y + h / 2 - fs * 0.4, w * 0.9);
      ctx.font = `500 ${fs * 0.55}px ${FONT}`;
      ctx.fillStyle = dark ? "#a6adb7" : "#5f6670";
      const steps = it.steps ? ` · step ${it.step ?? 0} of ${it.steps}` : "";
      ctx.fillText(`Smart Object (live HTML)${steps}`, x + w / 2, y + h / 2 + fs * 0.6, w * 0.9);
      ctx.textAlign = "start";
    }
  }

  const blob: Blob | null = await new Promise((res) => canvas.toBlob(res, "image/png"));
  if (!blob) throw new Error("could not encode the snapshot");
  const { b64 } = await import("./live");
  return {
    png: b64(new Uint8Array(await blob.arrayBuffer())),
    width: W,
    height: H,
    page,
    region: { x: Math.round(area.x), y: Math.round(area.y), w: Math.round(area.w), h: Math.round(area.h) },
    note: "Smart Objects are drawn as labelled cards; ink, text and images are as on the board.",
  };
}
