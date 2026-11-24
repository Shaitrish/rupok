// Custom stamp definitions on the board: the teacher's device library ("My
// stamps", kept in IndexedDB by customStamps.ts) and the definitions stored
// in the open lesson (the root `stamps` map, read with
// `board.stamp_defs_json()`). A placed custom stamp names its definition in
// its `stamp` key; the lesson's copy wins, so a shared lesson shows the
// stamp exactly as it was made. Kept free of other board modules so both
// stamps.ts (drawing) and customStamps.ts (storage, behaviour) can use it.
//
// Definitions are checked in Rust (`rupok_live::stamps::check`): SVG is
// sanitised, behaviour values come from a small set. A Smart stamp's HTML
// only ever runs as a Smart Object, in the sandboxed iframe.

import { createSignal } from "solid-js";

export type StampArt = "svg" | "emoji" | "text" | "smart";

export interface StampBehavior {
  animation?: "none" | "pop" | "sparkle" | "bounce" | "spin" | "fade-in";
  burst?: "none" | "stars" | "confetti" | "hearts" | "sparkles";
  burst_colors?: string[];
  counter?: boolean;
  sound?: "none" | "chime" | "pop" | "ding" | "tada" | "clap" | "whoosh";
  fade_after?: number;
}

export interface CustomStamp {
  id: string;
  name: string;
  keywords?: string[];
  art: StampArt;
  svg?: string;
  emoji?: string;
  text?: string;
  /** `#rrggbb`: the pill colour, or the SVG's currentColor. */
  color?: string;
  /** Default height, slide pixels. */
  size?: number;
  behavior?: StampBehavior;
  /** Smart stamp: its HTML (device library only; lessons keep a bundle). */
  html?: string;
  bundle?: string;
  width?: number;
  height?: number;
}

/** The teacher's stamps on this device, in their order. */
export const [myStamps, setMyStamps] = createSignal<CustomStamp[]>([]);
/** Custom stamps stored in the open lesson, by id. */
export const [lessonStamps, setLessonStamps] = createSignal<Record<string, CustomStamp>>({});

let lastLessonJson = "";
/** Refresh the lesson's definitions from the engine (cheap when unchanged). */
export function syncLessonStamps(json: string) {
  if (json === lastLessonJson) return;
  lastLessonJson = json;
  try {
    const raw = JSON.parse(json) as Record<string, CustomStamp>;
    for (const [id, d] of Object.entries(raw)) d.id = id;
    setLessonStamps(raw);
  } catch {
    setLessonStamps({});
  }
}

/** A custom stamp by id: the lesson's copy, else the device library's. */
export function resolveStamp(id: string | undefined): CustomStamp | undefined {
  if (!id) return undefined;
  return lessonStamps()[id] ?? myStamps().find((s) => s.id === id);
}

export const isCustomStampId = (id: string) => !!resolveStamp(id);

/** Default box of a custom stamp, slide pixels (mirrors rupok_live::stamps). */
export function customSize(d: CustomStamp, size?: number): { w: number; h: number } {
  if (d.art === "smart") {
    const dw = d.width ?? 200, dh = d.height ?? 200;
    const h = size ?? d.size ?? dh;
    return { w: Math.round((dw * h) / dh), h };
  }
  const h = size ?? d.size ?? 96;
  if (d.art === "text") {
    const chars = [...(d.text ?? d.name)].length;
    return { w: Math.round(Math.max(h * 1.8, h * 0.42 * chars + h * 0.9)), h };
  }
  return { w: h, h };
}

/** A data: URL of a custom SVG stamp with `currentColor` set to `color`.
 * Drawn through an <img>, so even a sanitised SVG can never run code. */
export function svgDataUrl(svg: string, color: string): string {
  const tinted = svg.replace(/currentColor/g, color);
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(tinted)}`;
}

/** The stamp editor sheet (StampEditor.tsx): a stamp to edit, `{}` for a new one. */
export const [stampEditor, setStampEditor] = createSignal<{ id?: string } | null>(null);
