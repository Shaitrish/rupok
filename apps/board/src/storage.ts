// Autosave of the current lesson into IndexedDB (per browser profile).
// Failures are non-fatal: private windows or blocked storage just skip it.
import { APP_SLUG } from "./brand";

const DB = APP_SLUG;
const STORE = "lessons";
const KEY = "current";

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

export async function autosaveStore(bytes: Uint8Array): Promise<void> {
  try {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      tx.objectStore(STORE).put(bytes, KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
  } catch {
    /* storage unavailable */
  }
}

export async function autosaveLoad(): Promise<Uint8Array | null> {
  try {
    const db = await open();
    const v = await new Promise<unknown>((resolve, reject) => {
      const req = db.transaction(STORE, "readonly").objectStore(STORE).get(KEY);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    db.close();
    return v instanceof Uint8Array ? v : null;
  } catch {
    return null;
  }
}

export async function autosaveClear(): Promise<void> {
  try {
    const db = await open();
    db.transaction(STORE, "readwrite").objectStore(STORE).delete(KEY);
    db.close();
  } catch { /* ignore */ }
}

// The teacher's default page design for new lessons (design.ts), per device.
const DESIGN_KEY = "design-default";

export async function designDefaultStore(value: unknown | null): Promise<boolean> {
  try {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(STORE, "readwrite");
      if (value === null) tx.objectStore(STORE).delete(DESIGN_KEY);
      else tx.objectStore(STORE).put(value, DESIGN_KEY);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
    return true;
  } catch {
    return false;
  }
}

export async function designDefaultLoad(): Promise<unknown | null> {
  try {
    const db = await open();
    const v = await new Promise<unknown>((resolve, reject) => {
      const req = db.transaction(STORE, "readonly").objectStore(STORE).get(DESIGN_KEY);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    db.close();
    return v ?? null;
  } catch {
    return null;
  }
}
