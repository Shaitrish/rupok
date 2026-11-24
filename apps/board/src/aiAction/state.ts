// "AI in action": preferences and shared signals. The flow lives in
// progress.ts (working cards), classMode.ts (no prompts for routine edits,
// Undo toasts, destructive confirmations), solveStream.ts (Solve this line
// by line) and student.ts (students ask AI). UI: AiActionLayer.tsx.

import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { jsonStore } from "../settings/bind";
import { mode } from "../state";

/** Settings → AI & agents → AI in action (settings/defs/aiAction.ts). */
export const aiPrefs = jsonStore("rupok.aiAction", {
  /** Class mode: board edits by the AI apply at once (with an Undo toast);
   * only destructive actions ask. "teach": in Teach only. */
  classMode: "teach" as "teach" | "always" | "off",
  /** Show a "working" card on the board while the AI builds something. */
  progress: true,
  /** A small sparkle when a result lands. */
  celebrate: true,
  /** A soft chime when a result lands (off by default). */
  sound: false,
  /** Solve this writes the solution line by line as the AI works it out
   * (when the teacher does not check it first, or in class mode). */
  streamSolve: true,
  /** Students may ask the AI to show them something in Student view. */
  studentsAsk: false,
  /** Asks per student every 10 minutes. */
  studentLimit: 3,
});

export const classModeOn = () => {
  const p = aiPrefs.read().classMode;
  return p === "always" || (p === "teach" && mode() === "teach");
};

export type CardKind = "object" | "handwriting" | "text" | "shape" | "other";
export type CardState = "working" | "landing" | "done" | "failed";
export interface Rect { x: number; y: number; w: number; h: number }

/** A "working" card: where the AI's result will land, what it is doing. */
export interface Card {
  id: string;
  /** chat: the in-app chat; solve: Solve this; agent: show_progress; student: a student's ask. */
  source: "chat" | "solve" | "agent" | "student";
  agent: string;
  title: string;
  step: string;
  kind: CardKind;
  plan: { content: string; status: string }[];
  /** Recent tool steps, newest last. */
  tools: string[];
  percent?: number;
  /** 1-based page the result goes on. */
  page: number;
  rect: Rect;
  state: CardState;
  error?: string;
  /** Partial Smart Object HTML shown as a live thumbnail. */
  preview?: string;
  /** How many times a Smart Object failed its checks in this card. */
  fixes: number;
  started: number;
  updated: number;
}

export const [cards, setCards] = createStore<{ list: Card[] }>({ list: [] });

/** "Claude added a Smart Object · Undo" (class mode). */
export interface AiToast { id: number; agent: string; label: string; undo?: () => void }
export const [aiToast, setAiToast] = createSignal<AiToast | null>(null);

/** A destructive action waiting for the teacher's OK (class mode). */
export interface ConfirmReq { id: number; agent: string; label: string; detail?: string; answer: (ok: boolean) => void; until: number }
export const [confirmReq, setConfirmReq] = createSignal<ConfirmReq | null>(null);

/** A sparkle where a result landed (slide px on `page`). */
export const [sparkle, setSparkle] = createSignal<{ x: number; y: number; page: number; n: number } | null>(null);
