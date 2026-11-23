// The Library session: which lesson the board is editing, and saving it back
// to its own file.
//
// - Landing: `/board/` opens the Library when no lesson is open. A reload
//   keeps the open lesson (sessionStorage). Before the first-run choice the
//   board starts straight on a lesson, as it always did (a fresh board, or
//   the last lesson), so trying Rupok from the website stays one click.
// - Autosave: the controller's autosave (storage.ts, a recovery copy in
//   IndexedDB) hands its bytes here; they are patched with the course,
//   library metadata and a cover thumbnail, and written crash-safely to the
//   lesson's file. The board's title is the file name.
// - Safety: a lesson is edited in one tab at a time (lock.ts); a document
//   that is not the bound lesson is never written over it; a file changed
//   on disk by an agent or a sync client is reloaded (no local changes) or
//   offered as a choice (local changes).

import { createSignal } from "solid-js";
import type { Controller } from "../controller";
import { forgetLessonPath, isDesktop, type LessonFile } from "../platform";
import { mode, setLessonOpened, setSavedRevision, showToast, studentEntry } from "../state";
import { autosaveLoad } from "../storage";
import type { DesktopDriver } from "./desktop";
import { idbGet, idbPut } from "./idb";
import { acquire, heldElsewhere, onLockLost, release } from "./lock";
import {
  COPIES_DIR, SHARED_DIR, basename, cleanName, courseFromPath, dirname, isInside, isSpecial, newLessonId, stem,
} from "./paths";
import { type Section, homeSection, libPrefs, markOpened, moveOpened, setLibPrefs } from "./prefs";
import { type FileLibrary, openStore, requestFolder } from "./store";
import { coverKey, renderCover } from "./thumbs";
import type { CopiedFrom, LessonLibraryMeta, LessonSummary } from "./types";
import { patchLesson, summaryFromBytes } from "./zip";
import { CloudError, explain } from "../cloud/api";
import { cloudLibrary, cloudLibraryReady, initCloud, refreshUsage } from "../cloud/account";
import { openSharedFromUrl } from "../cloud/viewer";
import { CloudLibrary, LessonSync } from "../cloud/library";

/** The lesson the board is bound to. */
export interface Current {
  path: string;
  /** manifest `created` of the document: tells documents apart. */
  created: number;
  id: string;
  author?: string;
  copiedFrom?: CopiedFrom;
  /** Size and mtime we last read or wrote; a difference means someone else wrote it. */
  known: { size: number; mtime: number } | null;
  /** The library the lesson lives in when it is not the local one (Rupok Cloud). */
  lib?: FileLibrary;
  /** Cloud lessons: Loro push/pull. */
  sync?: LessonSync;
}

export type Notice =
  | { kind: "changed"; path: string }
  | { kind: "lost" }
  | { kind: "error"; message: string };

export const [store, setStore] = createSignal<FileLibrary | null>(null);
export const [libraryOpen, setLibraryOpen] = createSignal(false);
export const [current, setCurrent] = createSignal<Current | null>(null);
/** Bumped on every change, so open views reload. */
export const [version, setVersion] = createSignal(0);
export const bump = () => setVersion((v) => v + 1);
/** Browser folder waiting for the teacher to grant access again. */
export const [pendingFolder, setPendingFolder] = createSignal<FileSystemDirectoryHandle | null>(null);
export const [notice, setNotice] = createSignal<Notice | null>(null);
export const [section, setSection] = createSignal<Section>("recent");
/** The folder shown in My lessons, My copies or Shared with me. */
export const [folder, setFolder] = createSignal("");
export const [roleChoice, setRoleChoice] = createSignal(false);
export const [lockPrompt, setLockPrompt] = createSignal<{ path: string; answer: (take: boolean) => void } | null>(null);
/** Student viewer (`?view`): the lesson file the student opened. */
/** The library the Library UI acts on (the local one, or Rupok Cloud). */
export const [activeLib, setActiveLib] = createSignal<FileLibrary | null>(null);
export const [received, setReceived] = createSignal<{ bytes: Uint8Array; summary: LessonSummary | null; savedTo?: string } | null>(null);

let ctl: Controller;
let ready: Promise<void> = Promise.resolve();
/** Revision of an untouched, unbound board; its first change makes it a lesson. */
let draftRev: number | null = null;
/** Last board revision written to the bound lesson. */
let libRev = -1;
/** Board revision when the lesson was bound. The engine's revision only grows
 * and every load bumps it, so autosave bytes taken earlier belong to the
 * document that was there before and are dropped. */
let bindRev = 0;
let saving: Promise<void> = Promise.resolve();
let lastCover = { key: "", png: null as Uint8Array | null };
let externalLoad = false;
let suspended = false;
let stopWatch = () => {};
/** The last title → file name rename tried (a taken name is not retried every save). */
let renameTried = "";

const SESSION_KEY = "rupok.library.open";
const MIRROR_KEY = "rupok.library.mirror";

const lockKey = (path: string, lib: FileLibrary | null | undefined = current()?.lib ?? store()) => `${lib?.id}|${path}`;
/** The library of the bound lesson. */
const libOf = (cur: Current | null) => cur?.lib ?? store();

// ------------------------------------------------------------ small state

function writeSession(path: string | null) {
  try {
    if (path) sessionStorage.setItem(SESSION_KEY, JSON.stringify({ store: libOf(current())?.id, path }));
    else sessionStorage.removeItem(SESSION_KEY);
  } catch { /* storage unavailable */ }
}
function readSession(): { store: string; path: string } | null {
  try {
    return JSON.parse(sessionStorage.getItem(SESSION_KEY) || "null");
  } catch {
    return null;
  }
}
/** Which lesson the recovery copy (storage.ts) belongs to. */
function writeMirror(cur: Current) {
  if (cur.lib) return;
  try {
    localStorage.setItem(MIRROR_KEY, JSON.stringify({ store: store()?.id, path: cur.path, created: cur.created }));
  } catch { /* storage unavailable */ }
}
function readMirror(): { store: string; path: string; created: number } | null {
  try {
    return JSON.parse(localStorage.getItem(MIRROR_KEY) || "null");
  } catch {
    return null;
  }
}

/** Bind the board to a lesson file. `renamed`: the same file under a new name. */
function bind(cur: Current, renamed = false) {
  if (!renamed) bindRev = ctl.board.revision();
  setCurrent(cur);
  writeSession(cur.path);
  writeMirror(cur);
  draftRev = null;
  lastCover = { key: "", png: null };
  markOpened(lockKey(cur.path, libOf(cur)));
}

function unbind() {
  release();
  setCurrent(null);
  writeSession(null);
}

function boardHasContent(): boolean {
  try {
    const pages = JSON.parse(ctl.board.pages_json()) as { item_count: number }[];
    return pages.length > 1 || pages.some((p) => p.item_count > 0);
  } catch {
    return false;
  }
}

const author = () => libPrefs().author.trim() || undefined;

// ------------------------------------------------------------------ start

/** Wire the Library to the board. Call once, right after the board is ready. */
export function startLibrary(c: Controller) {
  ctl = c;
  c.libraryHooks = {
    beforeLoad: () => flush(),
    loaded: (f, bytes) => void onExternalLesson(f, bytes),
    saved: (bytes, rev) => void onAutosave(bytes, rev),
  };
  onLockLost(() => {
    suspended = true;
    setNotice({ kind: "lost" });
  });
  draftRev = c.board.revision();
  window.addEventListener("pagehide", () => void flush());
  window.addEventListener("focus", () => current()?.sync && void pullCloud());
  document.addEventListener("visibilitychange", () => document.visibilityState === "hidden" && void flush());
  setInterval(poll, 5000);
  void initCloud();
  // Student viewer opened from a Rupok Cloud share link (`?view&src=…&share=…`).
  if (studentEntry) openSharedFromUrl(c);
  ready = (async () => {
    try {
      await useStore();
    } catch (e) {
      console.warn("library unavailable", e);
      return;
    }
    if (studentEntry) return;
    await migrate();
    await land();
  })();
}

/** Open the provider for this platform and the teacher's choice. */
export async function useStore() {
  const { store: s, pendingFolder: pf } = await openStore(libPrefs().location);
  stopWatch();
  setStore(s);
  setPendingFolder(pf ?? null);
  stopWatch = s.watch(onExternalChange);
  bump();
}

/** Grant access to the picked browser folder again (needs a click). */
export async function grantFolder(): Promise<boolean> {
  const h = pendingFolder();
  if (!h || !(await requestFolder(h))) return false;
  setPendingFolder(null);
  bump();
  return true;
}

/** First run: the autosaved lesson (storage.ts) becomes a library lesson. */
async function migrate() {
  const s = store();
  if (!s) return;
  const done = await idbGet<boolean>("kv", "migrated").catch(() => true);
  if (done) return;
  try {
    const legacy = await autosaveLoad();
    const sum = legacy ? await summaryFromBytes(legacy).catch(() => null) : null;
    if (legacy && sum && (sum.pages > 1 || boardHasContent()) && !pendingFolder()) {
      const path = await s.freeLessonPath("", sum.title || "Untitled lesson");
      const cover = boardHasContent() ? await renderCover(ctl.board).catch(() => null) : null;
      const bytes = await patchLesson(legacy, { library: { id: newLessonId(), ...(author() ? { author: author() } : {}) }, ...(cover ? { thumb: cover } : {}) });
      await s.write(path, bytes);
      try {
        localStorage.setItem(MIRROR_KEY, JSON.stringify({ store: s.id, path, created: sum.created }));
      } catch { /* storage unavailable */ }
      showToast(`Your lesson is now in the Library as “${stem(basename(path))}”`, 4000);
    }
    await idbPut("kv", "migrated", true);
  } catch (e) {
    // The autosave stays where it was; we try again next start.
    console.warn("library migration failed", e);
  }
}

/** Read and remove a URL parameter, so a reload does not repeat it. */
function takeParam(name: string): string | null {
  const url = new URL(location.href);
  const v = url.searchParams.get(name);
  if (v === null) return null;
  url.searchParams.delete(name);
  history.replaceState(history.state, "", url.pathname + url.search.replace(/=(?=&|$)/g, "") + url.hash);
  return v;
}

async function land() {
  if (externalLoad) return;
  const s = store();
  if (!s) return;
  const force = takeParam("library") !== null;
  // `?open=<path>`: a link straight to a lesson ("Open my copy" from the student viewer).
  const open = takeParam("open");
  if (open && !pendingFolder() && (await openNow(open))) return;
  // `?cloud=<path>`: a lesson in Rupok Cloud (a copy just made from a share link).
  const cloudPath = takeParam("cloud");
  const cl = cloudPath ? await cloudLibraryReady() : null;
  if (cloudPath && cl && (await openNow(cloudPath, { lib: cl }))) return;
  if (!force && !pendingFolder()) {
    const sess = readSession();
    const cl = sess?.store.startsWith("cloud:") ? await cloudLibraryReady() : null;
    if (sess && cl && sess.store === cl.id && (await openNow(sess.path, { quiet: true, lib: cl }))) return;
    if (sess && sess.store === s.id && (await s.stat(sess.path).catch(() => null)) && (await openNow(sess.path, { quiet: true }))) return;
    if (!libPrefs().setupDone) {
      // Before the Library is set up: continue the last lesson, or a fresh board.
      const m = readMirror();
      if (m && m.store === s.id && (await s.stat(m.path).catch(() => null)) && (await openNow(m.path, { quiet: true }))) return;
      return;
    }
  }
  if (externalLoad) return;
  showLibrary();
}

// ----------------------------------------------------------- open / close

/** Show the Library home. */
let visited = false;
export function showLibrary(at?: Section) {
  visited = true;
  const sec = at ?? homeSection();
  setSection(sec);
  setFolder(sectionBase(sec));
  setLibraryOpen(true);
  if (!libPrefs().setupDone) setRoleChoice(true);
  bump();
}

/** The top bar's Library button: save, close the lesson, show the Library. */
export async function openLibrary() {
  if (studentEntry) {
    location.href = `${location.pathname}?library`;
    return;
  }
  await ready;
  await closeLesson();
  // Back where the teacher was; the first time, the home section.
  if (visited) {
    setLibraryOpen(true);
    bump();
  } else showLibrary();
}

/** Save and let go of the bound lesson (the board keeps showing it, unbound). */
export async function closeLesson() {
  await flush();
  unbind();
}

/** Ask whether to take a lesson over from another tab. */
function askLock(path: string): Promise<boolean> {
  return new Promise((resolve) => setLockPrompt({ path, answer: (take) => { setLockPrompt(null); resolve(take); } }));
}

/** Open a library lesson on the board. Resolves false if it was not opened. */
export async function openLesson(path: string, opts: { quiet?: boolean; lib?: FileLibrary } = {}): Promise<boolean> {
  await ready;
  return openNow(path, opts);
}

/** `openLesson` without waiting for start-up (start-up itself calls this). */
async function openNow(path: string, opts: { quiet?: boolean; lib?: FileLibrary } = {}): Promise<boolean> {
  const s = opts.lib ?? store();
  if (!s) return false;
  const remote = s instanceof CloudLibrary ? s : null;
  let takeOver = false;
  if (await heldElsewhere(lockKey(path, s))) {
    if (opts.quiet || !(await askLock(path))) return false;
    takeOver = true;
  }
  try {
    await closeLesson();
    let bytes = await s.read(path);
    const sum = await summaryFromBytes(bytes);
    let recovered = false;
    // A save that did not finish (tab killed, disk full): the recovery copy is newer.
    const m = remote ? null : readMirror();
    if (m && m.store === s.id && m.path === path) {
      const legacy = await autosaveLoad();
      const ls = legacy ? await summaryFromBytes(legacy).catch(() => null) : null;
      if (legacy && ls && ls.created === sum.created && ls.modified > sum.modified + 1000) {
        bytes = legacy;
        recovered = true;
      }
    }
    ctl.board.load(bytes);
    ctl.fileHandle = null;
    forgetLessonPath();
    setLessonOpened(true);
    const r0 = ctl.board.revision();
    // The file name is the lesson's name (renamed in the Library, or by an agent).
    const name = stem(basename(path));
    if (ctl.board.title() !== name) ctl.board.set_title(name);
    const st = remote ? null : await s.driver.stat(path);
    let sync: LessonSync | undefined;
    if (remote) {
      const node = await remote.node(path);
      if (!node) throw new Error("not found in Rupok Cloud");
      sync = new LessonSync(remote.api, node.id);
      await sync.opened(ctl.board);
    }
    bind({
      path, created: sum.created, id: sum.library?.id ?? newLessonId(), author: sum.library?.author, copiedFrom: sum.library?.copiedFrom,
      known: st ? { size: st.size, mtime: st.mtime } : null, lib: remote ?? undefined, sync,
    });
    acquire(lockKey(path, s), takeOver);
    suspended = false;
    libRev = recovered ? -1 : r0;
    setSavedRevision(libRev);
    // Students read received lessons in the Student view; their own copies are theirs to edit.
    const viewOnly = libPrefs().role === "student" && isInside(path, SHARED_DIR);
    if (viewOnly && mode() !== "view") ctl.setMode("view");
    else if (!viewOnly && mode() === "view") ctl.setMode("prepare");
    ctl.updateInsets(true);
    setLibraryOpen(false);
    if (recovered) showToast("Recovered changes that had not been saved yet");
    return true;
  } catch (e: any) {
    if (!opts.quiet) showToast(`Could not open “${stem(basename(path))}”: ${e?.message ?? e}`, 5000);
    return false;
  }
}

/** Bind the lesson the board holds now to a new file at `path` and save it. */
export async function adoptBoard(path: string, meta: Partial<Current> = {}) {
  const bytes = ctl.board.save();
  const sum = await summaryFromBytes(bytes);
  unbind();
  bind({ path, created: sum.created, id: meta.id ?? newLessonId(), author: meta.author ?? author(), copiedFrom: meta.copiedFrom, known: null, lib: meta.lib });
  acquire(lockKey(path, libOf(current())));
  suspended = false;
  libRev = -1;
  await queue(() => saveBytes(bytes, ctl.board.revision(), true));
}

// ------------------------------------------------------------------ saving

function queue(fn: () => Promise<void>): Promise<void> {
  saving = saving.then(fn).catch((e) => {
    console.warn("library save failed", e);
    setNotice({ kind: "error", message: explain(e) });
  });
  return saving;
}

/** Write any unsaved change of the bound lesson now. */
export function flush(): Promise<void> {
  if (!ctl || studentEntry || suspended) return saving;
  const rev = ctl.board.revision();
  if (current() ? rev === libRev : draftRev === null || rev === draftRev || libraryOpen()) return saving;
  const bytes = ctl.board.save();
  return queue(() => saveBytes(bytes, rev));
}

function onAutosave(bytes: Uint8Array, rev: number) {
  if (studentEntry || suspended) return;
  void queue(() => saveBytes(bytes, rev));
}

async function saveBytes(bytes: Uint8Array, rev: number, force = false): Promise<void> {
  let cur = current();
  const s = libOf(cur);
  if (!s || (pendingFolder() && !cur?.lib) || suspended) return;
  if (rev === libRev && !force && cur) return;
  // Taken before this lesson was on the board (the autosave awaits IndexedDB first).
  if (cur && !force && rev < bindRev) return;
  const sum = await summaryFromBytes(bytes);
  if (!cur) {
    // An unbound board: a fresh board's first change makes it a lesson.
    if (libraryOpen() || draftRev === null || rev === draftRev) return;
    const cl = libPrefs().saveTo === "cloud" ? cloudLibrary() : null;
    const target = cl ?? s;
    const path = await target.freeLessonPath("", ctl.board.title() || sum.title || "Untitled lesson");
    cur = { path, created: sum.created, id: newLessonId(), author: author(), known: null, lib: cl ?? undefined };
    bind(cur);
    acquire(lockKey(path));
    renameTried = `${path}>${cleanName(ctl.board.title() || sum.title, "Untitled lesson")}`;
  } else if (sum.created !== cur.created) {
    // Another document replaced the lesson on the board (Settings → New empty
    // lesson, an agent…). Never write it over the bound file: keep it as a new lesson.
    release();
    const path = await s.freeLessonPath(isSpecial(cur.path) ? "" : dirname(cur.path), ctl.board.title() || sum.title || "Untitled lesson");
    cur = { path, created: sum.created, id: newLessonId(), author: author(), known: null, lib: cur.lib };
    bind(cur);
    acquire(lockKey(path, s));
  }
  // The title is the file name.
  const want = cleanName(ctl.board.title() || sum.title, "Untitled lesson");
  if (want !== stem(basename(cur.path)) && (cur.known || cur.sync) && renameTried !== `${cur.path}>${want}`) {
    renameTried = `${cur.path}>${want}`;
    try {
      const np = await libOf(cur)!.rename(cur.path, want);
      if (np !== cur.path) {
        moveOpened(libOf(cur)!.id, cur.path, np);
        cur = { ...cur, path: np };
        bind(cur, true);
        acquire(lockKey(np, libOf(cur)));
      }
    } catch { /* keep the old name */ }
  }
  // Changed on disk since we read or wrote it (an agent, a sync client, another device)?
  if (!force && cur.known) {
    const st = await s.driver.stat(cur.path);
    if (st && (st.size !== cur.known.size || st.mtime !== cur.known.mtime)) {
      setNotice({ kind: "changed", path: cur.path });
      return;
    }
  }
  const key = coverKey(ctl.board);
  if (key !== lastCover.key) lastCover = { key, png: await renderCover(ctl.board, key).catch(() => null) };
  const lib: LessonLibraryMeta = { id: cur.id };
  if (cur.author) lib.author = cur.author;
  if (cur.copiedFrom) lib.copiedFrom = cur.copiedFrom;
  const course = courseFromPath(cur.path) ?? undefined;
  const out = await patchLesson(bytes, { library: lib, course, ...(lastCover.png ? { thumb: lastCover.png } : {}) });
  if (cur.lib) {
    // Rupok Cloud: push Loro updates (or a whole file the server merges).
    if (cur.sync) await cur.sync.save(ctl.board, out);
    else {
      await cur.lib.write(cur.path, out);
      const node = await (cur.lib as CloudLibrary).node(cur.path);
      if (node) {
        const sync = new LessonSync((cur.lib as CloudLibrary).api, node.id);
        await sync.opened(ctl.board);
        const latest = current();
        if (latest && latest.path === cur.path) setCurrent({ ...latest, sync });
      }
    }
    void refreshUsage();
  } else {
    const e = await s.write(cur.path, out);
    const latest = current();
    if (latest && latest.path === cur.path) setCurrent({ ...latest, known: { size: e.size, mtime: e.modified } });
  }
  libRev = rev;
  setSavedRevision(rev);
  if (notice()?.kind === "error") setNotice(null);
  bump();
}

// ------------------------------------------------- files opened from outside

/** A lesson opened from a file (Ctrl+O, drag onto the board, the OS, a link). */
async function onExternalLesson(f: LessonFile, bytes: Uint8Array) {
  externalLoad = true;
  // At once: an autosave of the new document must not land in the old file.
  unbind();
  draftRev = null;
  await ready;
  const s = store();
  if (!s) return;
  const sum = await summaryFromBytes(bytes).catch(() => null);
  if (studentEntry) {
    setReceived({ bytes, summary: sum });
    // Keep what the teacher sent in "Shared with me", so it is there next time.
    await keepReceived(bytes, sum, f.name).catch(() => null);
    return;
  }
  let path: string | null = null;
  // Desktop: a file inside the library folder is edited where it is.
  if (isDesktop && f.path && s.kind === "desktop") {
    const root = (s.driver as DesktopDriver).info.root.replace(/[\\/]+$/, "");
    const norm = (p: string) => p.replace(/\\/g, "/");
    if (norm(f.path).toLowerCase().startsWith(norm(root).toLowerCase() + "/")) path = norm(f.path).slice(norm(root).length + 1);
  }
  try {
    path ??= await keepReceived(bytes, sum, f.name);
    const st = await s.driver.stat(path);
    const stored = await summaryFromBytes(await s.read(path)).catch(() => sum);
    bind({
      path, created: sum?.created ?? 0, id: stored?.library?.id ?? newLessonId(), author: stored?.library?.author, copiedFrom: stored?.library?.copiedFrom,
      known: st ? { size: st.size, mtime: st.mtime } : null,
    });
    acquire(lockKey(path), true);
    suspended = false;
    libRev = ctl.board.revision();
    setSavedRevision(libRev);
    if (libPrefs().role === "student" && isInside(path, SHARED_DIR) && mode() !== "view") ctl.setMode("view");
    setLibraryOpen(false);
    bump();
  } catch (e) {
    console.warn("could not keep the opened lesson in the library", e);
  }
}

/**
 * Store a received lesson in "Shared with me". A newer copy of the same
 * lesson (same library id, or same title and creation time) replaces the
 * older one instead of piling up duplicates.
 */
export async function keepReceived(bytes: Uint8Array, sum: LessonSummary | null, fileName: string): Promise<string> {
  const s = store()!;
  sum ??= await summaryFromBytes(bytes);
  let path: string | null = null;
  try {
    for (const e of await s.list(SHARED_DIR)) {
      if (e.kind !== "lesson") continue;
      const c = s.summary(e.path);
      if ((sum.library?.id && e.id === sum.library.id) || (c && c.created === sum.created && c.title === sum.title)) {
        path = e.path;
        break;
      }
    }
  } catch { /* folder does not exist yet */ }
  path ??= await s.freeLessonPath(SHARED_DIR, sum.title || stem(fileName));
  const lib: LessonLibraryMeta = sum.library ?? { id: newLessonId() };
  await s.write(path, await patchLesson(bytes, { library: lib }));
  return path;
}

// ---------------------------------------------------- outside changes

function onExternalChange(paths: string[]) {
  bump();
  const cur = current();
  if (cur && (paths.length === 0 || paths.some((p) => p === cur.path || isInside(cur.path, p)))) void checkOpenLesson();
}

let lastPull = 0;
function poll() {
  const s = store();
  if (!s || document.visibilityState !== "visible") return;
  // Rupok Cloud: bring in other devices' edits every 15 s while a lesson is open.
  if (current()?.sync && Date.now() - lastPull > 15000) void pullCloud();
  // Folders without change notifications (no FileSystemObserver): look now and then.
  if (s.kind === "folder" && !(globalThis as any).FileSystemObserver) {
    if (current()) void checkOpenLesson();
    if (libraryOpen()) bump();
  }
}

/** Pull other devices' changes into the open cloud lesson, then push ours. */
export function pullCloud(): Promise<void> {
  lastPull = Date.now();
  return queue(async () => {
    const cur = current();
    if (!cur?.sync || suspended) return;
    const hadLocal = ctl.board.revision() !== libRev;
    let n = 0;
    try {
      n = await cur.sync.pull(ctl.board);
    } catch (e) {
      if (e instanceof CloudError && e.isOffline) return;
      throw e;
    }
    if (!n) return;
    ctl.sync(true);
    if (!hadLocal) {
      libRev = ctl.board.revision();
      setSavedRevision(libRev);
    }
  });
}

function checkOpenLesson(): Promise<void> {
  return queue(async () => {
    const s = store();
    const cur = current();
    if (!s || !cur || !cur.known || suspended) return;
    const st = await s.driver.stat(cur.path);
    if (!st) {
      // Moved or deleted outside Rupok: the next save writes it back.
      setCurrent({ ...cur, known: null });
      return;
    }
    if (st.size === cur.known.size && st.mtime === cur.known.mtime) return;
    if (ctl.board.revision() === libRev) await reloadFromDisk();
    else setNotice({ kind: "changed", path: cur.path });
  });
}

async function reloadFromDisk() {
  const s = store();
  const cur = current();
  if (!s || !cur) return;
  const bytes = await s.read(cur.path);
  const sum = await summaryFromBytes(bytes);
  const page = ctl.board.page_index();
  ctl.board.load(bytes);
  const n = (JSON.parse(ctl.board.pages_json()) as unknown[]).length;
  ctl.board.goto_page(Math.min(page, Math.max(0, n - 1)));
  const st = await s.driver.stat(cur.path);
  setCurrent({ ...cur, created: sum.created, known: st ? { size: st.size, mtime: st.mtime } : null });
  libRev = ctl.board.revision();
  setSavedRevision(libRev);
  showToast("Updated with changes made outside Rupok");
}

/** The open lesson changed on disk while it had local changes: pick a side. */
export async function resolveConflict(keepMine: boolean) {
  setNotice(null);
  if (keepMine) {
    const s = store();
    const cur = current();
    if (!s || !cur) return;
    const bytes = ctl.board.save();
    await queue(() => saveBytes(bytes, ctl.board.revision(), true));
  } else {
    await queue(reloadFromDisk);
  }
}

/** After another tab took the lesson over: back to the Library, nothing written. */
export function leaveLostLesson() {
  setNotice(null);
  suspended = false;
  setCurrent(null);
  writeSession(null);
  showLibrary();
}

/** First-run choice. */
export function chooseRole(role: "teacher" | "student") {
  setLibPrefs({ role, setupDone: true });
  setRoleChoice(false);
  setSection(homeSection());
  setFolder(sectionBase(homeSection()));
  // Ask the browser to keep the library when space runs low (a click is a good moment).
  void navigator.storage?.persist?.().catch(() => false);
  bump();
}

/** The folder a section starts in. */
export const sectionBase = (s: Section): string => (s === "shared" ? SHARED_DIR : s === "copies" ? COPIES_DIR : "");

export const controllerRef = () => ctl;
export const whenReady = () => ready;
