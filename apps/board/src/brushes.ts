// Pen styles ("brushes"): the list the pen popover shows, which tool offers
// which, and a small Canvas 2D sketch of each for the swatches.
//
// The ink itself is drawn by the GPU renderer (crates/rupok-render, ink.wgsl)
// from the stroke's `brush` key; the shapes here only preview it. Each tool
// remembers its own style in prefs (`penBrush`, `hlBrush`, `shapeBrush`).

import { type Prefs, prefs, setPrefs } from "./state";

export type BrushId =
  | "pen" | "ballpoint" | "fountain" | "marker" | "pencil" | "chalk" | "brush"
  | "dashed" | "dotted" | "neon" | "rainbow" | "magic";

export interface BrushInfo {
  id: BrushId;
  label: string;
  /** One line for the tooltip. */
  hint: string;
}

export const BRUSHES: BrushInfo[] = [
  { id: "pen", label: "Pen", hint: "Smooth, follows pen pressure" },
  { id: "ballpoint", label: "Ballpoint", hint: "Crisp, always the same width" },
  { id: "fountain", label: "Fountain", hint: "Calligraphy: thick and thin with the direction" },
  { id: "marker", label: "Marker", hint: "Wide chisel tip, a little see-through" },
  { id: "pencil", label: "Pencil", hint: "Thin and grainy" },
  { id: "chalk", label: "Chalk", hint: "Dusty edge, made for the chalkboard" },
  { id: "brush", label: "Brush", hint: "Soft, with tapered ends" },
  { id: "dashed", label: "Dashed", hint: "A dashed line" },
  { id: "dotted", label: "Dotted", hint: "A dotted line" },
  { id: "neon", label: "Neon", hint: "A glowing line" },
  { id: "rainbow", label: "Rainbow", hint: "Colours change along the line" },
  { id: "magic", label: "Magic ink", hint: "Disappears a few seconds after you lift the pen: point without cleaning up" },
];

/** Which styles each tool offers. */
export const TOOL_BRUSHES: Record<"pen" | "highlighter" | "shape", BrushId[]> = {
  pen: BRUSHES.map((b) => b.id),
  highlighter: ["pen", "marker", "dashed", "rainbow"],
  shape: ["pen", "ballpoint", "marker", "pencil", "chalk", "dashed", "dotted", "neon", "rainbow"],
};

const PREF: Record<"pen" | "highlighter" | "shape", "penBrush" | "hlBrush" | "shapeBrush"> = {
  pen: "penBrush", highlighter: "hlBrush", shape: "shapeBrush",
};

export const isBrush = (v: unknown): v is BrushId => BRUSHES.some((b) => b.id === v);
export const brushInfo = (id: string) => BRUSHES.find((b) => b.id === id) ?? BRUSHES[0];

/** The remembered style of a tool. */
export function brushOf(kind: "pen" | "highlighter" | "shape", p: Prefs = prefs()): BrushId {
  const v = p[PREF[kind]];
  return isBrush(v) && TOOL_BRUSHES[kind].includes(v) ? v : "pen";
}

export function setBrushOf(kind: "pen" | "highlighter" | "shape", id: BrushId) {
  setPrefs({ [PREF[kind]]: id } as Partial<Prefs>);
}

/** The opacity a tool's new strokes get: the marker is see-through. */
export function opacityOf(kind: "pen" | "highlighter" | "shape", p: Prefs = prefs()): number {
  return brushOf(kind, p) === "marker" ? p.markerOpacity : 1;
}

// ---------------------------------------------------------------- preview

/** Deterministic noise for the sketches (no Math.random: previews must not
 * flicker when they redraw). */
function hash(i: number): number {
  const x = Math.sin(i * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
}

/** The sample stroke: a gentle wave across the swatch. */
function samplePath(w: number, h: number, n = 48): [number, number, number][] {
  const out: [number, number, number][] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const x = w * (0.14 + 0.72 * t);
    const y = h * (0.56 - 0.26 * Math.sin(t * Math.PI * 1.6 + 0.3));
    // A pen pressing harder in the middle.
    const p = 0.45 + 0.55 * Math.sin(t * Math.PI);
    out.push([x, y, p]);
  }
  return out;
}

function rgba(c: number, a = 1): string {
  return `rgba(${(c >>> 24) & 255}, ${(c >>> 16) & 255}, ${(c >>> 8) & 255}, ${(((c & 255) / 255) * a).toFixed(3)})`;
}

/** Hue at fraction `t` of the rainbow (matches rupok_ink::brush::rainbow_rgb). */
export function rainbowCss(t: number): string {
  return `hsl(${Math.round(((t % 1) + 1) % 1 * 360)}, 72%, 56%)`;
}

/**
 * Sketch `id` into a canvas of `w`×`h` CSS px (device pixels from `dpr`) in
 * `color` (0xRRGGBBAA), with a line of about `size` px. Approximations of
 * the GPU look, cheap enough to redraw on every colour change.
 */
export function drawBrushPreview(cv: HTMLCanvasElement, id: BrushId, color: number, size: number, dpr = window.devicePixelRatio || 1) {
  const w = cv.clientWidth || 56, h = cv.clientHeight || 40;
  if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
    cv.width = Math.round(w * dpr);
    cv.height = Math.round(h * dpr);
  }
  const g = cv.getContext("2d");
  if (!g) return;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  g.lineCap = "round";
  g.lineJoin = "round";
  const pts = samplePath(w, h);
  const r = Math.max(1, Math.min(size, 9)) / 2;
  const seg = (i: number, lw: number, style: string | CanvasGradient) => {
    g.beginPath();
    g.moveTo(pts[i - 1][0], pts[i - 1][1]);
    g.lineTo(pts[i][0], pts[i][1]);
    g.lineWidth = lw;
    g.strokeStyle = style;
    g.stroke();
  };
  const whole = (lw: number, style: string, dash: number[] = []) => {
    g.beginPath();
    pts.forEach(([x, y], i) => (i ? g.lineTo(x, y) : g.moveTo(x, y)));
    g.setLineDash(dash);
    g.lineWidth = lw;
    g.strokeStyle = style;
    g.stroke();
    g.setLineDash([]);
  };
  const c = rgba(color);
  switch (id) {
    case "ballpoint":
      whole(2 * r * 0.8, c);
      break;
    case "fountain":
      for (let i = 1; i < pts.length; i++) {
        const dx = pts[i][0] - pts[i - 1][0], dy = pts[i][1] - pts[i - 1][1];
        const len = Math.hypot(dx, dy) || 1;
        // Nib at about 40° ("/"): wide across it, thin along it.
        const across = Math.abs((dx / len) * -0.643 - (dy / len) * 0.766);
        seg(i, 2 * r * (0.16 + 0.84 * across) * (0.6 + 0.6 * pts[i][2]), c);
      }
      break;
    case "marker":
      g.globalAlpha = prefs().markerOpacity;
      whole(2 * r * 1.5, c);
      g.globalAlpha = 1;
      break;
    case "pencil":
      for (let i = 1; i < pts.length; i++) seg(i, 2 * r * 0.7, rgba(color, 0.45 + 0.5 * hash(i)));
      break;
    case "chalk":
      for (let i = 1; i < pts.length; i++) {
        for (let k = 0; k < 3; k++) {
          const j = i * 3 + k;
          const ox = (hash(j) - 0.5) * r, oy = (hash(j + 99) - 0.5) * r;
          g.beginPath();
          g.moveTo(pts[i - 1][0] + ox, pts[i - 1][1] + oy);
          g.lineTo(pts[i][0] + ox, pts[i][1] + oy);
          g.lineWidth = r * (0.6 + 0.8 * hash(j + 7));
          g.strokeStyle = rgba(color, 0.35 + 0.45 * hash(j + 3));
          g.stroke();
        }
      }
      break;
    case "brush":
      for (let i = 1; i < pts.length; i++) {
        const t = Math.min(i, pts.length - 1 - i) / 10;
        seg(i, 2 * r * 1.2 * (0.12 + 0.88 * Math.min(1, t)) * (0.4 + 0.6 * pts[i][2]), c);
      }
      break;
    case "dashed":
      whole(2 * r, c, [2 * r * 2, 2 * r * 2.6]);
      break;
    case "dotted":
      whole(2 * r, c, [0.01, 2 * r * 2]);
      break;
    case "neon":
    case "magic":
      g.shadowColor = rgba(color);
      g.shadowBlur = 8;
      whole(2 * r, c);
      g.shadowBlur = 0;
      whole(Math.max(1, r * 0.8), "rgba(255,255,255,0.75)");
      if (id === "magic") {
        // Fades towards its start: it disappears.
        const fade = g.createLinearGradient(0, 0, w * 0.7, 0);
        fade.addColorStop(0, "rgba(0,0,0,1)");
        fade.addColorStop(1, "rgba(0,0,0,0)");
        g.globalCompositeOperation = "destination-out";
        g.fillStyle = fade;
        g.fillRect(0, 0, w, h);
        g.globalCompositeOperation = "source-over";
      }
      break;
    case "rainbow":
      for (let i = 1; i < pts.length; i++) seg(i, 2 * r, rainbowCss(i / pts.length));
      break;
    default:
      for (let i = 1; i < pts.length; i++) seg(i, 2 * r * (0.35 + 0.65 * pts[i][2]), c);
  }
}
