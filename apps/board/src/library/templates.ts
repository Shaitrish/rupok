// Templates for new lessons. Every page is a 16:9 slide; a template picks
// the page pattern, the board colour and how many pages to start with.

import { type DesignTarget, applyDesignDefault } from "../design";

export type TemplateId = "blank" | "slides" | "lined" | "grid";

export interface Template {
  id: TemplateId;
  label: string;
  hint: string;
  pattern: "plain" | "grid" | "dots" | "lines";
  bg: number;
  pages: number;
}

export const TEMPLATES: Template[] = [
  { id: "blank", label: "Blank", hint: "One white page", pattern: "plain", bg: 0xffffffff, pages: 1 },
  { id: "slides", label: "16:9 slides", hint: "Five blank slides", pattern: "plain", bg: 0xffffffff, pages: 5 },
  { id: "lined", label: "Lined", hint: "Ruled paper for writing", pattern: "lines", bg: 0xfbf7edff, pages: 1 },
  { id: "grid", label: "Grid", hint: "Squares for maths and graphs", pattern: "grid", bg: 0xffffffff, pages: 1 },
];

/** The parts of the board a template needs. */
export interface TemplateTarget extends DesignTarget {
  new_lesson(title: string): void;
  set_background(pattern: string, rgba: number): void;
  add_page(pattern: string): number;
  goto_page(i: number): boolean;
}

/** Start a new lesson on the board from a template, with the teacher's
 * default page design (design.ts): its branding always, its look unless the
 * template has its own (Lined, Grid). */
export function applyTemplate(board: TemplateTarget, title: string, id: TemplateId) {
  const t = TEMPLATES.find((x) => x.id === id) ?? TEMPLATES[0];
  board.new_lesson(title);
  if (t.pattern !== "plain" || t.bg !== 0xffffffff) board.set_background(t.pattern, t.bg);
  for (let i = 1; i < t.pages; i++) {
    board.add_page(t.pattern);
    if (t.bg !== 0xffffffff) board.set_background(t.pattern, t.bg);
  }
  applyDesignDefault(board, { style: t.pattern === "plain" && t.bg === 0xffffffff });
  board.goto_page(0);
}
