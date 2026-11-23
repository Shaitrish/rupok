// Handwriting → text: lasso some ink, tap "To text", and it becomes a text
// item in the same place, size and colour (one undo brings the ink back).
//
// Recognition goes through a small provider interface so backends can be
// added without touching the UI:
//   - chrome: Chrome's built-in recognizer (navigator.createHandwritingRecognizer,
//     offline, ChromeOS only). Text, English.
//   - claude: Claude with the teacher's own API key (Settings → AI). English,
//     Bangla and maths. The key lives only in this browser's localStorage
//     under its own entry (never in prefs, the exported UI config, autosave
//     or lesson files) and is sent only to api.anthropic.com.
//   - cloud:  Rupok Cloud, a future hosted recognizer (stub, never available yet).

import { createSignal } from "solid-js";
import { APP_SLUG } from "./brand";

/** One stroke as `Board.selection_ink_json` gives it: `pts` is flat
 * `[x, y, t, …]` in world units, `t` in ms since the first stroke began. */
export interface InkStroke { color: string; width: number; pts: number[] }
export interface InkSample { bounds: [number, number, number, number]; strokes: InkStroke[] }

export interface Recognition {
  kind: "text" | "math";
  /** Plain text; for maths a Unicode rendering (x² + 3x − 4 = 0). */
  text: string;
  /** LaTeX for maths (no delimiters), else empty. */
  latex: string;
}

export type ProviderId = "chrome" | "claude" | "cloud";

export interface HandwritingProvider {
  id: ProviderId;
  label: string;
  /** What it reads, for the settings and the "how to enable" popover. */
  reads: string;
  available(): Promise<boolean>;
  recognize(ink: InkSample, signal?: AbortSignal): Promise<Recognition>;
}

// ----------------------------------------------------------- the API key

const KEY_STORE = `${APP_SLUG}.anthropic-api-key`;

export function claudeKey(): string {
  try {
    return localStorage.getItem(KEY_STORE) ?? "";
  } catch {
    return "";
  }
}

/** Save (or with "" remove) the teacher's key, on this device only. */
export function setClaudeKey(key: string) {
  try {
    if (key.trim()) localStorage.setItem(KEY_STORE, key.trim());
    else localStorage.removeItem(KEY_STORE);
  } catch { /* storage unavailable */ }
  setKeyVersion((v) => v + 1);
}

/** Bumped when the key changes, so the UI can re-check providers. */
export const [keyVersion, setKeyVersion] = createSignal(0);

// ------------------------------------------------------- ink → PNG image

/** The ink drawn dark on white, cropped with a margin, longest side ≤ 1024 px. */
export async function inkToPng(ink: InkSample): Promise<{ base64: string; width: number; height: number }> {
  const [x0, y0, x1, y1] = ink.bounds;
  const pad = 24;
  const w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
  // Small writing is scaled up so strokes stay legible; big boards down.
  const s = Math.min(1024 / (w + 2 * pad), 1024 / (h + 2 * pad), 4);
  const cw = Math.max(32, Math.round((w + 2 * pad) * s));
  const ch = Math.max(32, Math.round((h + 2 * pad) * s));
  const canvas = document.createElement("canvas");
  canvas.width = cw;
  canvas.height = ch;
  const g = canvas.getContext("2d")!;
  g.fillStyle = "#ffffff";
  g.fillRect(0, 0, cw, ch);
  g.strokeStyle = "#111418";
  g.lineCap = "round";
  g.lineJoin = "round";
  for (const st of ink.strokes) {
    g.lineWidth = Math.max(2, st.width * s * 0.8);
    g.beginPath();
    for (let i = 0; i < st.pts.length; i += 3) {
      const px = (st.pts[i] - x0 + pad) * s, py = (st.pts[i + 1] - y0 + pad) * s;
      if (i === 0) g.moveTo(px, py);
      else g.lineTo(px, py);
    }
    if (st.pts.length === 3) g.lineTo((st.pts[0] - x0 + pad) * s + 0.1, (st.pts[1] - y0 + pad) * s);
    g.stroke();
  }
  const blob: Blob = await new Promise((res, rej) => canvas.toBlob((b) => (b ? res(b) : rej(new Error("could not draw the ink"))), "image/png"));
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return { base64: btoa(bin), width: cw, height: ch };
}

// ------------------------------------------------- Chrome's recognizer

// Chrome's Handwriting Recognition API is not in the TypeScript DOM types.
interface HwStroke { addPoint(p: { x: number; y: number; t: number }): void }
interface HwDrawing {
  addStroke(s: HwStroke): void;
  getPrediction(): Promise<{ text: string }[]>;
  clear(): void;
}
interface HwRecognizer { startDrawing(hints?: object): HwDrawing; finish(): void }
type HwNavigator = Navigator & {
  createHandwritingRecognizer?: (c: { languages: string[] }) => Promise<HwRecognizer>;
  queryHandwritingRecognizer?: (c: { languages: string[] }) => Promise<object | null>;
};
const hwNav = () => navigator as HwNavigator;

export const chromeProvider: HandwritingProvider = {
  id: "chrome",
  label: "Chrome's built-in recognizer",
  reads: "English text, offline (Chromebooks)",
  async available() {
    const n = hwNav();
    if (typeof n.createHandwritingRecognizer !== "function") return false;
    try {
      return n.queryHandwritingRecognizer ? !!(await n.queryHandwritingRecognizer({ languages: ["en"] })) : true;
    } catch {
      return false;
    }
  },
  async recognize(ink) {
    const n = hwNav();
    const Stroke = (window as unknown as { HandwritingStroke?: new () => HwStroke }).HandwritingStroke;
    if (!n.createHandwritingRecognizer || !Stroke) throw new Error("Chrome's handwriting recognizer is not available here");
    const rec = await n.createHandwritingRecognizer({ languages: ["en"] });
    try {
      const drawing = rec.startDrawing({ recognitionType: "text", inputType: "stylus", alternatives: 1 });
      for (const st of ink.strokes) {
        const s = new Stroke();
        for (let i = 0; i < st.pts.length; i += 3) s.addPoint({ x: st.pts[i], y: st.pts[i + 1], t: st.pts[i + 2] });
        drawing.addStroke(s);
      }
      const best = (await drawing.getPrediction())[0]?.text ?? "";
      drawing.clear();
      return { kind: "text", text: best, latex: "" };
    } finally {
      rec.finish();
    }
  },
};

// ---------------------------------------------------------------- Claude

const CLAUDE_MODEL = "claude-opus-5-5";

const CLAUDE_SYSTEM = `You read a teacher's handwriting from a classroom whiteboard and transcribe it exactly.
The writing may be English, Bangla (বাংলা) or mathematics, or a mix.
- Keep the language and script as written. Write Bangla in Unicode Bangla script with correct conjuncts (যুক্তাক্ষর) and vowel signs.
- Do not correct spelling, translate, explain or solve anything.
- Keep line breaks as "\\n".
- If it is mathematics, set kind to "math", give "text" as plain Unicode (², ³, √, ×, ÷, −, ≤, ≥, ≠, π, θ, fractions as a/b) and "latex" as LaTeX without $ delimiters.
- Otherwise set kind to "text" and latex to "".
- If nothing is legible, return an empty text.`;

const RESULT_SCHEMA = {
  type: "object",
  properties: {
    kind: { type: "string", enum: ["text", "math"] },
    text: { type: "string" },
    latex: { type: "string" },
  },
  required: ["kind", "text", "latex"],
  additionalProperties: false,
};

export const claudeProvider: HandwritingProvider = {
  id: "claude",
  label: "Claude (your API key)",
  reads: "English, Bangla and maths, online",
  async available() {
    return claudeKey().startsWith("sk-ant-");
  },
  async recognize(ink, signal) {
    const apiKey = claudeKey();
    if (!apiKey) throw new Error("Add your Claude API key in Settings → AI");
    const png = await inkToPng(ink);
    // Loaded on first use only: most boards never need it.
    const { default: Anthropic } = await import("@anthropic-ai/sdk");
    // Browser-direct: the key is the teacher's own and goes only to api.anthropic.com.
    const client = new Anthropic({ apiKey, dangerouslyAllowBrowser: true, maxRetries: 1 });
    let res: Awaited<ReturnType<typeof client.beta.messages.create>>;
    try {
      res = await client.beta.messages.create(
      {
        model: CLAUDE_MODEL,
        max_tokens: 2048,
        // If a safety classifier declines, the API retries on a fallback model.
        betas: ["server-side-fallback-2026-07-01"],
        fallbacks: "default",
        // Transcription is quick work: low effort keeps it fast in class.
        output_config: { effort: "low", format: { type: "json_schema", schema: RESULT_SCHEMA } },
        system: CLAUDE_SYSTEM,
        messages: [
          {
            role: "user",
            content: [
              { type: "image", source: { type: "base64", media_type: "image/png", data: png.base64 } },
              { type: "text", text: "Transcribe this handwriting." },
            ],
          },
        ],
      },
      { signal },
      );
    } catch (e) {
      if (e instanceof Anthropic.AuthenticationError || e instanceof Anthropic.PermissionDeniedError) {
        throw new Error("Claude did not accept the API key. Check it in Settings → AI.");
      }
      if (e instanceof Anthropic.RateLimitError) throw new Error("Claude is busy right now. Try again in a moment.");
      if (e instanceof Anthropic.APIConnectionError) throw new Error("No connection to api.anthropic.com.");
      throw e;
    }
    if (res.stop_reason === "refusal") throw new Error("Claude declined to read this image");
    if (res.stop_reason === "max_tokens") throw new Error("The handwriting was too long to convert at once");
    const block = res.content.find((b) => b.type === "text");
    if (!block || block.type !== "text") throw new Error("No answer from Claude");
    const out = JSON.parse(block.text) as Partial<Recognition>;
    return {
      kind: out.kind === "math" ? "math" : "text",
      text: typeof out.text === "string" ? out.text : "",
      latex: typeof out.latex === "string" ? out.latex : "",
    };
  },
};

// ----------------------------------------------------------- Rupok Cloud

// TODO(rupok-cloud): a hosted recognizer behind the same interface, for
// schools without their own API key. Not built yet: `available()` stays
// false so the UI never offers it.
export const cloudProvider: HandwritingProvider = {
  id: "cloud",
  label: "Rupok Cloud",
  reads: "coming later",
  async available() {
    return false;
  },
  async recognize() {
    throw new Error("Rupok Cloud is not available yet");
  },
};

/** In preference order: Claude when the teacher added a key (it reads
 * Bangla and maths), then the offline Chrome recognizer. */
export const PROVIDERS: HandwritingProvider[] = [claudeProvider, chromeProvider, cloudProvider];

export async function pickProvider(): Promise<HandwritingProvider | null> {
  for (const p of PROVIDERS) if (await p.available()) return p;
  return null;
}

// ------------------------------------------------------------- placement

/** Where the text goes: over the ink, with a font size that matches the
 * handwriting's line height. */
export function textPlacement(bounds: [number, number, number, number], text: string) {
  const [x0, y0, x1, y1] = bounds;
  const lines = Math.max(1, text.split("\n").length);
  const h = Math.max(1, y1 - y0);
  const size = Math.min(400, Math.max(14, h / lines / 1.1));
  const y = y0 + (h - lines * size * 1.25) / 2;
  return { x: x0, y, w: Math.max(size, x1 - x0), size: Math.round(size * 10) / 10 };
}
