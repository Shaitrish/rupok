// Hand-off of a lesson file from the website to the board.
//
// The RPK viewer page (/open/) can't pass a File through a URL, so it stashes
// the picked file in IndexedDB and navigates to /board/?view&pending=1; the
// board takes it on startup and opens it. Kept tiny and dependency-free: the
// marketing pages import it and must stay light.
//
// Also: files the OS hands to the installed PWA ("Open with Rupok", the
// manifest's file_handlers) arrive through `launchQueue`.

const DB = "rupok-pending";
const STORE = "files";
const KEY = "lesson";
/** A stash older than this was abandoned (tab closed before the board loaded). */
const MAX_AGE_MS = 10 * 60 * 1000;

interface Stash {
  name: string;
  type: string;
  bytes: ArrayBuffer;
  at: number;
}

function db(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function run<T>(mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  return db().then(
    (d) =>
      new Promise<T>((resolve, reject) => {
        const tx = d.transaction(STORE, mode);
        const req = fn(tx.objectStore(STORE));
        tx.oncomplete = () => {
          d.close();
          resolve(req.result);
        };
        tx.onerror = tx.onabort = () => {
          d.close();
          reject(tx.error);
        };
      }),
  );
}

/** Website side: keep `file` for the board. Rejects if storage is blocked. */
export async function stashPendingLesson(file: File, name = file.name): Promise<void> {
  // Bytes rather than the Blob itself: older Safari can't store Blobs in IndexedDB.
  const stash: Stash = { name, type: file.type, bytes: await file.arrayBuffer(), at: Date.now() };
  await run("readwrite", (s) => s.put(stash, KEY));
}

/**
 * Board side: the lesson the website stashed, if the URL asks for one
 * (`?pending`). Clears the stash and the URL flag so a reload doesn't reopen it.
 */
export async function takePendingLesson(): Promise<File | null> {
  const url = new URL(location.href);
  if (!url.searchParams.has("pending")) return null;
  url.searchParams.delete("pending");
  history.replaceState(history.state, "", url.pathname + url.search.replace(/=(?=&|$)/g, "") + url.hash);
  try {
    const stash = (await run("readonly", (s) => s.get(KEY))) as Stash | undefined;
    await run("readwrite", (s) => s.delete(KEY));
    if (!stash || !(stash.bytes instanceof ArrayBuffer) || Date.now() - stash.at > MAX_AGE_MS) return null;
    return new File([stash.bytes], stash.name, { type: stash.type });
  } catch {
    return null; // storage unavailable: the student can still choose the file
  }
}

/** Files the OS opened with the installed app (File Handling API). */
export function onLaunchFiles(cb: (files: File[]) => void): void {
  const lq = (window as any).launchQueue;
  if (!lq?.setConsumer) return;
  lq.setConsumer(async (params: { files?: { getFile(): Promise<File> }[] }) => {
    if (!params.files?.length) return;
    cb(await Promise.all(params.files.map((h) => h.getFile())));
  });
}
