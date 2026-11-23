// What the Library UI does: new lessons, folders and courses, copies,
// import and export, Recent, and where the library lives.

import { APP_NAME } from "../brand";
import { desktopInvoke, isDesktop } from "../platform";
import { mode, showToast, studentEntry } from "../state";
import { chooseLibraryRoot, resetLibraryRoot } from "./desktop";
import {
  COPIES_DIR, LESSON_EXT, SHARED_DIR, basename, cleanName, courseFromPath, isInside, isLessonName, join, newLessonId, split, stem,
} from "./paths";
import { libPrefs, markOpened, setLibPrefs } from "./prefs";
import {
  activeLib, adoptBoard, bump, closeLesson, controllerRef, current, flush, keepReceived, received, setLibraryOpen, setReceived, store, useStore,
} from "./session";
import { type FileLibrary, copyLibrary, folderStore, pickFolder } from "./store";
import { type TemplateId, applyTemplate } from "./templates";
import { coverKey, renderCover } from "./thumbs";
import type { CopiedFrom, Entry, LessonLibraryMeta } from "./types";
import { patchLesson, summaryFromBytes, writeZip } from "./zip";

const author = () => libPrefs().author.trim() || undefined;
const need = (): FileLibrary => {
  const s = activeLib() ?? store();
  if (!s) throw new Error("The library is not available in this browser");
  return s;
};

// -------------------------------------------------------------- creating

/** Start a new lesson from a template in `dir` and open it. */
export async function newLesson(dir: string, name: string, template: TemplateId): Promise<string> {
  const s = need();
  const ctl = controllerRef();
  await closeLesson();
  const path = await s.freeLessonPath(dir, name || "Untitled lesson");
  applyTemplate(ctl.board, stem(basename(path)), template);
  if (mode() !== "prepare") ctl.setMode("prepare");
  await adoptBoard(path, s === store() ? {} : { lib: s });
  ctl.updateInsets(true);
  setLibraryOpen(false);
  return path;
}

export async function newFolder(dir: string, name: string): Promise<string> {
  const p = await need().createFolder(join(dir, cleanName(name, "New folder")));
  bump();
  return p;
}

/**
 * A course: `Subject/Course/` with numbered chapter folders
 * ("01 Number systems"), so chapters sort in teaching order.
 */
export async function newCourse(subject: string, course: string, chapters: string[]): Promise<string> {
  const s = need();
  const subj = cleanName(subject, "General");
  const subjPath = (await s.driver.stat(subj)) ? subj : await s.createFolder(subj);
  const coursePath = await s.createFolder(join(subjPath, cleanName(course, "New course")));
  const list = chapters.map((c) => c.trim()).filter(Boolean);
  const width = Math.max(2, String(list.length).length);
  for (const [i, c] of list.entries()) {
    const name = /^\d+[\s._-]/.test(c) ? c : `${String(i + 1).padStart(width, "0")} ${c}`;
    await s.createFolder(join(coursePath, cleanName(name)));
  }
  bump();
  return coursePath;
}

/**
 * Rename a lesson or folder. A lesson's manifest title follows the new name,
 * so file browsers and agents (`list_lessons`) show it before it is opened.
 */
export async function renameItem(path: string, name: string): Promise<string> {
  const s = need();
  const np = await s.rename(path, name);
  if (np !== path && isLessonName(basename(np))) {
    try {
      await s.write(np, await patchLesson(await s.read(np), { title: stem(basename(np)) }));
    } catch { /* the name is what matters; the title catches up when opened */ }
  }
  bump();
  return np;
}

// ---------------------------------------------------------------- copies

/** An independent copy that remembers where it came from (manifest `library.copiedFrom`). */
async function copyBytes(bytes: Uint8Array, toDir: string): Promise<string> {
  const s = need();
  const sum = await summaryFromBytes(bytes);
  const copiedFrom: CopiedFrom = { id: sum.library?.id ?? "", title: sum.title, at: Date.now() };
  if (sum.library?.author) copiedFrom.author = sum.library.author;
  const lib: LessonLibraryMeta = { id: newLessonId(), copiedFrom };
  if (author()) lib.author = author();
  const path = await s.freeLessonPath(toDir, sum.title);
  await s.write(path, await patchLesson(bytes, { library: lib }));
  markOpened(`${s.id}|${path}`);
  bump();
  return path;
}

/** Where copies go: students' "My copies", teachers' "My lessons". */
export const copiesDir = () => (libPrefs().role === "student" ? COPIES_DIR : "");

/** "Make a copy" of a library lesson. */
export async function makeCopy(path: string): Promise<string> {
  return copyBytes(await need().read(path), copiesDir());
}

/** "Save a copy to my library" for the lesson on the board (received or `?view`). */
export async function saveCopy(): Promise<string | null> {
  try {
    const s = need();
    let bytes: Uint8Array;
    const cur = current();
    if (studentEntry) bytes = received()?.bytes ?? controllerRef().board.save();
    else if (cur) {
      await flush();
      bytes = await s.read(cur.path);
    } else bytes = controllerRef().board.save();
    const dir = studentEntry ? COPIES_DIR : copiesDir();
    const path = await copyBytes(bytes, dir);
    if (studentEntry) {
      const r = received();
      if (r) setReceived({ ...r, savedTo: path });
    }
    showToast(`Saved a copy to ${dir || "My lessons"}: “${stem(basename(path))}”`, 3500);
    return path;
  } catch (e: any) {
    showToast(`Could not save a copy: ${e?.message ?? e}`, 5000);
    return null;
  }
}

// ---------------------------------------------------------------- import

/** Pick `.rpk` lessons and PDFs to import. */
export async function pickImportFiles(): Promise<File[]> {
  if (isDesktop) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: true, filters: [{ name: "Lessons and PDFs", extensions: ["rpk", "pdf"] }] });
    if (!picked) return [];
    const { readFile } = await import("@tauri-apps/plugin-fs");
    const out: File[] = [];
    for (const p of Array.isArray(picked) ? picked : [picked]) {
      const name = p.split(/[\\/]/).pop() || p;
      out.push(new File([await readFile(p)], name, { type: name.toLowerCase().endsWith(".pdf") ? "application/pdf" : "" }));
    }
    return out;
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.accept = `${LESSON_EXT},.pdf,application/pdf`;
    input.onchange = () => resolve(Array.from(input.files ?? []));
    input.oncancel = () => resolve([]);
    input.click();
  });
}

/** A PDF becomes a new lesson (every page a slide) through the board's PDF import. */
async function importPdfLesson(dir: string, f: File): Promise<string> {
  const s = need();
  const ctl = controllerRef();
  await closeLesson();
  const path = await s.freeLessonPath(dir, f.name.replace(/\.pdf$/i, ""));
  applyTemplate(ctl.board, stem(basename(path)), "blank");
  const { importPdf } = await import("../pdf");
  await importPdf(ctl, f);
  ctl.board.set_title(stem(basename(path)));
  ctl.board.goto_page(0);
  const bytes = ctl.board.save();
  const thumb = await renderCover(ctl.board, coverKey(ctl.board)).catch(() => null);
  const lib: LessonLibraryMeta = { id: newLessonId() };
  if (author()) lib.author = author();
  const course = courseFromPath(path) ?? undefined;
  await s.write(path, await patchLesson(bytes, { library: lib, course, ...(thumb ? { thumb } : {}) }));
  return path;
}

/** Import lessons and PDFs into `dir`. Returns the new paths. */
export async function importInto(dir: string, files: File[]): Promise<string[]> {
  const s = need();
  const out: string[] = [];
  for (const f of files) {
    try {
      if (isLessonName(f.name.toLowerCase())) {
        const bytes = new Uint8Array(await f.arrayBuffer());
        const sum = await summaryFromBytes(bytes).catch(() => {
          throw new Error("not a Rupok lesson");
        });
        if (isInside(dir, SHARED_DIR)) {
          out.push(await keepReceived(bytes, sum, f.name));
          continue;
        }
        const path = await s.freeLessonPath(dir, sum.title || stem(f.name));
        const lib = sum.library ?? { id: newLessonId(), ...(author() ? { author: author() } : {}) };
        const course = courseFromPath(path) ?? undefined;
        await s.write(path, await patchLesson(bytes, { library: lib, course }));
        out.push(path);
      } else if (f.name.toLowerCase().endsWith(".pdf") || f.type === "application/pdf") {
        out.push(await importPdfLesson(dir, f));
      } else {
        showToast(`${f.name}: only .rpk lessons and PDFs can be imported into the Library`, 4000);
      }
    } catch (e: any) {
      showToast(`Could not import ${f.name}: ${e?.message ?? e}`, 5000);
    }
  }
  bump();
  if (out.length) showToast(`Imported ${out.length} lesson${out.length === 1 ? "" : "s"}`);
  return out;
}

// ---------------------------------------------------------------- export

/** Save bytes to a file the teacher picks (desktop dialog, browser picker or download). */
async function saveAs(name: string, bytes: Uint8Array, ext: "rpk" | "zip"): Promise<string | null> {
  const mime = ext === "rpk" ? "application/vnd.rupok.lesson+zip" : "application/zip";
  if (isDesktop) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const picked = await save({ defaultPath: name, filters: [{ name: ext === "rpk" ? `${APP_NAME} lesson` : "Zip archive", extensions: [ext] }] });
    if (!picked) return null;
    const path = picked.toLowerCase().endsWith(`.${ext}`) ? picked : `${picked}.${ext}`;
    await desktopInvoke(ext === "rpk" ? "save_lesson" : "library_save_zip", bytes, { "x-rupok-path": encodeURIComponent(path) });
    return path;
  }
  const w = window as any;
  if (w.showSaveFilePicker) {
    try {
      const h = await w.showSaveFilePicker({ suggestedName: name, types: [{ description: ext === "rpk" ? `${APP_NAME} lesson` : "Zip archive", accept: { [mime]: [`.${ext}`] } }] });
      const wr = await h.createWritable();
      await wr.write(bytes);
      await wr.close();
      return h.name;
    } catch (e: any) {
      if (e?.name === "AbortError") return null;
      if (e?.name !== "SecurityError") throw e;
    }
  }
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([bytes as BlobPart], { type: mime }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
  return name;
}

export async function exportLesson(path: string) {
  try {
    const saved = await saveAs(basename(path), await need().read(path), "rpk");
    if (saved) showToast(`Exported ${saved.split(/[\\/]/).pop()}`);
  } catch (e: any) {
    showToast(`Export failed: ${e?.message ?? e}`, 5000);
  }
}

/** A folder (or the whole library) as one zip, folder layout kept. */
export async function exportFolder(path: string) {
  try {
    const s = need();
    const top = path ? basename(path) : `${APP_NAME} library`;
    const lessons = (await s.walk(path)).filter((e) => e.kind === "lesson");
    if (!lessons.length) return showToast("This folder has no lessons to export");
    const files = [];
    for (const e of lessons) {
      const rel = split(e.path).slice(split(path).length).join("/");
      files.push({ name: `${top}/${rel}`, bytes: await s.read(e.path), modified: e.modified });
    }
    const saved = await saveAs(`${top}.zip`, writeZip(files), "zip");
    if (saved) showToast(`Exported ${lessons.length} lesson${lessons.length === 1 ? "" : "s"} to ${saved.split(/[\\/]/).pop()}`);
  } catch (e: any) {
    showToast(`Export failed: ${e?.message ?? e}`, 5000);
  }
}

// ---------------------------------------------------------------- views

/** Lessons opened or changed most recently, across the whole library. */
export async function recentLessons(limit = 40): Promise<Entry[]> {
  const s = need();
  const opened = libPrefs().opened;
  const lessons = (await s.walk("", { includeShared: true })).filter((e) => e.kind === "lesson");
  const score = (e: Entry) => Math.max(opened[`${s.id}|${e.path}`] ?? 0, e.modified);
  return lessons.sort((a, b) => score(b) - score(a)).slice(0, limit);
}

export interface StorageInfo {
  usage: number;
  quota: number;
  persisted: boolean;
}

/** Browser storage used by this site against its quota. */
export async function storageEstimate(): Promise<StorageInfo | null> {
  if (isDesktop || !navigator.storage?.estimate) return null;
  try {
    const { usage = 0, quota = 0 } = await navigator.storage.estimate();
    const persisted = (await navigator.storage.persisted?.()) ?? false;
    return { usage, quota, persisted };
  } catch {
    return null;
  }
}

/** Ask the browser not to clear the library when space runs low. */
export async function persistStorage(): Promise<boolean> {
  try {
    return (await navigator.storage?.persist?.()) ?? false;
  } catch {
    return false;
  }
}

// ------------------------------------------------------------- location

/** Browser: keep the library in a real folder (File System Access API). */
export async function useFolder(copyExisting: (n: number) => Promise<boolean>): Promise<boolean> {
  const h = await pickFolder();
  if (!h) return false;
  await closeLesson();
  const from = store();
  const to = folderStore(h);
  if (from && from.kind !== "folder") {
    const n = (await from.walk("", { includeShared: true }).catch(() => [])).filter((e) => e.kind === "lesson").length;
    if (n && (await copyExisting(n))) {
      const copied = await copyLibrary(from, to);
      showToast(`Copied ${copied} lesson${copied === 1 ? "" : "s"} into “${h.name}”`);
    }
  }
  setLibPrefs({ location: "folder" });
  await useStore();
  return true;
}

/** Browser: back to the private browser storage. */
export async function useBrowserStorage() {
  await closeLesson();
  setLibPrefs({ location: "browser" });
  await useStore();
}

/** Desktop: choose another library folder. */
export async function changeDesktopFolder(): Promise<boolean> {
  await closeLesson();
  const info = await chooseLibraryRoot();
  if (!info) return false;
  await useStore();
  return true;
}

export async function resetDesktopFolder() {
  await closeLesson();
  await resetLibraryRoot();
  await useStore();
}
