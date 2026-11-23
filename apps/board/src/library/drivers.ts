// File-system drivers: the few primitives a library provider needs. The
// Library logic (trash, duplicates, metadata) is written once in store.ts on
// top of these, so every platform behaves the same.
//
// - HandleDriver: a FileSystemDirectoryHandle. The same code serves the
//   Origin Private File System (`navigator.storage.getDirectory()`) and a
//   real folder picked with the File System Access API (Chromium).
// - IdbDriver: files in IndexedDB, for browsers without OPFS.
// - MemoryDriver: tests.
// - DesktopDriver (desktop.ts): Tauri commands on a real folder.

import { basename, checkPath, dirname, isInside, join, split } from "./paths";
import { idb, idbGet, prefixRange, req, tx } from "./idb";
import type { StoreKind } from "./types";

export interface RawEntry {
  name: string;
  dir: boolean;
  size: number;
  /** Modification time, ms since the Unix epoch (0 for folders when unknown). */
  mtime: number;
}

export interface FsDriver {
  readonly kind: StoreKind;
  /** Stable id of the location (cache keys, locks). */
  readonly id: string;
  readonly label: string;
  /** Direct children of a folder ("" = root). Throws if it does not exist. */
  list(dir: string): Promise<RawEntry[]>;
  stat(path: string): Promise<RawEntry | null>;
  read(path: string): Promise<Uint8Array>;
  /** Bytes [start, end) of a file; optional (falls back to `read`). */
  readRange?(path: string, start: number, end: number): Promise<Uint8Array>;
  /** Create or replace a file crash-safely; parent folders are created. */
  write(path: string, bytes: Uint8Array): Promise<void>;
  /** Create a folder and its parents; fine if it exists. */
  mkdir(path: string): Promise<void>;
  /** Move a file or folder; fails if `to` exists. */
  rename(from: string, to: string): Promise<void>;
  /** Copy a file or folder; fails if `to` exists. */
  copy(from: string, to: string): Promise<void>;
  /** Delete a file or folder (recursively). */
  remove(path: string): Promise<void>;
  /** Changes made by others (agents, sync clients); optional. */
  watch?(cb: (paths: string[]) => void): () => void;
}

export class ExistsError extends Error {
  constructor(path: string) {
    super(`“${basename(path)}” already exists`);
    this.name = "ExistsError";
  }
}

/** Copy a tree with the driver's own primitives (fallback for move/copy). */
export async function copyTree(d: FsDriver, from: string, to: string): Promise<void> {
  const st = await d.stat(from);
  if (!st) throw new Error(`“${basename(from)}” does not exist`);
  if (await d.stat(to)) throw new ExistsError(to);
  if (!st.dir) return d.write(to, await d.read(from));
  await d.mkdir(to);
  for (const c of await d.list(from)) await copyTree(d, join(from, c.name), join(to, c.name));
}

// ------------------------------------------------------------------ memory

interface MemNode {
  dir: boolean;
  bytes?: Uint8Array;
  mtime: number;
}

/** An in-memory file system (tests, and a last resort when storage is blocked). */
export class MemoryDriver implements FsDriver {
  readonly kind: StoreKind = "memory";
  files = new Map<string, MemNode>();
  private clock = Date.now();
  constructor(readonly id = "memory", readonly label = "Memory") {}

  private now() {
    // Strictly increasing, so tests can tell writes apart.
    this.clock = Math.max(this.clock + 1, Date.now());
    return this.clock;
  }
  private exists(p: string) {
    return p === "" || this.files.has(p);
  }
  async list(dir: string) {
    checkPath(dir);
    const n = this.files.get(dir);
    if (dir && (!n || !n.dir)) throw new Error(`folder “${dir}” does not exist`);
    const out: RawEntry[] = [];
    for (const [p, v] of this.files) {
      if (dirname(p) === dir && p !== dir) out.push({ name: basename(p), dir: v.dir, size: v.bytes?.length ?? 0, mtime: v.mtime });
    }
    return out;
  }
  async stat(path: string) {
    checkPath(path);
    if (path === "") return { name: "", dir: true, size: 0, mtime: 0 };
    const v = this.files.get(path);
    return v ? { name: basename(path), dir: v.dir, size: v.bytes?.length ?? 0, mtime: v.mtime } : null;
  }
  async read(path: string) {
    const v = this.files.get(checkPath(path));
    if (!v || v.dir || !v.bytes) throw new Error(`“${basename(path)}” does not exist`);
    return v.bytes.slice();
  }
  async readRange(path: string, start: number, end: number) {
    return (await this.read(path)).subarray(start, end);
  }
  async write(path: string, bytes: Uint8Array) {
    checkPath(path);
    const cur = this.files.get(path);
    if (cur?.dir) throw new ExistsError(path);
    await this.mkdir(dirname(path));
    this.files.set(path, { dir: false, bytes: bytes.slice(), mtime: this.now() });
  }
  async mkdir(path: string) {
    checkPath(path);
    const parts = split(path);
    for (let i = 1; i <= parts.length; i++) {
      const p = parts.slice(0, i).join("/");
      const v = this.files.get(p);
      if (v && !v.dir) throw new ExistsError(p);
      if (!v) this.files.set(p, { dir: true, mtime: this.now() });
    }
  }
  async rename(from: string, to: string) {
    checkPath(from);
    checkPath(to);
    if (!this.files.has(from)) throw new Error(`“${basename(from)}” does not exist`);
    if (this.exists(to)) throw new ExistsError(to);
    if (isInside(to, from)) throw new Error("A folder cannot be moved into itself");
    await this.mkdir(dirname(to));
    for (const [p, v] of [...this.files]) {
      if (isInside(p, from)) {
        this.files.delete(p);
        this.files.set(to + p.slice(from.length), v);
      }
    }
  }
  async copy(from: string, to: string) {
    if (isInside(to, from)) throw new Error("A folder cannot be copied into itself");
    return copyTree(this, from, to);
  }
  async remove(path: string) {
    checkPath(path);
    if (!path) throw new Error("refusing to delete the library root");
    for (const p of [...this.files.keys()]) if (isInside(p, path)) this.files.delete(p);
  }
}

// ------------------------------------------------------- directory handles

const isNotFound = (e: any) => e?.name === "NotFoundError";
const isTypeMismatch = (e: any) => e?.name === "TypeMismatchError";

/**
 * A library on a FileSystemDirectoryHandle: the Origin Private File System
 * or a folder the teacher picked ("Use a folder on this computer").
 *
 * Writes use `createWritable()`, which writes to a temporary swap file and
 * atomically replaces the target on `close()`: a crash or a full disk never
 * leaves a half-written lesson.
 */
export class HandleDriver implements FsDriver {
  constructor(readonly kind: "opfs" | "folder", readonly id: string, readonly label: string, readonly root: FileSystemDirectoryHandle) {}

  private async dir(path: string, create = false): Promise<FileSystemDirectoryHandle> {
    let d = this.root;
    for (const part of split(checkPath(path))) d = await d.getDirectoryHandle(part, { create });
    return d;
  }
  private async parentOf(path: string, create = false): Promise<[FileSystemDirectoryHandle, string]> {
    checkPath(path);
    return [await this.dir(dirname(path), create), basename(path)];
  }
  private async handle(path: string): Promise<FileSystemHandle | null> {
    if (!path) return this.root;
    try {
      const [d, name] = await this.parentOf(path);
      try {
        return await d.getFileHandle(name);
      } catch (e) {
        if (isTypeMismatch(e)) return await d.getDirectoryHandle(name);
        if (isNotFound(e)) return null;
        throw e;
      }
    } catch (e) {
      if (isNotFound(e) || isTypeMismatch(e)) return null;
      throw e;
    }
  }
  async list(dir: string) {
    const d = await this.dir(dir);
    const out: RawEntry[] = [];
    for await (const [name, h] of (d as any).entries() as AsyncIterable<[string, FileSystemHandle]>) {
      if (h.kind === "directory") out.push({ name, dir: true, size: 0, mtime: 0 });
      else {
        try {
          const f = await (h as FileSystemFileHandle).getFile();
          out.push({ name, dir: false, size: f.size, mtime: f.lastModified });
        } catch {
          /* vanished or unreadable: skip */
        }
      }
    }
    return out;
  }
  async stat(path: string) {
    const h = await this.handle(path);
    if (!h) return null;
    if (h.kind === "directory") return { name: basename(path), dir: true, size: 0, mtime: 0 };
    const f = await (h as FileSystemFileHandle).getFile();
    return { name: basename(path), dir: false, size: f.size, mtime: f.lastModified };
  }
  private async file(path: string): Promise<File> {
    const [d, name] = await this.parentOf(path);
    return (await d.getFileHandle(name)).getFile();
  }
  async read(path: string) {
    return new Uint8Array(await (await this.file(path)).arrayBuffer());
  }
  async readRange(path: string, start: number, end: number) {
    return new Uint8Array(await (await this.file(path)).slice(start, end).arrayBuffer());
  }
  async write(path: string, bytes: Uint8Array) {
    const [d, name] = await this.parentOf(path, true);
    const fh = await d.getFileHandle(name, { create: true });
    const w = await (fh as any).createWritable();
    try {
      await w.write(bytes);
      await w.close();
    } catch (e) {
      try { await w.abort(); } catch { /* already closed */ }
      throw e;
    }
  }
  async mkdir(path: string) {
    await this.dir(path, true);
  }
  async rename(from: string, to: string) {
    if (isInside(to, from)) throw new Error("A folder cannot be moved into itself");
    const h = await this.handle(from);
    if (!h) throw new Error(`“${basename(from)}” does not exist`);
    if (await this.handle(to)) throw new ExistsError(to);
    const [dest, name] = await this.parentOf(to, true);
    // FileSystemHandle.move() where supported (files everywhere in Chromium,
    // folders in newer versions); otherwise copy, then delete.
    if (typeof (h as any).move === "function") {
      try {
        await (h as any).move(dest, name);
        return;
      } catch {
        if (await this.handle(to)) return;
      }
    }
    await copyTree(this, from, to);
    await this.remove(from);
  }
  async copy(from: string, to: string) {
    if (isInside(to, from)) throw new Error("A folder cannot be copied into itself");
    return copyTree(this, from, to);
  }
  async remove(path: string) {
    if (!path) throw new Error("refusing to delete the library root");
    const [d, name] = await this.parentOf(path);
    try {
      await d.removeEntry(name, { recursive: true });
    } catch (e) {
      if (!isNotFound(e)) throw e;
    }
  }
  watch(cb: (paths: string[]) => void): () => void {
    // FileSystemObserver (Chromium 129+): changes by sync clients and agents.
    const FSO = (globalThis as any).FileSystemObserver;
    if (!FSO) return () => {};
    let obs: any;
    try {
      obs = new FSO((records: any[]) => {
        const paths = records.map((r) => (r.relativePathComponents ?? []).join("/")).filter((p: string) => p && !/\.(tmp|crswap)$/.test(p));
        if (paths.length) cb(paths);
      });
      void obs.observe(this.root, { recursive: true }).catch(() => {});
    } catch {
      return () => {};
    }
    return () => obs?.disconnect?.();
  }
}

// ----------------------------------------------------------------- IndexedDB

interface IdbMeta {
  dir: boolean;
  size: number;
  mtime: number;
}

/** Files in IndexedDB: browsers without OPFS (or with OPFS blocked). */
export class IdbDriver implements FsDriver {
  readonly kind: StoreKind = "idb";
  constructor(readonly id = "idb", readonly label = "This browser") {}

  private key(p: string) {
    return `${this.id}:${p}`;
  }
  private async all(prefix: string): Promise<[string, IdbMeta][]> {
    const db = await idb();
    const store = db.transaction("fsmeta", "readonly").objectStore("fsmeta");
    const range = prefixRange(this.key(prefix));
    const [keys, vals] = await Promise.all([req(store.getAllKeys(range)), req(store.getAll(range))]);
    const base = this.key("").length;
    return keys.map((k, i) => [String(k).slice(base), vals[i] as IdbMeta]);
  }
  async list(dir: string) {
    checkPath(dir);
    if (dir && !(await this.stat(dir))?.dir) throw new Error(`folder “${dir}” does not exist`);
    const rows = await this.all(dir ? dir + "/" : "");
    return rows.filter(([p]) => dirname(p) === dir && p !== dir).map(([p, m]) => ({ name: basename(p), dir: m.dir, size: m.size, mtime: m.mtime }));
  }
  async stat(path: string) {
    checkPath(path);
    if (!path) return { name: "", dir: true, size: 0, mtime: 0 };
    const m = await idbGet<IdbMeta>("fsmeta", this.key(path));
    return m ? { name: basename(path), dir: m.dir, size: m.size, mtime: m.mtime } : null;
  }
  async read(path: string) {
    checkPath(path);
    const b = await idbGet<Uint8Array>("fsdata", this.key(path));
    if (!(b instanceof Uint8Array)) throw new Error(`“${basename(path)}” does not exist`);
    return b;
  }
  async readRange(path: string, start: number, end: number) {
    return (await this.read(path)).subarray(start, end);
  }
  async write(path: string, bytes: Uint8Array) {
    checkPath(path);
    const st = await this.stat(path);
    if (st?.dir) throw new ExistsError(path);
    await this.mkdir(dirname(path));
    // One transaction: data and metadata change together or not at all.
    await tx(["fsmeta", "fsdata"], "readwrite", (t) => {
      t.objectStore("fsdata").put(bytes.slice(), this.key(path));
      t.objectStore("fsmeta").put({ dir: false, size: bytes.length, mtime: Date.now() } satisfies IdbMeta, this.key(path));
    });
  }
  async mkdir(path: string) {
    checkPath(path);
    const parts = split(path);
    const missing: string[] = [];
    for (let i = 1; i <= parts.length; i++) {
      const p = parts.slice(0, i).join("/");
      const st = await this.stat(p);
      if (st && !st.dir) throw new ExistsError(p);
      if (!st) missing.push(p);
    }
    if (missing.length) {
      await tx("fsmeta", "readwrite", (t) => {
        for (const p of missing) t.objectStore("fsmeta").put({ dir: true, size: 0, mtime: Date.now() } satisfies IdbMeta, this.key(p));
      });
    }
  }
  async rename(from: string, to: string) {
    checkPath(from);
    checkPath(to);
    if (isInside(to, from)) throw new Error("A folder cannot be moved into itself");
    if (!(await this.stat(from))) throw new Error(`“${basename(from)}” does not exist`);
    if (await this.stat(to)) throw new ExistsError(to);
    await this.mkdir(dirname(to));
    const rows = [...(await this.all(from + "/")), [from, (await idbGet<IdbMeta>("fsmeta", this.key(from)))!] as [string, IdbMeta]];
    const datas = await Promise.all(rows.map(([p, m]) => (m.dir ? null : this.read(p))));
    await tx(["fsmeta", "fsdata"], "readwrite", (t) => {
      rows.forEach(([p, m], i) => {
        const np = to + p.slice(from.length);
        t.objectStore("fsmeta").delete(this.key(p));
        t.objectStore("fsmeta").put(m, this.key(np));
        if (!m.dir) {
          t.objectStore("fsdata").delete(this.key(p));
          t.objectStore("fsdata").put(datas[i], this.key(np));
        }
      });
    });
  }
  async copy(from: string, to: string) {
    if (isInside(to, from)) throw new Error("A folder cannot be copied into itself");
    return copyTree(this, from, to);
  }
  async remove(path: string) {
    checkPath(path);
    if (!path) throw new Error("refusing to delete the library root");
    await tx(["fsmeta", "fsdata"], "readwrite", (t) => {
      for (const s of ["fsmeta", "fsdata"] as const) {
        t.objectStore(s).delete(this.key(path));
        t.objectStore(s).delete(prefixRange(this.key(path + "/")));
      }
    });
  }
}
