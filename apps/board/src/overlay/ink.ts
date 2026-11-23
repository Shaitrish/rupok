// Engine glue for "draw over any app": a Board on a transparent, full-window
// canvas. No page frame, no canvas background, no DOM items: only ink over
// whatever is on the screen. The camera is pinned at (0, 0, zoom 1), so world
// units are CSS pixels of the monitor.
//
// This deliberately does not reuse the main Controller: the overlay is its
// own small lesson and must never load or autosave over the teacher's lesson.

import init, { Board } from "../wasm/rupok.js";
import wasmUrl from "../wasm/rupok_bg.wasm?url";
import { prefs } from "../state";
import { desktopInvoke } from "../platform";

export type OverlayTool = "pen" | "highlighter" | "eraser" | "laser";

/** Commands from the toolbar to the ink window. */
export type OverlayCommand =
  | { type: "tool"; tool: OverlayTool; color: number; width: number }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "clear" }
  | { type: "save" };

/** Ink window → toolbar. */
export interface OverlayStatus {
  ready?: boolean;
  saving?: boolean;
}

/** Window labels and events shared with apps/desktop/src/overlay.rs. */
export const INK_WINDOW = "overlay";
export const TOOLBAR_WINDOW = "overlay-toolbar";
export const CMD_EVENT = "rupok://overlay-cmd";
export const STATUS_EVENT = "rupok://overlay-status";
export const CLICK_THROUGH_EVENT = "rupok://overlay-click-through";

/** Screenshot from `overlay_capture`: `[w: u32 LE][h: u32 LE][RGBA…]`. */
function decodeFrame(buf: ArrayBuffer): ImageData {
  const v = new DataView(buf);
  const w = v.getUint32(0, true);
  const h = v.getUint32(4, true);
  return new ImageData(new Uint8ClampedArray(buf, 8, w * h * 4), w, h);
}

function toBlob(c: HTMLCanvasElement, type: string, quality?: number): Promise<Blob | null> {
  return new Promise((resolve) => c.toBlob(resolve, type, quality));
}

export class OverlayInk {
  board: Board | null = null;
  private root!: HTMLElement;
  private canvas!: HTMLCanvasElement;
  private tool: Extract<OverlayCommand, { type: "tool" }> = { type: "tool", tool: "pen", color: 0xe5484dff, width: 4 };

  async init(root: HTMLElement, canvas: HTMLCanvasElement) {
    this.root = root;
    this.canvas = canvas;
    await init({ module_or_path: wasmUrl });
    const board = await Board.create(canvas, prefs().forceWebgl);
    this.board = board;
    const p = prefs();
    board.set_smoothing(p.smoothing);
    board.set_pressure_gamma(p.pressure);
    board.set_touch_mode("draw"); // fingers draw; there is nothing to pan
    board.set_insets(0, 0, 0, 0);
    this.resize();
    new ResizeObserver(() => this.resize()).observe(root);
    this.apply();
    requestAnimationFrame(this.loop);
  }

  private resize() {
    if (!this.board) return;
    const r = this.root.getBoundingClientRect();
    this.board.resize(r.width, r.height, window.devicePixelRatio || 1);
    this.pin();
  }

  private pin() {
    const [x, y, z] = this.board!.camera();
    if (x !== 0 || y !== 0 || z !== 1) this.board!.set_camera(0, 0, 1);
  }

  private loop = () => {
    if (this.board) {
      this.pin();
      this.board.frame();
    }
    requestAnimationFrame(this.loop);
  };

  /** Remember the tool (the toolbar may speak before the engine is up). */
  setTool(cmd: Extract<OverlayCommand, { type: "tool" }>) {
    this.tool = cmd;
    this.apply();
  }

  private apply() {
    const b = this.board;
    if (!b) return;
    const { tool, color, width } = this.tool;
    b.set_tool(tool);
    if (tool === "pen" || tool === "highlighter") b.set_color(color);
    if (tool !== "laser") b.set_width(width);
  }

  undo() { this.board?.undo(); }
  redo() { this.board?.redo(); }
  clear() { this.board?.clear_page(); }

  attachInput(el: HTMLElement) {
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
      if (!this.board) return;
      el.setPointerCapture(e.pointerId);
      const r = this.root.getBoundingClientRect();
      this.board.pointer_down(e.pointerId, e.clientX - r.left, e.clientY - r.top, e.pressure, e.timeStamp, e.pointerType, e.button);
      e.preventDefault();
    });
    el.addEventListener("pointermove", (e) => {
      if (!this.board || !el.hasPointerCapture(e.pointerId)) return;
      const coalesced = e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
      const predicted = e.getPredictedEvents ? e.getPredictedEvents() : [];
      this.board.pointer_move(e.pointerId, flat(coalesced.length ? coalesced : [e]), flat(predicted));
    });
    el.addEventListener("pointerup", (e) => {
      if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
      this.board?.pointer_up(e.pointerId);
    });
    el.addEventListener("pointercancel", (e) => this.board?.pointer_cancel(e.pointerId));
    el.addEventListener("contextmenu", (e) => e.preventDefault());
  }

  /**
   * Draw a frame now and copy the ink onto `ctx`. Runs in one task with the
   * draw, while the WebGL/WebGPU drawing buffer is still readable.
   */
  private drawInk(ctx: CanvasRenderingContext2D, w: number, h: number) {
    this.resize(); // invalidates the renderer, so frame() redraws
    this.board?.frame();
    ctx.drawImage(this.canvas, 0, 0, w, h);
  }

  /**
   * "Save to lesson": screenshot of the screen under the overlay (taken by
   * the desktop app with the overlay hidden) with the ink on top, sent to the
   * board window as a new page. Where the screen cannot be captured
   * (Wayland), the ink alone is saved. Returns a message for the teacher.
   */
  async saveToLesson(): Promise<string> {
    let shot: ImageData | null = null;
    let why = "";
    try {
      shot = decodeFrame(await desktopInvoke<ArrayBuffer>("overlay_capture"));
    } catch (e: any) {
      why = String(e?.message ?? e);
    }
    const w = shot?.width ?? this.canvas.width;
    const h = shot?.height ?? this.canvas.height;
    const out = document.createElement("canvas");
    out.width = w;
    out.height = h;
    const ctx = out.getContext("2d")!;
    if (shot) ctx.putImageData(shot, 0, 0);
    this.drawInk(ctx, w, h);
    let blob = await toBlob(out, "image/webp", 0.92);
    if (!blob || blob.type !== "image/webp") blob = await toBlob(out, "image/png");
    if (!blob) throw new Error("could not encode the snapshot");
    await desktopInvoke("overlay_deliver_page", new Uint8Array(await blob.arrayBuffer()));
    return shot
      ? "Saved the screen and your ink as a new page"
      : `Saved your ink as a new page (no screenshot: ${why})`;
  }
}
