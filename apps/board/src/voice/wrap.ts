// Line breaking for text that must stay inside a box: Voice text on the
// board, and the agents' `add_text` with a width (live.ts).
//
// Board text items never wrap by themselves (`.item.text` has a minimum
// width, not a width), so text that has to fit a box gets its line breaks
// written into it, measured with the board's own font. What the teacher sees
// in the voice box is then exactly the text item it becomes, and every reader
// (PDF export, snapshots, other apps) shows the same lines.
//
// `wrapText` is pure (the measure function is passed in) and unit-tested in
// apps/board/test/voice-board.test.ts.

/** Width of a string in the units of the box. */
export type Measure = (s: string) => number;

/** Grapheme clusters, so a Bangla conjunct or a vowel sign is never split. */
function graphemes(s: string): string[] {
  const Seg = (Intl as any).Segmenter;
  if (Seg) return Array.from(new Seg(undefined, { granularity: "grapheme" }).segment(s), (x: any) => x.segment as string);
  return Array.from(s);
}

/** Break a word wider than the box into pieces that fit. */
function breakWord(word: string, width: number, measure: Measure): string[] {
  const out: string[] = [];
  let cur = "";
  for (const g of graphemes(word)) {
    if (cur && measure(cur + g) > width) {
      out.push(cur);
      cur = g;
    } else cur += g;
  }
  if (cur) out.push(cur);
  return out;
}

/**
 * Break `text` into lines no wider than `width`: at spaces where it can, and
 * inside a word only when the word alone is wider than the box. Existing
 * line breaks are kept; the space at a break is dropped. Returns the lines.
 */
export function wrapLines(text: string, width: number, measure: Measure): string[] {
  const out: string[] = [];
  if (!(width > 0)) return text.split("\n");
  for (const para of text.split("\n")) {
    const words = para.split(/ +/);
    let line = "";
    for (const raw of words) {
      if (raw === "") continue;
      const next = line ? `${line} ${raw}` : raw;
      if (measure(next) <= width) {
        line = next;
        continue;
      }
      if (line) out.push(line);
      if (measure(raw) <= width) {
        line = raw;
        continue;
      }
      const pieces = breakWord(raw, width, measure);
      out.push(...pieces.slice(0, -1));
      line = pieces[pieces.length - 1] ?? "";
    }
    out.push(line);
  }
  return out;
}

/** {@link wrapLines} joined with `\n`. */
export const wrapText = (text: string, width: number, measure: Measure) => wrapLines(text, width, measure).join("\n");

/** Tidy spacing so the wrapping maps back onto the text exactly. */
export const tidyText = (s: string) => s.replace(/[ \t]+/g, " ").replace(/ *\n */g, "\n");

/** One rendered line of the box: the part already final, then the part still being recognised. */
export interface LiveLine { done: string; live: string }

/**
 * The box's lines: `done + live` wrapped to `width` with `measure`, each line
 * split where the final text ends. Pure (unit-tested).
 */
export function splitLines(done: string, liveText: string, width: number, measure: (s: string) => number): LiveLine[] {
  const full = tidyText(done + liveText);
  const cut = tidyText(done).length;
  const lines = wrapLines(full, width, measure);
  const out: LiveLine[] = [];
  let p = 0;
  for (const line of lines) {
    const a = p, b = p + line.length;
    out.push({ done: full.slice(a, Math.max(a, Math.min(b, cut))), live: full.slice(Math.max(a, Math.min(b, cut)), b) });
    p = b;
    // The space or line break the wrap consumed.
    if (full[p] === " " || full[p] === "\n") p++;
  }
  return out;
}

// ------------------------------------------------------------ board font

let ctx: CanvasRenderingContext2D | null = null;
let family: string | null = null;

/** The board's text font family (`.item.text` uses `--font`). */
function boardFamily(): string {
  if (family) return family;
  const f = getComputedStyle(document.documentElement).getPropertyValue("--font").trim();
  family = f || "sans-serif";
  return family;
}

/** The CSS font of a board text item at `size` world units. */
export const boardFont = (size: number) => `500 ${size}px ${boardFamily()}`;

/** A measure for board text at font size `size` (world units). The browser
 * shapes the canvas text like the DOM text (Bangla included). */
export function boardMeasure(size: number): Measure {
  ctx ??= document.createElement("canvas").getContext("2d");
  const c = ctx;
  if (!c) return (s) => s.length * size * 0.55;
  const font = boardFont(size);
  return (s) => {
    c.font = font;
    return c.measureText(s).width;
  };
}

/** Make sure the board font is loaded before measuring `text` with it. */
export async function loadBoardFont(size: number, text: string): Promise<void> {
  try {
    await document.fonts.load(boardFont(size), text || "A");
  } catch { /* measure with what there is */ }
}

/** A little room so a line measured at the limit never wraps in the DOM. */
export const WRAP_SLACK = 2;
