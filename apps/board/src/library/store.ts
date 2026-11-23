// The Library store: one implementation of `LibraryStore` over any
// `FsDriver`, and the choice of provider for this platform.
//
//   desktop (Tauri)  → DesktopDriver: ~/Documents/Rupok (changeable)
//   browser          → a folder the teacher picked (File System Access API,
//                      Chromium), else the Origin Private File System, else
//                      IndexedDB
//
// Layout on every provider (docs/library.md):
//   Subject/Course/Chapter/lesson.rpk     the course tree ("My lessons")
//   My copies/…                           students' own copies
//   Shared with me/…                      lessons opened from files or links
//   .trash/<id>/{item, trash.json}        the trash (30 days)

import { isDesktop } from "../platform";
import { DesktopDriver, libraryInfo } from "./desktop";
import { type FsDriver, HandleDriver, IdbDriver, MemoryDriver, type RawEntry } from "./drivers";
import { idbGet, idbPut } from "./idb";
import {
  LESSON_EXT, SHARED_DIR, TRASH_DIR, basename, checkPath, cleanName, dirname, isInside, isLessonName, join, newLessonId, stem, uniqueName,
} from "./paths";
import type { Entry, LessonSummary, LibraryStore, StoreKind, TrashEntry } from "./types";
import { patchLesson, readSummary, summaryFromBytes } from "./zip";

/** Bump when the cached summary shape changes. */
const CACHE_VERSION = 2;
/** Items stay in the trash this long. */
export const TRASH_DAYS = 30;

interface CachedMeta {
  v: number;
  size: number;
  mtime: number;
  broken?: boolean;
  title: string;
  created: number;
  modified: number;
  pages: number;
  course?: LessonSummary["course"];
  library?: LessonSummary["library"];
  thumb: Blob | null;
}

interface TrashInfo {
  from: string;
  trashedAt: number;
  name: string;
}

const hiddenName = (n: string) => n.startsWith(".") || /\.(tmp|crswap)$/i.test(n) || /^(desktop\.ini|thumbs\.db)$/i.test(n);

/** Run `fn` over `items` with at most `n` at a time. */
async function pool<T>(items: T[], n: number, fn: (t: T) => Promise<void>) {
  let i = 0;
  await Promise.all(Array.from({ length: Math.min(n, items.length) }, async () => {
    while (i < items.length) await fn(items[i++]);
  }));
}

export class FileLibrary implements LibraryStore {
  private memo = new Map<string, CachedMeta>();
  private urls = new Map<string, { stamp: string; url: string | null }>();
  private channel: BroadcastChannel | null = null;

  constructor(readonly driver: FsDriver) {
    try {
      this.channel = new BroadcastChannel("rupok-library");
    } catch {
      this.channel = null;
    }
  }
  get kind(): StoreKind {
    return this.driver.kind;
  }
  get id(): string {
    return this.driver.id;
  }
  get label(): string {
    return this.driver.label;
  }

  // ---------------------------------------------------------- metadata

  private key(path: string) {
    return `${this.id}|${path}`;
  }

  private async summaryOf(path: string, size: number): Promise<LessonSummary> {
    const d = this.driver;
    if (d.readRange) return readSummary(size, (a, b) => d.readRange!(path, a, b));
    return summaryFromBytes(await d.read(path));
  }

  private toCache(r: { size: number; mtime: number }, s: LessonSummary): CachedMeta {
    return {
      v: CACHE_VERSION, size: r.size, mtime: r.mtime, title: s.title, created: s.created, modified: s.modified, pages: s.pages,
      course: s.course, library: s.library, thumb: s.thumb ? new Blob([s.thumb as BlobPart], { type: "image/png" }) : null,
    };
  }

  private async cached(e: Entry): Promise<CachedMeta> {
    const key = this.key(e.path);
    const fresh = (c?: CachedMeta) => !!c && c.v === CACHE_VERSION && c.size === e.size && c.mtime === e.modified;
    let c = this.memo.get(key);
    if (fresh(c)) return c!;
    c = await idbGet<CachedMeta>("meta", key).catch(() => undefined);
    if (!fresh(c)) {
      try {
        c = this.toCache({ size: e.size, mtime: e.modified }, await this.summaryOf(e.path, e.size));
      } catch {
        c = { v: CACHE_VERSION, size: e.size, mtime: e.modified, broken: true, title: e.name, created: 0, modified: e.modified, pages: 0, thumb: null };
      }
      void idbPut("meta", key, c).catch(() => {});
    }
    this.memo.set(key, c!);
    return c!;
  }

  private thumbUrl(path: string, c: CachedMeta): string | null {
    const key = this.key(path);
    const stamp = `${c.size}:${c.mtime}`;
    const prev = this.urls.get(key);
    if (prev?.stamp === stamp) return prev.url;
    if (prev?.url) URL.revokeObjectURL(prev.url);
    const url = c.thumb ? URL.createObjectURL(c.thumb) : null;
    this.urls.set(key, { stamp, url });
    return url;
  }

  protected async fill(e: Entry): Promise<Entry> {
    if (e.kind !== "lesson") return e;
    const c = await this.cached(e);
    e.title = c.title;
    e.pages = c.pages;
    e.course = c.course;
    e.id = c.library?.id;
    e.author = c.library?.author;
    e.copiedFrom = c.library?.copiedFrom;
    e.thumb = this.thumbUrl(e.path, c);
    if (!e.modified) e.modified = c.modified;
    return e;
  }

  protected entryOf(path: string, r: RawEntry): Entry {
    return { kind: r.dir ? "folder" : "lesson", path, name: r.dir ? r.name : stem(r.name), size: r.size, modified: r.mtime };
  }

  /** The cached summary of a lesson, if this store has read it. */
  summary(path: string): CachedMeta | undefined {
    return this.memo.get(this.key(path));
  }

  // ------------------------------------------------------------- reads

  async list(dir: string): Promise<Entry[]> {
    checkPath(dir);
    const raw = await this.driver.list(dir);
    const out = raw
      .filter((r) => !hiddenName(r.name) && (r.dir || isLessonName(r.name)) && !(dir === "" && r.dir && r.name === SHARED_DIR))
      .map((r) => this.entryOf(join(dir, r.name), r));
    await pool(out, 6, async (e) => void (await this.fill(e)));
    return out;
  }

  async walk(dir = "", opts: { includeShared?: boolean } = {}): Promise<Entry[]> {
    const out: Entry[] = [];
    const queue = [dir];
    if (opts.includeShared && dir === "" && (await this.driver.stat(SHARED_DIR))?.dir) queue.push(SHARED_DIR);
    while (queue.length) {
      const d = queue.shift()!;
      let items: Entry[];
      try {
        items = await this.list(d);
      } catch {
        continue;
      }
      for (const e of items) {
        out.push(e);
        if (e.kind === "folder") queue.push(e.path);
      }
    }
    return out;
  }

  async stat(path: string): Promise<Entry | null> {
    checkPath(path);
    const r = await this.driver.stat(path);
    if (!r) return null;
    return this.fill(this.entryOf(path, { ...r, name: basename(path) }));
  }

  async read(path: string): Promise<Uint8Array> {
    return this.driver.read(checkPath(path));
  }

  // ------------------------------------------------------------ writes

  async write(path: string, bytes: Uint8Array): Promise<Entry> {
    checkPath(path);
    if (!isLessonName(basename(path))) throw new Error(`Lessons must end in ${LESSON_EXT}`);
    await this.driver.write(path, bytes);
    const r = await this.driver.stat(path);
    if (!r) throw new Error("The lesson was not saved");
    // Prime the cache from the bytes we hold: the new thumbnail shows at once.
    try {
      const c = this.toCache(r, await summaryFromBytes(bytes));
      this.memo.set(this.key(path), c);
      void idbPut("meta", this.key(path), c).catch(() => {});
    } catch { /* re-read on next list */ }
    this.notify([path]);
    return this.fill(this.entryOf(path, { ...r, name: basename(path) }));
  }

  /** Names in a folder (for unique names); empty if it does not exist. */
  async names(dir: string): Promise<Set<string>> {
    try {
      return new Set((await this.driver.list(dir)).map((r) => r.name));
    } catch {
      return new Set();
    }
  }

  /** A free path for a new lesson called `title` in `dir`. */
  async freeLessonPath(dir: string, title: string): Promise<string> {
    return join(dir, uniqueName(await this.names(dir), cleanName(title, "Untitled lesson"), LESSON_EXT));
  }

  async createFolder(path: string): Promise<string> {
    checkPath(path);
    const dir = dirname(path);
    const name = uniqueName(await this.names(dir), cleanName(basename(path), "New folder"));
    const out = join(dir, name);
    await this.driver.mkdir(out);
    this.notify([out]);
    return out;
  }

  private moveCache(from: string, to: string) {
    for (const [k, v] of [...this.memo]) {
      const p = k.slice(this.id.length + 1);
      if (k.startsWith(this.id + "|") && isInside(p, from)) {
        this.memo.delete(k);
        const np = to + p.slice(from.length);
        this.memo.set(this.key(np), v);
        void idbPut("meta", this.key(np), v).catch(() => {});
      }
    }
  }

  async move(path: string, toDir: string): Promise<string> {
    checkPath(path);
    checkPath(toDir);
    if (dirname(path) === toDir) return path;
    if (isInside(toDir, path)) throw new Error("A folder can't be moved into itself");
    const st = await this.driver.stat(path);
    if (!st) throw new Error(`“${stem(basename(path))}” no longer exists`);
    const dest = join(toDir, uniqueName(await this.names(toDir), stem(basename(path)), st.dir ? "" : LESSON_EXT));
    await this.driver.rename(path, dest);
    this.moveCache(path, dest);
    this.notify([path, dest]);
    return dest;
  }

  async rename(path: string, newName: string): Promise<string> {
    checkPath(path);
    const st = await this.driver.stat(path);
    if (!st) throw new Error(`“${stem(basename(path))}” no longer exists`);
    const ext = st.dir ? "" : LESSON_EXT;
    const base = cleanName(newName, st.dir ? "New folder" : "Untitled lesson");
    const dir = dirname(path);
    if (base + ext === basename(path)) return path;
    const others = await this.names(dir);
    others.delete(basename(path));
    const name = uniqueName(others, base, ext);
    const dest = join(dir, name);
    if (dest === path) return path;
    if (dest.toLowerCase() === path.toLowerCase()) {
      // Case-only rename: go through a temporary name (case-insensitive file systems).
      const tmp = join(dir, `.rename-${Date.now()}${ext}`);
      await this.driver.rename(path, tmp);
      await this.driver.rename(tmp, dest);
    } else {
      await this.driver.rename(path, dest);
    }
    this.moveCache(path, dest);
    this.notify([path, dest]);
    return dest;
  }

  async duplicate(path: string): Promise<string> {
    checkPath(path);
    const st = await this.driver.stat(path);
    if (!st) throw new Error(`“${stem(basename(path))}” no longer exists`);
    const dir = dirname(path);
    const dest = join(dir, uniqueName(await this.names(dir), `${stem(basename(path))} (copy)`, st.dir ? "" : LESSON_EXT));
    if (st.dir) {
      await this.driver.copy(path, dest);
      this.notify([dest]);
    } else {
      // A duplicate is a new lesson: it gets its own id.
      const bytes = await this.driver.read(path);
      const s = await summaryFromBytes(bytes).catch(() => null);
      const patched = s ? await patchLesson(bytes, { library: { ...s.library, id: newLessonId() } }) : bytes;
      await this.write(dest, patched);
    }
    return dest;
  }

  // ------------------------------------------------------------- trash

  async trash(path: string): Promise<void> {
    checkPath(path);
    if (!path || isInside(path, TRASH_DIR)) throw new Error("This item can't be moved to the trash");
    const rand = Math.random().toString(36).slice(2, 8);
    const id = `${Date.now().toString(36)}-${rand}`;
    const base = join(TRASH_DIR, id);
    const name = basename(path);
    await this.driver.mkdir(base);
    const info: TrashInfo = { from: path, trashedAt: Date.now(), name };
    await this.driver.write(join(base, "trash.json"), new TextEncoder().encode(JSON.stringify(info)));
    await this.driver.rename(path, join(base, name));
    this.moveCache(path, join(base, name));
    this.notify([path]);
  }

  private async trashInfo(id: string): Promise<TrashInfo | null> {
    try {
      const info = JSON.parse(new TextDecoder().decode(await this.driver.read(join(TRASH_DIR, id, "trash.json"))));
      return typeof info?.from === "string" && typeof info?.name === "string" ? info : null;
    } catch {
      return null;
    }
  }

  async listTrash(): Promise<TrashEntry[]> {
    let dirs: RawEntry[];
    try {
      dirs = (await this.driver.list(TRASH_DIR)).filter((r) => r.dir);
    } catch {
      return [];
    }
    const out: TrashEntry[] = [];
    const cutoff = Date.now() - TRASH_DAYS * 86400e3;
    await pool(dirs, 6, async (d) => {
      const info = await this.trashInfo(d.name);
      const base = join(TRASH_DIR, d.name);
      if (!info) return;
      if (info.trashedAt < cutoff) {
        await this.driver.remove(base).catch(() => {});
        return;
      }
      const r = await this.driver.stat(join(base, info.name)).catch(() => null);
      if (!r) return;
      const e = await this.fill(this.entryOf(join(base, info.name), { ...r, name: info.name }));
      out.push({ ...e, trashId: d.name, from: info.from, trashedAt: info.trashedAt });
    });
    return out.sort((a, b) => b.trashedAt - a.trashedAt);
  }

  async restore(trashId: string): Promise<string> {
    const info = await this.trashInfo(checkPath(trashId));
    if (!info) throw new Error("This item is no longer in the trash");
    const base = join(TRASH_DIR, trashId);
    const item = join(base, info.name);
    const st = await this.driver.stat(item);
    if (!st) throw new Error("This item is no longer in the trash");
    const dir = dirname(info.from);
    await this.driver.mkdir(dir);
    const dest = join(dir, uniqueName(await this.names(dir), stem(info.name), st.dir ? "" : LESSON_EXT));
    await this.driver.rename(item, dest);
    this.moveCache(item, dest);
    await this.driver.remove(base).catch(() => {});
    this.notify([dest]);
    return dest;
  }

  async deletePermanently(trashId: string): Promise<void> {
    await this.driver.remove(join(TRASH_DIR, checkPath(trashId)));
  }

  async emptyTrash(): Promise<void> {
    if (await this.driver.stat(TRASH_DIR)) await this.driver.remove(TRASH_DIR);
  }

  // ------------------------------------------------------------- watch

  protected notify(paths: string[]) {
    try {
      this.channel?.postMessage({ type: "changed", store: this.id, paths });
    } catch { /* closed */ }
  }

  watch(cb: (paths: string[]) => void): () => void {
    const stopDriver = this.driver.watch?.((paths) => cb(paths.filter((p) => !hiddenName(basename(p)) || isInside(p, TRASH_DIR)))) ?? (() => {});
    // Other tabs on the same storage.
    const onMsg = (e: MessageEvent) => e.data?.type === "changed" && e.data.store === this.id && cb(e.data.paths ?? []);
    let ch: BroadcastChannel | null = null;
    try {
      ch = new BroadcastChannel("rupok-library");
      ch.addEventListener("message", onMsg);
    } catch { /* unsupported */ }
    return () => {
      stopDriver();
      ch?.close();
    };
  }
}

// ---------------------------------------------------------------- providers

export type FolderState = "granted" | "prompt" | "denied";

export interface OpenedStore {
  store: FileLibrary;
  /** Browser folder whose permission must be granted by a click first. */
  pendingFolder?: FileSystemDirectoryHandle;
}

/** True when the browser can use a real folder (File System Access API). */
export const canUseFolder = (): boolean => !isDesktop && typeof (window as any).showDirectoryPicker === "function";

export async function opfsStore(): Promise<FileLibrary> {
  const root = await navigator.storage.getDirectory();
  const lib = await root.getDirectoryHandle("Rupok", { create: true });
  // createWritable is what makes writes atomic; without it use IndexedDB.
  if (typeof (globalThis as any).FileSystemFileHandle?.prototype?.createWritable !== "function") throw new Error("no createWritable");
  return new FileLibrary(new HandleDriver("opfs", "opfs", "This browser", lib));
}

export function folderStore(h: FileSystemDirectoryHandle): FileLibrary {
  return new FileLibrary(new HandleDriver("folder", `folder:${h.name}`, h.name, h));
}

/** The provider for this platform and the teacher's choice. */
export async function openStore(location: "browser" | "folder"): Promise<OpenedStore> {
  if (isDesktop) return { store: new FileLibrary(new DesktopDriver(await libraryInfo())) };
  if (location === "folder" && canUseFolder()) {
    const h = await idbGet<FileSystemDirectoryHandle>("kv", "folderHandle").catch(() => undefined);
    if (h) {
      const perm = await (h as any).queryPermission?.({ mode: "readwrite" }).catch(() => "prompt");
      const store = folderStore(h);
      return perm === "granted" ? { store } : { store, pendingFolder: h };
    }
  }
  try {
    return { store: await opfsStore() };
  } catch {
    try {
      await idbGet("kv", "probe");
      return { store: new FileLibrary(new IdbDriver()) };
    } catch {
      return { store: new FileLibrary(new MemoryDriver("memory", "This tab only")) };
    }
  }
}

/** Ask for (or re-grant) read/write access to a picked folder. Needs a click. */
export async function requestFolder(h: FileSystemDirectoryHandle): Promise<boolean> {
  try {
    return (await (h as any).requestPermission({ mode: "readwrite" })) === "granted";
  } catch {
    return false;
  }
}

/** "Use a folder on this computer": pick it and remember the handle. */
export async function pickFolder(): Promise<FileSystemDirectoryHandle | null> {
  try {
    const h: FileSystemDirectoryHandle = await (window as any).showDirectoryPicker({ id: "rupok-library", mode: "readwrite", startIn: "documents" });
    await idbPut("kv", "folderHandle", h);
    return h;
  } catch (e: any) {
    if (e?.name === "AbortError") return null;
    throw e;
  }
}

/** Copy every lesson and folder of `from` into `to`, skipping names that exist. */
export async function copyLibrary(from: FileLibrary, to: FileLibrary, onProgress?: (n: number) => void): Promise<number> {
  let n = 0;
  const walk = async (dir: string) => {
    for (const r of await from.driver.list(dir).catch(() => [] as RawEntry[])) {
      if (hiddenName(r.name)) continue;
      const p = join(dir, r.name);
      if (r.dir) {
        await to.driver.mkdir(p);
        await walk(p);
      } else if (isLessonName(r.name) && !(await to.driver.stat(p))) {
        await to.driver.write(p, await from.driver.read(p));
        onProgress?.(++n);
      }
    }
  };
  await walk("");
  return n;
}
