// A small ZIP reader/writer for `.rpk` packages (spec/rupok-format.md).
//
// - `readSummary` reads only the central directory, `manifest.json` and
//   `thumbs/0.png`, through a range reader, so browsing a library of large
//   lessons never inflates their documents or media.
// - `patchLesson` rewrites a package with a new `manifest.json` (the
//   `course` and `library` keys) and a fresh `thumbs/0.png`, copying every
//   other entry byte for byte. The engine writes the document; this adds
//   what only the Library knows.
// - `writeZip` builds a stored ZIP (folder export).

import type { CourseRef, LessonLibraryMeta, LessonSummary } from "./types";

export interface ZipEntry {
  name: string;
  flags: number;
  method: number;
  time: number;
  date: number;
  crc: number;
  csize: number;
  usize: number;
  /** Offset of the local file header. */
  offset: number;
}

/** Reads bytes [start, end) of the archive. */
export type RangeReader = (start: number, end: number) => Promise<Uint8Array>;

const SIG_LOCAL = 0x04034b50;
const SIG_CENTRAL = 0x02014b50;
const SIG_EOCD = 0x06054b50;
const SIG_Z64_LOC = 0x07064b50;
const SIG_Z64_EOCD = 0x06064b50;
const MAX_ENTRIES = 100_000;
const MAX_MANIFEST = 1 << 20;
const MAX_THUMB = 8 << 20;

const dv = (b: Uint8Array) => new DataView(b.buffer, b.byteOffset, b.byteLength);
const utf8 = new TextDecoder("utf-8");
const enc = new TextEncoder();

/** Parse the central directory of an archive of `size` bytes. */
export async function readCentral(size: number, read: RangeReader): Promise<ZipEntry[]> {
  if (size < 22) throw new Error("not a zip file");
  const tailStart = Math.max(0, size - (65535 + 22));
  const tail = await read(tailStart, size);
  const t = dv(tail);
  let eocd = -1;
  for (let i = tail.length - 22; i >= 0; i--) {
    if (t.getUint32(i, true) === SIG_EOCD) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error("not a zip file (no end of central directory)");
  let count = t.getUint16(eocd + 10, true);
  let cdSize = t.getUint32(eocd + 12, true);
  let cdOffset = t.getUint32(eocd + 16, true);
  if (count === 0xffff || cdSize === 0xffffffff || cdOffset === 0xffffffff) {
    // ZIP64: the locator sits right before the classic record.
    const loc = eocd - 20;
    if (loc < 0 || t.getUint32(loc, true) !== SIG_Z64_LOC) throw new Error("bad zip64 locator");
    const recOff = Number(t.getBigUint64(loc + 8, true));
    const rec = await read(recOff, recOff + 56);
    const r = dv(rec);
    if (r.getUint32(0, true) !== SIG_Z64_EOCD) throw new Error("bad zip64 record");
    count = Number(r.getBigUint64(32, true));
    cdSize = Number(r.getBigUint64(40, true));
    cdOffset = Number(r.getBigUint64(48, true));
  }
  if (count > MAX_ENTRIES || cdOffset + cdSize > size) throw new Error("corrupt zip directory");
  const cd = cdOffset >= tailStart ? tail.subarray(cdOffset - tailStart, cdOffset - tailStart + cdSize) : await read(cdOffset, cdOffset + cdSize);
  const c = dv(cd);
  const out: ZipEntry[] = [];
  let p = 0;
  for (let i = 0; i < count; i++) {
    if (p + 46 > cd.length || c.getUint32(p, true) !== SIG_CENTRAL) throw new Error("corrupt zip directory entry");
    const nameLen = c.getUint16(p + 28, true);
    const extraLen = c.getUint16(p + 30, true);
    const commentLen = c.getUint16(p + 32, true);
    const e: ZipEntry = {
      flags: c.getUint16(p + 8, true),
      method: c.getUint16(p + 10, true),
      time: c.getUint16(p + 12, true),
      date: c.getUint16(p + 14, true),
      crc: c.getUint32(p + 16, true),
      csize: c.getUint32(p + 20, true),
      usize: c.getUint32(p + 24, true),
      offset: c.getUint32(p + 42, true),
      name: utf8.decode(cd.subarray(p + 46, p + 46 + nameLen)),
    };
    // ZIP64 extra field (id 1): the 0xffffffff fields, in this order.
    if (e.usize === 0xffffffff || e.csize === 0xffffffff || e.offset === 0xffffffff) {
      let x = p + 46 + nameLen;
      const end = x + extraLen;
      while (x + 4 <= end) {
        const id = c.getUint16(x, true);
        const len = c.getUint16(x + 2, true);
        if (id === 1) {
          let q = x + 4;
          if (e.usize === 0xffffffff) { e.usize = Number(c.getBigUint64(q, true)); q += 8; }
          if (e.csize === 0xffffffff) { e.csize = Number(c.getBigUint64(q, true)); q += 8; }
          if (e.offset === 0xffffffff) { e.offset = Number(c.getBigUint64(q, true)); }
          break;
        }
        x += 4 + len;
      }
    }
    out.push(e);
    p += 46 + nameLen + extraLen + commentLen;
  }
  return out;
}

/** Where an entry's (compressed) data starts. */
async function dataOffset(e: ZipEntry, read: RangeReader): Promise<number> {
  const h = await read(e.offset, e.offset + 30);
  const v = dv(h);
  if (h.length < 30 || v.getUint32(0, true) !== SIG_LOCAL) throw new Error(`corrupt local header for ${e.name}`);
  return e.offset + 30 + v.getUint16(26, true) + v.getUint16(28, true);
}

async function inflateRaw(data: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([data as BlobPart]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

/** The uncompressed bytes of one entry (Stored or Deflate). */
export async function readEntry(e: ZipEntry, read: RangeReader, max = Infinity): Promise<Uint8Array> {
  if (e.flags & 1) throw new Error("encrypted entries are not supported");
  if (e.usize > max || e.csize > max) throw new Error(`${e.name} is too large`);
  const start = await dataOffset(e, read);
  const raw = await read(start, start + e.csize);
  if (e.method === 0) return raw;
  if (e.method === 8) return inflateRaw(raw);
  throw new Error(`unsupported compression method ${e.method}`);
}

/** Title, pages, course, library metadata and cover thumbnail of a package. */
export async function readSummary(size: number, read: RangeReader): Promise<LessonSummary> {
  const entries = await readCentral(size, read);
  const me = entries.find((e) => e.name === "manifest.json");
  if (!me) throw new Error("not a lesson (no manifest.json)");
  const m = JSON.parse(utf8.decode(await readEntry(me, read, MAX_MANIFEST)));
  if (typeof m !== "object" || !m || typeof m.title !== "string") throw new Error("invalid manifest");
  const te = entries.find((e) => e.name === "thumbs/0.png");
  let thumb: Uint8Array | null = null;
  if (te) {
    try {
      thumb = await readEntry(te, read, MAX_THUMB);
    } catch {
      thumb = null;
    }
  }
  return {
    title: m.title,
    created: Number(m.created) || 0,
    modified: Number(m.modified) || 0,
    pages: Number(m.pages) || 0,
    course: m.course && typeof m.course === "object" ? m.course : undefined,
    library: sanitizeLibrary(m.library),
    thumb,
  };
}

/** Summary of a package held in memory. */
export function summaryFromBytes(bytes: Uint8Array): Promise<LessonSummary> {
  return readSummary(bytes.length, async (s, e) => bytes.subarray(s, Math.min(e, bytes.length)));
}

function sanitizeLibrary(v: unknown): LessonLibraryMeta | undefined {
  if (!v || typeof v !== "object") return undefined;
  const o = v as Record<string, unknown>;
  if (typeof o.id !== "string" || !o.id) return undefined;
  const out: LessonLibraryMeta = { id: o.id.slice(0, 64) };
  if (typeof o.author === "string" && o.author) out.author = o.author.slice(0, 120);
  const c = o.copiedFrom as Record<string, unknown> | undefined;
  if (c && typeof c === "object" && typeof c.id === "string") {
    out.copiedFrom = {
      id: c.id.slice(0, 64),
      title: typeof c.title === "string" ? c.title.slice(0, 200) : "",
      at: Number(c.at) || 0,
    };
    if (typeof c.author === "string" && c.author) out.copiedFrom.author = c.author.slice(0, 120);
  }
  return out;
}

// --------------------------------------------------------------- writing

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

export function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function dosTime(ms: number): { time: number; date: number } {
  const d = new Date(ms);
  return {
    time: (d.getHours() << 11) | (d.getMinutes() << 5) | (d.getSeconds() >> 1),
    date: (Math.max(0, d.getFullYear() - 1980) << 9) | ((d.getMonth() + 1) << 5) | d.getDate(),
  };
}

interface OutEntry {
  name: Uint8Array;
  flags: number;
  method: number;
  time: number;
  date: number;
  crc: number;
  csize: number;
  usize: number;
  data: Uint8Array;
}

/** Serialise entries as a classic (non-ZIP64) archive. Null if too big. */
function serialize(entries: OutEntry[]): Uint8Array | null {
  let size = 22;
  for (const e of entries) size += 30 + e.name.length + e.data.length + 46 + e.name.length;
  if (size > 0xfffffff0 || entries.length > 0xfffe) return null;
  const out = new Uint8Array(size);
  const v = dv(out);
  let p = 0;
  const offsets: number[] = [];
  for (const e of entries) {
    offsets.push(p);
    v.setUint32(p, SIG_LOCAL, true);
    v.setUint16(p + 4, e.method === 8 ? 20 : 10, true);
    v.setUint16(p + 6, e.flags, true);
    v.setUint16(p + 8, e.method, true);
    v.setUint16(p + 10, e.time, true);
    v.setUint16(p + 12, e.date, true);
    v.setUint32(p + 14, e.crc, true);
    v.setUint32(p + 18, e.csize, true);
    v.setUint32(p + 22, e.usize, true);
    v.setUint16(p + 26, e.name.length, true);
    v.setUint16(p + 28, 0, true);
    out.set(e.name, p + 30);
    out.set(e.data, p + 30 + e.name.length);
    p += 30 + e.name.length + e.data.length;
  }
  const cdStart = p;
  entries.forEach((e, i) => {
    v.setUint32(p, SIG_CENTRAL, true);
    v.setUint16(p + 4, 20, true);
    v.setUint16(p + 6, e.method === 8 ? 20 : 10, true);
    v.setUint16(p + 8, e.flags, true);
    v.setUint16(p + 10, e.method, true);
    v.setUint16(p + 12, e.time, true);
    v.setUint16(p + 14, e.date, true);
    v.setUint32(p + 16, e.crc, true);
    v.setUint32(p + 20, e.csize, true);
    v.setUint32(p + 24, e.usize, true);
    v.setUint16(p + 28, e.name.length, true);
    // extra, comment, disk, internal attrs = 0
    v.setUint32(p + 38, 0, true);
    v.setUint32(p + 42, offsets[i], true);
    out.set(e.name, p + 46);
    p += 46 + e.name.length;
  });
  v.setUint32(p, SIG_EOCD, true);
  v.setUint16(p + 8, entries.length, true);
  v.setUint16(p + 10, entries.length, true);
  v.setUint32(p + 12, p - cdStart, true);
  v.setUint32(p + 16, cdStart, true);
  return out;
}

function stored(name: string, data: Uint8Array, ms = Date.now()): OutEntry {
  const nb = enc.encode(name);
  const { time, date } = dosTime(ms);
  // Bit 11: the name is UTF-8.
  return { name: nb, flags: 0x0800, method: 0, time, date, crc: crc32(data), csize: data.length, usize: data.length, data };
}

/** A stored ZIP of `files` (already-compressed `.rpk` files gain nothing from deflate). */
export function writeZip(files: { name: string; bytes: Uint8Array; modified?: number }[]): Uint8Array {
  const out = serialize(files.map((f) => stored(f.name, f.bytes, f.modified || Date.now())));
  if (!out) throw new Error("The folder is too large to export as one zip (over 4 GB)");
  return out;
}

export interface LessonPatch {
  /** Replace manifest `title` (the document keeps its own until next opened). */
  title?: string;
  /** Replace manifest `library` (undefined keeps it). */
  library?: LessonLibraryMeta;
  /** Replace manifest `course` (undefined keeps it; an empty object removes it). */
  course?: CourseRef;
  /** New cover thumbnail (`thumbs/0.png`); stale page thumbnails are dropped. Undefined keeps them. */
  thumb?: Uint8Array | null;
}

/**
 * Rewrite a package with an updated `manifest.json` and cover thumbnail.
 * Every other entry is copied unchanged; `mimetype` stays first and
 * Stored. Returns the input unchanged if it cannot be patched safely.
 */
export async function patchLesson(bytes: Uint8Array, patch: LessonPatch): Promise<Uint8Array> {
  const read: RangeReader = async (s, e) => bytes.subarray(s, Math.min(e, bytes.length));
  let entries: ZipEntry[];
  try {
    entries = await readCentral(bytes.length, read);
  } catch {
    return bytes;
  }
  const me = entries.find((e) => e.name === "manifest.json");
  if (!me || entries.some((e) => e.flags & 1)) return bytes;
  const manifest = JSON.parse(utf8.decode(await readEntry(me, read, MAX_MANIFEST)));
  if (patch.title !== undefined) manifest.title = patch.title;
  if (patch.library !== undefined) manifest.library = patch.library;
  if (patch.course !== undefined) {
    const c: Record<string, string> = {};
    for (const k of ["subject", "course", "chapter"] as const) if (patch.course[k]) c[k] = patch.course[k]!;
    if (Object.keys(c).length) manifest.course = c;
    else delete manifest.course;
  }
  const out: OutEntry[] = [];
  for (const e of entries) {
    if (e.name === "manifest.json") {
      out.push(stored("manifest.json", enc.encode(JSON.stringify(manifest, null, 2)), Number(manifest.modified) || Date.now()));
      continue;
    }
    if (patch.thumb !== undefined && e.name.startsWith("thumbs/")) continue;
    const start = await dataOffset(e, read);
    out.push({
      name: enc.encode(e.name),
      flags: e.flags & ~0x0008, // sizes are in the local header: no data descriptor
      method: e.method,
      time: e.time,
      date: e.date,
      crc: e.crc,
      csize: e.csize,
      usize: e.usize,
      data: bytes.subarray(start, start + e.csize),
    });
  }
  if (patch.thumb) out.push(stored("thumbs/0.png", patch.thumb));
  return serialize(out) ?? bytes;
}
