// The caption PDF export draws over a media item's poster (export-pdf.ts):
// what it is, its title, and for links the URL plus a QR code a student can
// scan from a printed handout.

import { drawQr } from "./qr";

export interface ExportMedia {
  kind: string;
  title: string;
  url?: string;
}

const FONT = '"Figtree Variable", Figtree, "Noto Sans Bengali Variable", system-ui, sans-serif';

function fit(g: CanvasRenderingContext2D, text: string, maxW: number): string {
  if (g.measureText(text).width <= maxW) return text;
  let t = text;
  while (t.length > 1 && g.measureText(`${t}…`).width > maxW) t = t.slice(0, -1);
  return `${t}…`;
}

/** A caption card for the bottom-left of the poster rect `[x, y, w, h]`
 * (world units), as a canvas and the world rect to place it at. */
export function mediaCaption(m: ExportMedia, rect: [number, number, number, number]): { canvas: HTMLCanvasElement; rect: [number, number, number, number] } | null {
  const [x, y, w, h] = rect;
  if (w < 120 || h < 80) return null;
  const pad = Math.max(8, Math.min(w, h) * 0.035);
  const ch = Math.min(h - 2 * pad, Math.max(64, h * 0.3));
  const cw = Math.min(w - 2 * pad, ch * (m.url ? 3.6 : 3));
  const scale = Math.min(4, 1400 / cw);
  const c = document.createElement("canvas");
  c.width = Math.round(cw * scale);
  c.height = Math.round(ch * scale);
  const g = c.getContext("2d")!;
  g.scale(scale, scale);
  const r = ch * 0.14;
  g.fillStyle = "#ffffff";
  g.beginPath();
  g.roundRect(0, 0, cw, ch, r);
  g.fill();
  g.strokeStyle = "rgba(17, 20, 24, 0.14)";
  g.lineWidth = 1.2;
  g.stroke();
  let tx = ch * 0.14;
  if (m.url) {
    const q = ch * 0.84;
    drawQr(g, m.url, ch * 0.08, ch * 0.08, q);
    tx = ch * 0.08 + q + ch * 0.1;
  }
  const tw = cw - tx - ch * 0.12;
  const label = m.kind === "youtube" ? "YOUTUBE" : m.kind === "web" ? "WEB PAGE" : m.kind === "audio" ? "AUDIO" : "VIDEO";
  const hint = m.url ? (m.kind === "youtube" ? "Scan to watch" : "Scan to open") : "In the Rupok lesson";
  g.textBaseline = "alphabetic";
  g.fillStyle = "#087f5b";
  g.font = `700 ${ch * 0.13}px ${FONT}`;
  g.fillText(fit(g, `${label} · ${hint.toUpperCase()}`, tw), tx, ch * 0.28);
  g.fillStyle = "#111418";
  g.font = `700 ${ch * 0.19}px ${FONT}`;
  g.fillText(fit(g, m.title || (m.kind === "youtube" ? "YouTube video" : "Media"), tw), tx, ch * 0.54);
  if (m.url) {
    g.fillStyle = "#3a3f45";
    g.font = `500 ${ch * 0.12}px ${FONT}`;
    // The URL over up to two lines.
    let rest = m.url;
    for (let line = 0; line < 2 && rest; line++) {
      let n = rest.length;
      while (n > 1 && g.measureText(rest.slice(0, n)).width > tw) n--;
      const part = line === 1 && n < rest.length ? fit(g, rest, tw) : rest.slice(0, n);
      g.fillText(part, tx, ch * (0.74 + line * 0.15));
      rest = rest.slice(n);
    }
  }
  return { canvas: c, rect: [x + pad, y + h - pad - ch, cw, ch] };
}
