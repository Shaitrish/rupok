// Library types: the storage abstraction every provider implements.
//
// A library is a plain folder of `.rpk` files: `Subject/Course/Chapter/
// lesson.rpk` (see docs/library.md). The same layout is used on every
// platform, so the desktop app, the browser, `rupok mcp` agents and sync
// clients (Drive, OneDrive) all see the same files.

/** Teacher or student: changes the Library's defaults, never what is allowed. */
export type Role = "teacher" | "student";

/** Where the lesson sits in a course; mirrors manifest `course`. */
export interface CourseRef {
  subject?: string;
  course?: string;
  chapter?: string;
}

/** The lesson a copy was made from ("Make a copy" / "Save a copy"). */
export interface CopiedFrom {
  id: string;
  title: string;
  author?: string;
  /** Wall-clock time of the copy (ms since the Unix epoch). */
  at: number;
}

/**
 * Library metadata stored in the lesson manifest under the `library` key
 * (readers ignore unknown manifest keys; see docs/library.md#lesson-metadata).
 */
export interface LessonLibraryMeta {
  /** Stable lesson id: kept across renames and moves, new for every copy. */
  id: string;
  /** Name of the person who made the lesson (Settings → Library → Your name). */
  author?: string;
  copiedFrom?: CopiedFrom;
}

/** The summary of an `.rpk` read without decoding the document. */
export interface LessonSummary {
  title: string;
  created: number;
  modified: number;
  pages: number;
  course?: CourseRef;
  library?: LessonLibraryMeta;
  /** PNG bytes of `thumbs/0.png`, when present. */
  thumb?: Uint8Array | null;
}

/** A folder or a lesson in the library. */
export interface Entry {
  kind: "folder" | "lesson";
  /** Path relative to the library root, `/`-separated, no leading slash. */
  path: string;
  /** Folder name, or the lesson file name without `.rpk`. */
  name: string;
  /** Size in bytes (lessons). */
  size: number;
  /** Last modification time (ms since the Unix epoch); 0 when unknown. */
  modified: number;
  // Lessons only (from the manifest):
  title?: string;
  pages?: number;
  course?: CourseRef;
  id?: string;
  author?: string;
  copiedFrom?: CopiedFrom;
  /** Object URL of the cover thumbnail, or null when the file has none. */
  thumb?: string | null;
}

/** An item in the trash. */
export interface TrashEntry extends Entry {
  trashId: string;
  /** Where it was before it was trashed. */
  from: string;
  trashedAt: number;
}

/** Which kind of storage backs the library. */
export type StoreKind = "desktop" | "opfs" | "folder" | "idb" | "memory" | "cloud";

/**
 * The storage abstraction. Paths are relative to the library root. Every
 * provider shares one implementation (`FileLibrary`) over a small file
 * system driver (`FsDriver`), so trash, duplicates and metadata behave the
 * same everywhere; a cloud provider can implement this interface directly.
 */
export interface LibraryStore {
  readonly kind: StoreKind;
  /** Stable id of this storage location (cache keys, locks, sessions). */
  readonly id: string;
  /** Human label: "Documents/Rupok", "This browser", a folder name. */
  readonly label: string;
  /** Direct children of a folder ("" is the root), with lesson metadata. */
  list(dir: string): Promise<Entry[]>;
  /** Every folder and lesson below `dir`, recursively (not the trash). */
  walk(dir?: string, opts?: { includeShared?: boolean }): Promise<Entry[]>;
  /** One entry with metadata, or null when it does not exist. */
  stat(path: string): Promise<Entry | null>;
  read(path: string): Promise<Uint8Array>;
  /** Write a lesson crash-safely (temporary file, then rename). */
  write(path: string, bytes: Uint8Array): Promise<Entry>;
  /** Create a folder (and its parents); returns the path actually created. */
  createFolder(path: string): Promise<string>;
  /** Move into another folder; returns the new path. */
  move(path: string, toDir: string): Promise<string>;
  /** Rename in place; returns the new path. */
  rename(path: string, newName: string): Promise<string>;
  /** Copy next to the original ("Name (copy)"); returns the new path. */
  duplicate(path: string): Promise<string>;
  trash(path: string): Promise<void>;
  listTrash(): Promise<TrashEntry[]>;
  /** Put a trashed item back where it was; returns its path. */
  restore(trashId: string): Promise<string>;
  deletePermanently(trashId: string): Promise<void>;
  emptyTrash(): Promise<void>;
  /** Changes made outside this tab (agents, sync clients, other tabs). */
  watch(cb: (paths: string[]) => void): () => void;
}
