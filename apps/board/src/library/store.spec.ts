// Provider tests for the Library store. They run in a real browser so the
// Origin Private File System is covered too (OPFS has no Node shim):
//
//   const { runStoreSpecs } = await import("/src/library/store.spec.ts");
//   const results = await runStoreSpecs();   // [{ driver, name, ok, error }]
//
// The e2e runner (scratchpad e2e/library-unit.mjs, see docs/library.md) opens
// the dev server and calls this. Not imported by the app, so never bundled.

import { HandleDriver, IdbDriver, MemoryDriver } from "./drivers";
import { COPIES_DIR, SHARED_DIR, checkPath, cleanName, courseFromPath, uniqueName, withoutOrder } from "./paths";
import { FileLibrary } from "./store";
import { patchLesson, readCentral, readEntry, summaryFromBytes, writeZip } from "./zip";

export interface SpecResult {
  driver: string;
  name: string;
  ok: boolean;
  error?: string;
}

const enc = new TextEncoder();
const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4]);

/** A minimal package: enough for summaries (the document is opaque bytes here). */
export function fakeLesson(title: string, extra: Record<string, unknown> = {}, thumb?: Uint8Array): Uint8Array {
  const manifest = { format: "rupok-lesson", version: 1, title, created: 1763870400000, modified: Date.now(), generator: "spec", pages: 2, ...extra };
  const files: { name: string; bytes: Uint8Array }[] = [
    { name: "mimetype", bytes: enc.encode("application/vnd.rupok.lesson+zip") },
    { name: "manifest.json", bytes: enc.encode(JSON.stringify(manifest)) },
    { name: "doc.loro", bytes: new Uint8Array(512).map((_, i) => i % 251) },
    { name: "assets/abc.png", bytes: PNG },
  ];
  if (thumb) files.push({ name: "thumbs/0.png", bytes: thumb });
  return writeZip(files);
}

function assert(cond: unknown, msg: string): asserts cond {
  if (!cond) throw new Error(msg);
}
const eq = (a: unknown, b: unknown, msg: string) => assert(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} ≠ ${JSON.stringify(b)}`);
const rejects = async (p: Promise<unknown>, msg: string) => {
  try {
    await p;
  } catch {
    return;
  }
  throw new Error(`expected an error: ${msg}`);
};

type Spec = [string, (s: FileLibrary) => Promise<void>];

const SPECS: Spec[] = [
  ["write, list, stat and read a lesson with metadata", async (s) => {
    const bytes = fakeLesson("Binary numbers", { course: { subject: "ICT" }, library: { id: "l_1", author: "Ms Rahman" } }, PNG);
    const e = await s.write("ICT/Grade 9 ICT/01 Number systems/Binary numbers.rpk", bytes);
    eq(e.kind, "lesson", "kind");
    eq(e.name, "Binary numbers", "name has no extension");
    eq(e.title, "Binary numbers", "title from manifest");
    eq(e.pages, 2, "pages");
    eq(e.id, "l_1", "library id");
    eq(e.author, "Ms Rahman", "author");
    assert(typeof e.thumb === "string" && e.thumb.startsWith("blob:"), "thumbnail URL");
    const top = await s.list("");
    eq(top.map((x) => [x.kind, x.name]), [["folder", "ICT"]], "root lists the subject folder");
    const ch = await s.list("ICT/Grade 9 ICT/01 Number systems");
    eq(ch.length, 1, "one lesson in the chapter");
    eq(ch[0].size, bytes.length, "size");
    assert(ch[0].modified > 0, "modified time");
    eq(Array.from(await s.read(ch[0].path)), Array.from(bytes), "read returns the bytes");
    eq(await s.stat("ICT/nope.rpk"), null, "missing file stats null");
    await rejects(s.write("ICT/notes.txt", bytes), "only .rpk files are written");
  }],
  ["folders get unique names", async (s) => {
    eq(await s.createFolder("Maths"), "Maths", "first");
    eq(await s.createFolder("Maths"), "Maths (2)", "second");
    eq(await s.createFolder("Maths/Algebra/Linear"), "Maths/Algebra/Linear", "nested parents created");
    eq(await s.createFolder("a/b:c?"), "a/b-c-", "reserved characters cleaned");
  }],
  ["move lessons and folders", async (s) => {
    await s.write("One.rpk", fakeLesson("One"));
    await s.write("Dest/One.rpk", fakeLesson("Other one"));
    await s.createFolder("Dest");
    eq(await s.move("One.rpk", "Dest"), "Dest/One (2).rpk", "a name clash gets a number");
    eq(await s.stat("One.rpk"), null, "gone from the old place");
    await s.createFolder("Dest/Inner");
    await rejects(s.move("Dest", "Dest/Inner"), "a folder cannot move into itself");
    eq(await s.move("Dest/Inner", ""), "Inner", "move to the root");
    eq(await s.move("Inner", ""), "Inner", "same folder is a no-op");
  }],
  ["rename keeps the extension and handles case-only changes", async (s) => {
    await s.write("Old.rpk", fakeLesson("Old"));
    eq(await s.rename("Old.rpk", "New name"), "New name.rpk", "renamed");
    eq(await s.rename("New name.rpk", "new name"), "new name.rpk", "case-only rename");
    await s.write("Taken.rpk", fakeLesson("Taken"));
    eq(await s.rename("new name.rpk", "Taken"), "Taken (2).rpk", "a taken name gets a number");
    await s.createFolder("F");
    eq(await s.rename("F", "Folder"), "Folder", "folders too");
  }],
  ["duplicate gives a new id; folders copy recursively", async (s) => {
    await s.write("L.rpk", fakeLesson("L", { library: { id: "l_orig" } }));
    const d = await s.duplicate("L.rpk");
    eq(d, "L (copy).rpk", "copy name");
    const e = await s.stat(d);
    assert(e?.id && e.id !== "l_orig", `the copy has its own id (${e?.id})`);
    await s.write("Course/Ch/A.rpk", fakeLesson("A"));
    eq(await s.duplicate("Course"), "Course (copy)", "folder copy");
    assert(await s.stat("Course (copy)/Ch/A.rpk"), "contents copied");
  }],
  ["trash, restore and delete forever", async (s) => {
    await s.write("Sub/T.rpk", fakeLesson("T"));
    await s.trash("Sub/T.rpk");
    eq(await s.stat("Sub/T.rpk"), null, "gone");
    let t = await s.listTrash();
    eq(t.length, 1, "one item in the trash");
    eq(t[0].from, "Sub/T.rpk", "remembers where it was");
    eq(t[0].title, "T", "metadata in the trash");
    eq((await s.walk("")).filter((e) => e.kind === "lesson").length, 0, "walk skips the trash");
    eq(await s.restore(t[0].trashId), "Sub/T.rpk", "restored to its place");
    await s.trash("Sub/T.rpk");
    await s.write("Sub/T.rpk", fakeLesson("New T"));
    t = await s.listTrash();
    eq(await s.restore(t[0].trashId), "Sub/T (2).rpk", "restored next to a newer file");
    await s.trash("Sub");
    t = await s.listTrash();
    eq(t.map((x) => x.kind), ["folder"], "folders go to the trash whole");
    await s.deletePermanently(t[0].trashId);
    eq((await s.listTrash()).length, 0, "deleted forever");
    await s.write("X.rpk", fakeLesson("X"));
    await s.trash("X.rpk");
    await s.emptyTrash();
    eq((await s.listTrash()).length, 0, "empty trash");
  }],
  ["special folders and hidden files", async (s) => {
    await s.write(`${SHARED_DIR}/From teacher.rpk`, fakeLesson("From teacher"));
    await s.write(`${COPIES_DIR}/Mine.rpk`, fakeLesson("Mine"));
    await s.write("Root.rpk", fakeLesson("Root"));
    await s.driver.write(".hidden.rpk", fakeLesson("Hidden"));
    await s.driver.write("Root.rpk.tmp", fakeLesson("Temp"));
    const names = (await s.list("")).map((e) => e.name).sort();
    eq(names, [COPIES_DIR, "Root"], "Shared with me, dot files and temp files are not in My lessons");
    eq((await s.walk("")).filter((e) => e.kind === "lesson").length, 2, "walk: My lessons and My copies");
    eq((await s.walk("", { includeShared: true })).filter((e) => e.kind === "lesson").length, 3, "walk with Shared with me");
  }],
  ["paths are checked", async (s) => {
    for (const p of ["../x.rpk", "a/../b.rpk", "/abs.rpk", "a\\b.rpk", "a//b.rpk"]) await rejects(s.read(p), p);
  }],
];

/** Pure helpers: paths, names and the ZIP patcher. */
async function pureSpecs(): Promise<SpecResult[]> {
  const out: SpecResult[] = [];
  const run = async (name: string, fn: () => Promise<void> | void) => {
    try {
      await fn();
      out.push({ driver: "pure", name, ok: true });
    } catch (e: any) {
      out.push({ driver: "pure", name, ok: false, error: String(e?.message ?? e) });
    }
  };
  await run("course from path", () => {
    eq(courseFromPath("ICT/Grade 9 ICT/01 Number systems/Binary.rpk"), { subject: "ICT", course: "Grade 9 ICT", chapter: "Number systems" }, "full");
    eq(courseFromPath("Binary.rpk"), {}, "root");
    eq(courseFromPath(`${SHARED_DIR}/x.rpk`), null, "special folders keep their own");
    eq(withoutOrder("01 Number systems"), "Number systems", "order prefix");
    eq(withoutOrder("2024"), "2024", "a bare number stays");
  });
  await run("names", () => {
    eq(cleanName("  a/b\\c:d*?  "), "a-b-c-d-", "separators");
    eq(cleanName("..hidden"), "hidden", "no leading dots");
    eq(cleanName("CON"), "CON_", "Windows reserved");
    eq(cleanName("শিক্ষক ক্লাস ৯"), "শিক্ষক ক্লাস ৯", "Bangla is kept");
    eq(uniqueName(new Set(["A.rpk", "a (2).rpk"]), "a", ".rpk"), "a (3).rpk", "unique, case-insensitive");
    let threw = false;
    try { checkPath("a/../b"); } catch { threw = true; }
    assert(threw, "checkPath rejects ..");
  });
  await run("patchLesson rewrites manifest and thumbnail, keeps the rest", async () => {
    const src = fakeLesson("P", { course: { subject: "Old" }, unknownKey: 7 }, new Uint8Array([9, 9, 9]));
    const out = await patchLesson(src, { library: { id: "l_x", author: "A" }, course: { subject: "ICT", chapter: "Ch" }, thumb: PNG });
    const s = await summaryFromBytes(out);
    eq(s.library, { id: "l_x", author: "A" }, "library");
    eq(s.course, { subject: "ICT", chapter: "Ch" }, "course");
    eq(Array.from(s.thumb ?? []), Array.from(PNG), "new thumbnail");
    const entries = await readCentral(out.length, async (a, b) => out.subarray(a, b));
    eq(entries[0].name, "mimetype", "mimetype first");
    eq(entries[0].method, 0, "mimetype stored");
    eq(entries.filter((e) => e.name.startsWith("thumbs/")).length, 1, "stale thumbnails dropped");
    const reader = async (a: number, b: number) => out.subarray(a, b);
    const m = JSON.parse(new TextDecoder().decode(await readEntry(entries.find((e) => e.name === "manifest.json")!, reader)));
    eq(m.unknownKey, 7, "unknown manifest keys are preserved");
    const doc = entries.find((e) => e.name === "doc.loro")!;
    eq(doc.usize, 512, "document copied unchanged");
    const cleared = await summaryFromBytes(await patchLesson(out, { course: {} }));
    eq(cleared.course, undefined, "an empty course removes the key");
  });
  return out;
}

/** Run every spec on the memory, IndexedDB and OPFS drivers. */
export async function runStoreSpecs(): Promise<SpecResult[]> {
  const results = await pureSpecs();
  const rand = () => Math.random().toString(36).slice(2, 8);
  const makers: [string, () => Promise<FileLibrary>][] = [
    ["memory", async () => new FileLibrary(new MemoryDriver(`mem-${rand()}`))],
    ["idb", async () => new FileLibrary(new IdbDriver(`spec-${rand()}`))],
  ];
  if (navigator.storage?.getDirectory) {
    makers.push(["opfs", async () => {
      const root = await navigator.storage.getDirectory();
      const dir = await root.getDirectoryHandle(`spec-${rand()}`, { create: true });
      return new FileLibrary(new HandleDriver("opfs", `opfs-spec-${rand()}`, "spec", dir));
    }]);
  }
  for (const [driver, make] of makers) {
    for (const [name, fn] of SPECS) {
      try {
        await fn(await make());
        results.push({ driver, name, ok: true });
      } catch (e: any) {
        results.push({ driver, name, ok: false, error: String(e?.message ?? e) });
      }
    }
  }
  // Clean up the OPFS test folders.
  try {
    const root = await navigator.storage.getDirectory();
    for await (const [name] of (root as any).entries()) if (String(name).startsWith("spec-")) await root.removeEntry(name, { recursive: true });
  } catch { /* nothing to clean */ }
  return results;
}
