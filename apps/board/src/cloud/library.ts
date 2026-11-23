// Rupok Cloud as a Library provider. `CloudDriver` maps the Library's path
// primitives (`Subject/Course/Chapter/lesson.rpk`) onto the cloud's folder
// tree of node ids; `CloudLibrary` uses the server's own trash, copies and
// metadata (no summary downloads while browsing), and `LessonSync` keeps an
// open cloud lesson in sync with Loro updates (push / pull), falling back
// to whole-file saves that the server merges.

import { type FsDriver, type RawEntry, ExistsError } from "../library/drivers";
import { LESSON_EXT, basename, dirname, isLessonName, join, split, stem } from "../library/paths";
import { FileLibrary } from "../library/store";
import type { Entry, StoreKind, TrashEntry } from "../library/types";
import { readCentral, readEntry } from "../library/zip";
import { type CloudApi, CloudError, type CloudNode, type VV } from "./api";
import { covers, decodeVV, encodeVV } from "./vv";

const fileName = (n: CloudNode) => (n.kind === "lesson" ? n.name + LESSON_EXT : n.name);

export class CloudDriver implements FsDriver {
  readonly kind: StoreKind = "cloud";
  readonly label = "Rupok Cloud";
  private dirs = new Map<string, { at: number; nodes: CloudNode[] }>();
  constructor(readonly api: CloudApi, readonly id: string) {}

  invalidate(dir?: string) {
    if (dir === undefined) this.dirs.clear();
    else this.dirs.delete(dir);
  }
  async children(dir: string): Promise<CloudNode[]> {
    const hit = this.dirs.get(dir);
    if (hit && Date.now() - hit.at < 4000) return hit.nodes;
    const parent = dir ? await this.node(dir) : null;
    if (dir && (!parent || parent.kind !== "folder")) throw new Error(`folder “${dir}” does not exist`);
    const nodes = (await this.api.list(parent?.id ?? null)).filter((n) => !n.trashed_at);
    this.dirs.set(dir, { at: Date.now(), nodes });
    return nodes;
  }
  /** The node at a path, or null. */
  async node(path: string): Promise<CloudNode | null> {
    if (!path) return null;
    const nodes = await this.children(dirname(path)).catch(() => [] as CloudNode[]);
    return nodes.find((n) => fileName(n) === basename(path)) ?? null;
  }
  async need(path: string): Promise<CloudNode> {
    const n = await this.node(path);
    if (!n) throw new Error(`“${stem(basename(path))}” no longer exists`);
    return n;
  }
  private raw(n: CloudNode): RawEntry {
    return { name: fileName(n), dir: n.kind === "folder", size: n.lesson?.size ?? 0, mtime: n.updated_at };
  }
  async list(dir: string) {
    return (await this.children(dir)).map((n) => this.raw(n));
  }
  async stat(path: string) {
    if (!path) return { name: "", dir: true, size: 0, mtime: 0 };
    const n = await this.node(path);
    return n ? this.raw(n) : null;
  }
  async read(path: string) {
    return this.api.download((await this.need(path)).id);
  }
  async write(path: string, bytes: Uint8Array) {
    const n = await this.node(path);
    if (n) await this.api.replace(n.id, bytes);
    else {
      await this.mkdir(dirname(path));
      const parent = dirname(path) ? await this.need(dirname(path)) : null;
      await this.api.upload(bytes, stem(basename(path)), parent?.id ?? null);
    }
    this.invalidate(dirname(path));
  }
  async mkdir(path: string) {
    let cur = "";
    for (const part of split(path)) {
      const next = join(cur, part);
      const n = await this.node(next);
      if (!n) {
        const parent = cur ? await this.need(cur) : null;
        await this.api.createFolder(part, parent?.id ?? null);
        this.invalidate(cur);
      } else if (n.kind !== "folder") throw new ExistsError(next);
      cur = next;
    }
  }
  async rename(from: string, to: string) {
    const n = await this.need(from);
    if (await this.node(to)) throw new ExistsError(to);
    const name = n.kind === "lesson" ? stem(basename(to)) : basename(to);
    const patch: { name?: string; parent_id?: string | null } = { name };
    if (dirname(from) !== dirname(to)) {
      await this.mkdir(dirname(to));
      patch.parent_id = dirname(to) ? (await this.need(dirname(to))).id : null;
    }
    await this.api.update(n.id, patch);
    this.invalidate();
  }
  async copy(from: string, to: string) {
    const n = await this.need(from);
    const parent = dirname(to) ? await this.need(dirname(to)) : null;
    const c = await this.api.copy(n.id, parent?.id ?? null);
    const want = n.kind === "lesson" ? stem(basename(to)) : basename(to);
    if (c.name !== want) await this.api.update(c.id, { name: want }).catch(() => {});
    this.invalidate();
  }
  async remove(path: string) {
    await this.api.trash((await this.need(path)).id);
    this.invalidate();
  }
}

export class CloudLibrary extends FileLibrary {
  private thumbs = new Map<string, { v: number; url: string | null }>();
  constructor(readonly cloudDriver: CloudDriver) {
    super(cloudDriver);
  }
  get api() {
    return this.cloudDriver.api;
  }
  node(path: string) {
    return this.cloudDriver.node(path);
  }

  /** Metadata comes from the server's node; thumbnails from the lesson's entries. */
  protected override async fill(e: Entry): Promise<Entry> {
    if (e.kind !== "lesson") return e;
    const n = await this.cloudDriver.node(e.path);
    if (!n?.lesson) return e;
    e.title = n.lesson.title;
    e.pages = n.lesson.pages;
    e.id = n.id;
    const v = n.lesson.content_version;
    const hit = this.thumbs.get(n.id);
    if (hit && hit.v === v) e.thumb = hit.url;
    else {
      let url: string | null = null;
      try {
        const png = await this.api.bytes("GET", `/lessons/${encodeURIComponent(n.id)}/entries/thumbs/0.png`);
        url = URL.createObjectURL(new Blob([png as BlobPart], { type: "image/png" }));
      } catch { /* no thumbnail */ }
      if (hit?.url) URL.revokeObjectURL(hit.url);
      this.thumbs.set(n.id, { v, url });
      e.thumb = url;
    }
    return e;
  }

  override async duplicate(path: string): Promise<string> {
    const n = await this.cloudDriver.need(path);
    const c = await this.api.copy(n.id, n.parent_id);
    const want = `${n.name} (copy)`;
    const taken = await this.names(dirname(path));
    let name = want;
    for (let i = 2; taken.has(n.kind === "lesson" ? name + LESSON_EXT : name); i++) name = `${n.name} (copy ${i})`;
    await this.api.update(c.id, { name }).catch(() => {});
    this.cloudDriver.invalidate();
    return join(dirname(path), n.kind === "lesson" ? name + LESSON_EXT : name);
  }

  override async trash(path: string): Promise<void> {
    await this.cloudDriver.remove(path);
    this.notify([path]);
  }
  override async listTrash(): Promise<TrashEntry[]> {
    const items = await this.api.listTrash();
    return items.map((n) => ({
      kind: n.kind, path: fileName(n), name: n.name, size: n.lesson?.size ?? 0, modified: n.updated_at,
      title: n.lesson?.title, pages: n.lesson?.pages, id: n.id, thumb: null,
      trashId: n.id, from: fileName(n), trashedAt: n.trashed_at ?? n.updated_at,
    }));
  }
  override async restore(trashId: string): Promise<string> {
    await this.api.restore(trashId);
    this.cloudDriver.invalidate();
    return trashId;
  }
  override async deletePermanently(trashId: string): Promise<void> {
    await this.api.deleteForever(trashId);
  }
  override async emptyTrash(): Promise<void> {
    await this.api.json("DELETE", "/trash");
  }
  override async write(path: string, bytes: Uint8Array): Promise<Entry> {
    await this.cloudDriver.write(path, bytes);
    this.notify([path]);
    const e = await this.stat(path);
    if (!e) throw new Error("The lesson was not saved to Rupok Cloud");
    return e;
  }
  override watch(): () => void {
    return () => {};
  }
}

// ---------------------------------------------------------------- sync

/** The parts of the board sync needs. */
export interface SyncBoard {
  live_version(): Uint8Array;
  live_export_updates(from: Uint8Array): Uint8Array;
  live_import_updates(bytes: Uint8Array): void;
}

const SKIP = new Set(["mimetype", "manifest.json", "doc.loro"]);

/** Keeps one open cloud lesson in sync. */
export class LessonSync {
  private serverVV: VV | null;
  private entries = new Map<string, number>();
  private thumbCrc = -1;
  constructor(readonly api: CloudApi, readonly lessonId: string) {
    this.serverVV = this.loadVV();
  }
  private key() {
    return `rupok.cloud.vv.${this.lessonId}`;
  }
  private loadVV(): VV | null {
    try {
      return JSON.parse(localStorage.getItem(this.key()) || "null");
    } catch {
      return null;
    }
  }
  private setVV(vv: VV) {
    this.serverVV = vv;
    try {
      localStorage.setItem(this.key(), JSON.stringify(vv));
    } catch { /* storage unavailable */ }
  }

  /** Right after the lesson was downloaded and loaded: the server has exactly this. */
  async opened(board: SyncBoard) {
    this.setVV(decodeVV(board.live_version()));
    try {
      for (const e of await this.api.entries(this.lessonId)) this.entries.set(e.path, e.size);
    } catch { /* uploaded again on demand */ }
  }

  /** Upload assets the server lacks (they must arrive before the update that uses them). */
  private async putEntries(rpk: Uint8Array) {
    const read = async (a: number, b: number) => rpk.subarray(a, b);
    for (const e of await readCentral(rpk.length, read)) {
      if (SKIP.has(e.name) || e.name.endsWith("/")) continue;
      if (e.name.startsWith("thumbs/")) {
        if (e.name !== "thumbs/0.png" || e.crc === this.thumbCrc) continue;
        this.thumbCrc = e.crc;
        await this.api.putEntry(this.lessonId, e.name, await readEntry(e, read)).catch(() => {});
        continue;
      }
      if (this.entries.has(e.name)) continue;
      await this.api.putEntry(this.lessonId, e.name, await readEntry(e, read));
      this.entries.set(e.name, e.usize);
    }
  }

  /** Push local changes. Falls back to a whole-file save the server merges. */
  async save(board: SyncBoard, rpk: Uint8Array): Promise<"pushed" | "nothing" | "replaced"> {
    try {
      await this.putEntries(rpk);
      const local = decodeVV(board.live_version());
      if (this.serverVV && covers(this.serverVV, local)) return "nothing";
      let from = this.serverVV ?? {};
      for (let attempt = 0; attempt < 2; attempt++) {
        try {
          const r = await this.api.push(this.lessonId, board.live_export_updates(encodeVV(from)));
          this.setVV(r.head ?? local);
          return "pushed";
        } catch (e) {
          if (e instanceof CloudError && e.code === "missing_deps" && e.details?.head && attempt === 0) {
            from = e.details.head as VV;
            continue;
          }
          throw e;
        }
      }
      throw new Error("push failed");
    } catch (e) {
      if (e instanceof CloudError && (e.isQuota || e.isAuth || e.isOffline || e.status === 403 || e.status === 404 || e.status === 410)) throw e;
      await this.api.replace(this.lessonId, rpk);
      this.setVV(decodeVV(board.live_version()));
      return "replaced";
    }
  }

  /** Bring in what other devices pushed. Returns the number of updates imported. */
  async pull(board: SyncBoard): Promise<number> {
    let n = 0;
    for (let round = 0; round < 8; round++) {
      const { updates, more } = await this.api.pull(this.lessonId, decodeVV(board.live_version()));
      for (const u of updates) board.live_import_updates(u);
      n += updates.length;
      if (!more) break;
    }
    return n;
  }
}

/** True for a library path that is a lesson. */
export const isCloudLesson = (path: string) => isLessonName(basename(path));
