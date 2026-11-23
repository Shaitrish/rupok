// Library paths: relative, `/`-separated, never `..`. The same rules are
// enforced again on the Rust side for the desktop provider
// (apps/desktop/src/library.rs).

import { FILE_EXT } from "../brand";
import type { CourseRef } from "./types";

export const LESSON_EXT = `.${FILE_EXT}`;
/** Lessons opened from files or links (sidebar: "Shared with me"). */
export const SHARED_DIR = "Shared with me";
/** Students' own copies of received lessons. */
export const COPIES_DIR = "My copies";
/** Trashed items; a dot folder, so agents (`rupok mcp`) and most tools skip it. */
export const TRASH_DIR = ".trash";

export const split = (p: string): string[] => p.split("/").filter(Boolean);
export const join = (...parts: string[]): string => parts.flatMap(split).join("/");
export const dirname = (p: string): string => split(p).slice(0, -1).join("/");
export const basename = (p: string): string => split(p).pop() ?? "";
export const isLessonName = (name: string): boolean => name.toLowerCase().endsWith(LESSON_EXT) && name.length > LESSON_EXT.length;
/** File name without the `.rpk` extension. */
export const stem = (name: string): string => (isLessonName(name) ? name.slice(0, -LESSON_EXT.length) : name);
/** True when `path` is `dir` or inside it ("" contains everything). */
export const isInside = (path: string, dir: string): boolean => !dir || path === dir || path.startsWith(dir + "/");

/** Throws unless `path` is a safe relative library path ("" is the root). */
export function checkPath(path: string): string {
  if (path.includes("\0") || path.includes("\\") || path.startsWith("/")) throw new Error(`Invalid path: ${path}`);
  const parts = path.split("/");
  if (path && parts.some((p) => !p || p === "." || p === "..")) throw new Error(`Invalid path: ${path}`);
  return path;
}

const RESERVED = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;

/**
 * A file or folder name that is safe on Windows, macOS, Linux and sync
 * clients: no separators or reserved characters, no leading dots (hidden),
 * no trailing dots or spaces, at most 120 characters.
 */
export function cleanName(name: string, fallback = "Untitled"): string {
  let n = name
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .replace(/[\\/:*?"<>|]+/g, "-")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^[.\s]+/, "")
    .replace(/[.\s]+$/, "");
  if (n.length > 120) n = n.slice(0, 120).trim();
  if (RESERVED.test(n)) n += "_";
  return n || fallback;
}

/** `base.ext`, or `base (2).ext`, `base (3).ext`… whichever is free. */
export function uniqueName(taken: Set<string>, base: string, ext = ""): string {
  const lower = new Set([...taken].map((t) => t.toLowerCase()));
  let name = `${base}${ext}`;
  for (let i = 2; lower.has(name.toLowerCase()); i++) name = `${base} (${i})${ext}`;
  return name;
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
/** Natural order: "2 Fractions" before "10 Decimals". */
export const naturalCompare = (a: string, b: string): number => collator.compare(a, b);

/** "01 Number systems" → "Number systems" (the ordering prefix is not part of the name). */
export const withoutOrder = (name: string): string => name.replace(/^\d+[\s._-]+(?=\S)/, "");

/** Special top-level folders that are not part of the course tree. */
export const isSpecial = (path: string): boolean => isInside(path, SHARED_DIR) || isInside(path, COPIES_DIR) || isInside(path, TRASH_DIR);

/**
 * The course a lesson belongs to, from its folder: `Subject/Course/Chapter/
 * lesson.rpk`. Null for lessons in the special folders (they keep the
 * `course` they arrived with).
 */
export function courseFromPath(path: string): CourseRef | null {
  if (isSpecial(path)) return null;
  const dirs = split(dirname(path));
  const ref: CourseRef = {};
  if (dirs[0]) ref.subject = withoutOrder(dirs[0]);
  if (dirs[1]) ref.course = withoutOrder(dirs[1]);
  if (dirs[2]) ref.chapter = withoutOrder(dirs[2]);
  return ref;
}

/** What a folder is in the course tree, by depth. */
export function folderRole(path: string): "subject" | "course" | "chapter" | "folder" {
  if (isSpecial(path)) return "folder";
  const d = split(path).length;
  return d === 1 ? "subject" : d === 2 ? "course" : d === 3 ? "chapter" : "folder";
}

/** A short random id: `l_` + 16 hex digits. */
export function newLessonId(): string {
  const b = new Uint8Array(8);
  crypto.getRandomValues(b);
  return "l_" + Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}
