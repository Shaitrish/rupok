// The Smart Dock: the board's drawing toolbar. A few core tools are always
// on it (pen, highlighter, eraser, laser, undo), plus what the mode pins
// (modes.ts → dock.pinned); the rest of the mode's tools live in ⋯. Colours
// and sizes are not inline: the pen and highlighter show theirs as a dot, the
// three recent colours sit next to them, and tapping the active tool again
// (or a long-press) opens the full palette, sizes and pressure.
//
// It shows what the moment needs: "Use object" and a step pager when a Smart
// Object is on screen, replay controls while a replay runs, text size while
// editing text. It drags anywhere (DockShell), minimises to a bubble, fades
// out of the way of ink drawn next to it, and comes to the teacher when
// summoned (dock.ts → summonDock).

import { type JSX, For, Match, Show, Switch, createEffect, createMemo, createSignal, on, onCleanup, onMount } from "solid-js";
import { controller } from "../controller";
import { CORE_TOOLS, TOOLBAR_ITEMS, type ModeConfig, type ToolbarItem } from "../modes";
import {
  type DomItem, HL_COLORS, PEN_COLORS, WIDTHS, type Tool, camera, canRedo, canUndo, color, editingText, hex, hlColor, isView, items,
  lessonOpened, mode, modeConfig, pageIndex, pages, prefs, replay, selectionIds, setColor, setHlColor, setPrefs, setSizes, setWidth,
  showToast, sizeKey, sizes, studentEntry, tool, width,
} from "../state";
import {
  type DockEdge, type DockPlace, dockFrame, dockMinimized, dockPlace, inkForBoard, isVertical, lastShape, placeAround, recentColors,
  registerDockSummon, resetDock, screenBucket, setDockMinimized, setDockPlace, setDockSide, setLastShape, useColor,
  hlOpacity, setHlOpacityStored,
} from "../dock";
import { DockPopover, DockShell, longPress } from "./DockShell";
import { isPhone, keyboard, pagesDrawer, setPagesDrawer, togglePagesDrawer } from "../viewport";
import { addPageLike } from "./Chrome";
import { Icon } from "./Icon";
import type { IconName } from "../icons.generated";
import { APP_SLUG, FILE_EXT } from "../brand";

const META = new Map(TOOLBAR_ITEMS.map((t) => [t.id, t]));
const INK_TOOLS: Tool[] = ["pen", "highlighter", "rect", "ellipse", "line", "arrow"];
const SHAPES: Tool[] = ["rect", "ellipse", "line", "arrow"];
/** Tools whose strokes the dock gets out of the way of. */
const DRAWING: Tool[] = ["pen", "highlighter", "eraser", "laser", "rect", "ellipse", "line", "arrow"];
const HL_WIDTHS = [14, 22, 34];
/** Eraser diameters, CSS px: from a fingertip to a board duster. */
const ERASER_WIDTHS = [32, 64, 120, 200];
const TEXT_SIZES = [24, 32, 40, 56, 72, 96, 128];

/** A touch screen (a smartboard, a tablet): Teach keeps finger-sized cells. */
const COARSE = typeof matchMedia !== "undefined" && matchMedia("(any-pointer: coarse)").matches;
/** Teach: the dock fades after a few idle seconds (hover, a key or a touch brings it back). */
const TEACH_IDLE_MS = 3000;
/** Ink drawn this close to the dock fades it (CSS px from its edge). */
const FADE_NEAR = 120;
/** It comes back this long after the pen lifts, or at once when a hovering pen or mouse comes this close. */
const FADE_HOLD_MS = 800;
const WAKE_NEAR = 56;

type Group = "ink" | "colors" | "create" | "point" | "context" | "history" | "nav" | "exit" | "end";
type Kind = "tool" | "colors" | "shapes" | "smartink" | "undo" | "redo" | "pager" | "replay" | "exit" | "context" | "more";
interface Entry { key: string; kind: Kind; group: Group; id?: ToolbarItem }
const GROUP: Partial<Record<ToolbarItem, Group>> = {
  pen: "ink", highlighter: "ink", eraser: "ink", smartink: "create", text: "create",
  lasso: "point", laser: "point", hand: "point", interact: "point", undo: "history", redo: "history", pager: "nav", replay: "nav", exit: "exit",
};

/** On a phone, what moves into ⋯ first when the dock is too long for the
 * screen ("colors:N" keeps N recent colours). Pen, eraser, the pager and ⋯
 * always stay; exit and undo go last (on the narrowest phones in Teach). */
const PHONE_DROP = ["redo", "hand", "laser", "colors:2", "lasso", "colors:1", "smartink", "interact", "replay", "text", "shapes", "highlighter", "colors:0", "exit", "undo"];
/** Phone dock metrics (Dock.css): 44px cells with no gap; a 1px separator with 1.5px margins. */
const PHONE_CELL = 44, PHONE_SEP = 4;

/** What sits on the dock, in the mode's tool order, and what goes in ⋯.
 * `room` (CSS px along the dock, phones only) moves tools into ⋯ until the
 * dock fits, so no tool hides in a scrolled strip. */
export function dockLayout(m: ModeConfig, core: boolean, allowMore: boolean, room = Infinity, cell = PHONE_CELL, sep = PHONE_SEP): { bar: Entry[]; overflow: ToolbarItem[]; colors: number } {
  const pinned = new Set(m.dock.pinned);
  const shapesOn = pinned.has("shapes");
  const bar: Entry[] = [];
  const overflow: ToolbarItem[] = [];
  let shapes = false;
  for (const t of m.tools) {
    if (SHAPES.includes(t as Tool) || (t === "smartink" && shapesOn)) {
      if (!shapesOn) overflow.push(t);
      else if (!shapes) {
        shapes = true;
        bar.push({ key: "shapes", kind: "shapes", group: "create" });
      }
      continue;
    }
    if ((core && CORE_TOOLS.includes(t)) || pinned.has(t)) {
      const kind: Kind = ["smartink", "undo", "redo", "pager", "replay", "exit"].includes(t) ? (t as Kind) : "tool";
      bar.push({ key: t, kind, id: t, group: GROUP[t] ?? "point" });
    } else overflow.push(t);
  }
  // Recent colours right after the ink tools.
  const lastInk = bar.map((e) => e.group).lastIndexOf("ink");
  if (m.chrome.colors && core && bar.some((e) => e.id === "pen" || e.id === "highlighter")) {
    bar.splice(lastInk + 1, 0, { key: "colors", kind: "colors", group: "colors" });
  }
  // The contextual section goes before history (else navigation, else the end).
  let at = bar.findIndex((e) => e.group === "history");
  if (at < 0) at = bar.findIndex((e) => e.group === "nav" || e.group === "exit");
  bar.splice(at < 0 ? bar.length : at, 0, { key: "context", kind: "context", group: "context" });
  if (overflow.length || allowMore) bar.push({ key: "more", kind: "more", group: "end" });
  let colors = 3;
  if (Number.isFinite(room)) {
    // The pager's "12 / 30" may run a little wider than a cell.
    const cells = (e: Entry) => (e.kind === "colors" ? colors : e.kind === "pager" ? 3.3 : e.kind === "context" ? 0 : 1);
    const length = () => {
      let n = 0; // (the minimise chevron moves into ⋯ on a phone)
      let seps = 0;
      bar.forEach((e, i) => {
        n += cells(e);
        if (i === 0 || e.kind === "context" || e.kind === "more") return;
        let j = i - 1;
        while (j >= 0 && bar[j].kind === "context") j--;
        if (j >= 0 && bar[j].group !== e.group) seps++;
      });
      // ⋯ appears once something has moved into it.
      if (overflow.length && !bar.some((e) => e.kind === "more")) n++;
      return n * cell + seps * sep;
    };
    for (const d of PHONE_DROP) {
      if (length() <= room) break;
      if (d.startsWith("colors:")) {
        colors = Math.min(colors, Number(d.slice(7)));
        const ci = bar.findIndex((e) => e.kind === "colors");
        if (colors === 0 && ci >= 0) bar.splice(ci, 1);
        continue;
      }
      const i = bar.findIndex((e) => (d === "shapes" ? e.kind === "shapes" : e.id === d));
      if (i < 0) continue;
      bar.splice(i, 1);
      if (d === "shapes") overflow.push(...(m.tools.filter((t) => SHAPES.includes(t as Tool) || t === "smartink") as ToolbarItem[]));
      else overflow.push(d as ToolbarItem);
    }
    if (overflow.length && !bar.some((e) => e.kind === "more")) bar.push({ key: "more", kind: "more", group: "end" });
  }
  return { bar, overflow: [...new Set(overflow)], colors };
}

function pick(t: Tool) {
  controller.setTool(t);
  if (INK_TOOLS.includes(t)) {
    controller.board.set_color(t === "highlighter" ? hlColor() : color());
    controller.board.set_width(sizes()[sizeKey(t)] ?? width());
  } else if (t === "eraser") {
    controller.board.set_width(sizes().eraser);
  }
  if (SHAPES.includes(t)) setLastShape(t);
}

const iconOf = (t: ToolbarItem): IconName => META.get(t)?.icon ?? "pen";
const labelOf = (t: ToolbarItem) => META.get(t)?.label ?? t;
const tipOf = (t: ToolbarItem, extra = "") => {
  const m = META.get(t);
  return `${m?.label ?? t}${m?.key ? `  ·  ${m.key}` : ""}${extra}`;
};
const same = (a: number, b: number) => a >>> 8 === b >>> 8;

/** Blur only where it is cheap: a hardware GPU on a machine with some headroom
 * (`rupok.dockBlur` = on/off overrides, for measuring). Measured cost: see
 * the Smart Dock report; while ink is drawn next to the dock it is off anyway. */
function blurIsCheap(): boolean {
  try {
    const forced = localStorage.getItem(`${APP_SLUG}.dockBlur`);
    if (forced === "on" || forced === "off") return forced === "on";
  } catch { /* storage unavailable */ }
  if (matchMedia("(prefers-reduced-transparency: reduce)").matches) return false;
  const nav = navigator as Navigator & { deviceMemory?: number };
  if ((nav.hardwareConcurrency ?? 4) <= 2 || (nav.deviceMemory ?? 4) <= 2) return false;
  try {
    const gl = document.createElement("canvas").getContext("webgl");
    if (!gl) return false;
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    const r = String(ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return !/swiftshader|llvmpipe|softpipe|software|basic render/i.test(r);
  } catch {
    return false;
  }
}

/** Keeps a section mounted while it animates its width (or height) to zero. */
function Reveal(props: { when: boolean; class?: string; children: JSX.Element }) {
  const [mounted, setMounted] = createSignal(props.when);
  const [open, setOpen] = createSignal(false);
  createEffect(
    on(
      () => props.when,
      (w) => {
        if (w) {
          setMounted(true);
          let r2 = 0;
          const r1 = requestAnimationFrame(() => (r2 = requestAnimationFrame(() => setOpen(true))));
          onCleanup(() => (cancelAnimationFrame(r1), cancelAnimationFrame(r2)));
        } else {
          setOpen(false);
          const t = window.setTimeout(() => setMounted(false), 260);
          onCleanup(() => clearTimeout(t));
        }
      },
    ),
  );
  return (
    <Show when={mounted()}>
      <div class={`dock-reveal ${props.class ?? ""}`} classList={{ open: open() }}>
        <div class="dock-reveal-in">{props.children}</div>
      </div>
    </Show>
  );
}

function pickFile(accept: string): Promise<File | undefined> {
  return new Promise((resolve) => {
    const i = document.createElement("input");
    i.type = "file";
    i.accept = accept;
    i.onchange = () => resolve(i.files?.[0]);
    i.click();
  });
}

type Pop = { kind: "pen" | "highlighter" | "eraser" | "shapes" | "more"; anchor: HTMLElement };

export default function Dock(props: { onSettings?: (section?: string) => void }) {
  const cfg = () => modeConfig();
  const core = () => mode() !== "view";
  // ------------------------------------------------------------ geometry
  const [viewport, setViewport] = createSignal({ w: window.innerWidth, h: window.innerHeight });
  /** The room along the dock (px) and its cell sizes, so tools that do not
   * fit go into ⋯ instead of a scrolled strip. Phones: the whole edge.
   * Tablets: a bottom or top dock stays clear of the zoom chip on both sides
   * (it is centred), after its grip and minimise button. */
  const fitSpec = (): { room: number; cell: number; sep: number } => {
    const p = place(), v = vertical();
    const f = frame(p.edge, v, p.x);
    const { w, h } = viewport();
    const large = prefs().density === "large";
    if (isPhone()) return { room: (v ? h - f.top - f.bottom : w - f.left - f.right) - 8, cell: large ? 52 : PHONE_CELL, sep: PHONE_SEP };
    if (v) return { room: Infinity, cell: 0, sep: 0 };
    const teach = mode() === "teach";
    const tb = teach ? (large ? 48 : 40) : prefs().density === "compact" ? 40 : large ? 56 : 48;
    const pad = teach ? (large ? 6 : 4) : prefs().density === "compact" ? 4 : large ? 8 : 6;
    // Under 1100px the zoom chip floats above a bottom dock (styles.css), as on phones.
    const side = cfg().chrome.zoom && p.edge === "bottom" && w >= 1100 ? 12 + 136 + 8 : Math.max(f.left, f.right);
    const extras = 2 * (Math.max(40, tb - 8) + 2); // grip and minimise
    return { room: w - 2 * side - 2 * pad - extras, cell: tb + 2, sep: 15 };
  };
  const bucket = () => screenBucket(viewport().w, viewport().h);
  const [dragPlace, setDragPlace] = createSignal<DockPlace | null>(null);
  const place = () => dragPlace() ?? dockPlace(mode(), cfg(), bucket());
  const vertical = () => isVertical(place(), cfg().dock.orientation);
  const minimized = () => dockMinimized(mode(), cfg());
  // On a phone the pages panel is a drawer over the board: the dock ignores it.
  const pagesOn = () => prefs().showPages && cfg().chrome.pages && !isPhone();
  const frame = (edge: DockEdge, v: boolean, x?: number) => dockFrame(cfg(), edge, v, viewport().w, viewport().h, pagesOn(), x);
  // Compared by value: a stored mode config is re-parsed on every prefs
  // change, and the buttons must not be re-created (popovers anchor to them).
  const layout = createMemo(() => dockLayout(cfg(), core(), true, fitSpec().room, fitSpec().cell, fitSpec().sep), undefined, {
    equals: (a, b) => JSON.stringify(a) === JSON.stringify(b),
  });
  const setMin = (m: boolean) => {
    setPop(null);
    setDockMinimized(mode(), cfg(), m);
  };
  let el: HTMLElement | undefined;
  let lastSize = { w: 640, h: 60 };

  // The board keeps the docked edge clear ("fit page" centres in the rest).
  createEffect(
    on(
      (): "bottom" | "top" | "left" | "right" | "none" => (minimized() || place().edge === "float" ? "none" : place().edge as "bottom" | "top" | "left" | "right"),
      (side) => {
        setDockSide(side);
        // CSS keeps the zoom chip clear of a bottom dock on phones (styles.css).
        document.documentElement.dataset.dockSide = cfg().tools.length ? side : "none";
        if (controller.board) controller.updateInsets(false);
      },
    ),
  );

  // On-screen keyboard: text being typed stays above it and above the dock
  // (which the keyboard pushes up: dock.ts → dockFrame).
  createEffect(
    on([keyboard, editingText], ([kb, id]) => {
      if (!kb || !id) return;
      requestAnimationFrame(() => {
        const node = document.querySelector<HTMLElement>(`.item.text[data-item="${CSS.escape(id)}"]`);
        if (!node) return;
        const r = node.getBoundingClientRect();
        const limit = window.innerHeight - kb - (el?.offsetHeight ?? 52) - 24;
        if (r.bottom > limit) controller.board.pan(0, -Math.min(r.bottom - limit, Math.max(0, r.top - 80)));
      });
    }),
  );

  // ------------------------------------------------------------- popovers
  const [pop, setPop] = createSignal<Pop | null>(null);
  const toggle = (kind: Pop["kind"], anchor: HTMLElement) => setPop(pop()?.kind === kind ? null : { kind, anchor });
  createEffect(on([mode, () => !!replay(), () => !!editingText()], () => setPop(null), { defer: true }));

  // -------------------------------------------------------------- fading
  const [idle, setIdle] = createSignal(false);
  const [inking, setInking] = createSignal(false);
  let idleTimer: number | undefined;
  let fadeTimer: number | undefined;
  const wake = () => {
    setIdle(false);
    clearTimeout(idleTimer);
    if (mode() === "teach") idleTimer = window.setTimeout(() => setIdle(true), TEACH_IDLE_MS);
  };
  const unfade = () => {
    clearTimeout(fadeTimer);
    setInking(false);
  };
  createEffect(on(mode, wake));

  // -------------------------------------------------------------- summon
  const summon = (x: number, y: number) => {
    setPop(null);
    if (minimized()) setDockMinimized(mode(), cfg(), false);
    unfade();
    wake();
    const v = vertical();
    const size = el && !minimized() ? { w: el.offsetWidth, h: el.offsetHeight } : lastSize;
    // Next to the hand: centred on it, a little above the fingers.
    const p = placeAround(x, v ? y : y - size.h - 24, size, frame("float", v), v);
    setDockPlace(mode(), cfg(), bucket(), p);
  };

  onMount(() => {
    document.documentElement.dataset.dockBlur = blurIsCheap() ? "on" : "off";
    controller.board.set_highlighter_opacity(hlOpacity());
    // The engine starts with its own pen; make the ink match what the dock shows.
    if (tool() === "pen") {
      controller.board.set_color(color());
      controller.board.set_width(sizes().pen ?? width());
    }
    registerDockSummon(summon);
    const onResize = () => setViewport({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", onResize);

    let drawing: { id: number; type: string } | null = null;
    let rect: DOMRect | null = null;
    const dist = (x: number, y: number, r: DOMRect) => Math.hypot(Math.max(r.left - x, 0, x - r.right), Math.max(r.top - y, 0, y - r.bottom));
    const onBoard = (e: Event) => !!(e.target as Element | null)?.closest?.(".input-layer");

    // Teach: a finger double-tap on the board brings the dock to it, when
    // fingers don't write (Pan & zoom, or Auto once the pen has been used),
    // so two quick dots are never mistaken for it. Not on a Smart Object in
    // use (its frame takes the pointer; "interact" is skipped too).
    let penSeen = false;
    let tap: { x: number; y: number; t: number; id: number } | null = null;
    let lastTap: { x: number; y: number; t: number } | null = null;
    const fingersNavigate = () => prefs().touchMode === "navigate" || (prefs().touchMode === "auto" && penSeen);

    const down = (e: PointerEvent) => {
      if (e.pointerType === "pen" && onBoard(e)) penSeen = true;
      if (e.pointerType === "touch" && mode() === "teach" && onBoard(e) && tool() !== "interact" && fingersNavigate()) {
        tap = { x: e.clientX, y: e.clientY, t: e.timeStamp, id: e.pointerId };
      }
      if (!el || !onBoard(e)) return;
      rect = el.getBoundingClientRect();
      // The other hand touching the faded dock while a pen writes: wake it, don't draw.
      if (inking() && drawing && drawing.type !== "touch" && e.pointerType === "touch" && dist(e.clientX, e.clientY, rect) === 0) {
        e.stopPropagation();
        e.preventDefault();
        unfade();
        return;
      }
      if (!DRAWING.includes(tool()) || e.button !== 0) return;
      drawing = { id: e.pointerId, type: e.pointerType };
      clearTimeout(fadeTimer);
      if (dist(e.clientX, e.clientY, rect) < FADE_NEAR) setInking(true);
    };
    const move = (e: PointerEvent) => {
      if (drawing && e.pointerId === drawing.id) {
        if (!inking() && rect && dist(e.clientX, e.clientY, rect) < FADE_NEAR) setInking(true);
        return;
      }
      if (e.buttons === 0 && e.pointerType !== "touch") {
        // Hovering (mouse, or a pen in range) counts as activity; drawing does not.
        wake();
        if (inking() && !drawing && el && dist(e.clientX, e.clientY, el.getBoundingClientRect()) < WAKE_NEAR) unfade();
      }
    };
    const up = (e: PointerEvent) => {
      if (tap && e.pointerId === tap.id) {
        const t = tap;
        tap = null;
        if (e.type === "pointerup" && e.timeStamp - t.t < 260 && Math.hypot(e.clientX - t.x, e.clientY - t.y) < 12) {
          if (lastTap && e.timeStamp - lastTap.t < 360 && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 40) {
            lastTap = null;
            const [x, y] = [e.clientX, e.clientY];
            queueMicrotask(() => summon(x, y));
          } else lastTap = { x: e.clientX, y: e.clientY, t: e.timeStamp };
        } else lastTap = null;
      }
      if (drawing && e.pointerId === drawing.id) {
        drawing = null;
        clearTimeout(fadeTimer);
        if (inking()) fadeTimer = window.setTimeout(() => setInking(false), FADE_HOLD_MS);
      }
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerup", up, true);
    window.addEventListener("pointercancel", up, true);
    window.addEventListener("keydown", wake);
    wake();
    onCleanup(() => {
      registerDockSummon(null);
      clearTimeout(idleTimer);
      clearTimeout(fadeTimer);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("pointercancel", up, true);
      window.removeEventListener("keydown", wake);
    });
  });

  // ---------------------------------------------------- contextual state
  const darkBoard = () => ((pages()[pageIndex()]?.bg_color ?? 0xffffffff) >>> 8) < 0x808080;
  const inkKind = () => (tool() === "highlighter" ? "highlighter" : "pen") as "pen" | "highlighter";
  const activeColor = () => (tool() === "highlighter" ? hlColor() : color());
  let lastInk: Tool = "pen";
  createEffect(() => {
    const t = tool();
    if (t === "pen" || t === "highlighter" || SHAPES.includes(t)) lastInk = t;
    if (SHAPES.includes(t)) setLastShape(t);
  });
  const pickColor = (c: number) => {
    if (!INK_TOOLS.includes(tool())) pick(lastInk);
    const k = inkKind();
    if (k === "highlighter") setHlColor(c);
    else setColor(c);
    controller.board.set_color(c);
    useColor(k, c);
  };

  /** Smart Objects on screen (current page, inside the viewport). */
  const visibleObjects = createMemo<DomItem[]>(() => {
    const [cx, cy, z] = camera();
    const { w, h } = viewport();
    const x1 = cx + w / z, y1 = cy + h / z;
    return items.filter((i) => i.kind === "object" && i.x < x1 && i.x + i.w > cx && i.y < y1 && i.y + i.h > cy);
  });
  /** The object a step pager drives: the selected one, else the first unfinished, else the first. */
  const stepObj = createMemo<DomItem | null>(() => {
    const objs = visibleObjects().filter((o) => (o.steps ?? 0) > 0);
    if (!objs.length) return null;
    const sel = selectionIds();
    return objs.find((o) => sel.includes(o.id)) ?? objs.find((o) => (o.step ?? 0) < (o.steps ?? 0)) ?? objs[0];
  });
  const interactPinned = () => layout().bar.some((e) => e.id === "interact");
  const showUse = () => visibleObjects().length > 0 && !interactPinned();
  const showSteps = () => !!stepObj() && !isView();
  const contextOn = () => showUse() || showSteps();
  // Sticky copies, so the section keeps its content while it animates closed.
  const stickyUse = createMemo<boolean>((prev) => (contextOn() ? showUse() : prev), false);
  const stickyStep = createMemo<DomItem | null>((prev) => (contextOn() ? stepObj() ?? prev : prev), null);
  let toolBeforeUse: Tool = "pen";
  const toggleUse = () => {
    if (tool() === "interact") pick(isView() ? "hand" : toolBeforeUse);
    else {
      toolBeforeUse = tool();
      controller.setTool("interact");
    }
  };
  const step = (d: number) => {
    const o = stickyStep();
    if (o) controller.objects.setStep(o.id, (o.step ?? 0) + d);
  };

  // ------------------------------------------------------------ sizing
  const iconSize = () => {
    const d = prefs().density;
    if (isPhone()) return d === "large" ? 26 : 22; // 44px cells (52 large) for thumbs
    // Teach is compact, except on a touch board, where fingers need the room (Dock.css).
    if (mode() === "teach") return COARSE ? (d === "large" ? 28 : 24) : d === "large" ? 24 : 20;
    return d === "compact" ? 20 : d === "large" ? 28 : 24;
  };
  const dotFor = (t: "pen" | "highlighter" | "eraser") => {
    const w = sizes()[t] ?? (t === "pen" ? width() : 22);
    const list = t === "pen" ? WIDTHS : t === "highlighter" ? HL_WIDTHS : ERASER_WIDTHS;
    let i = list.findIndex((x) => x >= w);
    if (i < 0) i = list.length - 1;
    return 6 + i * 2;
  };

  // ------------------------------------------------------------- render
  const label = () => (isView() ? "Lesson viewer" : mode() === "teach" ? "Teaching tools" : "Drawing tools");
  const sepBefore = (i: number) => {
    const bar = layout().bar;
    const e = bar[i];
    if (i === 0 || e.kind === "context" || e.kind === "more") return false;
    let j = i - 1;
    while (j >= 0 && bar[j].kind === "context") j--;
    return j >= 0 && bar[j].group !== e.group;
  };

  const ToolButton = (p: { id: Tool }) => {
    const on = () => tool() === p.id;
    const options = p.id === "pen" || p.id === "highlighter" || p.id === "eraser";
    let btn!: HTMLButtonElement;
    const lp = longPress(() => options && setPop({ kind: p.id as Pop["kind"], anchor: btn }));
    return (
      <button
        ref={btn}
        class="tool"
        data-tour={p.id === "pen" ? "pen" : undefined}
        classList={{ on: on(), "has-options": options }}
        title={tipOf(p.id, options ? "  ·  tap again for colours and sizes" : "")}
        aria-label={labelOf(p.id)}
        aria-pressed={on()}
        aria-haspopup={options ? "dialog" : undefined}
        onPointerDown={options ? lp.onPointerDown : undefined}
        onClick={() => {
          if (lp.consumed()) return;
          if (options && on()) toggle(p.id as Pop["kind"], btn);
          else {
            setPop(null);
            pick(p.id);
          }
        }}
      >
        <Icon name={iconOf(p.id)} size={iconSize()} active={on()} />
        <Show when={p.id === "pen" || p.id === "highlighter"}>
          <span
            class="dock-dot"
            style={{
              background: hex(p.id === "pen" ? color() : hlColor()),
              width: `${dotFor(p.id as "pen")}px`,
              height: `${dotFor(p.id as "pen")}px`,
            }}
          />
        </Show>
        <Show when={p.id === "eraser"}>
          <span class="dock-dot ring" style={{ width: `${dotFor("eraser")}px`, height: `${dotFor("eraser")}px` }} />
        </Show>
      </button>
    );
  };

  const Colors = () => (
    <For each={recentColors(inkKind()).slice(0, layout().colors)}>
      {(c) => {
        const shown = () => (inkKind() === "pen" ? inkForBoard(c, darkBoard()) : c);
        const on = () => INK_TOOLS.includes(tool()) && same(shown(), activeColor());
        let btn!: HTMLButtonElement;
        const lp = longPress(() => {
          if (!INK_TOOLS.includes(tool())) pick(lastInk);
          setPop({ kind: inkKind(), anchor: btn });
        });
        return (
          <button
            ref={btn}
            class="tool dock-color"
            data-tour="color"
            classList={{ on: on() }}
            title={`${hex(shown())}  ·  hold for all colours`}
            aria-label={`Colour ${hex(shown())}`}
            aria-pressed={on()}
            onPointerDown={lp.onPointerDown}
            onClick={() => !lp.consumed() && (setPop(null), pickColor(shown()))}
          >
            <span class="swatch-dot" style={{ background: hex(shown()) }} />
          </button>
        );
      }}
    </For>
  );

  const ShapesButton = () => {
    let btn!: HTMLButtonElement;
    const on = () => SHAPES.includes(tool());
    const shown = () => (on() ? tool() : lastShape());
    const lp = longPress(() => setPop({ kind: "shapes", anchor: btn }));
    return (
      <button
        ref={btn}
        class="tool has-caret"
        classList={{ on: on(), smart: prefs().smartShapes !== "off" }}
        title={`Shapes: ${labelOf(shown())}  ·  tap again for all shapes and Smart shapes`}
        aria-label="Shapes"
        aria-pressed={on()}
        aria-haspopup="dialog"
        onPointerDown={lp.onPointerDown}
        onClick={() => {
          if (lp.consumed()) return;
          if (on()) toggle("shapes", btn);
          else {
            setPop(null);
            pick(lastShape());
          }
        }}
      >
        <Icon name={iconOf(shown())} size={iconSize()} active={on()} />
        <span class="dock-caret" />
      </button>
    );
  };

  const SmartInkToggle = (p: { row?: boolean }) => {
    const on = () => prefs().smartShapes !== "off";
    const state = () => (prefs().smartShapes === "off" ? "Off" : prefs().smartShapes === "hold" ? "Hold to snap" : "Snap on lift");
    return (
      <button
        class={p.row ? "dock-row" : "tool"}
        classList={{ on: on() }}
        title={`Smart shapes: ${state().toLowerCase()}  ·  M`}
        aria-label="Smart shapes"
        aria-pressed={on()}
        onClick={() => controller.toggleSmartInk()}
      >
        <Icon name="smartink" size={p.row ? 20 : iconSize()} active={on()} />
        <Show when={p.row}>
          <span class="grow">Smart shapes</span>
          <span class="dock-state">{state()}</span>
        </Show>
      </button>
    );
  };

  const pagerSteps = () => showSteps();
  const Pager = () => (
    <>
      <button
        class="tool"
        disabled={pageIndex() === 0}
        onClick={() => (pagerSteps() ? controller.board.goto_page(pageIndex() - 1) : controller.prev())}
        title="Back  ·  ←"
        aria-label="Previous page"
      >
        <Icon name="prev" size={iconSize()} />
      </button>
      <button
        class="viewer-count"
        classList={{ on: pagesDrawer() }}
        aria-live="polite"
        title="Show all pages  ·  G"
        aria-label={`Page ${pageIndex() + 1} of ${pages().length}. Show all pages`}
        aria-expanded={pagesDrawer()}
        onClick={togglePagesDrawer}
      >
        {pageIndex() + 1} / {pages().length}
      </button>
      <button
        class="tool"
        disabled={pageIndex() >= pages().length - 1 && (mode() !== "teach" || pagerSteps())}
        onClick={() => (pagerSteps() ? controller.board.goto_page(pageIndex() + 1) : controller.next())}
        title="Next  ·  →"
        aria-label="Next page"
      >
        <Icon name="next" size={iconSize()} />
      </button>
      {/* Teach: a new page (same look) without leaving the slide. */}
      <Show when={mode() === "teach" && !isPhone()}>
        <button class="tool" title="New page after this one (same look)" aria-label="New page" onClick={() => addPageLike(false)}>
          <Icon name="plus" size={iconSize()} />
        </button>
      </Show>
    </>
  );

  const Context = () => (
    <Reveal when={contextOn()} class="dock-context">
      <div class="dock-sep" />
      <Show when={stickyUse()}>
        <button
          class="tool dock-use"
          classList={{ on: tool() === "interact" }}
          title="Use the Smart Object (click, drag, type in it)  ·  I"
          aria-label="Use object"
          aria-pressed={tool() === "interact"}
          onClick={toggleUse}
        >
          <Icon name="interact" size={iconSize()} active={tool() === "interact"} />
        </button>
      </Show>
      <Show when={stickyStep() && !isView()}>
        <button class="tool" disabled={(stickyStep()?.step ?? 0) <= 0} onClick={() => step(-1)} title="Previous step" aria-label="Previous step">
          <Icon name="prev" size={iconSize()} />
        </button>
        <span class="dock-steps" aria-live="polite" title={stickyStep()?.title}>
          {stickyStep()?.step ?? 0}/{stickyStep()?.steps}
        </span>
        <button
          class="tool"
          disabled={(stickyStep()?.step ?? 0) >= (stickyStep()?.steps ?? 0)}
          onClick={() => step(1)}
          title="Next step"
          aria-label="Next step"
        >
          <Icon name="next" size={iconSize()} />
        </button>
      </Show>
    </Reveal>
  );

  let moreBtn!: HTMLButtonElement;
  const Entry = (p: { e: Entry }) => (
    <Switch>
      <Match when={p.e.kind === "tool"}>
        <Show when={p.e.id === "interact"} fallback={<ToolButton id={p.e.id as Tool} />}>
          <button
            class="tool"
            classList={{ on: tool() === "interact" }}
            title={tipOf("interact")}
            aria-label={labelOf("interact")}
            aria-pressed={tool() === "interact"}
            onClick={toggleUse}
          >
            <Icon name="interact" size={iconSize()} active={tool() === "interact"} />
          </button>
        </Show>
      </Match>
      <Match when={p.e.kind === "colors"}>
        <Colors />
      </Match>
      <Match when={p.e.kind === "shapes"}>
        <ShapesButton />
      </Match>
      <Match when={p.e.kind === "smartink"}>
        <SmartInkToggle />
      </Match>
      <Match when={p.e.kind === "undo"}>
        <button class="tool" disabled={!canUndo()} title={tipOf("undo")} aria-label="Undo" onClick={() => controller.board.undo()}>
          <Icon name="undo" size={iconSize()} />
        </button>
      </Match>
      <Match when={p.e.kind === "redo"}>
        <button class="tool" disabled={!canRedo()} title={tipOf("redo")} aria-label="Redo" onClick={() => controller.board.redo()}>
          <Icon name="redo" size={iconSize()} />
        </button>
      </Match>
      <Match when={p.e.kind === "pager"}>
        <Pager />
      </Match>
      <Match when={p.e.kind === "replay"}>
        <Show
          when={isView()}
          fallback={
            <button class="tool" title="Replay this page stroke by stroke" aria-label="Replay" onClick={() => controller.board.replay_start()}>
              <Icon name="replay" size={iconSize()} />
            </button>
          }
        >
          <button class="btn solid viewer-replay" onClick={() => controller.board.replay_start()} title="Replay this page stroke by stroke">
            <Icon name="replay" size={iconSize()} /><span>Replay</span>
          </button>
        </Show>
      </Match>
      <Match when={p.e.kind === "exit"}>
        <button class="tool dock-exit" title={`Exit ${mode() === "teach" ? "Teach" : "mode"}  ·  Esc`} aria-label="Exit mode" onClick={() => controller.exitMode()}>
          <Icon name="exit" size={iconSize()} />
        </button>
      </Match>
      <Match when={p.e.kind === "context"}>
        <Context />
      </Match>
      <Match when={p.e.kind === "more"}>
        <button
          ref={moreBtn}
          class="tool"
          classList={{ on: pop()?.kind === "more" }}
          title="More tools"
          aria-label="More tools"
          aria-haspopup="menu"
          aria-expanded={pop()?.kind === "more"}
          onClick={() => toggle("more", moreBtn)}
        >
          <Icon name="more" size={iconSize()} />
        </button>
      </Match>
    </Switch>
  );

  // Replay: its controls take the place of the tools.
  const [speed, setSpeed] = createSignal(2);
  const fmt = (ms: number) => {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  };
  const r = () => replay() ?? { t: 0, d: 0, playing: false };
  const ReplayControls = () => (
    <Show when={replay()}>
      {
        <div class="dock-section dock-replay" role="group" aria-label="Replay">
          <button class="tool soft" onClick={() => controller.board.replay_play(!r().playing, speed())} aria-label={r().playing ? "Pause" : "Play"}>
            <Icon name={r().playing ? "pause" : "play"} size={iconSize()} active />
          </button>
          <Show when={!vertical()}>
            <input
              class="range dock-range"
              type="range"
              min="0"
              max={r().d}
              step="10"
              value={r().t}
              style={{ "--p": `${r().d ? (r().t / r().d) * 100 : 0}%` }}
              onInput={(e) => controller.board.replay_seek(Number(e.currentTarget.value))}
              aria-label="Replay position"
            />
          </Show>
          <span class="dock-time">{fmt(r().t)}<Show when={!vertical()}> / {fmt(r().d)}</Show></span>
          <button
            class="tool dock-speed"
            title="Replay speed"
            aria-label={`Speed ${speed()}×`}
            onClick={() => {
              const s = speed() >= 8 ? 1 : speed() * 2;
              setSpeed(s);
              controller.board.replay_play(r().playing, s);
            }}
          >
            {speed()}×
          </button>
          <div class="dock-sep" />
          <button class="tool" title="Close replay  ·  Esc" aria-label="Close replay" onClick={() => controller.board.replay_stop()}>
            <Icon name="close" size={iconSize()} />
          </button>
        </div>
      }
    </Show>
  );

  // Editing text: its size takes the place of the drawing tools.
  const editItem = () => items.find((i) => i.id === editingText());
  const resizeText = (dir: 1 | -1) => {
    const it = editItem();
    if (!it?.size) return;
    const cur = it.size;
    const next = dir > 0 ? TEXT_SIZES.find((s) => s > cur + 0.5) : [...TEXT_SIZES].reverse().find((s) => s < cur - 0.5);
    if (!next) return;
    const k = next / cur;
    const node = document.querySelector<HTMLElement>(`.item.text[data-item="${CSS.escape(it.id)}"]`);
    controller.board.resize_text(it.id, it.x, it.y, Math.round(it.w * k), Math.round((node?.offsetHeight ?? it.h) * k), next);
  };
  const keepFocus = (e: Event) => e.preventDefault();
  const TextControls = () => (
    <div class="dock-section dock-text" role="group" aria-label="Text" onPointerDown={keepFocus} onMouseDown={keepFocus}>
      <Icon name="text" size={iconSize()} />
      <button class="tool" title="Smaller text" aria-label="Smaller text" disabled={(editItem()?.size ?? 0) <= TEXT_SIZES[0]} onClick={() => resizeText(-1)}>
        <span class="dock-a small">A</span>
      </button>
      <span class="dock-steps" aria-live="polite">{Math.round(editItem()?.size ?? 0)}</span>
      <button class="tool" title="Larger text" aria-label="Larger text" disabled={(editItem()?.size ?? 0) >= TEXT_SIZES[TEXT_SIZES.length - 1]} onClick={() => resizeText(1)}>
        <span class="dock-a">A</span>
      </button>
      <div class="dock-sep" />
      <For each={recentColors("pen").map((c) => inkForBoard(c, darkBoard()))}>
        {(c) => {
          const on = () => (editItem()?.color ?? "").slice(0, 7).toLowerCase() === hex(c);
          return (
            <button
              class="tool dock-color"
              classList={{ on: on() }}
              title={`Text colour ${hex(c)}`}
              aria-label={`Text colour ${hex(c)}`}
              aria-pressed={on()}
              onClick={() => {
                const it = editItem();
                if (it) controller.board.set_text_color(it.id, c);
              }}
            >
              <span class="swatch-dot" style={{ background: hex(c) }} />
            </button>
          );
        }}
      </For>
      <div class="dock-sep" />
      <button class="btn soft dock-done" onClick={() => (document.activeElement as HTMLElement | null)?.blur()}>Done</button>
    </div>
  );

  // Bubble: the current tool and its colour.
  const bubbleTool = (): ToolbarItem => tool();
  const bubbleColor = () => (tool() === "highlighter" ? hlColor() : INK_TOOLS.includes(tool()) ? color() : null);

  const popTool = () => pop()?.kind as "pen" | "highlighter" | "eraser";
  const sizeOptions = () => (popTool() === "eraser" ? ERASER_WIDTHS : popTool() === "highlighter" ? HL_WIDTHS : WIDTHS);
  const setSize = (w: number) => {
    const t = popTool();
    if (t === "pen") setWidth(w);
    setSizes({ ...sizes(), [sizeKey(t)]: w });
    controller.board.set_width(w);
  };
  /** Clear page asks for a second tap: the time of the first. */
  const [clearArmed, setClearArmed] = createSignal(0);
  const armedNow = () => clearArmed() > 0;

  return (
    <>
      <Show when={cfg().tools.length > 0}>
        <DockShell
          place={place()}
          vertical={vertical()}
          orientation={cfg().dock.orientation}
          frame={frame}
          onPlace={(p, done) => {
            if (done) {
              setDockPlace(mode(), cfg(), bucket(), p);
              setDragPlace(null);
            } else setDragPlace(p);
          }}
          minimized={minimized()}
          onMinimized={setMin}
          class="toolbar dock"
          classList={{
            viewer: isView(),
            teach: mode() === "teach",
            idle: mode() === "teach" && idle() && !pop(),
            inking: inking(),
            lefty: prefs().hand === "left",
          }}
          label={label()}
          iconSize={iconSize()}
          bubbleLabel={`Open the dock (${labelOf(bubbleTool())})`}
          bubble={
            <>
              <Icon name={iconOf(bubbleTool())} size={24} active />
              <Show when={bubbleColor() !== null}>
                <span class="dock-dot" style={{ background: hex(bubbleColor()!) }} />
              </Show>
            </>
          }
          onActivity={() => {
            wake();
            unfade();
          }}
          onDragStart={() => setPop(null)}
          ref={(d) => {
            el = d;
            if (!minimized()) queueMicrotask(() => el && (lastSize = { w: el.offsetWidth || lastSize.w, h: el.offsetHeight || lastSize.h }));
          }}
        >
          <Switch
            fallback={
              <For each={layout().bar}>
                {(e, i) => (
                  <>
                    <Show when={sepBefore(i())}>
                      <div class="dock-sep" />
                    </Show>
                    <Entry e={e} />
                  </>
                )}
              </For>
            }
          >
            <Match when={replay()}>
              <ReplayControls />
            </Match>
            <Match when={editingText() && editItem()}>
              <TextControls />
            </Match>
          </Switch>
        </DockShell>
      </Show>

      <Show when={pop()} keyed>
        {(pp) => {
          const p = () => pp;
          return (
          <DockPopover
            anchor={p().anchor}
            dock={() => el}
            vertical={vertical()}
            hand={prefs().hand}
            label={p().kind === "more" ? "More tools" : p().kind === "shapes" ? "Shapes" : `${labelOf(p().kind as Tool)} options`}
            onClose={() => setPop(null)}
            class={`dock-pop-${p().kind}`}
          >
            <Switch>
              <Match when={p().kind === "pen" || p().kind === "highlighter"}>
                <div class="dock-pop-title">{p().kind === "pen" ? "Pen" : "Highlighter"}</div>
                <div class="dock-palette" role="group" aria-label="Colours">
                  <For each={p().kind === "pen" ? PEN_COLORS : HL_COLORS}>
                    {(c) => (
                      <button
                        class="tool dock-color"
                        classList={{ on: same(c, activeColor()) }}
                        title={hex(c)}
                        aria-label={`Colour ${hex(c)}`}
                        onClick={() => {
                          pickColor(c);
                          setPop(null);
                        }}
                      >
                        <span class="swatch-dot" style={{ background: hex(c) }} />
                      </button>
                    )}
                  </For>
                </div>
              </Match>
              <Match when={p().kind === "eraser"}>
                <div class="dock-pop-title">Eraser</div>
                <div class="seg dock-seg" role="radiogroup" aria-label="What the eraser removes">
                  <For each={[["partial", "Rub out"], ["stroke", "Whole strokes"]] as const}>
                    {([v, l]) => (
                      <button
                        role="radio"
                        aria-checked={prefs().eraserMode === v}
                        classList={{ on: prefs().eraserMode === v }}
                        title={v === "partial" ? "Rub out only what the eraser covers, like a duster" : "Erase every stroke the eraser touches"}
                        onClick={() => {
                          setPrefs({ eraserMode: v });
                          controller.applyPrefs();
                        }}
                      >
                        {l}
                      </button>
                    )}
                  </For>
                </div>
              </Match>
            </Switch>
            <Show when={(p().kind === "pen" || p().kind === "highlighter" || p().kind === "eraser") && cfg().chrome.sizes}>
              <div class="dock-sizes" role="group" aria-label="Sizes">
                <For each={sizeOptions()}>
                  {(w, i) => (
                    <button
                      class="tool"
                      classList={{ on: sizes()[sizeKey(popTool())] === w }}
                      title={`Size ${w}`}
                      aria-label={`Size ${w}`}
                      aria-pressed={sizes()[sizeKey(popTool())] === w}
                      onClick={() => setSize(w)}
                    >
                      <span class="size-dot" classList={{ ring: popTool() === "eraser" }} style={{ width: `${4 + i() * 4}px`, height: `${4 + i() * 4}px` }} />
                    </button>
                  )}
                </For>
              </div>
            </Show>
            <Show when={p().kind === "pen"}>
              <label class="dock-slider">
                <span>Pressure · {prefs().pressure < 0.8 ? "soft" : prefs().pressure > 1.2 ? "firm" : "normal"}</span>
                <input
                  class="slider"
                  type="range"
                  min="0.3"
                  max="2"
                  step="0.05"
                  value={prefs().pressure}
                  onInput={(e) => {
                    setPrefs({ pressure: Number(e.currentTarget.value) });
                    controller.applyPrefs();
                  }}
                />
              </label>
            </Show>
            <Show when={p().kind === "highlighter"}>
              <label class="dock-slider">
                <span>Opacity · {Math.round(hlOpacity() * 100)}%</span>
                <input
                  class="slider"
                  type="range"
                  min="0.2"
                  max="0.8"
                  step="0.02"
                  value={hlOpacity()}
                  onInput={(e) => setHlOpacity(Number(e.currentTarget.value))}
                />
              </label>
            </Show>
            <Show when={p().kind === "eraser" && !isView()}>
              <button
                class="dock-row danger"
                classList={{ armed: armedNow() }}
                aria-live="polite"
                onClick={() => {
                  const now = Date.now();
                  if (now - clearArmed() > 3000) {
                    setClearArmed(now);
                    window.setTimeout(() => clearArmed() === now && setClearArmed(0), 3000);
                    return;
                  }
                  setClearArmed(0);
                  controller.board.clear_page();
                  showToast("Page cleared. Undo brings it back.");
                  setPop(null);
                }}
              >
                <Icon name="broom" size={20} />
                <span class="grow">{armedNow() ? "Tap again to clear the page" : "Clear page"}</span>
              </button>
            </Show>
            <Show when={p().kind === "shapes"}>
              <div class="dock-pop-title">Shapes</div>
              <div class="dock-shapes" role="group" aria-label="Shapes">
                <For each={SHAPES}>
                  {(s) => (
                    <button
                      class="tool"
                      classList={{ on: tool() === s }}
                      title={tipOf(s)}
                      aria-label={labelOf(s)}
                      aria-pressed={tool() === s}
                      onClick={() => {
                        pick(s);
                        setPop(null);
                      }}
                    >
                      <Icon name={iconOf(s)} size={iconSize()} active={tool() === s} />
                    </button>
                  )}
                </For>
              </div>
              <Show when={cfg().tools.includes("smartink")}>
                <div class="dock-hr" />
                <SmartInkToggle row />
              </Show>
            </Show>
            <Show when={p().kind === "more"}>
              <div class="dock-menu" role="menu">
                <button
                  class="dock-row"
                  role="menuitemcheckbox"
                  aria-checked={pagesDrawer()}
                  onClick={() => {
                    setPop(null);
                    setPagesDrawer(!pagesDrawer());
                  }}
                >
                  <Icon name="pages" size={20} active={pagesDrawer()} /><span class="grow">Show pages</span>
                  <kbd>G</kbd>
                </button>
                <Show when={!isView()}>
                  <button
                    class="dock-row"
                    role="menuitem"
                    onClick={() => {
                      setPop(null);
                      addPageLike(false);
                    }}
                  >
                    <Icon name="plus" size={20} /><span class="grow">New page</span>
                  </button>
                </Show>
                <div class="dock-hr" />
                <For each={layout().overflow.filter((t) => t !== "pager")}>
                  {(t) => (
                    <button
                      class="dock-row"
                      role="menuitem"
                      classList={{ on: tool() === t || (t === "smartink" && prefs().smartShapes !== "off") }}
                      disabled={(t === "undo" && !canUndo()) || (t === "redo" && !canRedo())}
                      onClick={() => {
                        setPop(null);
                        if (t === "smartink") controller.toggleSmartInk();
                        else if (t === "undo") controller.board.undo();
                        else if (t === "redo") controller.board.redo();
                        else if (t === "replay") controller.board.replay_start();
                        else if (t === "exit") controller.exitMode();
                        else if (t === "interact") toggleUse();
                        else pick(t as Tool);
                      }}
                    >
                      <Icon name={iconOf(t)} size={20} active={tool() === t} />
                      <span class="grow">{labelOf(t)}</span>
                      <Show when={META.get(t)?.key}>
                        <kbd>{META.get(t)!.key}</kbd>
                      </Show>
                    </button>
                  )}
                </For>
                <Show when={layout().overflow.length}>
                  <div class="dock-hr" />
                </Show>
                <Show when={!isView()}>
                  <button
                    class="dock-row"
                    role="menuitem"
                    onClick={async () => {
                      setPop(null);
                      const f = await pickFile("image/*");
                      if (f) void controller.insertImage(f);
                    }}
                  >
                    <Icon name="image" size={20} /><span class="grow">Insert image</span>
                  </button>
                </Show>
                <button
                  class="dock-row"
                  role="menuitemcheckbox"
                  aria-checked={prefs().hand === "left"}
                  onClick={() => setPrefs({ hand: prefs().hand === "left" ? "right" : "left" })}
                >
                  <Icon name="pen" size={20} /><span class="grow">Left-handed</span>
                  <span class="dock-check" classList={{ on: prefs().hand === "left" }} />
                </button>
                <Show when={isPhone()}>
                  <button class="dock-row" role="menuitem" onClick={() => setMin(true)}>
                    <Icon name="down" size={20} /><span class="grow">Minimise the dock</span>
                  </button>
                </Show>
                <button
                  class="dock-row"
                  role="menuitem"
                  onClick={() => {
                    setPop(null);
                    resetDock(mode());
                  }}
                >
                  <Icon name="restart" size={20} /><span class="grow">Reset dock position</span>
                </button>
                <Show when={props.onSettings && cfg().chrome.settings}>
                  <button
                    class="dock-row"
                    role="menuitem"
                    onClick={() => {
                      setPop(null);
                      props.onSettings?.("modes.editor");
                    }}
                  >
                    <Icon name="settings" size={20} /><span class="grow">Customize dock…</span>
                  </button>
                </Show>
              </div>
            </Show>
          </DockPopover>
          );
        }}
      </Show>

      <Show when={isView() && studentEntry && !lessonOpened()}>
        <div class="view-empty" role="dialog" aria-label="Open a lesson">
          <div class="badge"><Icon name="open" size={24} active /></div>
          <h2>Open a lesson</h2>
          <p>Got a <code>.{FILE_EXT}</code> file from your teacher? Open it to flip through the pages and replay the class stroke by stroke.</p>
          <button class="btn solid" onClick={() => controller.openFile()}>
            <Icon name="open" /><span>Open lesson file</span>
          </button>
          <p class="hint">You can also drop the file anywhere on this page.</p>
        </div>
      </Show>
    </>
  );
}

// Highlighter opacity: stored per stroke by the engine, remembered by the dock.
function setHlOpacity(a: number) {
  setHlOpacityStored(a);
  controller.board.set_highlighter_opacity(hlOpacity());
}
