// The Library's IndexedDB database (`rupok-library`), separate from the
// board's autosave database (`rupok`, storage.ts) so neither schema can
// break the other.
//
// - `meta`:   lesson summary cache, key `<store id>|<path>` (never authoritative)
// - `kv`:     small values: the chosen folder handle, migration flag
// - `fsmeta` / `fsdata`: the IndexedDB file system used when the browser has
//             no Origin Private File System (IdbDriver)

const NAME = "rupok-library";
export type IdbStore = "meta" | "kv" | "fsmeta" | "fsdata";

let dbp: Promise<IDBDatabase> | null = null;

export function idb(): Promise<IDBDatabase> {
  if (!dbp) {
    dbp = new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(NAME, 1);
      req.onupgradeneeded = () => {
        for (const s of ["meta", "kv", "fsmeta", "fsdata"]) if (!req.result.objectStoreNames.contains(s)) req.result.createObjectStore(s);
      };
      req.onsuccess = () => {
        const db = req.result;
        db.onversionchange = () => {
          db.close();
          dbp = null;
        };
        resolve(db);
      };
      req.onerror = () => reject(req.error);
      req.onblocked = () => reject(new Error("library database is blocked"));
    });
    dbp.catch(() => (dbp = null));
  }
  return dbp;
}

/** Run `fn` in one transaction; resolves with its result once committed. */
export async function tx<T>(stores: IdbStore | IdbStore[], mode: IDBTransactionMode, fn: (t: IDBTransaction) => T | Promise<T>): Promise<T> {
  const db = await idb();
  return new Promise<T>((resolve, reject) => {
    const t = db.transaction(stores, mode);
    let result: T;
    t.oncomplete = () => resolve(result);
    t.onerror = t.onabort = () => reject(t.error ?? new Error("transaction aborted"));
    Promise.resolve(fn(t)).then((r) => (result = r), (e) => { try { t.abort(); } catch { /* done */ } reject(e); });
  });
}

export const req = <T>(r: IDBRequest<T>): Promise<T> =>
  new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });

export async function idbGet<T>(store: IdbStore, key: string): Promise<T | undefined> {
  const db = await idb();
  return req(db.transaction(store, "readonly").objectStore(store).get(key)) as Promise<T | undefined>;
}

export function idbPut(store: IdbStore, key: string, value: unknown): Promise<void> {
  return tx(store, "readwrite", (t) => void t.objectStore(store).put(value, key));
}

export function idbDelete(store: IdbStore, key: string | IDBKeyRange): Promise<void> {
  return tx(store, "readwrite", (t) => void t.objectStore(store).delete(key));
}

/** Keys from `prefix` up to `prefix` + U+FFFF. */
export const prefixRange = (prefix: string) => IDBKeyRange.bound(prefix, prefix + "￿");
