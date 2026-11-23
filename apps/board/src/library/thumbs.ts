// Cover thumbnails for the Library: page 1 of the lesson, drawn from the
// engine's page preview (`Board.page_preview_json`) onto a 480×270 canvas
// and stored in the package as `thumbs/0.png` (spec: a cache, readers must
// tolerate it missing or stale). Theme-independent: the page is drawn in its
// own colours, so the same file looks right in light and dark.

import { BOARD_COLORS } from "../state";

const W = 480;
const H = 270;
const MAX_POINTS = 64;

type Rect = [number, number, number, number];
type PreviewItem =
  | { kind: "stroke"; color: string; width: number; opacity: number; pts: number[] }
  | { kind: "text"; rect: Rect; text: string; size: number; color: string }
  | { kind: "image"; rect: Rect; asset: string }
  | { kind: "object"; rect: Rect; title: string };
interface Preview {
  frame: { w: number; h: number } | null;
  bg: number;
  pattern: string;
  bounds: Rect | null;
  items: PreviewItem[];
}

/** The parts of the board the renderer needs. */
export interface PreviewSource {
  page_preview_json(index: number, maxPoints: number): string;
  asset(name: string): Uint8Array | undefined;
}

const bitmaps = new Map<string, ImageBitmap>();

async function bitmapFor(src: PreviewSource, asset: string): Promise<ImageBitmap | null> {
  const hit = bitmaps.get(asset);
  if (hit) return hit;
  const bytes = src.asset(asset);
  if (!bytes) return null;
  try {
    const ext = asset.split(".").pop() ?? "";
    const type = ext === "svg" ? "image/svg+xml" : ext === "jpg" ? "image/jpeg" : `image/${ext}`;
    // Decode small: a thumbnail never needs the 2880 px PDF slide.
    const bmp = await createImageBitmap(new Blob([bytes as BlobPart], { type }), { resizeWidth: W * 2, resizeQuality: "medium" } as ImageBitmapOptions);
    if (bitmaps.size > 24) {
      const [k, b] = bitmaps.entries().next().value as [string, ImageBitmap];
      b.close();
      bitmaps.delete(k);
    }
    bitmaps.set(asset, bmp);
    return bmp;
  } catch {
    return null;
  }
}

const hex = (rgba: number) => "#" + (rgba >>> 8).toString(16).padStart(6, "0");

/** The page-1 preview JSON (cheap to compare: redraw only when it changed). */
export function coverKey(src: PreviewSource): string {
  try {
    return src.page_preview_json(0, MAX_POINTS);
  } catch {
    return "";
  }
}

/** Render page 1 as PNG bytes, or null if there is nothing to draw. */
export async function renderCover(src: PreviewSource, json = coverKey(src)): Promise<Uint8Array | null> {
  const p = (json ? JSON.parse(json) : null) as Preview | null;
  if (!p) return null;
  let area: Rect | null = p.frame ? [0, 0, p.frame.w, p.frame.h] : null;
  if (!area && p.bounds) {
    const [x0, y0, x1, y1] = p.bounds;
    const pad = Math.max(x1 - x0, y1 - y0) * 0.06 + 8;
    area = [x0 - pad, y0 - pad, x1 + pad, y1 + pad];
  }
  area ??= [0, 0, 1920, 1080];
  const aw = Math.max(1, area[2] - area[0]), ah = Math.max(1, area[3] - area[1]);
  const off = typeof OffscreenCanvas !== "undefined";
  const canvas: OffscreenCanvas | HTMLCanvasElement = off ? new OffscreenCanvas(W, H) : Object.assign(document.createElement("canvas"), { width: W, height: H });
  const ctx = canvas.getContext("2d") as CanvasRenderingContext2D | null;
  if (!ctx) return null;
  const bg = hex(p.bg ?? 0xffffffff);
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, W, H);
  const s = Math.min(W / aw, H / ah);
  ctx.setTransform(s, 0, 0, s, (W - aw * s) / 2 - area[0] * s, (H - ah * s) / 2 - area[1] * s);
  ctx.beginPath();
  ctx.rect(area[0], area[1], aw, ah);
  ctx.clip();
  // Page pattern, as on the board (styles.css .page-frame.*).
  const pat = BOARD_COLORS.find((b) => b.rgba === p.bg)?.pattern ?? "rgba(30, 60, 120, 0.10)";
  ctx.strokeStyle = pat;
  ctx.fillStyle = pat;
  ctx.lineWidth = 1.5;
  if (p.pattern === "grid" || p.pattern === "lines") {
    ctx.beginPath();
    for (let y = area[1] + 47; y < area[3]; y += 48) {
      ctx.moveTo(area[0], y);
      ctx.lineTo(area[2], y);
    }
    if (p.pattern === "grid") {
      for (let x = area[0] + 0.75; x < area[2]; x += 48) {
        ctx.moveTo(x, area[1]);
        ctx.lineTo(x, area[3]);
      }
    }
    ctx.stroke();
  } else if (p.pattern === "dots") {
    for (let y = area[1] + 20; y < area[3]; y += 40) for (let x = area[0] + 20; x < area[2]; x += 40) ctx.fillRect(x - 2, y - 2, 4, 4);
  }
  const minLine = 1 / s;
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
      const lw = Math.max(it.width * 0.9, minLine);
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
      ctx.stroke();
    } else if (it.kind === "image") {
      const [x, y, w, h] = it.rect;
      const bmp = await bitmapFor(src, it.asset);
      if (bmp) ctx.drawImage(bmp, x, y, w, h);
      else {
        ctx.globalAlpha = 0.12;
        ctx.fillStyle = "#60646c";
        ctx.fillRect(x, y, w, h);
      }
    } else if (it.kind === "object") {
      const [x, y, w, h] = it.rect;
      ctx.beginPath();
      ctx.roundRect(x, y, w, h, Math.min(w, h) * 0.04);
      ctx.fillStyle = "#20c997";
      ctx.globalAlpha = 0.1;
      ctx.fill();
      ctx.globalAlpha = 0.5;
      ctx.lineWidth = minLine * 1.5;
      ctx.strokeStyle = "#12b886";
      ctx.stroke();
      const fs = Math.max(18, Math.min(h * 0.08, 56));
      ctx.globalAlpha = 0.85;
      ctx.fillStyle = "#087f5b";
      ctx.font = `600 ${fs}px "Inter Variable", Inter, "Hind Siliguri", sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(it.title || "Smart Object", x + w / 2, y + h / 2, w * 0.9);
      ctx.textAlign = "start";
    } else if (it.kind === "text") {
      const [x, y, w] = it.rect;
      ctx.fillStyle = it.color;
      ctx.font = `500 ${it.size}px "Inter Variable", Inter, "Hind Siliguri", sans-serif`;
      ctx.textBaseline = "top";
      const lh = it.size * 1.25;
      it.text.split("\n").forEach((line, i) => ctx.fillText(line, x, y + i * lh, Math.max(w, it.size)));
    }
  }
  const blob = off
      ? await (canvas as OffscreenCanvas).convertToBlob({ type: "image/png" })
      : await new Promise<Blob | null>((r) => (canvas as HTMLCanvasElement).toBlob(r, "image/png"));
  return blob ? new Uint8Array(await blob.arrayBuffer()) : null;
}
