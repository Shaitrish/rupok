// Platform bridge: the same board runs in a browser and in the Tauri desktop
// shell (apps/desktop). Everything desktop-only is behind `isDesktop` and
// loaded with dynamic import(), so the browser never downloads it.
//
// Desktop file flow: native dialogs (@tauri-apps/plugin-dialog), reads through
// @tauri-apps/plugin-fs (the dialog puts picked paths in the fs scope), and
// saves through the `save_lesson` command, which validates the package and
// writes it atomically (temp file + rename) on the Rust side.

import { APP_NAME, FILE_EXT, LESSON_MIME } from "./brand";
import { showToast } from "./state";

/** True inside the Tauri desktop app. */
export const isDesktop: boolean = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** A file to import; `path` is set when it came from disk in the desktop app. */
export type LessonFile = File & { path?: string };

const core = () => import("@tauri-apps/api/core");
const dialogs = () => import("@tauri-apps/plugin-dialog");

const OPEN_EXTENSIONS = [FILE_EXT, "pdf", "html", "htm", "png", "jpg", "jpeg", "gif", "webp", "svg"];
const MIME: Record<string, string> = {
  [FILE_EXT]: LESSON_MIME, pdf: "application/pdf", html: "text/html", htm: "text/html",
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", svg: "image/svg+xml",
};

const baseName = (path: string) => path.split(/[\\/]/).pop() || path;
const extOf = (name: string) => name.split(".").pop()?.toLowerCase() ?? "";
const isLessonName = (name: string) => extOf(name) === FILE_EXT;

// ------------------------------------------------------------ Tauri helpers

/** Call a Rust command (desktop only). */
export async function desktopInvoke<T>(cmd: string, args?: Record<string, unknown> | Uint8Array, headers?: Record<string, string>): Promise<T> {
  const { invoke } = await core();
  return invoke<T>(cmd, args, headers ? { headers } : undefined);
}

/** Listen to an app event (desktop only). Resolves to an unsubscribe function. */
export async function desktopListen<T>(event: string, cb: (payload: T) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(event, (e) => cb(e.payload));
}

/** Send an event to one window (desktop only). */
export async function desktopEmitTo(label: string, event: string, payload?: unknown): Promise<void> {
  const { emitTo } = await import("@tauri-apps/api/event");
  return emitTo(label, event, payload);
}

// ------------------------------------------------------------------ saving

/** Where Ctrl+S writes: a path (desktop) or a FileSystemFileHandle (browser). */
let target: string | { name: string; createWritable(): Promise<any> } | null = null;

/** Forget the save location, so the next save asks where (new or opened lesson). */
export function forgetLessonPath() {
  target = null;
}

/**
 * Save lesson bytes. Asks for a location the first time, then writes to the
 * same place on every later call (Ctrl+S). Returns the path or file name, or
 * null if the teacher cancelled.
 */
export async function saveLessonBytes(suggestedName: string, bytes: Uint8Array): Promise<string | null> {
  if (isDesktop) {
    let path = typeof target === "string" ? target : null;
    if (!path) {
      const { save } = await dialogs();
      const picked = await save({ defaultPath: suggestedName, filters: [{ name: `${APP_NAME} lesson`, extensions: [FILE_EXT] }] });
      if (!picked) return null;
      // GTK dialogs do not add the extension.
      path = isLessonName(picked) ? picked : `${picked}.${FILE_EXT}`;
    }
    await desktopInvoke("save_lesson", bytes, { "x-rupok-path": encodeURIComponent(path) });
    target = path;
    return path;
  }

  const w = window as any;
  if (w.showSaveFilePicker) {
    try {
      let handle = target && typeof target !== "string" ? target : null;
      handle ??= await w.showSaveFilePicker({
        suggestedName,
        types: [{ description: `${APP_NAME} lesson`, accept: { [LESSON_MIME]: [`.${FILE_EXT}`] } }],
      });
      const writable = await handle!.createWritable();
      await writable.write(bytes);
      await writable.close();
      target = handle;
      return handle!.name;
    } catch (e: any) {
      if (e?.name === "AbortError") return null;
      throw e;
    }
  }

  // No file system access: download a copy each time.
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([bytes as BlobPart], { type: LESSON_MIME }));
  a.download = suggestedName;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
  return suggestedName;
}

/**
 * Save an exported PDF. Desktop: a save dialog, then the `save_pdf` command
 * checks the bytes and writes them atomically. Browser: a download. Returns
 * the path or file name, or null if the teacher cancelled.
 */
export async function savePdfBytes(suggestedName: string, bytes: Uint8Array): Promise<string | null> {
  if (isDesktop) {
    const { save } = await dialogs();
    const picked = await save({ defaultPath: suggestedName, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (!picked) return null;
    const path = extOf(picked) === "pdf" ? picked : `${picked}.pdf`;
    await desktopInvoke("save_pdf", bytes, { "x-rupok-path": encodeURIComponent(path) });
    return path;
  }
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "application/pdf" }));
  a.download = suggestedName;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
  return suggestedName;
}

// ----------------------------------------------------------------- opening

async function readPaths(paths: string[]): Promise<LessonFile[]> {
  const { readFile } = await import("@tauri-apps/plugin-fs");
  const out: LessonFile[] = [];
  for (const path of paths) {
    try {
      const bytes = await readFile(path);
      const name = baseName(path);
      const file = new File([bytes], name, { type: MIME[extOf(name)] ?? "" }) as LessonFile;
      Object.defineProperty(file, "path", { value: path });
      out.push(file);
    } catch (e: any) {
      showToast(`Could not open ${baseName(path)}: ${e?.message ?? e}`, 5000);
    }
  }
  return out;
}

/** Ask for lessons, PDFs, images or Smart Objects to open. Empty if cancelled. */
export async function openLessonFiles(): Promise<LessonFile[]> {
  if (isDesktop) {
    const { open } = await dialogs();
    const picked = await open({
      multiple: true,
      filters: [{ name: "Lessons, PDFs, images and Smart Objects", extensions: OPEN_EXTENSIONS }],
    });
    if (!picked) return [];
    return readPaths(Array.isArray(picked) ? picked : [picked]);
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.accept = `.${FILE_EXT},.pdf,.html,.htm,image/*`;
    input.onchange = () => resolve(Array.from(input.files ?? []));
    input.oncancel = () => resolve([]);
    input.click();
  });
}

/**
 * Files the operating system opens in Rupok: a double-clicked `.rpk`, "Open
 * with", or a second launch with a file (desktop only). Returns an unsubscribe
 * function.
 */
export function onOpenFile(cb: (files: LessonFile[]) => void): () => void {
  if (!isDesktop) return () => {};
  let stop: (() => void) | null = null;
  let stopped = false;
  const drain = async () => {
    const paths = await desktopInvoke<string[]>("take_pending_files");
    if (paths.length) cb(await readPaths(paths));
  };
  desktopListen("rupok://open-files", () => void drain()).then((un) => (stopped ? un() : (stop = un)));
  void drain(); // files from the command line arrived before we listened
  return () => {
    stopped = true;
    stop?.();
  };
}

// --------------------------------------------------------- board wiring

/** The parts of the board controller the bridge needs (no import cycle). */
export interface LessonHost {
  fileHandle: unknown;
  importFiles(files: File[]): Promise<void>;
  board: {
    add_page(pattern: string): number;
    pages_json(): string;
    page_index(): number;
    add_image(bytes: Uint8Array, ext: string, w: number, h: number): string | undefined;
  };
}

/**
 * Import files into the board. When a lesson read from disk loaded, Ctrl+S
 * saves back to that file. (`importFiles` clears `fileHandle` only when a
 * lesson really loaded, which is how a failed load is told apart.)
 */
export async function openLessonInto(host: LessonHost, files: LessonFile[]) {
  if (!files.length) return;
  const before = host.fileHandle;
  const pending = {};
  host.fileHandle = pending;
  await host.importFiles(files);
  const lesson = files.filter((f) => f.path && isLessonName(f.name)).pop();
  if (host.fileHandle === null && lesson?.path) {
    target = lesson.path;
    host.fileHandle = lesson.path;
  } else if (host.fileHandle === pending) {
    host.fileHandle = before;
  }
}

/**
 * Desktop wiring for the board window: files opened by the OS, and pages
 * saved from the overlay ("Save to lesson"). Call once after the board is up.
 */
export function attachDesktopBridge(host: LessonHost) {
  if (!isDesktop) return;
  onOpenFile((files) => void openLessonInto(host, files));
  void desktopListen("rupok://overlay-page", async () => {
    try {
      const bytes = new Uint8Array(await desktopInvoke<ArrayBuffer>("take_overlay_page"));
      const ext = bytes[0] === 0x89 ? "png" : "webp";
      const bmp = await createImageBitmap(new Blob([bytes], { type: `image/${ext}` }));
      const [iw, ih] = [bmp.width, bmp.height];
      bmp.close();
      // A new slide filled by the snapshot (scaled up or down to the frame).
      host.board.add_page("plain");
      const page = JSON.parse(host.board.pages_json())[host.board.page_index()];
      const s = Math.min((page?.frame?.w ?? 1920) / iw, (page?.frame?.h ?? 1080) / ih);
      host.board.add_image(bytes, ext, Math.round(iw * s), Math.round(ih * s));
      showToast("Added the overlay snapshot as a new page");
    } catch (e: any) {
      showToast(`Could not add the overlay snapshot: ${e?.message ?? e}`, 5000);
    }
  });
}

// ----------------------------------------------------------------- overlay

/** Show the draw-over-any-app overlay (desktop; no-op in the browser). */
export async function openOverlay(): Promise<void> {
  if (isDesktop) await desktopInvoke("overlay_show");
}

/** Hide the overlay (desktop; no-op in the browser). The ink is kept. */
export async function closeOverlay(): Promise<void> {
  if (isDesktop) await desktopInvoke("overlay_hide");
}
