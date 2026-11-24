// What the teacher can attach to a chat message, and how it becomes context
// for the agent: @-mentions (@page 3, @selection, @object <name>, @lesson
// <name>, @file), dropped or pasted files, and the Rupok /commands.
// Each attachment resolves to `chat` context entries (docs/ai-agents.md §
// "`chat`"): board JSON, pictures, lesson bytes (the relay turns them into an
// outline), local files.

import type { Controller } from "./controller";
import { renderSnapshot } from "./snapshot";
import { store } from "./library/session";
import { pageIndex, pages, title } from "./state";
import { brandingFor, lessonBranding } from "./design";

export type CtxEntry = Record<string, unknown> & { kind: string };

export interface Attachment {
  id: string;
  kind: "page" | "selection" | "object" | "lesson" | "file" | "image";
  /** Chip label: "Page 3", "Selection (2)", "Ohm's law", "worksheet.pdf". */
  label: string;
  /** Small preview (data URL) for pictures. */
  thumb?: string;
  resolve(c: Controller, images: boolean): Promise<CtxEntry[]>;
}

let seq = 0;
const aid = () => `a${++seq}`;

function b64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

const op = (c: Controller, o: Record<string, unknown>) => c.board.live_apply(JSON.stringify(o), "Teacher");

async function snap(c: Controller, o: Record<string, unknown>): Promise<CtxEntry[]> {
  try {
    const s = await renderSnapshot(c, { width: 1024, ...o }, "Teacher");
    return [{ kind: "image", data: s.png, mime: "image/png" }];
  } catch {
    return [];
  }
}

export function pageAttachment(page: number): Attachment {
  return {
    id: aid(), kind: "page", label: `Page ${page}`,
    async resolve(c, images) {
      const out: CtxEntry[] = [{ kind: "page", uri: `rupok://board/page/${page}`, text: op(c, { op: "get_page", page }) }];
      return images ? [...out, ...(await snap(c, { page }))] : out;
    },
  };
}

export function selectionAttachment(count: number): Attachment {
  return {
    id: aid(), kind: "selection", label: `Selection (${count})`,
    async resolve(c, images) {
      const sel = JSON.parse(op(c, { op: "get_selection" }));
      const page = sel.page ?? c.board.page_index() + 1;
      const out: CtxEntry[] = [{ kind: "selection", uri: `rupok://board/page/${page}/selection`, text: JSON.stringify(sel) }];
      const ids = (sel.items ?? []).map((i: any) => i.id);
      return images && ids.length ? [...out, ...(await snap(c, { page, item_ids: ids }))] : out;
    },
  };
}

export function objectAttachment(page: number, item: any): Attachment {
  return {
    id: aid(), kind: "object", label: item.title || "Smart Object",
    async resolve(c, images) {
      const out: CtxEntry[] = [{ kind: "object", uri: `rupok://board/page/${page}/object/${item.id}`, text: JSON.stringify({ page, item }) }];
      return images ? [...out, ...(await snap(c, { page, item_ids: [item.id] }))] : out;
    },
  };
}

export function lessonAttachment(path: string, name: string): Attachment {
  return {
    id: aid(), kind: "lesson", label: name,
    async resolve() {
      const lib = store();
      if (!lib) return [];
      const bytes = await lib.read(path);
      return [{ kind: "lesson", name, data: b64(bytes) }];
    },
  };
}

export async function fileAttachment(file: File): Promise<Attachment> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const data = b64(bytes);
  const mime = file.type || (file.name.endsWith(".rpk") ? "application/vnd.rupok.lesson+zip" : "application/octet-stream");
  const image = mime.startsWith("image/") && mime !== "image/svg+xml";
  if (file.name.endsWith(".rpk")) {
    const name = file.name.replace(/\.rpk$/, "");
    return { id: aid(), kind: "lesson", label: name, resolve: async () => [{ kind: "lesson", name, data }] };
  }
  return {
    id: aid(), kind: image ? "image" : "file", label: file.name,
    thumb: image ? `data:${mime};base64,${data}` : undefined,
    resolve: async () => [image ? { kind: "image", data, mime } : { kind: "file", name: file.name, mime, data }],
  };
}

/** Biggest file we attach (the agent gets it base64 in one message). */
export const MAX_FILE_BYTES = 12 << 20;

// ---------------------------------------------------------------- @-mentions

export interface Suggestion {
  /** What the teacher sees. */
  label: string;
  /** Second line. */
  hint?: string;
  /** Replaces the typed "@…" in the text. */
  insert: string;
  /** Becomes an attachment; `"file"` opens the file picker. */
  make: (() => Attachment | Promise<Attachment>) | "file";
}

let lessonsCache: { at: number; list: { path: string; name: string }[] } | null = null;
async function lessons(): Promise<{ path: string; name: string }[]> {
  if (lessonsCache && Date.now() - lessonsCache.at < 30_000) return lessonsCache.list;
  const lib = store();
  if (!lib) return [];
  try {
    const all = await lib.walk("");
    lessonsCache = { at: Date.now(), list: all.filter((e) => e.kind === "lesson").map((e) => ({ path: e.path, name: e.name })) };
  } catch {
    lessonsCache = { at: Date.now(), list: [] };
  }
  return lessonsCache.list;
}

const norm = (s: string) => s.toLowerCase().replace(/\s+/g, " ").trim();

/** Suggestions for "@<query>". */
export async function mentionSuggestions(c: Controller, query: string): Promise<Suggestion[]> {
  const q = norm(query);
  const out: Suggestion[] = [];
  const pages = (JSON.parse(c.board.pages_json()) as unknown[]).length;
  const cur = c.board.page_index() + 1;
  let sel: any = null;
  try { sel = JSON.parse(op(c, { op: "get_selection" })); } catch { /* none */ }
  const selCount = sel?.items?.length ?? 0;

  const pageNum = /^(?:page\s*)?(\d+)$/.exec(q)?.[1];
  if (pageNum) {
    const n = Number(pageNum);
    if (n >= 1 && n <= pages) out.push({ label: `page ${n}`, hint: n === cur ? "the page on screen" : `of ${pages}`, insert: `@page ${n}`, make: () => pageAttachment(n) });
  } else if (!q || "page".startsWith(q) || /^page\s*$/.test(q)) {
    out.push({ label: `page ${cur}`, hint: "the page on screen", insert: `@page ${cur}`, make: () => pageAttachment(cur) });
    for (let n = 1; n <= Math.min(pages, 8); n++) if (n !== cur) out.push({ label: `page ${n}`, insert: `@page ${n}`, make: () => pageAttachment(n) });
  }
  if (selCount && (!q || "selection".startsWith(q))) out.push({ label: "selection", hint: `${selCount} item(s) you lassoed`, insert: "@selection", make: () => selectionAttachment(selCount) });

  // "@obj" is the start of the word "object": list them all.
  const objQ = "object".startsWith(q) ? "" : q.replace(/^object\s*/, "");
  if (!q || "object".startsWith(q) || q.startsWith("object") || objQ.length > 1) {
    try {
      const outline = JSON.parse(op(c, { op: "get_outline" }));
      for (const p of outline.pages ?? []) {
        for (const it of p.items ?? []) {
          if (it.kind !== "object") continue;
          const t = it.title || "Smart Object";
          if (objQ && !norm(t).includes(objQ)) continue;
          out.push({ label: `object ${t}`, hint: `Smart Object on page ${p.page}`, insert: `@object ${t}`, make: () => objectAttachment(p.page, it) });
        }
      }
    } catch { /* no outline */ }
  }

  const lesQ = "lesson".startsWith(q) ? "" : q.replace(/^lesson\s*/, "");
  if (!q || "lesson".startsWith(q) || q.startsWith("lesson") || lesQ.length > 1) {
    for (const l of (await lessons()).filter((l) => !lesQ || norm(l.name).includes(lesQ)).slice(0, 6)) {
      out.push({ label: `lesson ${l.name}`, hint: l.path, insert: `@lesson ${l.name}`, make: () => lessonAttachment(l.path, l.name) });
    }
  }
  if (!q || "file".startsWith(q)) out.push({ label: "file…", hint: "a picture, PDF or .rpk from this computer", insert: "", make: "file" });
  return out.slice(0, 12);
}

// ---------------------------------------------------------------- /commands

export interface SlashCommand {
  name: string;
  description: string;
  hint?: string;
  /** "agent": the agent's own (sent as text); "rupok": handled here. */
  source: "agent" | "rupok";
}

/** What a Rupok command does with its arguments. */
export type CommandAction =
  | { prompt: string; attachPage?: boolean }
  | { local: "clear" | "new" }
  | { solve: string }
  | { error: string };

const LANGS: Record<string, string> = { bn: "Bangla (বাংলা)", en: "English", hi: "Hindi", ar: "Arabic", ur: "Urdu" };

export const RUPOK_COMMANDS: SlashCommand[] = [
  { name: "solve", description: "Solve what is on this page, step by step", hint: "optional: which problem", source: "rupok" },
  { name: "quiz", description: "Make a quiz from this page on a new page", hint: "number of questions", source: "rupok" },
  { name: "explain", description: "Explain this page simply, for the class", hint: "optional: for which class", source: "rupok" },
  { name: "translate", description: "Translate this page onto a new page", hint: "language, e.g. bn", source: "rupok" },
  { name: "design", description: "Design the page background, theme and branding", hint: "what you want, e.g. chalkboard, physics, our logo", source: "rupok" },
  { name: "stamp", description: "Make a custom stamp, with its look and behaviour (or logic)", hint: "what it should look like and do", source: "rupok" },
  { name: "dock", description: "Arrange the Smart Dock for you", hint: "e.g. pen and eraser on the left, undo on the right, bigger", source: "rupok" },
  { name: "clear", description: "Clear the chat view (the agent keeps its memory)", source: "rupok" },
  { name: "new", description: "Start a new chat", source: "rupok" },
];

export function runRupokCommand(name: string, args: string): CommandAction {
  const a = args.trim();
  switch (name) {
    case "solve": return { solve: a };
    case "quiz": {
      const n = Math.max(1, Math.min(20, Number(/\d+/.exec(a)?.[0] ?? 5)));
      return { prompt: `Make a ${n}-question quiz from this page on a new page after it, as a Smart Object with one question per build step and the answer revealed on Next.${a.replace(/\d+/, "").trim() ? ` Focus: ${a.replace(/\d+/, "").trim()}.` : ""}`, attachPage: true };
    }
    case "explain": return { prompt: `Explain what is on this page simply and briefly, as I would say it to ${a || "the class"}. Answer here in the chat; do not change the board.`, attachPage: true };
    case "translate": {
      const lang = LANGS[a.toLowerCase()] ?? (a || "Bangla (বাংলা)");
      return { prompt: `Translate this page into ${lang} on a new page right after it, keeping the same layout (same positions and sizes). Keep numbers and formulas as they are.`, attachPage: true };
    }
    case "design": return { prompt: designPrompt(a), attachPage: true };
    case "stamp": return { prompt: stampPrompt(a), attachPage: false };
    case "dock": return { prompt: dockPrompt(a), attachPage: false };
    case "clear": return { local: "clear" };
    case "new": return { local: "new" };
    default: return { error: `Unknown command /${name}` };
  }
}

/** `/design`: the teacher's wish, the current design as context, and the house rules. */
export function designPrompt(wish: string): string {
  const p = pages()[pageIndex()];
  const hex = (c: number) => "#" + (c >>> 8).toString(16).padStart(6, "0");
  const current = {
    lesson: title(), page: pageIndex() + 1, pages: pages().length,
    color: hex(p?.bg_color ?? 0xffffffff), pattern: p?.pattern ?? "plain",
    background_image: p?.background ? { fit: p.background.fit, opacity: p.background.opacity, dim: p.background.dim } : null,
    branding: brandingFor(p, lessonBranding()),
  };
  return [
    `Design the pages of this lesson: ${wish || "a calm, readable look that suits the topic"}.`,
    "Use the set_page_design tool (colour, pattern, a background drawn as SVG for the 1920×1080 page) and set_branding (a small logo in a corner, a header or footer line). Ask before changing every page if I did not say so.",
    "Rules: keep the middle of the page calm and low-contrast so handwriting stays readable; on a dark page the ink is light, on a light page dark; decoration goes to the edges; no text in the background image; keep any logo small (logo_width about 0.1) in a corner. Header and footer text may use {lesson}, {page}, {pages}, {date}, {teacher}, {school}, {subject}, {course} and {chapter}.",
    `The current design: ${JSON.stringify(current)}.`,
  ].join("\n");
}

/** `/stamp`: a custom stamp for the teacher's "My stamps", with the house rules. */
export function stampPrompt(wish: string): string {
  return [
    `Make me a custom classroom stamp: ${wish || "a cheerful one for a great answer"}.`,
    "Use create_stamp (call list_stamps first to see what exists). Artwork: simple, bold SVG (viewBox about 100×100, fill=\"currentColor\" for the main colour, no text in it), or an emoji, or a short label (text art).",
    "Behaviour without code: animation (pop, sparkle, bounce, spin, fade-in), a burst (stars, confetti, hearts, sparkles), counter, sound (chime, pop, ding, tada, clap, whoosh), fade_after seconds. Keep it celebratory but brief: one animation, at most one burst and one short sound.",
    "If it needs logic (a dice that rolls on tap, a timer, a tap counter), use art \"smart\" with html: a tiny Smart Object (read_object_spec), the whole body tappable, large and readable on a projector, offline, no network.",
    "Then place one with add_stamp on the current page so I can see it, and tell me its name in one line.",
  ].join("\n");
}

/** `/dock`: arrange the Smart Dock as the teacher asks. */
export function dockPrompt(wish: string): string {
  return [
    `Arrange my Smart Dock: ${wish || "make it comfortable for teaching on a big board"}.`,
    "Call list_dock_items and get_dock_layout first, then set_dock_layout (groups of button ids in order, position left/right/top/bottom, an optional split second dock, size s/m/l, large icons) for the mode I mean (teach or prepare; both if I did not say).",
    "Pen, highlighter, eraser, laser and undo always stay on a dock. Mention any warnings it returns. Offer to save it with save_dock_preset if I like it. Reply in one or two lines.",
  ].join("\n");
}

// ------------------------------------------- optional modules of other features

/** Solve-this (another feature): used by /solve when it is built. */
const solveModules = import.meta.glob("./solve/index.ts");
export async function solveApi(): Promise<{ solve?: (opts: { prompt?: string; source?: string }) => unknown } | null> {
  const load = Object.values(solveModules)[0];
  return load ? ((await load()) as any) : null;
}
