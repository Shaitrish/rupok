// Desktop provider: a real folder on disk (default ~/Documents/Rupok),
// through the `library_*` commands in apps/desktop/src/library_cmd.rs.
// The Rust side sandboxes every path to the library root, validates `.rpk`
// bytes before writing, writes atomically (temp file, fsync, rename) and
// watches the folder, so lessons written by `rupok mcp` agents or a sync
// client show up here.

import { desktopInvoke, desktopListen } from "../platform";
import type { FsDriver, RawEntry } from "./drivers";
import type { StoreKind } from "./types";

export interface DesktopLibraryInfo {
  /** Absolute path of the library folder. */
  root: string;
  /** The default location (Documents/Rupok). */
  defaultRoot: string;
  /** True when the teacher chose another folder in Settings. */
  custom: boolean;
}

export const libraryInfo = () => desktopInvoke<DesktopLibraryInfo>("library_info");
/** Native folder picker; resolves to the new info, or null if cancelled. */
export const chooseLibraryRoot = () => desktopInvoke<DesktopLibraryInfo | null>("library_choose_root");
export const resetLibraryRoot = () => desktopInvoke<DesktopLibraryInfo>("library_reset_root");

const bytesOf = (v: ArrayBuffer | Uint8Array | number[]) =>
  v instanceof Uint8Array ? v : v instanceof ArrayBuffer ? new Uint8Array(v) : new Uint8Array(v);

export class DesktopDriver implements FsDriver {
  readonly kind: StoreKind = "desktop";
  readonly id: string;
  readonly label: string;
  constructor(readonly info: DesktopLibraryInfo) {
    this.id = `desktop:${info.root}`;
    // "Documents/Rupok" reads better than the full path.
    const parts = info.root.split(/[\\/]/).filter(Boolean);
    this.label = parts.slice(-2).join("/") || info.root;
  }
  list(dir: string) {
    return desktopInvoke<RawEntry[]>("library_list", { dir });
  }
  stat(path: string) {
    return desktopInvoke<RawEntry | null>("library_stat", { path });
  }
  async read(path: string) {
    return bytesOf(await desktopInvoke<ArrayBuffer>("library_read", { path }));
  }
  async readRange(path: string, start: number, end: number) {
    return bytesOf(await desktopInvoke<ArrayBuffer>("library_read_range", { path, start, end }));
  }
  async write(path: string, bytes: Uint8Array) {
    await desktopInvoke("library_write", bytes, { "x-rupok-path": encodeURIComponent(path) });
  }
  async mkdir(path: string) {
    await desktopInvoke("library_mkdir", { path });
  }
  async rename(from: string, to: string) {
    await desktopInvoke("library_rename", { from, to });
  }
  async copy(from: string, to: string) {
    await desktopInvoke("library_copy", { from, to });
  }
  async remove(path: string) {
    await desktopInvoke("library_remove", { path });
  }
  watch(cb: (paths: string[]) => void): () => void {
    let un: (() => void) | null = null;
    let stopped = false;
    void desktopListen<string[]>("rupok://library-changed", (paths) => cb(paths ?? [])).then((u) => (stopped ? u() : (un = u)));
    return () => {
      stopped = true;
      un?.();
    };
  }
}
