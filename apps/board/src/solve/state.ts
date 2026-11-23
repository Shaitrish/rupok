// Solve this: preferences, the teacher's handwriting (calibration) and the
// shared signals of the feature (solve/index.ts has the flow).

import { createSignal } from "solid-js";
import { jsonStore } from "../settings/bind";

/** Settings → AI & agents → Solve this (settings/defs/solve.ts). */
export const solvePrefs = jsonStore("rupok.solve", {
  /** "teacher": the teacher's own handwriting where taught; "neat": a standard hand. */
  style: "teacher" as "teacher" | "neat",
  /** Pen speed multiplier (1 = brisk board writing). */
  speed: 1,
  /** Show the solution to the teacher first; it goes on the board on "Write it on the board". */
  preview: true,
  /** In Teach, each Next (clicker, key, swipe) writes the next line. */
  teachStep: true,
  /** Students may use Solve this in Student view. */
  students: false,
});

/** A solution waiting for the teacher's check. */
export interface PendingSolution {
  id: number;
  agent: string;
  /** The `write_handwriting` op as the agent sent it (defaults filled in). */
  op: any;
  lines: string[];
  at: number;
}

export const [pending, setPending] = createSignal<PendingSolution[]>([]);
/** `[shown, total, waiting, gate, lines]` while AI handwriting is being written. */
export const [reveal, setReveal] = createSignal<number[] | null>(null);
/** A solve in progress (between the request and the agent's answer). */
export const [solving, setSolving] = createSignal(false);
export const [calibrateOpen, setCalibrateOpen] = createSignal(false);

// ------------------------------------------------------------ handwriting

/** One sample: strokes as flat `[x, y, x, y, …]` in writing units (baseline
 * y = 0, x-height line y = −14), the frame `rupok_ink::write` and the
 * recogniser share. */
export type Sample = number[][];
const GLYPH_KEY = "rupok.handwriting.v1";
/** Samples kept per character. */
export const SAMPLES_PER_CHAR = 3;

/** What the calibration screen asks for: digits, common letters, maths. */
export const CALIBRATION_CHARS: { group: string; chars: string[] }[] = [
  { group: "Digits", chars: [..."0123456789"] },
  { group: "Letters", chars: [..."abcdefghijklmnopqrstuvwxyz"] },
  { group: "Maths", chars: ["+", "−", "=", "×", "÷", "(", ")", "/", "<", ">", ".", ",", "π", "θ", "α"] },
];

function loadGlyphs(): Record<string, Sample[]> {
  try {
    const v = JSON.parse(localStorage.getItem(GLYPH_KEY) || "{}");
    return v && typeof v === "object" && v.glyphs && typeof v.glyphs === "object" ? v.glyphs : {};
  } catch {
    return {};
  }
}

export const [glyphs, setGlyphsSignal] = createSignal<Record<string, Sample[]>>(loadGlyphs());
export const taughtCount = () => Object.keys(glyphs()).length;

/** Save the teacher's samples (per device; the library or cloud later). */
export function saveGlyphs(g: Record<string, Sample[]>) {
  setGlyphsSignal(g);
  try { localStorage.setItem(GLYPH_KEY, JSON.stringify({ v: 1, glyphs: g })); } catch { /* storage off */ }
}
