/**
 * Types for @rupok/object-sdk 0.1.0 (bridge protocol v1).
 * Spec: spec/smart-object.md
 *
 * The host injects the SDK, which defines the global `rupok`. To get these
 * types in an editor, add `/// <reference types="@rupok/object-sdk" />`.
 */

/** Where the object is running. */
export type Mode = "edit" | "present" | "replay" | "student";

export type LogLevel = "info" | "warn" | "error";

/** Any JSON value. */
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

/** Standard theme tokens. Hosts may send more. */
export type ThemeToken =
  | "color.bg"
  | "color.fg"
  | "scheme"
  | "color.surface"
  | "color.line"
  | "color.accent"
  /** Accent-coloured text: at least 4.5:1 on the page. Never write text in `color.accent`. */
  | "color.accent-text"
  /** Text and icons on an accent fill (buttons). */
  | "color.on-accent"
  /** Highlight background; put `color.accent-text` on it. */
  | "color.accent-soft"
  | "color.accent2"
  | "color.muted"
  | "color.success"
  | "color.danger"
  | "font.body"
  | "font.heading"
  | "font.mono"
  | "radius";

/** Theme token to CSS value, e.g. `{ "color.accent": "#0ca678" }`. */
export type Theme = { [token: string]: string };

/** What `rupok.ready` resolves with. */
export interface InitInfo {
  /** The object's item id on the page; `"standalone"` without a host. */
  id: string;
  /** Current step, `0 <= step <= steps`. */
  step: number;
  /** Step count. */
  steps: number;
  /** Saved state, or `undefined` when nothing is saved. */
  state: Json | undefined;
  theme: Theme;
  mode: Mode;
  /** True when no rupok host answered (local preview). */
  standalone: boolean;
}

export interface StepChange {
  /** The step before this change (equals the current step for the initial call). */
  previous: number;
  /** The step count after this change. */
  count: number;
  /**
   * `"init"`: initial call or a host (re)initialisation.
   * `"host"`: the host sent a `step` message.
   * `"local"`: a change made inside the object (setCount, standalone stepping).
   */
  source: "init" | "host" | "local";
}

export type Unsubscribe = () => void;

export interface Steps {
  /** Current step, in `[0, count]`. `0` until init. */
  readonly current: number;
  /** Step count. `0` means static. */
  readonly count: number;
  /**
   * Called once with the initial step when `ready` resolves (or as a
   * microtask if subscribed later), then on every change of `current` or
   * `count`. Render the step as state: any step can arrive at any time.
   */
  onChange(cb: (current: number, change: StepChange) => void): Unsubscribe;
  /** Ask for the next step. No-op at `count`. */
  next(): void;
  /** Ask for the previous step. No-op at `0`. */
  prev(): void;
  /** Ask for step `n` (clamped to `[0, count]`). */
  goto(n: number): void;
  /** Declare the step count (integer, clamped to `[0, 500]`). */
  setCount(n: number): void;
}

export interface State {
  /** The saved state, or `undefined`. Returns a fresh copy each call. */
  load<T = Json>(): T | undefined;
  /**
   * Persist a JSON-serialisable value. Debounced by 200 ms; values over
   * 256 KiB of JSON are dropped with a console warning. Ignored before
   * `ready` resolves.
   */
  save(value: unknown): void;
}

export interface ThemeApi {
  get(token: ThemeToken | string): string | undefined;
  get<F>(token: ThemeToken | string, fallback: F): string | F;
  all(): Theme;
  /** Called with the initial theme once known, then on every change. */
  onChange(cb: (theme: Theme) => void): Unsubscribe;
}

export interface QuizPayload {
  questionId: string;
  /** The option id, text or number the learner chose. */
  answer: Json;
  correct?: boolean;
  /** Running score so far. */
  score?: number;
}

export interface TexOptions {
  /** Display (block) style, like `$$…$$`. Default `false` (inline). */
  display?: boolean;
}

/** Built-in LaTeX: KaTeX vendored by the host, loaded lazily. */
export interface MathApi {
  /** Typeset `$…$`, `$$…$$`, `\(…\)`, `\[…\]` and `[data-tex]` under `root` (default: the body). Resolves to the number of formulas rendered. */
  render(root?: Element | Document | null): Promise<number>;
  /** Load KaTeX now. Resolves to whether maths can be rendered. */
  load(): Promise<boolean>;
}

export interface Note37 {
  /** SDK and protocol version: `"1"`. */
  readonly version: "1";
  /** Resolves on init, or with defaults in standalone mode. Never rejects. */
  readonly ready: Promise<InitInfo>;
  /** Current mode; `"present"` until init. */
  readonly mode: Mode;
  /** Called when the mode changes. */
  onModeChange(cb: (mode: Mode) => void): Unsubscribe;
  readonly steps: Steps;
  readonly state: State;
  readonly theme: ThemeApi;
  readonly quiz: { submit(payload: QuizPayload): void };
  /** Ask the host for a new size in world units. Usually honoured only in edit mode. */
  resize(w: number, h: number): void;
  /** Log to the console and to the host. */
  log(level: LogLevel, ...args: unknown[]): void;
  /**
   * Typeset `latex` into `el` with KaTeX (replacing its content). `\hl{…}`
   * highlights a term in the theme accent. Re-rendering the same formula is a
   * no-op, so it is cheap to call on every change. Until KaTeX has loaded, `el`
   * keeps its current content. Resolves to `true` once rendered.
   */
  tex(el: Element, latex: string, options?: TexOptions): Promise<boolean>;
  readonly math: MathApi;
}

/* ------------------------------------------------- bridge protocol v1 */

export type ObjectToHostMessage =
  | { rupok: 1; type: "hello"; sdk: "1" }
  | { rupok: 1; type: "state"; state: Json }
  | { rupok: 1; type: "goto"; step: number }
  | { rupok: 1; type: "steps"; count: number }
  | { rupok: 1; type: "resize"; w: number; h: number }
  | { rupok: 1; type: "quiz"; payload: QuizPayload }
  | { rupok: 1; type: "log"; level: LogLevel; message: string }
  | { rupok: 1; type: "key"; key: "PageDown" | "PageUp" | "ArrowRight" | "ArrowLeft" | "ArrowDown" | "ArrowUp" | "Escape" }
  /** Reply to a host `snapshot` request (sent by the SDK automatically). */
  | { rupok: 1; type: "snapshot"; id: string | number; dataUrl: string }
  | { rupok: 1; type: "snapshot"; id: string | number; error: string };

export type HostToObjectMessage =
  | { rupok: 1; type: "init"; id: string; step: number; steps: number; state: Json; theme: Theme; mode: Mode }
  | { rupok: 1; type: "step"; step: number }
  | { rupok: 1; type: "theme"; theme: Theme }
  | { rupok: 1; type: "mode"; mode: Mode }
  /** Ask for a PNG of the object's viewport; `scale` defaults to 2, clamped to [0.25, 4]. */
  | { rupok: 1; type: "snapshot"; id: string | number; scale?: number };

declare global {
  interface Window {
    rupok: Note37;
  }
  var rupok: Note37;
}
