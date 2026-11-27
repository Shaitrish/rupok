// Board-app operations for the teacher's AI agent (docs/agent-capabilities.md):
// the parts of Rupok that are not the lesson document. Modes, the view,
// replay, celebrations, the current tool, the agent's own undo, PDF export,
// the Library, settings and the Smart Dock. live.ts calls `runBoardOp` for
// these and wraps the stamp operations with the device library ("My
// stamps") around the document op the core runs.
//
// The board enforces the rules before it gets here (live.ts): "Allow AI to
// edit live", Student view, and the agent's permission prompts in the chat.
// Settings and dock changes return an `undo`, which the AI chip's Undo runs.

import type { Controller } from "./controller";
import { type Mode, type ToolbarItem, DEFAULT_MODES, DOCK_PRESETS, TOOLBAR_ITEMS, CORE_TOOLS, SHAPES_PIN, dockWarnings, sanitizeMode, type DockConfig } from "./modes";
import { type Prefs, isView, mode, modeConfig, pages, prefs, setColor, setHlColor, setPrefs, setSizes, sizes, title, type Tool } from "./state";
import { celebrate, type CelebrateKind, freshStamps } from "./stamps";
import { agentSettings, setAgentSetting } from "./agentSettings";
import { applyPreset, placeable, savePreset, savedPresets } from "./components/DockEditor";
import { resetDock } from "./dock";
import { layout } from "./layout";
import { isBrush, setBrushOf } from "./brushes";
import { myStamps, resolveStamp } from "./stampLib";
import { deleteStamp as deleteLibStamp, saveStamp } from "./customStamps";

function b64(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export class AgentOpError extends Error {
  constructor(message: string, public data?: unknown) {
    super(message);
  }
}

export interface BoardOpResult {
  result: unknown;
  /** Puts a settings or dock change back (the AI chip's Undo). */
  undo?: () => void;
  /** The board changed what it shows (refresh). */
  changed?: boolean;
}

/** Ops this module handles (the rest go to the core's `live_apply`). */
export const BOARD_OPS = new Set([
  "set_mode", "zoom_to", "fit_page", "start_replay", "stop_replay", "celebrate", "set_tool", "undo", "export_pdf",
  "library_list", "library_open", "library_create_lesson", "library_move", "library_rename", "library_trash",
  "get_settings", "set_setting", "get_dock_layout", "set_dock_layout", "list_dock_items", "save_dock_preset", "apply_dock_preset",
]);

const fail = (m: string): never => {
  throw new AgentOpError(m);
};
const num = (v: unknown) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
const hexColor = (s: unknown): number | undefined => {
  if (typeof s !== "string" || !/^#?[0-9a-f]{6}$/i.test(s.trim())) return undefined;
  return ((parseInt(s.trim().replace("#", ""), 16) << 8) | 0xff) >>> 0;
};

/** Snapshot of the prefs a settings or dock change touches, for its undo. */
function prefsUndo(c: Controller, keys: (keyof Prefs)[]): () => void {
  const before: Partial<Prefs> = {};
  for (const k of keys) (before as any)[k] = structuredClone(prefs()[k]);
  return () => {
    setPrefs(before);
    c.applyPrefs();
    resetDock("prepare");
    resetDock("teach");
    c.updateInsets(mode() === "teach");
  };
}

const MODE_OF: Record<string, Mode> = { prepare: "prepare", teach: "teach", student: "view", view: "view" };

export async function runBoardOp(c: Controller, op: any, agent: string, lastAgentEdit: () => boolean): Promise<BoardOpResult> {
  switch (op.op) {
    // ------------------------------------------------------------ view
    case "set_mode": {
      const m = MODE_OF[String(op.mode)] ?? fail("mode must be prepare, teach or student");
      if (isView() && m !== "view") fail("The board is in Student view; only the teacher can leave it.");
      c.setMode(m);
      return { result: { mode: op.mode, page: c.board.page_index() + 1 }, changed: true };
    }
    case "fit_page":
      c.board.fit_page();
      return { result: { fitted: true }, changed: true };
    case "zoom_to": {
      const z = num(op.zoom) ?? 1;
      if (z < 0.1 || z > 8) fail("zoom must be between 0.1 and 8 (1 = the fitted slide)");
      const r = c.root.getBoundingClientRect();
      const fit = c.board.fit_zoom() || 1;
      const cam = c.board.camera();
      const zoom = fit * z;
      const cx = num(op.x) ?? cam[0] + r.width / 2 / cam[2];
      const cy = num(op.y) ?? cam[1] + r.height / 2 / cam[2];
      c.board.set_camera(cx - r.width / 2 / zoom, cy - r.height / 2 / zoom, zoom);
      return { result: { zoom: z, x: Math.round(cx), y: Math.round(cy) }, changed: true };
    }
    case "start_replay": {
      if (op.page !== undefined) {
        const n = num(op.page);
        if (!n || n < 1 || n > pages().length) fail(`page ${op.page} does not exist; the lesson has ${pages().length} page(s)`);
        c.board.goto_page(n! - 1);
      }
      const speed = Math.min(8, Math.max(0.5, num(op.speed) ?? 2));
      const d = c.board.replay_start();
      c.board.replay_play(true, speed);
      return { result: { page: c.board.page_index() + 1, duration_s: Math.round(d / 100) / 10, speed }, changed: true };
    }
    case "stop_replay":
      c.board.replay_stop();
      return { result: { stopped: true }, changed: true };
    case "celebrate": {
      const k = (op.kind ?? "stars") as CelebrateKind;
      if (!["stars", "confetti", "hearts"].includes(k)) fail("kind must be stars, confetti or hearts");
      celebrate(k);
      return { result: { celebrated: k } };
    }
    case "set_tool": {
      const tools: Tool[] = ["pen", "highlighter", "eraser", "lasso", "laser", "text", "shape", "stamp", "hand", "interact"];
      const t = (op.tool === "select" ? "lasso" : op.tool) as Tool;
      if (!tools.includes(t)) fail(`tool must be one of ${tools.join(", ")}`);
      if (isView() && t !== "hand" && t !== "interact") fail("Student view only pans and uses Smart Objects");
      const kind = t === "highlighter" ? "highlighter" : t === "shape" ? "shape" : "pen";
      if (op.brush !== undefined) {
        if (!isBrush(op.brush)) fail("brush must be a pen style such as pen, ballpoint, fountain, marker, pencil, chalk, brush, dashed, dotted, neon or rainbow");
        setBrushOf(kind, op.brush);
      }
      if (op.color !== undefined) {
        const col = hexColor(op.color) ?? fail("color must look like #rrggbb");
        if (t === "highlighter") setHlColor(col);
        else setColor(col);
        c.board.set_color(col);
      }
      if (op.width !== undefined) {
        const w = num(op.width);
        if (!w || w < 0.5 || w > 200) fail("width must be 0.5 to 200");
        setSizes({ ...sizes(), [t === "highlighter" ? "highlighter" : t === "eraser" ? "eraser" : "pen"]: w as number });
        c.board.set_width(w!);
      }
      c.setTool(t);
      c.applyPrefs();
      return { result: { tool: t } };
    }
    case "undo": {
      if (!lastAgentEdit()) fail("Your last change is no longer the newest one on the board (the teacher has changed it since), so it was not undone. Fix it with another edit, or ask the teacher.");
      c.board.undo();
      return { result: { undone: true }, changed: true };
    }

    // ---------------------------------------------------------- export
    case "export_pdf": {
      const n = pages().length;
      const sel = parsePages(String(op.pages ?? "all"), c.board.page_index(), n);
      const layout = ({ slides: "slides", "handout-2": "handout2", "handout-3": "handout3", handout2: "handout2", handout3: "handout3" } as const)[String(op.layout ?? "slides") as "slides"] ?? fail("layout must be slides, handout-2 or handout-3");
      const { exportPdf } = await import("./export-pdf");
      const bytes = await exportPdf(c, { pages: sel, layout, printFriendly: !!op.print_friendly, includeInk: op.ink !== false, title: title() }, () => {}, new AbortController().signal);
      let saved: string | null = null;
      if (op.download) {
        const { savePdfBytes } = await import("./platform");
        saved = await savePdfBytes(`${title().replace(/[\\/:*?"<>|]+/g, "-") || "lesson"}.pdf`, bytes);
      }
      return { result: { title: title(), pages: sel.length, layout: op.layout ?? "slides", bytes: bytes.length, saved_on_board: saved, pdf_base64: b64(bytes) } };
    }

    // ---------------------------------------------------------- library
    case "library_list":
    case "library_open":
    case "library_create_lesson":
    case "library_move":
    case "library_rename":
    case "library_trash":
      return library(c, op);

    // ---------------------------------------------------------- settings
    case "get_settings":
      return { result: { settings: agentSettings(), note: "change one with set_setting(key, value); AI keys, accounts, payments, storage and permissions are not listed: they are the teacher's alone" } };
    case "set_setting": {
      const { result, undo } = setAgentSetting(String(op.key), op.value);
      c.applyPrefs();
      c.updateInsets(false);
      return { result, undo, changed: true };
    }

    // -------------------------------------------------------------- dock
    case "get_dock_layout": {
      const m = dockMode(op.mode);
      const cfg = modeConfig(m);
      return { result: { mode: m, dock: cfg.dock, tools: cfg.tools, warnings: dockWarnings(m, cfg) } };
    }
    case "list_dock_items": {
      const items = [...TOOLBAR_ITEMS.map((t) => ({ id: t.id, label: t.label, key: t.key })), { id: SHAPES_PIN.id, label: SHAPES_PIN.label }]
        .filter((t) => t.id !== "rect" && t.id !== "ellipse" && t.id !== "line" && t.id !== "arrow" && t.id !== "shape")
        .map((t) => ({ ...t, always: CORE_TOOLS.includes(t.id as ToolbarItem), prepare: placeable("prepare").includes(t.id as any), teach: placeable("teach").includes(t.id as any) }));
      return {
        result: {
          items,
          positions: ["auto", "left", "right", "top", "bottom", "bottom-left", "bottom-right", "top-left", "top-right"],
          sizes: ["s", "m", "l"],
          presets: [...DOCK_PRESETS.map((p) => ({ name: p.name, hint: p.hint, builtin: true })), ...savedPresets().map((p) => ({ name: p.name, builtin: false }))],
          example: { position: "left", size: "l", groups: [{ label: "Ink", items: ["pen", "highlighter", "eraser"] }], split: { position: "right", groups: [{ items: ["undo", "redo"] }, { items: ["pager"] }] } },
          layout: layout(),
          styles: {
            dock: "The Smart Dock (Modern layout): one bar, groups separated.",
            islands: "The Classic layout: each group is an island along the bottom (or top) edge; the first at the left, the last at the right, the others in the middle. No top bar; the ☰ menu (menu) holds files, export, settings and help.",
          },
          classic_example: { style: "islands", position: "bottom", size: "l", groups: [{ label: "Menu", items: ["menu", "mode", "toolbox", "share", "zoom"] }, { label: "Tools", items: ["lasso", "pen", "eraser", "shapes", "hand", "recognize", "stamp", "undo", "redo"] }, { label: "Pages", items: ["addpage", "pager"] }] },
        },
      };
    }
    case "set_dock_layout": {
      const m = dockMode(op.mode);
      const layout = op.layout;
      if (!layout || typeof layout !== "object" || Array.isArray(layout)) fail("layout must be an object, e.g. {\"position\": \"left\", \"groups\": [{\"items\": [\"pen\", \"eraser\"]}]}");
      const known = new Set(["position", "orientation", "minimized", "pinned", "groups", "split", "size", "sizes", "large", "style"]);
      if ("style" in layout && layout.style !== "islands" && layout.style !== "dock") fail("style must be \"islands\" (the Classic layout: each group is an island) or \"dock\" (the Smart Dock)");
      const unknown = Object.keys(layout).filter((k) => !known.has(k));
      if (unknown.length) fail(`unknown layout keys: ${unknown.join(", ")} (allowed: ${[...known].join(", ")})`);
      const placeableIds = new Set<string>([...placeable(m), "rect", "ellipse", "line", "arrow", "shape"]);
      const items = [...(layout.groups ?? []), ...(layout.split?.groups ?? [])].flatMap((g: any) => (Array.isArray(g) ? g : g?.items ?? []));
      const bad = items.filter((x: unknown) => typeof x !== "string" || !placeableIds.has(x));
      if (bad.length) fail(`not dock buttons in ${m === "teach" ? "Teach" : "Prepare"}: ${bad.join(", ")} (list_dock_items shows the ids)`);
      const undo = prefsUndo(c, ["modes"]);
      const cur = modeConfig(m);
      const dock: Partial<DockConfig> = { ...cur.dock, ...layout };
      // A new grouping replaces the old one; a layout without groups keeps them.
      if ("groups" in layout && !layout.groups) delete dock.groups;
      const next = sanitizeMode(m, { chrome: cur.chrome, tools: cur.tools, dock });
      setPrefs({ modes: { ...prefs().modes, [m]: next } });
      resetDock(m);
      c.updateInsets(mode() === "teach");
      return { result: { mode: m, dock: next.dock, warnings: dockWarnings(m, next) }, undo, changed: true };
    }
    case "save_dock_preset": {
      const name = String(op.name ?? "").trim();
      if (!name) fail("name is required");
      if (DOCK_PRESETS.some((p) => p.name.toLowerCase() === name.toLowerCase())) fail(`“${name}” is a built-in layout; pick another name`);
      const modes = (op.modes ?? ["prepare", "teach"]).map((x: string) => dockMode(x)) as ("prepare" | "teach")[];
      const undo = prefsUndo(c, ["dockPresets"]);
      const p = savePreset(name, modes);
      return { result: { saved: p.name, modes: Object.keys(p.modes) }, undo };
    }
    case "apply_dock_preset": {
      const name = String(op.name ?? "").trim().toLowerCase();
      const p = [...DOCK_PRESETS, ...savedPresets()].find((x) => x.name.toLowerCase() === name)
        ?? fail(`no layout “${op.name}”; list_dock_items shows the layouts`);
      const undo = prefsUndo(c, ["modes", "hand"]);
      applyPreset(p!);
      return { result: { applied: p!.name, prepare: modeConfig("prepare").dock, teach: modeConfig("teach").dock }, undo, changed: true };
    }
  }
  throw new AgentOpError(`the board does not know \`${op.op}\``);
}

function dockMode(m: unknown): "prepare" | "teach" {
  if (m === undefined || m === null) return mode() === "teach" ? "teach" : "prepare";
  if (m === "prepare" || m === "teach") return m;
  return fail("mode must be prepare or teach (the Student view has a fixed dock)");
}

/** `all`, `current`, `3`, `2-5`, `1,3,5` → 0-based page indexes. */
export function parsePages(spec: string, current: number, n: number): number[] {
  const s = spec.trim().toLowerCase();
  if (s === "all" || s === "") return Array.from({ length: n }, (_, i) => i);
  if (s === "current") return [current];
  const out: number[] = [];
  for (const part of s.split(",")) {
    const m = /^(\d+)(?:\s*-\s*(\d+))?$/.exec(part.trim());
    if (!m) fail(`pages: use all, current, a page like 3 or a range like 2-5 (got “${spec}”)`);
    const a = Number(m![1]), b = Number(m![2] ?? m![1]);
    if (a < 1 || b > n || a > b) fail(`pages ${part.trim()} is outside 1–${n}`);
    for (let i = a; i <= b; i++) if (!out.includes(i - 1)) out.push(i - 1);
  }
  return out;
}

// --------------------------------------------------------------- library

async function library(c: Controller, op: any): Promise<BoardOpResult> {
  const session = await import("./library/session");
  const actions = await import("./library/actions");
  const s = session.store() ?? fail("the Library is not ready on this board (no storage chosen yet)");
  const str = (v: unknown, what: string) => (typeof v === "string" && v.trim() ? v.trim() : fail(`${what} is required`));
  switch (op.op) {
    case "library_list": {
      const entries = await s!.walk(typeof op.folder === "string" ? op.folder : "");
      const q = typeof op.query === "string" ? op.query.toLowerCase() : "";
      const list = entries
        .filter((e) => !q || (e.title ?? e.name).toLowerCase().includes(q))
        .slice(0, 500)
        .map((e) => ({ id: e.path, kind: e.kind, name: e.name, title: e.title, pages: e.pages, modified: e.modified ? new Date(e.modified).toISOString() : undefined }));
      return { result: { open: session.current()?.path ?? null, count: list.length, items: list } };
    }
    case "library_open": {
      const id = str(op.id, "id");
      const st = await s!.stat(id);
      if (!st || st.kind !== "lesson") fail(`no lesson “${id}” in the Library; library_list shows the ids`);
      const ok = await session.openLesson(id, { quiet: true });
      if (!ok) fail(`could not open “${id}”`);
      return { result: { opened: id, title: title(), pages: pages().length }, changed: true };
    }
    case "library_create_lesson": {
      const name = str(op.title, "title");
      const tpl = (op.template ?? "blank") as "blank" | "slides" | "lined" | "grid";
      if (!["blank", "slides", "lined", "grid"].includes(tpl)) fail("template must be blank, slides, lined or grid");
      const prev = session.current()?.path ?? null;
      const path = await actions.newLesson(typeof op.folder === "string" ? op.folder : "", name, tpl);
      if (!op.open && prev && prev !== path) await session.openLesson(prev, { quiet: true });
      return { result: { id: path, title: name, opened: !!op.open || !prev }, changed: true };
    }
    case "library_move": {
      const id = str(op.id, "id");
      const to = await s!.move(id, typeof op.folder === "string" ? op.folder : "");
      session.bump();
      return { result: { id: to, moved_from: id } };
    }
    case "library_rename": {
      const id = str(op.id, "id");
      const to = await actions.renameItem(id, str(op.name, "name"));
      return { result: { id: to, renamed_from: id } };
    }
    case "library_trash": {
      const id = str(op.id, "id");
      if (session.current()?.path === id) fail("that lesson is open on the board; open another lesson first");
      await s!.trash(id);
      session.bump();
      return { result: { trashed: id, note: "the teacher can restore it from the Library's trash" } };
    }
  }
  return fail("unknown library op");
}

// ----------------------------------------------------------------- stamps

/** Board-side half of the stamp ops: the device library joins the core op. */
export async function prepareStampOp(op: any): Promise<any> {
  switch (op.op) {
    case "add_stamp": {
      const lib = myStamps().find((s) => s.id === op.stamp);
      return lib ? { ...op, def: lib } : op;
    }
    case "update_stamp": {
      const lib = myStamps().find((s) => s.id === op.id);
      return lib ? { ...op, base: lib } : op;
    }
    case "delete_stamp":
      return { ...op, in_library: myStamps().some((s) => s.id === op.id) };
  }
  return op;
}

/** After the core ran a stamp op: keep the device library in step. */
export async function finishStampOp(op: any, result: any): Promise<any> {
  switch (op.op) {
    case "list_stamps":
      return { ...result, library: myStamps().map(({ html: _h, svg, ...s }) => ({ ...s, svg: svg && svg.length > 2000 ? `(${svg.length} bytes of SVG)` : svg, source: "library" })) };
    case "create_stamp":
    case "update_stamp": {
      const def = { ...(result.stamp ?? {}), html: op.html ?? myStamps().find((s) => s.id === result.id)?.html };
      try {
        await saveStamp(def);
        return { ...result, in_library: true };
      } catch (e: any) {
        return { ...result, in_library: false, library_error: String(e?.message ?? e) };
      }
    }
    case "delete_stamp": {
      const gone = await deleteLibStamp(op.id);
      return { ...result, deleted_from_library: gone };
    }
    case "add_stamp":
      if (result?.item_id) freshStamps.add(result.item_id);
      return { ...result, name: resolveStamp(op.stamp)?.name };
  }
  return result;
}

export const isStampOp = (name: string) => ["list_stamps", "create_stamp", "update_stamp", "delete_stamp", "add_stamp"].includes(name);

// Automated browsers only (the e2e suites): run a board op as an agent
// would, without a paired agent. Never exposed to a teacher's browser.
if (typeof navigator !== "undefined" && navigator.webdriver) {
  (window as any).rupokBoardOp = async (op: unknown) => {
    const { controller } = await import("./controller");
    return (await runBoardOp(controller, op, "e2e", () => false)).result;
  };
}
