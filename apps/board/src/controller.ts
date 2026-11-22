// Glue between the Rust board engine (WASM) and the Solid UI: render loop,
// pointer input, DOM-item sync, Smart Object bridge, files and autosave.

import { reconcile } from "solid-js/store";
import init, { Board } from "./wasm/rupok.js";
import wasmUrl from "./wasm/rupok_bg.wasm?url";
import {
  type DomItem, type PageInfo, type Tool,
  setCamera, setCanRedo, setCanUndo, setItems, setPageIndex, setPages, setReplay, setSelection, setStats,
  setTitle, setRevision, items, prefs, showToast, tool, setTool, setEditingText, revision, setSavedRevision, color, setColor,
} from "./state";
import { ObjectHost } from "./objects";
import { autosaveLoad, autosaveStore } from "./storage";
import { APP_NAME, FILE_EXT, LESSON_MIME } from "./brand";

export class Controller {
  board!: Board;
  objects = new ObjectHost(this);
  canvas!: HTMLCanvasElement;
  root!: HTMLElement;
  world!: HTMLElement;
  private lastRev = -1;
  private lastCam = "";
  private frames = 0;
  private fpsT = performance.now();
  private fps = 0;
  private idleSince = performance.now();
  private spaceHeld = false;
  private toolBeforeSpace: Tool | null = null;
  fileHandle: any = null;

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
    const saved = await autosaveLoad();
    if (saved) {
      try {
        this.board.load(saved);
      } catch (e) {
        console.warn("autosave could not be restored", e);
      }
    }
    this.setTool(tool());
    this.sync(true);
    setSavedRevision(this.board.revision());
    this.loop();
    setInterval(() => this.autosave(), 3000);
    window.addEventListener("beforeunload", () => this.autosave());
  }

  applyPrefs() {
    const p = prefs();
    this.board.set_touch_mode(p.touchMode);
    this.board.set_smoothing(p.smoothing);
    this.board.set_pressure_gamma(p.pressure);
  }

  resize() {
    const r = this.root.getBoundingClientRect();
    this.board.resize(r.width, r.height, window.devicePixelRatio || 1);
  }

  // ------------------------------------------------------------------ loop

  private loop = () => {
    const drew = this.board.frame();
    this.sync(false);
    const now = performance.now();
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
      this.lastRev = rev;
      setRevision(rev);
      const list: DomItem[] = JSON.parse(this.board.dom_items_json());
      setItems(reconcile(list, { key: "id" }));
      setPages(JSON.parse(this.board.pages_json()) as PageInfo[]);
      setPageIndex(this.board.page_index());
      const b = this.board.selection_bounds();
      setSelection(b.length ? Array.from(b) : null);
      setCanUndo(this.board.can_undo());
      setCanRedo(this.board.can_redo());
      setTitle(this.board.title());
      this.objects.syncItems(list);
    }
  }

  // ----------------------------------------------------------------- tools

  setTool(t: Tool) {
    // Leaving "use objects": take keyboard focus back from any object iframe so
    // shortcuts and the clicker reach the board again.
    if (t !== "interact" && document.activeElement instanceof HTMLIFrameElement) document.activeElement.blur();
    setTool(t);
    if (t === "interact" || t === "text") {
      this.board.set_tool("lasso");
      this.board.select_json("[]");
    } else {
      this.board.set_tool(t);
    }
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
      if (tool() === "text" && e.button === 0) {
        const [x, y] = pos(e);
        const id = this.board.add_text(x, y, "Text", 40);
        if (id) setEditingText(id);
        this.setTool("lasso");
        return;
      }
      el.setPointerCapture(e.pointerId);
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
    const up = (e: PointerEvent) => {
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      this.board.pointer_up(e.pointerId);
    };
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", (e) => this.board.pointer_cancel(e.pointerId));
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
        const r = this.root.getBoundingClientRect();
        if (e.ctrlKey || e.metaKey) {
          this.board.zoom_at(Math.exp(-e.deltaY * 0.01), e.clientX - r.left, e.clientY - r.top);
        } else {
          this.board.pan(-e.deltaX, -e.deltaY);
        }
      },
      { passive: false },
    );
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
    window.addEventListener("keydown", (e) => {
      const t = e.target as HTMLElement;
      if (t && (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
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
      if (e.key === "Delete" || e.key === "Backspace") { this.board.delete_selection(); return; }
      if (e.key === "Escape") { this.board.select_json("[]"); if (this.board.replay_state().length) this.board.replay_stop(); return; }
      const map: Record<string, Tool> = {
        p: "pen", h: "highlighter", e: "eraser", v: "lasso", s: "lasso", l: "laser", i: "interact",
        t: "text", r: "rect", o: "ellipse", a: "arrow", n: "line",
      };
      if (map[k]) { this.setTool(map[k]); return; }
      if (k === "f") this.toggleFullscreen();
      if (k === "0") this.board.fit_page();
      if (k === "=" || k === "+") this.zoomBy(1.25);
      if (k === "-") this.zoomBy(0.8);
    });
    window.addEventListener("keyup", (e) => {
      if (e.key === " " && this.spaceHeld) {
        this.spaceHeld = false;
        if (this.toolBeforeSpace) this.setTool(this.toolBeforeSpace);
      }
    });
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
    const r = this.root.getBoundingClientRect();
    this.board.zoom_at(f, r.width / 2, r.height / 2);
  }

  toggleFullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen().catch(() => {});
  }

  /** Screen margins covered by chrome, so fit/placement use the visible area. */
  updateInsets(fit = false) {
    const p = prefs();
    const tb = p.density === "compact" ? 48 : p.density === "large" ? 72 : 60; // cell + 2 × padding
    const inset = 12, gap = 12;
    let top = inset + 48 + gap, bottom = inset + gap, left = inset, right = inset;
    if (p.toolbar === "bottom") bottom += tb;
    if (p.toolbar === "left") left += tb + gap;
    if (p.toolbar === "right") right += tb + gap;
    if (p.showPages) left = Math.max(left, inset + 176 + gap) + (p.toolbar === "left" ? tb + gap : 0);
    this.board.set_insets(top, right, bottom, left);
    if (fit) this.board.fit_page();
  }

  /** Presenter "next": advance the topmost unfinished Smart Object, else next page. */
  next() {
    this.sync(false);
    const objs = [...items].filter((i) => i.kind === "object" && (i.steps ?? 0) > (i.step ?? 0));
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
    const objs = [...items].filter((i) => i.kind === "object" && (i.step ?? 0) > 0).reverse();
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
    const rev = this.board.revision();
    if (rev === this.savedAutosaveRev) return;
    try {
      const bytes = this.board.save();
      await autosaveStore(bytes);
      this.savedAutosaveRev = rev;
    } catch (e) {
      console.warn("autosave failed", e);
    }
  }
  private savedAutosaveRev = -1;

  async saveFile() {
    const bytes = this.board.save();
    const name = (this.board.title() || "lesson").replace(/[\\/:*?"<>|]+/g, "-") + `.${FILE_EXT}`;
    const w = window as any;
    try {
      if (w.showSaveFilePicker) {
        if (!this.fileHandle) {
          this.fileHandle = await w.showSaveFilePicker({
            suggestedName: name,
            types: [{ description: `${APP_NAME} lesson`, accept: { [LESSON_MIME]: [`.${FILE_EXT}`] } }],
          });
        }
        const writable = await this.fileHandle.createWritable();
        await writable.write(bytes);
        await writable.close();
      } else {
        const a = document.createElement("a");
        a.href = URL.createObjectURL(new Blob([bytes as BlobPart], { type: LESSON_MIME }));
        a.download = name;
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 2000);
      }
      setSavedRevision(revision());
      showToast(`Saved ${name}`);
    } catch (e: any) {
      if (e?.name !== "AbortError") showToast(`Save failed: ${e?.message ?? e}`);
    }
  }

  async openFile() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = `.${FILE_EXT},.pdf,.html,.htm,image/*`;
    input.onchange = () => input.files && this.importFiles(Array.from(input.files));
    input.click();
  }

  async importFiles(files: File[]) {
    for (const f of files) {
      const name = f.name.toLowerCase();
      try {
        if (name.endsWith(`.${FILE_EXT}`)) {
          this.board.load(new Uint8Array(await f.arrayBuffer()));
          this.fileHandle = null;
          showToast(`Opened ${f.name}`);
        } else if (name.endsWith(".pdf")) {
          const { importPdf } = await import("./pdf");
          await importPdf(this, f);
        } else if (name.endsWith(".html") || name.endsWith(".htm")) {
          const html = await f.text();
          const m = html.match(/<title>([^<]*)<\/title>/i);
          const steps = Number(html.match(/data-rupok-steps=["']?(\d+)/)?.[1] ?? 0);
          this.addObject(html, m?.[1]?.trim() || f.name.replace(/\.html?$/i, ""), 1280, 720, steps);
        } else if (f.type.startsWith("image/")) {
          await this.addImageFile(f);
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

  async addImageFile(f: File | Blob, maxW = 1280, maxH = 720) {
    const bytes = new Uint8Array(await f.arrayBuffer());
    const bmp = await createImageBitmap(f);
    const s = Math.min(1, maxW / bmp.width, maxH / bmp.height);
    const ext = (f.type.split("/")[1] || "png").replace("jpeg", "jpg").replace("svg+xml", "svg");
    this.board.add_image(bytes, ext, Math.round(bmp.width * s), Math.round(bmp.height * s));
    bmp.close();
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
