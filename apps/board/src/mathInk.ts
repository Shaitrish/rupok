// The offline handwritten-maths recogniser (rupok_ink::math via MathInk),
// shared by the live preview and "To text". It learns the teacher's own
// handwriting from the one shared store, `glyphs()` in solve/state.ts
// (`rupok.handwriting.v1`), written by "Teach Rupok my handwriting" and also
// used by Solve this to write in the teacher's hand.

import { MathInk } from "./wasm/rupok.js";
import { glyphs } from "./solve/state";

export interface MathCandidate {
  latex: string;
  confidence: number;
  symbols: { symbol: string; latex: string; bbox: [number, number, number, number]; strokes: number[]; confidence: number }[];
}

let rec: MathInk | null = null;
let learned = -1;

/** The recogniser, with the teacher's samples taught (again when they change). */
export function mathInk(): MathInk {
  rec ??= new MathInk();
  const g = glyphs();
  const n = Object.values(g).reduce((a, l) => a + l.length, 0);
  if (n !== learned) {
    try {
      rec.clear_user_templates(undefined);
      for (const [sym, list] of Object.entries(g))
        for (const sample of list) {
          // A sample is strokes of flat x, y pairs (writing units).
          const strokes = sample.filter((st) => st.length >= 2);
          if (!strokes.length) continue;
          const xy = Float32Array.from(strokes.flat());
          const counts = Uint32Array.from(strokes.map((st) => Math.floor(st.length / 2)));
          try { rec.add_user_template(sym, xy, counts); } catch { /* not a maths symbol */ }
        }
    } catch { /* keep the built-ins */ }
    learned = n;
  }
  return rec;
}

/** Below this, no conversion without a tap (and the board never converts on its own). */
export const AUTO_CONVERT = 0.85;
/** Lowest confidence at which a preview is shown. */
export const PREVIEW_CONFIDENCE = 0.55;

export function recognize(xy: number[] | Float32Array, counts: number[] | Uint32Array): MathCandidate[] {
  try {
    return JSON.parse(mathInk().recognize_json(Float32Array.from(xy), Uint32Array.from(counts)));
  } catch {
    return [];
  }
}

/**
 * Does a reading look like maths rather than words? It needs a digit, an
 * operator, a script, a fraction or a root, or a single letter next to one
 * of those; plain letter runs ("cat") are words.
 */
export function looksLikeMath(c: MathCandidate | undefined): boolean {
  if (!c || c.symbols.length < 2) return false;
  const t = c.latex;
  return /[0-9+\-=<>×÷·^_]|\\(frac|sqrt|times|div|cdot|pm|le|ge|neq|pi|theta|alpha|beta)/.test(t);
}

/** InkSample strokes (`pts: [x, y, t, …]`) as flat pairs and counts. */
export function fromInk(strokes: { pts: number[] }[]): { xy: number[]; counts: number[] } {
  const xy: number[] = [];
  const counts: number[] = [];
  for (const s of strokes) {
    let n = 0;
    for (let i = 0; i + 2 < s.pts.length; i += 3) {
      xy.push(s.pts[i], s.pts[i + 1]);
      n++;
    }
    counts.push(n);
  }
  return { xy, counts };
}

/** Font size and box for typeset maths over the ink's bounds. */
export function mathPlacement(bounds: number[], latex: string) {
  const [x0, y0, x1, y1] = bounds;
  const h = Math.max(1, y1 - y0);
  const tall = /\\frac|\^|_|\\sqrt/.test(latex);
  const size = Math.min(320, Math.max(16, h / (tall ? 1.6 : 1.15)));
  return { x: x0, y: y0 + Math.max(0, (h - size * 1.3) / 2), w: Math.max(size, x1 - x0), size: Math.round(size * 10) / 10 };
}
