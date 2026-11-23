// Glue between the Rust board engine (WASM) and the Solid UI: render loop,
// pointer input, DOM-item sync, Smart Object bridge, files and autosave.

import { reconcile } from "solid-js/store";
import init, { Board } from "./wasm/rupok.js";
import wasmUrl from "./wasm/rupok_bg.wasm?url";
import {
  type DomItem, type PageInfo, type Tool,
  setCamera, setCanRedo, setCanUndo, setItems, setPageIndex, setPages, setReplay, setSelection, setStats,
  setTitle, setRevision, items, prefs, setPrefs, showToast, tool, setTool, setEditingText, revision, setSavedRevision, color, setColor,
  editingText, selectionIds, setSelectionIds, setSelectionMeta, studentEntry, isView, setLessonOpened, mode, setModeSignal, modeConfig,
  smartFamilies,
} from "./state";
import { type Mode, type ToolbarItem, hasTopBar } from "./modes";
import { dockSide } from "./dock";
import { cssPx, isPhone, safeArea } from "./viewport";
import { ObjectHost } from "./objects";
import { attachTrackpadPinch } from "./pinch";
import { autosaveLoad, autosaveStore } from "./storage";
import { APP_NAME, FILE_EXT, LESSON_MIME } from "./brand";
import { forgetLessonPath, openLessonFiles, openLessonInto, saveLessonBytes } from "./platform";

/** Clipboard JSON from `Board.export_items_json` starts with this. */
const CLIP_RE = /^\s*\{"rupok":"clipboard\/v1"/;
const CLIP_BOUNDS_RE = /^\s*\{"rupok":"clipboard\/v1","bounds":\[([^\]]*)\]/;
/** Offset of a duplicate or a repeated paste, world units. */
const PASTE_STEP = 24;

function editableTarget(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null;
  return !!el && (el.isContentEditable || el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT");
}

export class Controller {
  board!: Board;
  objects = new ObjectHost(this);
  canvas!: HTMLCanvasElement;
  root!: HTMLElement;
  world!: HTMLElement;
  private lastRev = -1;
  private lastPage = -1;
  private lastCam = "";
  private frames = 0;
  private fpsT = performance.now();
  private fps = 0;
  private idleSince = performance.now();
  private spaceHeld = false;
  private toolBeforeSpace: Tool | null = null;
  fileHandle: any = null;
  /** Library hooks (src/library/session.ts): save to the lesson's file, adopt opened files. */
  libraryHooks: {
    beforeLoad?(): Promise<void>;
    loaded?(file: File, bytes: Uint8Array): void;
    saved?(bytes: Uint8Array, rev: number): void;
  } = {};

  async init(root: HTMLElement, canvas: HTMLCanvasElement, world: HTMLElement) {
    this.root = root;
    this.canvas = canvas;
    this.world = world;
    await init({ module_or_path: wasmUrl });
    this.board = await Board.create(canvas, prefs().forceWebgl);
    this.applyPrefs();
    this.resize();
    this.updateInsets(true);
    new ResizeObserver(() => this.resize()).observe(root);
    // The student viewer never reads or writes the teacher's autosave.
    const saved = studentEntry ? null : await autosaveLoad();
    if (saved) {
      try {
        this.board.load(saved);
      } catch (e) {
        console.warn("autosave could not be restored", e);
      }
    }
    this.board.set_read_only(isView());
    document.documentElement.dataset.mode = mode();
    this.setTool(isView() ? "hand" : tool());
    this.sync(true);
    setSavedRevision(this.board.revision());
    this.loop();
    if (!studentEntry) {
      setInterval(() => this.autosave(), 3000);
      window.addEventListener("beforeunload", () => this.autosave());
    }
    // Clicking into a Smart Object to use it dismisses the selection frame.
    window.addEventListener("blur", () =>
      setTimeout(() => {
        // Only in "use objects": elsewhere objects ignore the pointer, so focus
        // moving into one was not the teacher's doing.
        if (tool() === "interact" && document.activeElement instanceof HTMLIFrameElement && selectionIds().length) this.board.select_json("[]");
      }, 0),
    );
  }

  applyPrefs() {
    const p = prefs();
    this.board.set_touch_mode(p.touchMode);
    this.board.set_smoothing(p.smoothing);
    this.board.set_pressure_gamma(p.pressure);
    this.board.set_smart_ink(p.smartShapes, p.scratchOut);
    this.board.set_smart_families(smartFamilies(p));
  }

  /** Toolbar / M key: Smart shapes off, or back to the last on-mode. */
  toggleSmartInk() {
    const p = prefs();
    const cur = p.smartShapes;
    const on = cur === "off";
    setPrefs(cur === "off" ? { smartShapes: p.smartLast } : { smartShapes: "off", smartLast: cur });
    this.applyPrefs();
    showToast(
      !on ? "Smart shapes off"
        : p.smartLast === "hold" ? "Smart shapes on: hold the pen still to snap a shape"
          : "Smart shapes on: shapes snap when you lift the pen",
    );
  }

  private lastSize = { w: 0, h: 0 };
  resize() {
    const r = this.root.getBoundingClientRect();
    const wasFit = !!this.fitCam && this.board.camera().join(",") === this.fitCam;
    const was = this.lastSize;
    this.lastSize = { w: r.width, h: r.height };
    this.board.resize(r.width, r.height, window.devicePixelRatio || 1);
    // Teach keeps the slide fitted to the screen (window resize, fullscreen).
    // Elsewhere the slide re-fits when the screen turns (portrait ⇄ landscape)
    // or when it was still fitted, but never while text is being typed (an
    // on-screen keyboard resizing the window must not move the page).
    const turned = was.w > 0 && was.w > was.h !== r.width > r.height;
    if (mode() === "teach") this.updateInsets(true);
    else if (was.w > 0 && (turned || wasFit) && !editingText()) this.updateInsets(true);
  }

  // ----------------------------------------------------------------- modes

  private modeBeforeTeach: Mode = "prepare";
  private toolBeforeView: Tool = "pen";

  /** Switch mode: chrome and toolbar follow `modeConfig`, the Student view is
   * read-only, and Teach fits the slide to the screen. */
  setMode(m: Mode) {
    const prev = mode();
    if (m === prev) return;
    if (m === "teach") this.modeBeforeTeach = prev;
    setModeSignal(m);
    document.documentElement.dataset.mode = m;
    this.board.set_read_only(m === "view");
    // Teach: the slide touches the screen edges (no fit margin, no insets).
    this.board.set_fit_margin(m === "teach" ? 0 : 0.03);
    if (this.board.replay_state().length) this.board.replay_stop();
    // Keep the current tool only if the new mode's toolbar has it.
    const tools = modeConfig(m).tools;
    if (m === "view") this.toolBeforeView = tool();
    // Coming back from the Student view, pick up the tool used before it.
    const want = prev === "view" ? this.toolBeforeView : tool();
    if (m === "view") this.setTool("hand");
    else if (!tools.includes(want)) this.setTool((tools.find((t) => this.isDrawingTool(t)) as Tool) ?? "pen");
    else this.setTool(want);
    this.updateInsets(true);
  }

  /** Leave Teach (Esc / exit button) to the mode it was entered from. */
  exitMode() {
    this.setMode(mode() === "teach" ? (this.modeBeforeTeach === "teach" ? "prepare" : this.modeBeforeTeach) : "prepare");
  }

  private isDrawingTool(t: ToolbarItem): boolean {
    return !["undo", "redo", "pager", "replay", "exit", "smartink"].includes(t);
  }

  /** In Teach, a zoom or pan snaps back to the fitted slide once released
   * ("fit lock"); showing the zoom chip in Teach turns the lock off. */
  private pointersDown = 0;
  private lastNav = 0;
  private fitCam = "";
  private snapBack(now: number) {
    if (mode() !== "teach" || modeConfig().chrome.zoom || this.pointersDown > 0 || now - this.lastNav < 700) return;
    const cam = this.board.camera().join(",");
    if (this.fitCam && cam !== this.fitCam) {
      this.board.fit_page();
      this.fitCam = this.board.camera().join(",");
    }
  }

  // ------------------------------------------------------------------ loop

  private loop = () => {
    const drew = this.board.frame();
    this.sync(false);
    const now = performance.now();
    this.snapBack(now);
    if (drew) {
      this.frames++;
      this.idleSince = now;
    } else if (now - this.idleSince > 1500) {
      if (this.board.maintain()) this.idleSince = now;
    }
    if (now - this.fpsT > 500) {
      this.fps = Math.round((this.frames * 1000) / (now - this.fpsT));
      this.frames = 0;
      this.fpsT = now;
      if (prefs().showStats) {
        const s = JSON.parse(this.board.stats_json());
        setStats({ backend: s.backend, strokes: s.strokes, frameMs: s.frameMs, fps: this.fps });
      }
      const r = this.board.replay_state();
      setReplay(r.length ? { t: r[0], d: r[1], playing: r[2] > 0 } : null);
    }
    requestAnimationFrame(this.loop);
  };

  /** Pull engine state into the UI when the camera or document changed. */
  sync(force: boolean) {
    const cam = this.board.camera();
    const key = cam.join(",");
    if (force || key !== this.lastCam) {
      this.lastCam = key;
      const [x, y, z] = cam;
      this.world.style.transform = `scale(${z}) translate(${-x}px, ${-y}px)`;
      this.world.style.setProperty("--zoom", String(z));
      setCamera([x, y, z]);
    }
    const rev = this.board.revision();
    if (force || rev !== this.lastRev) {
      // A page change refits in the engine: remember Teach's fitted camera.
      if (mode() === "teach" && this.board.page_index() !== this.lastPage) this.fitCam = key;
      this.lastPage = this.board.page_index();
      this.lastRev = rev;
      setRevision(rev);
      const list: DomItem[] = JSON.parse(this.board.dom_items_json());
      setItems(reconcile(list, { key: "id" }));
      const pageList = JSON.parse(this.board.pages_json()) as PageInfo[];
      setPages(pageList);
      setPageIndex(this.board.page_index());
      // Teach paints the letterbox around the slide in the page colour.
      const bg = pageList[this.board.page_index()]?.bg_color ?? 0xffffffff;
      document.documentElement.style.setProperty("--board-page", "#" + (bg >>> 8).toString(16).padStart(6, "0"));
      const b = this.board.selection_bounds();
      setSelection(b.length ? Array.from(b) : null);
      setSelectionIds(JSON.parse(this.board.selection_json()) as string[]);
      setSelectionMeta(JSON.parse(this.board.selection_meta_json()));
      setCanUndo(this.board.can_undo());
      setCanRedo(this.board.can_redo());
      setTitle(this.board.title());
      this.objects.syncItems(list);
    }
  }

  // ----------------------------------------------------------------- tools

  setTool(t: Tool) {
    // Student view: only panning and using Smart Objects.
    if (isView() && t !== "interact") t = "hand";
    // Leaving "use objects": take keyboard focus back from any object iframe so
    // shortcuts and the clicker reach the board again.
    if (t !== "interact" && document.activeElement instanceof HTMLIFrameElement) document.activeElement.blur();
    setTool(t);
    if (t === "interact" || t === "text") {
      // A just-placed object stays selected in "use objects" so it can be
      // resized right away; the text tool starts fresh.
      this.board.set_tool("lasso");
      if (t === "text") this.board.select_json("[]");
    } else {
      this.board.set_tool(t);
    }
  }

  /** Tools in which a selection is shown and can be edited. */
  selecting(): boolean {
    const t = tool();
    return t === "lasso" || t === "interact";
  }

  // ------------------------------------------------------------- selection

  deleteSelection() {
    if (selectionIds().length) this.board.delete_selection();
  }

  duplicateSelection() {
    const ids = selectionIds();
    if (ids.length) this.board.duplicate_items(ids, PASTE_STEP, PASTE_STEP);
  }

  bringSelectionToFront() {
    const ids = selectionIds();
    if (ids.length) this.board.bring_to_front_many(ids);
  }

  groupSelection() {
    if (selectionIds().length < 2) return showToast("Select two or more items to group");
    if (this.board.group_selection()) showToast(`Grouped ${selectionIds().length} items`);
  }

  ungroupSelection() {
    const n = this.board.ungroup_selection();
    showToast(n ? `Ungrouped ${n === 1 ? "the group" : `${n} groups`}` : "Nothing to ungroup");
  }

  /** Merge selected ink into one unit; with box items, group instead. */
  mergeSelection() {
    const n = selectionIds().length;
    const r = this.board.merge_selection();
    if (r === "merged") showToast(`Merged ${n} strokes: they move and erase as one`);
    else if (r === "grouped") showToast("Only ink can be merged, so the selection was grouped instead");
    else showToast("Select two or more strokes to merge");
  }

  // ------------------------------------------------------------- clipboard

  /** In-app clipboard: always works, even when the system clipboard is blocked. */
  private clip: string | null = null;
  private pasteCount = 0;
  private pendingSystemCopy: string | null = null;

  /** Copy (or cut) the selection to the in-app clipboard and, when the
   * browser allows, the system clipboard as marked `text/plain`, so a paste
   * works in another tab or lesson. */
  copySelection(cut: boolean) {
    const ids = selectionIds();
    if (!ids.length) return;
    const json = this.board.export_items_json(ids);
    this.clip = json;
    // A paste after a cut lands in place; after a copy it is offset.
    this.pasteCount = cut ? -1 : 0;
    if (cut) this.board.delete_selection();
    // The native copy/cut event that follows the shortcut writes the system
    // clipboard; if it never comes, fall back to the async API.
    this.pendingSystemCopy = json;
    setTimeout(() => {
      if (this.pendingSystemCopy === json) {
        this.pendingSystemCopy = null;
        navigator.clipboard?.writeText(json).catch(() => {});
      }
    }, 0);
    const n = ids.length;
    showToast(`${cut ? "Cut" : "Copied"} ${n} item${n === 1 ? "" : "s"}`);
  }

  /** Paste clipboard JSON. Repeated pastes step down-right; items that would
   * land off-screen (e.g. from another tab) are centred in view instead. */
  paste(json: string) {
    if (isView()) return;
    this.pasteCount++;
    const step = Math.max(0, this.pasteCount) * PASTE_STEP;
    let dx = step, dy = step;
    const m = json.match(CLIP_BOUNDS_RE);
    if (m) {
      const [x0, y0, x1, y1] = m[1].split(",").map(Number);
      const [vx0, vy0, vx1, vy1] = this.visibleWorld();
      const onScreen = x1 + dx > vx0 && x0 + dx < vx1 && y1 + dy > vy0 && y0 + dy < vy1;
      if (!onScreen && [x0, y0, x1, y1].every(Number.isFinite)) {
        dx = (vx0 + vx1) / 2 - (x0 + x1) / 2;
        dy = (vy0 + vy1) / 2 - (y0 + y1) / 2;
      }
    }
    try {
      if (!this.selecting()) this.setTool("lasso");
      const ids = this.board.import_items_json(json, dx, dy);
      if (!ids.length) showToast("Nothing to paste");
    } catch (e: any) {
      showToast(`Could not paste: ${e?.message ?? e}`, 5000);
    }
  }

  /** The unobstructed part of the screen in world units: [x0, y0, x1, y1]. */
  visibleWorld(): [number, number, number, number] {
    const r = this.root.getBoundingClientRect();
    const [cx, cy, z] = this.board.camera();
    const [t, rt, b, l] = this.insets;
    return [cx + l / z, cy + t / z, cx + (r.width - rt) / z, cy + (r.height - b) / z];
  }

  private attachClipboard() {
    const handleCopy = (e: ClipboardEvent) => {
      if (!this.pendingSystemCopy || editableTarget(e.target)) return;
      e.clipboardData?.setData("text/plain", this.pendingSystemCopy);
      e.preventDefault();
      this.pendingSystemCopy = null;
    };
    window.addEventListener("copy", handleCopy);
    window.addEventListener("cut", handleCopy);
    // Registered before the app's own paste handler (files, HTML): a Rupok
    // clipboard is handled here and stops there.
    window.addEventListener("paste", (e) => {
      if (editableTarget(e.target)) return;
      if (isView()) {
        e.preventDefault();
        e.stopImmediatePropagation();
        return;
      }
      const text = e.clipboardData?.getData("text/plain") ?? "";
      const files = e.clipboardData?.files.length ?? 0;
      if (CLIP_RE.test(text)) {
        e.preventDefault();
        e.stopImmediatePropagation();
        this.paste(text);
      } else if (!files && !text.trim() && this.clip) {
        e.preventDefault();
        e.stopImmediatePropagation();
        this.paste(this.clip);
      }
    });
  }

  // ----------------------------------------------------------------- input

  attachInput(el: HTMLElement) {
    const pos = (e: PointerEvent) => {
      const r = this.root.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top] as const;
    };
    const flat = (list: PointerEvent[]) => {
      const out = new Float64Array(list.length * 4);
      const r = this.root.getBoundingClientRect();
      list.forEach((e, i) => {
        out[i * 4] = e.clientX - r.left;
        out[i * 4 + 1] = e.clientY - r.top;
        out[i * 4 + 2] = e.pressure;
        out[i * 4 + 3] = e.timeStamp;
      });
      return out;
    };

    el.addEventListener("pointerdown", (e) => {
      // A press on the board ends text editing (preventDefault below would
      // otherwise keep the focus, and the next keys, in the text).
      if (editingText() && document.activeElement instanceof HTMLElement) document.activeElement.blur();
      if (tool() === "text" && e.button === 0) {
        const [x, y] = pos(e);
        const id = this.board.add_text(x, y, "Text", 40);
        if (id) setEditingText(id);
        this.setTool("lasso");
        return;
      }
      el.setPointerCapture(e.pointerId);
      this.pointersDown++;
      const [x, y] = pos(e);
      this.board.pointer_down(e.pointerId, x, y, e.pressure, e.timeStamp, e.pointerType, e.button);
      e.preventDefault();
    });
    el.addEventListener("pointermove", (e) => {
      if (!el.hasPointerCapture(e.pointerId)) {
        this.hover(e);
        return;
      }
      const coalesced = e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
      const predicted = e.getPredictedEvents ? e.getPredictedEvents() : [];
      this.board.pointer_move(e.pointerId, flat(coalesced.length ? coalesced : [e]), flat(predicted));
      this.hover(e);
    });
    const released = () => {
      this.pointersDown = Math.max(0, this.pointersDown - 1);
      this.lastNav = performance.now();
    };
    const up = (e: PointerEvent) => {
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      released();
      this.board.pointer_up(e.pointerId);
    };
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", (e) => {
      released();
      this.board.pointer_cancel(e.pointerId);
    });
    el.addEventListener("contextmenu", (e) => e.preventDefault());
    el.addEventListener("dblclick", (e) => {
      // Double-click a text item to edit it.
      const hit = this.itemAt(e.clientX, e.clientY);
      if (hit?.kind === "text") setEditingText(hit.id);
      else if (hit?.kind === "object") this.objects.activate(hit.id);
    });

    this.root.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        this.lastNav = performance.now();
        const r = this.root.getBoundingClientRect();
        if (e.ctrlKey || e.metaKey) {
          this.board.zoom_at(Math.exp(-e.deltaY * 0.01), e.clientX - r.left, e.clientY - r.top);
        } else {
          this.board.pan(-e.deltaX, -e.deltaY);
        }
      },
      { passive: false },
    );
    // Safari and the macOS app send a trackpad pinch as gesture events.
    attachTrackpadPinch(this.root, (f, x, y) => {
      this.lastNav = performance.now();
      this.board.zoom_at(f, x, y);
    });
  }

  eraserCursor: ((x: number, y: number, show: boolean) => void) | null = null;
  private hover(e: PointerEvent) {
    if (this.eraserCursor) {
      const r = this.root.getBoundingClientRect();
      this.eraserCursor(e.clientX - r.left, e.clientY - r.top, tool() === "eraser");
    }
  }

  itemAt(clientX: number, clientY: number): DomItem | undefined {
    const r = this.root.getBoundingClientRect();
    const [cx, cy, z] = this.board.camera();
    const wx = cx + (clientX - r.left) / z;
    const wy = cy + (clientY - r.top) / z;
    return [...items].reverse().find((it) => wx >= it.x && wx <= it.x + it.w && wy >= it.y && wy <= it.y + it.h);
  }

  // -------------------------------------------------------------- keyboard

  attachKeys() {
    this.attachClipboard();
    window.addEventListener("keydown", (e) => {
      if (editableTarget(e.target)) return;
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      // Modes: F5 presents (Teach), Alt+1/2/3 picks a mode. By key position:
      // on a Mac, Option+1 types "¡", not "1".
      if (e.key === "F5") { e.preventDefault(); this.setMode("teach"); return; }
      const digit = /^Digit[1-3]$/.test(e.code) ? e.code.slice(5) : e.key;
      if (e.altKey && !mod && ["1", "2", "3"].includes(digit)) {
        e.preventDefault();
        this.setMode((["prepare", "teach", "view"] as const)[Number(digit) - 1]);
        return;
      }
      if (isView()) {
        this.viewKeys(e, mod, k);
        return;
      }
      if (mode() === "teach" && e.key === "Escape" && !selectionIds().length && !this.board.replay_state().length) {
        this.exitMode();
        return;
      }
      if (mod && k === "g") { e.preventDefault(); if (this.selecting()) e.shiftKey ? this.ungroupSelection() : this.groupSelection(); return; }
      if (mod && k === "e") { e.preventDefault(); if (this.selecting()) this.mergeSelection(); return; }
      if (mod && (k === "c" || k === "x") && !e.shiftKey && !e.altKey) {
        // Let the native copy/cut event follow so it can fill the system clipboard.
        if (this.selecting() && selectionIds().length) this.copySelection(k === "x");
        return;
      }
      if (mod && k === "d") { e.preventDefault(); if (this.selecting()) this.duplicateSelection(); return; }
      if (mod && k === "z") { e.preventDefault(); e.shiftKey ? this.board.redo() : this.board.undo(); return; }
      if (mod && k === "y") { e.preventDefault(); this.board.redo(); return; }
      if (mod && k === "s") { e.preventDefault(); this.saveFile(); return; }
      if (mod && k === "o") { e.preventDefault(); this.openFile(); return; }
      if (mod && k === "a") { e.preventDefault(); this.setTool("lasso"); this.board.select_all(); return; }
      if (mod) return;
      if (e.key === " " && !this.spaceHeld) {
        this.spaceHeld = true;
        this.toolBeforeSpace = tool();
        this.setTool("hand");
        e.preventDefault();
        return;
      }
      if (["PageDown", "ArrowRight", "ArrowDown"].includes(e.key)) { e.preventDefault(); this.next(); return; }
      if (["PageUp", "ArrowLeft", "ArrowUp"].includes(e.key)) { e.preventDefault(); this.prev(); return; }
      if (e.key === "Delete" || e.key === "Backspace") { if (this.selecting()) this.deleteSelection(); return; }
      if (e.key === "Escape") { this.board.select_json("[]"); if (this.board.replay_state().length) this.board.replay_stop(); return; }
      const map: Record<string, Tool> = {
        p: "pen", h: "highlighter", e: "eraser", v: "lasso", s: "lasso", l: "laser", i: "interact",
        t: "text", r: "rect", o: "ellipse", a: "arrow", n: "line",
      };
      // Tool keys work for the tools this mode's toolbar has (all in Prepare).
      if (map[k] && (mode() === "prepare" || modeConfig().tools.includes(map[k]))) { this.setTool(map[k]); return; }
      if (k === "m" && (mode() === "prepare" || modeConfig().tools.includes("smartink"))) { this.toggleSmartInk(); return; }
      if (k === "f") this.toggleFullscreen();
      if (k === "0") this.board.fit_page();
      if (k === "=" || k === "+") this.zoomBy(1.25);
      if (k === "-") this.zoomBy(0.8);
    });
    window.addEventListener("keyup", (e) => {
      if (isView()) return;
      if (e.key === " " && this.spaceHeld) {
        this.spaceHeld = false;
        if (this.toolBeforeSpace) this.setTool(this.toolBeforeSpace);
      }
    });
  }

  /** Student view keys: navigation, zoom, fullscreen, open. No editing. */
  private viewKeys(e: KeyboardEvent, mod: boolean, k: string) {
    if (mod) {
      if (k === "o") { e.preventDefault(); this.openFile(); }
      else if (k === "s" || k === "d") e.preventDefault();
      return;
    }
    if (["PageDown", "ArrowRight", "ArrowDown", " "].includes(e.key)) { e.preventDefault(); this.next(); return; }
    if (["PageUp", "ArrowLeft", "ArrowUp"].includes(e.key)) { e.preventDefault(); this.prev(); return; }
    if (e.key === "Escape") { if (this.board.replay_state().length) this.board.replay_stop(); else this.setTool("hand"); return; }
    if (k === "i") this.setTool(tool() === "interact" ? "hand" : "interact");
    if (k === "f") this.toggleFullscreen();
    if (k === "0") this.board.fit_page();
    if (k === "=" || k === "+") this.zoomBy(1.25);
    if (k === "-") this.zoomBy(0.8);
  }

  /** Change the board colour; flip a dark default pen to white on dark boards (and back). */
  setBoardColor(rgba: number) {
    const page = JSON.parse(this.board.pages_json())[this.board.page_index()];
    this.board.set_background(page?.pattern ?? "plain", rgba);
    const dark = (rgba >>> 8) < 0x808080;
    const c = color();
    if (dark && c === 0x1c2024ff) { setColor(0xffffffff); this.board.set_color(0xffffffff); }
    if (!dark && c === 0xffffffff) { setColor(0x1c2024ff); this.board.set_color(0x1c2024ff); }
  }

  zoomBy(f: number) {
    this.lastNav = performance.now();
    const r = this.root.getBoundingClientRect();
    this.board.zoom_at(f, r.width / 2, r.height / 2);
  }

  toggleFullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen().catch(() => {});
  }

  /** Screen margins covered by chrome: top, right, bottom, left (CSS px). */
  insets: [number, number, number, number] = [0, 0, 0, 0];
  /** Width taken by the docked AI chat panel (ChatPanel.tsx), and its side. */
  chatInset = 0;
  chatSide: "left" | "right" = "right";

  /** Screen margins covered by chrome, so fit/placement use the visible area. */
  updateInsets(fit = false) {
    const p = prefs();
    // Chrome sizes come from the CSS (phones use larger cells: viewport.ts).
    const tb = cssPx("--tb", p.density === "compact" ? 40 : p.density === "large" ? 56 : 48) + 2 * cssPx("--tb-pad", 6); // cell + 2 × padding
    const bar = cssPx("--bar", 48);
    const phone = isPhone();
    const safe = safeArea();
    const inset = phone ? 8 : 12, gap = phone ? 8 : 12;
    const cfg = modeConfig();
    let top = safe.top + inset + (hasTopBar(cfg) ? bar + gap : 0), bottom = safe.bottom + inset + gap, left = safe.left + inset, right = safe.right + inset;
    // The edge the Smart Dock is docked to (dock.ts); "none" when it floats or is minimised.
    const side = dockSide();
    const dock = cfg.tools.length > 0;
    if (!dock || side === "none") bottom = safe.bottom + inset;
    else if (side === "bottom") bottom += tb;
    else if (side === "top") top += tb + gap;
    else if (side === "left") left += tb + gap;
    else if (side === "right") right += tb + gap;
    // On a phone the pages panel is a drawer over the board: no room kept for it.
    if (p.showPages && cfg.chrome.pages && !phone) left = Math.max(left, inset + 176 + gap) + (dock && side === "left" ? tb + gap : 0);
    // The docked chat panel (Prepare only; it is an overlay in Teach).
    if (this.chatInset > 0 && this.chatSide === "right") right = Math.max(right, inset + this.chatInset);
    if (this.chatInset > 0 && this.chatSide === "left") left += this.chatInset;
    // Teach: chrome floats over the slide, which fills the screen.
    if (mode() === "teach") top = right = bottom = left = 0;
    this.insets = [top, right, bottom, left];
    this.board.set_insets(top, right, bottom, left);
    if (fit) {
      this.board.fit_page();
      this.fitCam = this.board.camera().join(",");
    }
  }

  /** Run before the presenter's Next; one returning true consumes it (e.g.
   * writing the next line of an AI solution, solve/index.ts). */
  nextHooks: (() => boolean)[] = [];

  /** Presenter "next": advance the topmost unfinished Smart Object, else next page. */
  next() {
    if (this.nextHooks.some((h) => h())) return;
    this.sync(false);
    // Student view cannot change object steps (read-only): pages only.
    const objs = isView() ? [] : [...items].filter((i) => i.kind === "object" && (i.steps ?? 0) > (i.step ?? 0));
    if (objs.length) {
      const o = objs[0];
      this.objects.setStep(o.id, (o.step ?? 0) + 1);
      return;
    }
    const n = JSON.parse(this.board.pages_json()).length;
    const i = this.board.page_index();
    if (i + 1 < n) this.board.goto_page(i + 1);
  }

  prev() {
    this.sync(false);
    const objs = isView() ? [] : [...items].filter((i) => i.kind === "object" && (i.step ?? 0) > 0).reverse();
    if (objs.length) {
      const o = objs[0];
      this.objects.setStep(o.id, (o.step ?? 0) - 1);
      return;
    }
    const i = this.board.page_index();
    if (i > 0) this.board.goto_page(i - 1);
  }

  // ----------------------------------------------------------------- files

  async autosave() {
    if (studentEntry) return;
    const rev = this.board.revision();
    if (rev === this.savedAutosaveRev) return;
    try {
      // The readable summary inside is refreshed at most once a minute here;
      // explicit saves, closing and exports call save().
      const bytes = this.board.save_autosave();
      await autosaveStore(bytes);
      this.savedAutosaveRev = rev;
      this.libraryHooks.saved?.(bytes, rev);
    } catch (e) {
      console.warn("autosave failed", e);
    }
  }
  private savedAutosaveRev = -1;

  async saveFile() {
    const bytes = this.board.save();
    const name = (this.board.title() || "lesson").replace(/[\\/:*?"<>|]+/g, "-") + `.${FILE_EXT}`;
    try {
      // A new or just-opened lesson (importFiles clears fileHandle) asks where to save.
      if (!this.fileHandle) forgetLessonPath();
      const saved = await saveLessonBytes(name, bytes);
      if (!saved) return;
      this.fileHandle = saved;
      setSavedRevision(revision());
      showToast(`Saved ${saved.split(/[\\/]/).pop()}`);
    } catch (e: any) {
      showToast(`Save failed: ${e?.message ?? e}`);
    }
  }

  async openFile() {
    await openLessonInto(this, await openLessonFiles());
  }

  async importFiles(files: File[]) {
    for (const f of files) {
      const name = f.name.toLowerCase();
      try {
        if (name.endsWith(`.${FILE_EXT}`)) {
          const bytes = new Uint8Array(await f.arrayBuffer());
          await this.libraryHooks.beforeLoad?.();
          this.board.load(bytes);
          this.fileHandle = null;
          setLessonOpened(true);
          showToast(`Opened ${f.name}`);
          this.libraryHooks.loaded?.(f, bytes);
        } else if (isView()) {
          showToast(`Open a .${FILE_EXT} lesson file to view it`);
        } else if (name.endsWith(".pdf")) {
          const { importPdf } = await import("./pdf");
          await importPdf(this, f);
        } else if (name.endsWith(".html") || name.endsWith(".htm")) {
          const html = await f.text();
          const m = html.match(/<title>([^<]*)<\/title>/i);
          const steps = Number(html.match(/data-rupok-steps=["']?(\d+)/)?.[1] ?? 0);
          this.addObject(html, m?.[1]?.trim() || f.name.replace(/\.html?$/i, ""), 1280, 720, steps);
        } else if (f.type.startsWith("image/")) {
          await this.insertImage(f);
        }
      } catch (e: any) {
        showToast(`Could not import ${f.name}: ${e?.message ?? e}`, 5000);
      }
    }
  }

  addObject(html: string, title: string, w: number, h: number, steps: number): string | null {
    try {
      const id = this.board.add_object_html(html, title, w, h, steps);
      this.setTool("interact");
      showToast(`Added “${title}”. Press I to use it, P to write on it.`);
      return id;
    } catch (e: any) {
      showToast(`Smart Object rejected:\n${e}`, 7000);
      return null;
    }
  }

  /** Replace items with a Smart Object at build step `step` with saved
   * `state`, as one undo step. */
  replaceWithObject(ids: string[], html: string, title: string, w: number, h: number, steps: number, step: number, state?: unknown): string | null {
    try {
      const id = this.board.replace_with_object_html(ids, html, title, w, h, steps, step, state === undefined ? undefined : JSON.stringify(state));
      this.setTool("interact");
      showToast(`“${title}” is live. Drag the curve or the sliders; Undo brings your sketch back.`);
      return id;
    } catch (e: any) {
      showToast(`Smart Object rejected:\n${e}`, 7000);
      return null;
    }
  }

  async addImageFile(f: File | Blob, maxW = 1280, maxH = 720) {
    const bytes = new Uint8Array(await f.arrayBuffer());
    const bmp = await createImageBitmap(f);
    const s = Math.min(1, maxW / bmp.width, maxH / bmp.height);
    const ext = (f.type.split("/")[1] || "png").replace("jpeg", "jpg").replace("svg+xml", "svg");
    const [w, h] = [Math.round(bmp.width * s), Math.round(bmp.height * s)];
    bmp.close();
    this.board.add_image(bytes, ext, w, h);
  }

  /** Insert an image the teacher picked or dropped: it lands selected, in
   * Select mode, ready to move or resize. */
  async insertImage(f: File | Blob) {
    if (!this.selecting()) this.setTool("lasso");
    await this.addImageFile(f);
  }

  assetUrls = new Map<string, string>();
  assetUrl(name: string): string {
    let u = this.assetUrls.get(name);
    if (!u) {
      const bytes = this.board.asset(name);
      if (!bytes) return "";
      const ext = name.split(".").pop() || "";
      const type = ext === "svg" ? "image/svg+xml" : ext === "jpg" ? "image/jpeg" : `image/${ext}`;
      u = URL.createObjectURL(new Blob([bytes as BlobPart], { type }));
      this.assetUrls.set(name, u);
    }
    return u;
  }
}

export const controller = new Controller();
