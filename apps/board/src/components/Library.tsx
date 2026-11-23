// The Library: the home screen at /board/ when no lesson is open.
//
// Sidebar (Recent, My lessons, Shared with me, My copies, Trash), breadcrumbs,
// search, sort, grid or list of folders and lesson cards with real
// thumbnails, multi-select, drag to move (mouse, pen and touch), rename,
// duplicate, trash/restore, new lesson/folder/course, import and export.

import "./Library.css";
import { For, type JSX, Show, createEffect, createMemo, createResource, createSignal, onCleanup, onMount } from "solid-js";
import { Icon, Mark } from "./Icon";
import { LIcon, ago, bytes } from "./LibraryIcon";
import { ConfirmDialog, LibDialog, LockDialog, MoveDialog, NameDialog, NewCourseDialog, NewLessonDialog, RoleDialog } from "./LibraryDialogs";
import { LibrarySettings } from "./LibrarySettings";
import { WORDMARK } from "../brand";
import { prefs, setPrefs } from "../state";
import type { LibIconName } from "../library/icons.generated";
import {
  COPIES_DIR, SHARED_DIR, basename, dirname, folderRole, isInside, isLessonName, isSpecial, naturalCompare, split, stem,
} from "../library/paths";
import { CloudAccount } from "./CloudAccount";
import { ShareHost } from "./ShareDialog";
import { SharedLessonBar } from "./SharedLessonBar";
import { cloud, cloudLibrary, cloudUser, refreshUsage } from "../cloud/account";
import { explain } from "../cloud/api";
import { type Section, libPrefs, moveOpened, setLibPrefs } from "../library/prefs";
import { heldElsewhere } from "../library/lock";
import {
  bump, chooseRole, controllerRef, folder, grantFolder, leaveLostLesson, libraryOpen, lockPrompt, notice, openLesson, pendingFolder, resolveConflict,
  roleChoice, section, setActiveLib, setFolder, setNotice, setSection, store, version,
} from "../library/session";
import {
  copiesDir, exportFolder, exportLesson, importInto, makeCopy, newCourse, newFolder, newLesson, persistStorage, pickImportFiles,
  recentLessons, renameItem, storageEstimate,
} from "../library/actions";
import type { Entry, TrashEntry } from "../library/types";
import { showToast } from "../state";
import { MicButton } from "./MicButton";

/** Cloud sync arrives with Rupok Cloud; the entry is stubbed until then. */
const CLOUD_ENABLED = false;

type Item = Entry | TrashEntry;
const isTrash = (e: Item): e is TrashEntry => "trashId" in e;
const keyOf = (e: Item) => (isTrash(e) ? e.trashId : e.path);

const SECTIONS: Record<Section, { label: string; icon: LibIconName; base: string }> = {
  recent: { label: "Recent", icon: "recent", base: "" },
  mine: { label: "My lessons", icon: "mine", base: "" },
  shared: { label: "Shared with me", icon: "shared", base: SHARED_DIR },
  copies: { label: "My copies", icon: "copies", base: COPIES_DIR },
  trash: { label: "Trash", icon: "trash", base: "" },
  cloud: { label: "Rupok Cloud", icon: "cloud", base: "" },
};
/** Items of the cloud's "Shared with me" (opened by downloading). */
const SHARED_CLOUD = "@cloud-shared/";

type Dialog =
  | { kind: "newLesson" }
  | { kind: "newFolder" }
  | { kind: "newCourse" }
  | { kind: "rename"; entry: Entry }
  | { kind: "move"; paths: string[] }
  | { kind: "confirm"; icon: LibIconName; title: string; body: JSX.Element; action: string; danger?: boolean; run: () => Promise<void> | void }
  | { kind: "settings" };

interface MenuState {
  x: number;
  y: number;
  items: ({ label: string; icon: LibIconName | "open" | "download" | "trash" | "restart" | "plus" | "import"; run: () => void; danger?: boolean } | "sep")[];
}

export function LibraryRoot() {
  return (
    <>
      <Show when={libraryOpen()}>
        <LibraryHome />
      </Show>
      <Show when={lockPrompt()}>{(p) => <LockDialog name={stem(basename(p().path))} answer={p().answer} />}</Show>
      <Notices />
      <ShareHost />
      <SharedLessonBar />
    </>
  );
}

/** Banners on the board: changed on disk, taken by another tab, save errors. */
function Notices() {
  return (
    <Show when={!libraryOpen() && notice()}>
      {(n) => (
        <>
          <Show when={n().kind === "lost"}><div class="lib-blocker" /></Show>
          <div class="lib-notice surface" role="alert">
            <LIcon name={n().kind === "lost" ? "lock" : "warn"} size={24} />
            <div class="grow">
              {n().kind === "changed" && (
                <><b>This lesson changed outside Rupok</b>An AI agent, a sync app or another device saved it while you were editing.</>
              )}
              {n().kind === "lost" && (
                <><b>Opened in another tab</b>This lesson is now being edited in another tab, so changes here are no longer saved.</>
              )}
              {n().kind === "error" && (
                <><b>Could not save to the Library</b>{(n() as { message: string }).message}. Your work is kept in this browser.</>
              )}
            </div>
            <div class="acts">
              <Show when={n().kind === "changed"}>
                <button class="btn" onClick={() => void resolveConflict(false)}>Load theirs</button>
                <button class="btn solid" onClick={() => void resolveConflict(true)}>Keep mine</button>
              </Show>
              <Show when={n().kind === "lost"}>
                <button class="btn solid" onClick={leaveLostLesson}><LIcon name="library" /><span class="label">Library</span></button>
              </Show>
              <Show when={n().kind === "error"}>
                <button class="btn icon" aria-label="Dismiss" onClick={() => setNotice(null)}><Icon name="close" /></button>
              </Show>
            </div>
          </div>
        </>
      )}
    </Show>
  );
}

function LibraryHome() {
  let root!: HTMLDivElement;
  let libSearch: HTMLInputElement | undefined; // voice input target (MicButton)
  const [query, setQuery] = createSignal("");
  const [sel, setSel] = createSignal<string[]>([]);
  const [anchor, setAnchor] = createSignal<string | null>(null);
  const [menu, setMenu] = createSignal<MenuState | null>(null);
  const [dialog, setDialog] = createSignal<Dialog | null>(null);
  const [fileDrag, setFileDrag] = createSignal(false);
  const [drag, setDrag] = createSignal<{ paths: string[]; label: string; x: number; y: number; over: string | null } | null>(null);
  let suppressClick = false;

  const sec = () => SECTIONS[section()];
  const student = () => libPrefs().role === "student";
  const navOrder = (): Section[] => {
    const base: Section[] = student() ? ["shared", "copies", "recent", "mine", "trash"] : ["recent", "mine", "shared", "trash"];
    if (cloudUser()) base.splice(base.indexOf("trash"), 0, "cloud");
    return base;
  };
  const [trashCloud, setTrashCloud] = createSignal(false);
  /** The library this view acts on: the device's, or Rupok Cloud. */
  const lib = () => (section() === "cloud" || (section() === "trash" && trashCloud()) ? cloudLibrary() : store());
  createEffect(() => setActiveLib(lib()));
  onCleanup(() => setActiveLib(null));
  /** The folder new things go into, and imports land in. */
  const targetDir = () => (section() === "mine" || section() === "copies" ? folder() : section() === "shared" ? SHARED_DIR : "");

  const go = (s: Section, dir = SECTIONS[s].base) => {
    setSection(s);
    setFolder(dir);
    setSel([]);
    setQuery("");
  };

  // ------------------------------------------------------------- data
  const [data] = createResource(
    () => ({ s: lib(), sec: section(), dir: folder(), q: query().trim().toLowerCase(), v: version(), pf: pendingFolder(), sort: libPrefs().sort, u: cloudUser()?.id }),
    async (k): Promise<{ folders: Entry[]; lessons: Item[] }> => {
      if (!k.s || (k.pf && k.s === store())) return { folders: [], lessons: [] };
      const sortLessons = (l: Entry[]) =>
        k.sort === "name" ? l.sort((a, b) => naturalCompare(a.name, b.name)) : l.sort((a, b) => b.modified - a.modified || naturalCompare(a.name, b.name));
      if (k.sec === "trash") return { folders: [], lessons: await k.s.listTrash() };
      if (k.q) {
        const scope = k.sec === "shared" || k.sec === "copies" ? SECTIONS[k.sec].base : "";
        const all = await k.s.walk(scope, { includeShared: k.sec === "recent" });
        const hit = (e: Entry) => e.name.toLowerCase().includes(k.q) || (e.title ?? "").toLowerCase().includes(k.q);
        return {
          folders: all.filter((e) => e.kind === "folder" && hit(e)).sort((a, b) => naturalCompare(a.name, b.name)),
          lessons: sortLessons(all.filter((e) => e.kind === "lesson" && hit(e))),
        };
      }
      if (k.sec === "recent") return { folders: [], lessons: await recentLessons() };
      const items = await k.s.list(k.dir).catch((e) => (k.sec === "cloud" && showToast(explain(e), 5000), [] as Entry[]));
      // Rupok Cloud: lessons other teachers shared with this account.
      if (k.sec === "shared" && k.dir === SHARED_DIR && k.u) {
        for (const n of await cloud.sharedWithMe().catch(() => [])) {
          if (n.kind !== "lesson") continue;
          items.push({ kind: "lesson", path: SHARED_CLOUD + n.id, name: n.name, size: n.lesson?.size ?? 0, modified: n.updated_at, title: n.lesson?.title, pages: n.lesson?.pages, author: n.owner?.name, thumb: null });
        }
      }
      return {
        folders: items.filter((e) => e.kind === "folder").sort((a, b) => naturalCompare(a.name, b.name)),
        lessons: sortLessons(items.filter((e) => e.kind === "lesson")),
      };
    },
  );
  const folders = () => data.latest?.folders ?? [];
  const lessons = () => data.latest?.lessons ?? [];
  const allItems = createMemo<Item[]>(() => [...folders(), ...lessons()]);
  const byKey = (k: string) => allItems().find((e) => keyOf(e) === k);

  const [storage, { refetch: refetchStorage }] = createResource(() => version(), () => storageEstimate());

  // -------------------------------------------------------- selection
  const isSel = (e: Item) => sel().includes(keyOf(e));
  const toggle = (e: Item) => {
    const k = keyOf(e);
    setSel((s) => (s.includes(k) ? s.filter((x) => x !== k) : [...s, k]));
    setAnchor(k);
  };
  const range = (e: Item) => {
    const keys = allItems().map(keyOf);
    const a = keys.indexOf(anchor() ?? keyOf(e));
    const b = keys.indexOf(keyOf(e));
    const [lo, hi] = a < b ? [a, b] : [b, a];
    setSel((s) => [...new Set([...s, ...keys.slice(lo, hi + 1)])]);
  };

  const activate = (e: Item, ev: MouseEvent) => {
    if (suppressClick) return;
    if (ev.shiftKey) return range(e);
    if (ev.ctrlKey || ev.metaKey || sel().length || isTrash(e)) return toggle(e);
    if (e.kind === "folder") {
      if (query()) setQuery("");
      setFolder(e.path);
      if (section() === "recent") setSection(isInside(e.path, SHARED_DIR) ? "shared" : "mine");
      setSel([]);
    } else if (e.path.startsWith(SHARED_CLOUD)) void openCloudShared(e.path.slice(SHARED_CLOUD.length), e.name);
    else void openLesson(e.path, section() === "cloud" ? { lib: cloudLibrary()! } : {});
  };
  /** A cloud lesson someone shared with this account: opened as a received lesson. */
  const openCloudShared = async (id: string, name: string) => {
    try {
      const bytes = await cloud.download(id);
      await controllerRef().importFiles([new File([bytes as BlobPart], `${name}.rpk`)]);
    } catch (e) {
      showToast(explain(e), 5000);
    }
  };
  const uploadToCloud = async (path: string) => {
    const c = cloudLibrary();
    if (!c) return;
    try {
      const dest = await c.freeLessonPath(isSpecial(path) ? "" : dirname(path), stem(basename(path)));
      await c.write(dest, await store()!.read(path));
      void refreshUsage();
      showToast(`Uploaded “${stem(basename(path))}” to Rupok Cloud`);
    } catch (e) {
      showToast(explain(e), 7000);
    }
  };
  const downloadLocal = async (path: string) => {
    const c = cloudLibrary();
    const local = store();
    if (!c || !local) return;
    try {
      const dest = await local.freeLessonPath("", stem(basename(path)));
      await local.write(dest, await c.read(path));
      showToast(`Saved a copy on this device: “${stem(basename(dest))}” in My lessons`);
    } catch (e) {
      showToast(explain(e), 5000);
    }
  };

  // ----------------------------------------------------------- actions
  const s = () => lib()!;
  const fail = (e: any) => showToast(explain(e), 6000);
  const lockedOut = async (paths: string[]) => {
    const busy = (await Promise.all(paths.map(async (p) => ((await heldElsewhere(`${s().id}|${p}`)) ? p : null)))).filter(Boolean) as string[];
    if (busy.length) showToast(`“${stem(basename(busy[0]))}” is open in another tab. Close it there first.`, 4500);
    return busy.length > 0;
  };
  const moveItems = async (paths: string[], dir: string) => {
    paths = paths.filter((p) => !isInside(dir, p) && dirname(p) !== dir);
    if (!paths.length || (await lockedOut(paths))) return;
    try {
      for (const p of paths) moveOpened(s().id, p, await s().move(p, dir));
      showToast(`Moved ${paths.length === 1 ? `“${stem(basename(paths[0]))}”` : `${paths.length} items`} to ${dir ? basename(dir) : "My lessons"}`);
    } catch (e) {
      fail(e);
    }
    setSel([]);
    bump();
  };
  const trashItems = async (paths: string[]) => {
    if (!paths.length || (await lockedOut(paths))) return;
    try {
      for (const p of paths) await s().trash(p);
      showToast(`Moved ${paths.length === 1 ? `“${stem(basename(paths[0]))}”` : `${paths.length} items`} to the trash`);
    } catch (e) {
      fail(e);
    }
    setSel([]);
    bump();
  };
  const duplicateItems = async (paths: string[]) => {
    try {
      for (const p of paths) await s().duplicate(p);
      showToast(paths.length === 1 ? "Duplicated" : `Duplicated ${paths.length} items`);
    } catch (e) {
      fail(e);
    }
    setSel([]);
    bump();
  };
  const exportItems = async (paths: string[]) => {
    if (paths.length === 1) {
      const e = byKey(paths[0]);
      return e?.kind === "folder" ? exportFolder(e.path) : exportLesson(paths[0]);
    }
    // Several: one zip of the common folder would include too much; export each.
    for (const p of paths) await (isLessonName(basename(p)) ? exportLesson(p) : exportFolder(p));
  };
  const restore = async (ids: string[]) => {
    try {
      for (const id of ids) await s().restore(id);
      showToast(ids.length === 1 ? "Restored" : `Restored ${ids.length} items`);
    } catch (e) {
      fail(e);
    }
    setSel([]);
    bump();
  };
  const deleteForever = (ids: string[]) =>
    setDialog({
      kind: "confirm", icon: "trash", danger: true, action: "Delete forever",
      title: ids.length === 1 ? "Delete forever?" : `Delete ${ids.length} items forever?`,
      body: "This can't be undone.",
      run: async () => {
        for (const id of ids) await s().deletePermanently(id).catch(fail);
        setSel([]);
        bump();
      },
    });
  const doImport = async (files?: File[]) => {
    const list = files ?? (await pickImportFiles());
    if (list.length) await importInto(targetDir(), list);
  };

  const menuFor = (e: Item, x: number, y: number) => {
    if (isTrash(e)) {
      setMenu({ x, y, items: [
        { label: "Restore", icon: "restart", run: () => void restore([e.trashId]) },
        { label: "Delete forever", icon: "trash", danger: true, run: () => deleteForever([e.trashId]) },
      ] });
      return;
    }
    const inShared = isInside(e.path, SHARED_DIR);
    const items: MenuState["items"] = [
      { label: "Open", icon: "open", run: () => (e.kind === "folder" ? (setFolder(e.path), setSection(inShared ? "shared" : section() === "recent" ? "mine" : section())) : void openLesson(e.path, section() === "cloud" ? { lib: cloudLibrary()! } : {})) },
    ];
    if (e.path.startsWith(SHARED_CLOUD)) {
      const id = e.path.slice(SHARED_CLOUD.length);
      setMenu({ x, y, items: [
        { label: "Open", icon: "open", run: () => void openCloudShared(id, e.name) },
        { label: "Make a copy in Rupok Cloud", icon: "copy", run: () => void cloud.copy(id, null).then(() => { showToast("Copied to your Rupok Cloud"); void refreshUsage(); bump(); }).catch(fail) },
      ] });
      return;
    }
    if (section() === "cloud" && e.kind === "lesson") items.push({ label: "Download a local copy", icon: "download", run: () => void downloadLocal(e.path) });
    else if (cloudUser() && e.kind === "lesson") items.push({ label: "Upload to Rupok Cloud", icon: "cloud", run: () => void uploadToCloud(e.path) });
    if (e.kind === "lesson" && (inShared || student())) items.push({ label: "Make a copy", icon: "copy", run: () => void makeCopy(e.path).then((p) => showToast(`Saved a copy to ${copiesDir() || "My lessons"}: “${stem(basename(p))}”`)).catch(fail) });
    items.push(
      { label: "Rename", icon: "rename", run: () => setDialog({ kind: "rename", entry: e }) },
      { label: "Duplicate", icon: "copy", run: () => void duplicateItems([e.path]) },
      { label: "Move to…", icon: "move", run: () => setDialog({ kind: "move", paths: [e.path] }) },
      { label: e.kind === "folder" ? "Export as zip" : "Export .rpk", icon: e.kind === "folder" ? "zip" : "download", run: () => void (e.kind === "folder" ? exportFolder(e.path) : exportLesson(e.path)) },
      "sep",
      { label: "Move to trash", icon: "trash", danger: true, run: () => void trashItems([e.path]) },
    );
    setMenu({ x, y, items });
  };
  const openMenu = (e: Item, ev: MouseEvent) => {
    ev.stopPropagation();
    ev.preventDefault();
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    const fromButton = (ev.currentTarget as HTMLElement).tagName === "BUTTON";
    menuFor(e, fromButton ? r.right : ev.clientX, fromButton ? r.bottom + 4 : ev.clientY);
  };
  const newMenu = (ev: MouseEvent) => {
    const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ x: r.left, y: r.bottom + 6, items: [
      { label: "New lesson", icon: "lesson", run: () => setDialog({ kind: "newLesson" }) },
      { label: "New folder", icon: "folderAdd", run: () => setDialog({ kind: "newFolder" }) },
      { label: "New course", icon: "course", run: () => setDialog({ kind: "newCourse" }) },
      "sep",
      { label: "Import lessons or PDFs", icon: "import", run: () => void doImport() },
    ] });
  };

  // ------------------------------------------------- drag to move (pointer)
  const dropTarget = (x: number, y: number, moving: string[]): string | null => {
    const el = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>("[data-drop]");
    const t = el?.dataset.drop;
    if (t === undefined) return null;
    if (t === "__trash") return t;
    return moving.some((m) => isInside(t, m)) ? null : t;
  };
  const startPointer = (ev: PointerEvent, e: Item) => {
    if (ev.button !== 0 || isTrash(e) || (ev.target as HTMLElement).closest(".lib-check, .lib-more")) return;
    const el = ev.currentTarget as HTMLElement;
    const pid = ev.pointerId;
    const [x0, y0] = [ev.clientX, ev.clientY];
    const mouse = ev.pointerType === "mouse";
    const paths = isSel(e) ? sel().filter((k) => byKey(k) && !isTrash(byKey(k)!)) : [e.path];
    const label = paths.length === 1 ? stem(basename(paths[0])) : `${paths.length} items`;
    let active = false;
    let armed = false;
    const begin = (x: number, y: number) => {
      active = true;
      try { el.setPointerCapture(pid); } catch { /* pointer gone */ }
      setDrag({ paths, label, x, y, over: null });
    };
    const timer = mouse ? 0 : window.setTimeout(() => (armed = true), 380);
    const move = (m: PointerEvent) => {
      if (m.pointerId !== pid) return;
      const d = Math.hypot(m.clientX - x0, m.clientY - y0);
      if (!active) {
        if (mouse && d > 6) begin(m.clientX, m.clientY);
        else if (!mouse && armed && d > 6) begin(m.clientX, m.clientY);
        else if (!mouse && !armed && d > 10) return end(false);
        if (!active) return;
      }
      m.preventDefault();
      setDrag({ paths, label, x: m.clientX, y: m.clientY, over: dropTarget(m.clientX, m.clientY, paths) });
    };
    const end = (commit: boolean) => {
      clearTimeout(timer);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
      const d = drag();
      setDrag(null);
      if (!active) {
        // Pen or touch: a long press without moving selects.
        if (commit && armed) {
          suppressClick = true;
          setTimeout(() => (suppressClick = false), 0);
          toggle(e);
        }
        return;
      }
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
      if (commit && d?.over != null) void (d.over === "__trash" ? trashItems(d.paths) : moveItems(d.paths, d.over));
    };
    const up = (u: PointerEvent) => u.pointerId === pid && end(true);
    const cancel = (c: PointerEvent) => c.pointerId === pid && end(false);
    window.addEventListener("pointermove", move, { passive: false });
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  };
  const dropOn = (path: string) => drag()?.over === path;

  // ---------------------------------------------------------- keyboard
  const keys = (e: KeyboardEvent) => {
    const t = e.target as HTMLElement;
    const editable = t?.isContentEditable || t?.tagName === "INPUT" || t?.tagName === "TEXTAREA" || t?.tagName === "SELECT";
    // Keys inside the Library never reach the (hidden) board's shortcuts.
    e.stopPropagation();
    if (editable || dialog()) return;
    const mod = e.ctrlKey || e.metaKey;
    const selected = sel().map(byKey).filter(Boolean) as Item[];
    if (e.key === "Escape") { setMenu(null); setSel([]); }
    else if (mod && e.key.toLowerCase() === "a") { e.preventDefault(); setSel(allItems().map(keyOf)); }
    else if ((e.key === "Delete" || e.key === "Backspace") && selected.length) {
      e.preventDefault();
      if (section() === "trash") deleteForever(selected.map(keyOf));
      else void trashItems(selected.map(keyOf));
    } else if (e.key === "F2" && selected.length === 1 && !isTrash(selected[0])) setDialog({ kind: "rename", entry: selected[0] });
    else if (e.key === "Enter" && selected.length === 1 && !isTrash(selected[0])) {
      const it = selected[0];
      if (it.kind === "folder") { setFolder(it.path); setSel([]); } else void openLesson(it.path, section() === "cloud" ? { lib: cloudLibrary()! } : {});
    } else if (mod && e.key.toLowerCase() === "f") { e.preventDefault(); root.querySelector<HTMLInputElement>(".lib-search input")?.focus(); }
  };
  onMount(() => {
    root.focus();
    // Keys and pastes with nothing focused would reach the board underneath.
    const bodyKeys = (e: KeyboardEvent) => {
      if (e.target === document.body || e.target === document.documentElement) keys(e);
    };
    const paste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement;
      if (t?.tagName === "INPUT" || t?.tagName === "TEXTAREA" || t?.isContentEditable) return;
      e.stopImmediatePropagation();
      const files = Array.from(e.clipboardData?.files ?? []);
      if (files.length) void doImport(files);
    };
    window.addEventListener("keydown", bodyKeys, true);
    window.addEventListener("paste", paste, true);
    onCleanup(() => {
      window.removeEventListener("keydown", bodyKeys, true);
      window.removeEventListener("paste", paste, true);
    });
  });

  // ------------------------------------------------------------- render
  const crumbs = createMemo(() => {
    const base = sec().base;
    if (section() === "recent" || section() === "trash" || query()) return [{ label: query() ? `Results for “${query()}”` : sec().label, path: null as string | null }];
    const parts = split(folder()).slice(split(base).length);
    const out: { label: string; path: string | null }[] = [{ label: sec().label, path: base }];
    parts.forEach((p, i) => out.push({ label: p, path: [...split(base), ...parts.slice(0, i + 1)].join("/") }));
    return out;
  });

  const subtitle = (): string | null => {
    if (query()) return null;
    switch (section()) {
      case "shared": return folder() === SHARED_DIR ? "Lessons you opened from a file or a link. They stay as they were sent; make a copy to write your own notes." : null;
      case "copies": return folder() === COPIES_DIR ? "Your own copies of lessons. Change them as much as you like: the original stays as it was." : null;
      case "trash": return "Items in the trash are deleted forever after 30 days.";
      case "cloud": return folder() === "" ? "Lessons in your Rupok Cloud space sync between your devices, and you can share them with a link." : null;
      default: return null;
    }
  };

  const meta = (e: Item) => {
    const where = section() === "recent" || query() ? (dirname(e.path) ? basename(dirname(e.path)) : "My lessons") : null;
    if (isTrash(e)) return `Deleted ${ago(e.trashedAt)} · from ${dirname(e.from) || "My lessons"}`;
    return [where, e.modified ? `Edited ${/^(Just now|Yesterday)$/.test(ago(e.modified)) ? ago(e.modified).toLowerCase() : ago(e.modified)}` : null].filter(Boolean).join(" · ");
  };

  const Thumb = (p: { e: Item }) => (
    <div class="lib-thumb">
      <Show when={p.e.thumb} fallback={<LIcon name={p.e.kind === "folder" ? "folder" : "lesson"} size={28} />}>
        <img src={p.e.thumb!} alt="" loading="lazy" draggable={false} />
      </Show>
      <Show when={p.e.kind === "lesson" && (p.e.pages ?? 0) > 0}>
        <span class="lib-pages">{p.e.pages} {p.e.pages === 1 ? "page" : "pages"}</span>
      </Show>
    </div>
  );

  const Check = (p: { e: Item }) => (
    <button class="lib-check" aria-label={isSel(p.e) ? "Deselect" : "Select"} aria-pressed={isSel(p.e)} onClick={(ev) => { ev.stopPropagation(); toggle(p.e); }}>
      <Show when={isSel(p.e)}><LIcon name="check" size={16} /></Show>
    </button>
  );
  const More = (p: { e: Item }) => (
    <button class="lib-more" aria-label="More actions" title="More actions" onClick={(ev) => openMenu(p.e, ev)}>
      <Icon name="more" size={16} />
    </button>
  );

  const FolderCard = (p: { e: Entry }) => {
    const role = () => folderRole(p.e.path);
    const cap = () => (role() === "folder" ? "Folder" : role()[0].toUpperCase() + role().slice(1));
    return (
      <div
        class="lib-folder"
        classList={{ sel: isSel(p.e), "drop-on": dropOn(p.e.path) }}
        data-drop={p.e.path}
        data-path={p.e.path}
        role="button"
        tabIndex={0}
        onClick={(ev) => activate(p.e, ev)}
        onPointerDown={(ev) => startPointer(ev, p.e)}
        onContextMenu={(ev) => openMenu(p.e, ev)}
        title={p.e.name}
      >
        <LIcon name={p.e.path === COPIES_DIR ? "copies" : role() === "course" ? "course" : "folder"} size={24} active={isSel(p.e)} />
        <span class="txt">
          <span class="nm">{p.e.name}</span>
          <span class="cap">{section() === "recent" || query() ? (dirname(p.e.path) || "My lessons") : cap()}</span>
        </span>
        <More e={p.e} />
      </div>
    );
  };

  const LessonCard = (p: { e: Item }) => (
    <div
      class="lib-card"
      classList={{ sel: isSel(p.e) }}
      data-path={p.e.path}
      role="button"
      tabIndex={0}
      onClick={(ev) => activate(p.e, ev)}
      onPointerDown={(ev) => startPointer(ev, p.e)}
      onContextMenu={(ev) => openMenu(p.e, ev)}
      title={p.e.title && p.e.title !== p.e.name ? `${p.e.name} (${p.e.title})` : p.e.name}
    >
      <Thumb e={p.e} />
      <div class="meta">
        <span class="t">{p.e.name}</span>
        <span class="s">
          <Show when={p.e.copiedFrom}>
            <span class="lib-chip" title={`Copy of “${p.e.copiedFrom!.title}”${p.e.copiedFrom!.author ? ` by ${p.e.copiedFrom!.author}` : ""}`}>Copy</span>
          </Show>
          <span>{meta(p.e)}</span>
        </span>
      </div>
      <Check e={p.e} />
      <More e={p.e} />
    </div>
  );

  const Row = (p: { e: Item }) => (
    <div
      class="lib-row"
      classList={{ sel: isSel(p.e), "drop-on": p.e.kind === "folder" && dropOn(p.e.path) }}
      data-path={p.e.path}
      {...(p.e.kind === "folder" && !isTrash(p.e) ? { "data-drop": p.e.path } : {})}
      role="button"
      tabIndex={0}
      onClick={(ev) => activate(p.e, ev)}
      onPointerDown={(ev) => startPointer(ev, p.e)}
      onContextMenu={(ev) => openMenu(p.e, ev)}
    >
      <Check e={p.e} />
      <Show when={p.e.kind === "lesson"} fallback={<span class="fold-ico"><LIcon name="folder" size={24} /></span>}>
        <Thumb e={p.e} />
      </Show>
      <span style={{ "min-width": "0", display: "flex", "flex-direction": "column" }}>
        <span class="t">{p.e.name}</span>
        <span class="s">{meta(p.e) || (p.e.kind === "folder" ? "Folder" : "")}</span>
      </span>
      <span class="col">{p.e.kind === "lesson" ? (p.e.pages ? `${p.e.pages} ${p.e.pages === 1 ? "page" : "pages"}` : "") : ""}</span>
      <span class="col">{p.e.kind === "lesson" && p.e.size ? bytes(p.e.size) : ""}</span>
      <More e={p.e} />
    </div>
  );

  const Empty = () => {
    const q = query();
    const sx = section();
    const atRoot = folder() === sec().base;
    const [icon, title, body]: [LibIconName, string, string] = q
      ? ["search", "Nothing found", `No lessons or folders match “${q}”.`]
      : sx === "trash" ? ["trash", "Trash is empty", "Things you delete stay here for 30 days, so you can bring them back."]
        : sx === "shared" ? ["shared", "Lessons you receive show up here", "Open a .rpk file your teacher sent on WhatsApp, e-mail or Drive, or drop it here."]
          : sx === "copies" ? ["copies", "No copies yet", "Open a lesson from Shared with me and choose “Save a copy” to keep your own version."]
            : sx === "recent" ? ["recent", "Nothing here yet", "Lessons you open or change appear here."]
              : sx === "cloud" && atRoot ? ["cloud", "Nothing in Rupok Cloud yet", "Make a lesson here, or choose “Upload to Rupok Cloud” on a lesson in My lessons."]
              : atRoot ? ["mine", "Your library is empty", "Make your first lesson, or a course to organise a whole class by chapter."]
                : ["folder", "This folder is empty", "Add a lesson, or drag lessons here."];
    return (
      <div class="lib-empty">
        <div class="badge"><LIcon name={icon} size={28} /></div>
        <h3>{title}</h3>
        <p>{body}</p>
        <Show when={!q && sx !== "trash"}>
          <div class="row">
            <Show when={sx === "shared"} fallback={
              <>
                <button class="btn solid" onClick={() => setDialog({ kind: "newLesson" })}><Icon name="plus" /><span class="label">New lesson</span></button>
                <Show when={sx === "mine" && atRoot}>
                  <button class="btn" onClick={() => setDialog({ kind: "newCourse" })}><LIcon name="course" /><span class="label">New course</span></button>
                </Show>
                <button class="btn" onClick={() => void doImport()}><Icon name="import" /><span class="label">Import</span></button>
              </>
            }>
              <button class="btn solid" onClick={() => void doImport()}><Icon name="import" /><span class="label">Import a lesson</span></button>
            </Show>
          </div>
        </Show>
      </div>
    );
  };

  const subjects = createMemo(() => folders().filter(() => section() === "mine" && folder() === "").map((f) => f.name));
  const selectedItems = () => sel().map(byKey).filter(Boolean) as Item[];
  const dark = () => document.documentElement.dataset.theme === "dark";

  return (
    <div
      class="lib"
      classList={{ selecting: sel().length > 0, dragging: !!drag() }}
      ref={root}
      tabIndex={-1}
      role="application"
      aria-label="Library"
      onKeyDown={keys}
      onDragOver={(e) => {
        if (!e.dataTransfer?.types.includes("Files")) return;
        e.preventDefault();
        e.stopPropagation();
        setFileDrag(true);
      }}
      onDragLeave={(e) => {
        e.stopPropagation();
        if (!e.relatedTarget || !root.contains(e.relatedTarget as Node)) setFileDrag(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setFileDrag(false);
        const files = Array.from(e.dataTransfer?.files ?? []);
        if (files.length) void doImport(files);
      }}
    >
      <aside class="lib-side" aria-label="Library sections">
        <div class="lib-brand">
          <Mark size={28} />
          <span class="wordmark"><b>{WORDMARK.accent}</b>{WORDMARK.rest}</span>
        </div>
        <button class="btn solid lib-new" onClick={newMenu} aria-haspopup="menu"><Icon name="plus" size={20} /><span class="label">New</span></button>
        <nav class="lib-nav">
          <For each={navOrder()}>
            {(id) => (
              <button
                classList={{ on: section() === id && !query(), "drop-on": (id === "mine" && dropOn("")) || (id === "trash" && dropOn("__trash")) }}
                data-section={id}
                {...(id === "mine" ? { "data-drop": "" } : id === "trash" ? { "data-drop": "__trash" } : {})}
                onClick={() => go(id)}
                aria-current={section() === id ? "page" : undefined}
              >
                <LIcon name={SECTIONS[id].icon} size={22} active={section() === id} />
                <span class="nav-label">{SECTIONS[id].label}</span>
              </button>
            )}
          </For>
          <Show when={CLOUD_ENABLED}>
            <button disabled><LIcon name="cloud" size={22} /><span class="nav-label">Cloud</span><span class="soon">Soon</span></button>
          </Show>
        </nav>
        <div class="lib-side-foot">
          <div class="lib-where" title={store()?.label}>
            <LIcon name={store()?.kind === "desktop" || store()?.kind === "folder" ? "laptop" : "browser"} size={16} />
            <span>{store()?.kind === "desktop" ? store()!.label : store()?.kind === "folder" ? `Folder: ${store()!.label}` : "Saved in this browser"}</span>
          </div>
          <Show when={storage()}>
            {(st) => (
              <>
                <div class="lib-meter" role="meter" aria-valuemin={0} aria-valuemax={st().quota} aria-valuenow={st().usage} aria-label="Storage used">
                  <i style={{ width: `${Math.min(100, Math.max(1, (st().usage / Math.max(1, st().quota)) * 100))}%` }} />
                </div>
                <span class="lib-foot-note">{bytes(st().usage)} of {bytes(st().quota)} used</span>
                <Show when={!st().persisted}>
                  <button class="btn" title="Ask the browser never to clear your lessons when space runs low" onClick={async () => { const ok = await persistStorage(); showToast(ok ? "Your lessons are kept safe in this browser" : "The browser did not allow it. Use a folder on this computer to be safe."); void refetchStorage(); }}>
                    <LIcon name="shield" /><span class="label">Keep lessons safe</span>
                  </button>
                </Show>
              </>
            )}
          </Show>
          <CloudAccount compact />
          <button class="btn" onClick={() => setDialog({ kind: "settings" })}><Icon name="settings" /><span class="label">Library settings</span></button>
        </div>
      </aside>

      <main class="lib-main">
        <header class="lib-head">
          <nav class="lib-crumbs" aria-label="Breadcrumbs">
            <For each={crumbs()}>
              {(c, i) => (
                <>
                  <Show when={i() > 0}><LIcon name="chevron" size={20} /></Show>
                  <button
                    classList={{ last: i() === crumbs().length - 1, "drop-on": c.path !== null && dropOn(c.path) }}
                    {...(c.path !== null && i() < crumbs().length - 1 ? { "data-drop": c.path } : {})}
                    onClick={() => c.path !== null && i() < crumbs().length - 1 && (setFolder(c.path), setSel([]))}
                    title={c.label}
                  >
                    {c.label}
                  </button>
                </>
              )}
            </For>
          </nav>
          <div class="lib-tools">
            <label class="lib-search">
              <LIcon name="search" />
              <input ref={libSearch} class="input" type="search" placeholder="Search lessons" aria-label="Search lessons" value={query()} onInput={(e) => setQuery(e.currentTarget.value)} />
              <MicButton target={() => libSearch} size={32} class="search-mic" label="Search lessons by voice" />
            </label>
            <select class="input" aria-label="Sort" value={libPrefs().sort} onChange={(e) => setLibPrefs({ sort: e.currentTarget.value as "modified" | "name" })}>
              <option value="modified">Last edited</option>
              <option value="name">Name</option>
            </select>
            <div class="seg" role="radiogroup" aria-label="View">
              <button classList={{ on: libPrefs().view === "grid" }} role="radio" aria-checked={libPrefs().view === "grid"} aria-label="Grid" title="Grid" onClick={() => setLibPrefs({ view: "grid" })}><LIcon name="grid" /></button>
              <button classList={{ on: libPrefs().view === "list" }} role="radio" aria-checked={libPrefs().view === "list"} aria-label="List" title="List" onClick={() => setLibPrefs({ view: "list" })}><LIcon name="list" /></button>
            </div>
            <Show when={section() !== "trash"}>
              <button class="btn" onClick={() => void doImport()} title="Import .rpk lessons or PDFs (or drop them here)"><Icon name="import" /><span class="label">Import</span></button>
            </Show>
            <Show when={section() === "trash" && cloudUser()}>
              <div class="seg" role="radiogroup" aria-label="Which trash">
                <button role="radio" style={{ width: "auto", padding: "0 12px" }} aria-checked={!trashCloud()} classList={{ on: !trashCloud() }} onClick={() => { setTrashCloud(false); setSel([]); }}>This device</button>
                <button role="radio" style={{ width: "auto", padding: "0 12px" }} aria-checked={trashCloud()} classList={{ on: trashCloud() }} data-trash="cloud" onClick={() => { setTrashCloud(true); setSel([]); }}>Rupok Cloud</button>
              </div>
            </Show>
            <Show when={section() === "trash" && lessons().length > 0}>
              <button class="btn danger" onClick={() => setDialog({ kind: "confirm", icon: "trash", danger: true, title: "Empty the trash?", body: "Everything in the trash is deleted forever. This can't be undone.", action: "Empty trash", run: async () => { await s().emptyTrash().catch(fail); bump(); } })}>
                <Icon name="trash" /><span class="label">Empty trash</span>
              </button>
            </Show>
            <button class="btn icon" onClick={() => setPrefs({ theme: dark() ? "light" : "dark" })} title="Light / dark" aria-label="Toggle theme"><Icon name={dark() ? "sun" : "moon"} /></button>
          </div>
        </header>
        <Show when={subtitle()}><p class="lib-sub">{subtitle()}</p></Show>
        <Show when={pendingFolder()}>
          {(h) => (
            <div class="lib-banner">
              <LIcon name="lock" />
              <span class="grow">Your library is in the folder <b>{h().name}</b>. The browser needs your permission to open it again.</span>
              <button class="btn solid" onClick={() => void grantFolder()}>Allow access</button>
            </div>
          )}
        </Show>

        <div class="lib-body">
          <Show when={data.latest} fallback={<div class="lib-empty"><p>Loading your lessons…</p></div>}>
            <Show when={allItems().length} fallback={<Empty />}>
              <Show when={libPrefs().view === "grid"} fallback={
                <div class="lib-rows" role="list"><For each={allItems()}>{(e) => <Row e={e} />}</For></div>
              }>
                <Show when={folders().length}>
                  <div class="lib-group">Folders</div>
                  <div class="lib-folders"><For each={folders()}>{(e) => <FolderCard e={e} />}</For></div>
                </Show>
                <Show when={lessons().length}>
                  <Show when={folders().length}><div class="lib-group">Lessons</div></Show>
                  <div class="lib-grid"><For each={lessons()}>{(e) => <LessonCard e={e} />}</For></div>
                </Show>
              </Show>
            </Show>
          </Show>
        </div>

        <Show when={sel().length}>
          <div class="lib-selbar surface" role="toolbar" aria-label="Selection">
            <span class="count">{sel().length} selected</span>
            <Show when={section() === "trash"} fallback={
              <>
                <button class="btn" onClick={() => setDialog({ kind: "move", paths: sel() })}><LIcon name="move" /><span class="label">Move</span></button>
                <button class="btn" onClick={() => void duplicateItems(sel())}><Icon name="duplicate" /><span class="label">Duplicate</span></button>
                <button class="btn" onClick={() => void exportItems(sel())}><Icon name="download" /><span class="label">Export</span></button>
                <button class="btn danger" onClick={() => void trashItems(sel())}><Icon name="trash" /><span class="label">Trash</span></button>
              </>
            }>
              <button class="btn" onClick={() => void restore(sel())}><Icon name="restart" /><span class="label">Restore</span></button>
              <button class="btn danger" onClick={() => deleteForever(sel())}><Icon name="trash" /><span class="label">Delete forever</span></button>
            </Show>
            <div class="vsep" />
            <button class="btn icon" aria-label="Clear selection" title="Clear selection · Esc" onClick={() => setSel([])}><Icon name="close" /></button>
          </div>
        </Show>
        <Show when={fileDrag()}><div class="lib-dropzone">Drop .rpk lessons or PDFs to import them here</div></Show>
      </main>

      <Show when={drag()}>
        {(d) => (
          <div class="lib-ghost" style={{ left: `${d().x}px`, top: `${d().y}px` }}>
            <Show when={d().paths.length > 1} fallback={<LIcon name={isLessonName(basename(d().paths[0])) ? "lesson" : "folder"} />}>
              <span class="n">{d().paths.length}</span>
            </Show>
            <span>{d().label}</span>
          </div>
        )}
      </Show>

      <Show when={menu()}>
        {(m) => (
          <>
            <div class="lib-menu-scrim" onPointerDown={() => setMenu(null)} onContextMenu={(e) => { e.preventDefault(); setMenu(null); }} />
            <div
              class="lib-menu"
              role="menu"
              ref={(el) => queueMicrotask(() => {
                // Keep the menu on screen.
                const r = el.getBoundingClientRect();
                el.style.left = `${Math.max(8, Math.min(m().x - (m().x > window.innerWidth - 240 ? r.width : 0), window.innerWidth - r.width - 8))}px`;
                el.style.top = `${Math.max(8, Math.min(m().y, window.innerHeight - r.height - 8))}px`;
              })}
              style={{ left: `${m().x}px`, top: `${m().y}px` }}
            >
              <For each={m().items}>
                {(it) =>
                  it === "sep" ? <hr /> : (
                    <button class="btn" classList={{ danger: it.danger }} role="menuitem" onClick={() => { setMenu(null); it.run(); }}>
                      {["open", "download", "trash", "restart", "plus", "import"].includes(it.icon) ? <Icon name={it.icon as any} /> : <LIcon name={it.icon as LibIconName} />}
                      <span class="label">{it.label}</span>
                    </button>
                  )
                }
              </For>
            </div>
          </>
        )}
      </Show>

      <Show when={dialog()}>
        {(d) => {
          const close = () => setDialog(null);
          const rootLabel = section() === "copies" ? "My copies" : section() === "shared" ? "Shared with me" : "My lessons";
          const dir = () => (targetDir() === SHARED_DIR ? "" : targetDir());
          const dirShown = () => split(dir()).slice(split(SECTIONS[section()].base).length).join("/");
          switch (d().kind) {
            case "newLesson":
              return <NewLessonDialog dir={dirShown()} rootLabel={rootLabel} onClose={close} onCreate={(name, t) => { close(); void newLesson(dir(), name, t).catch(fail); }} />;
            case "newFolder":
              return <NameDialog icon="folderAdd" title="New folder" label="Folder name" initial="New folder" action="Create folder" onClose={close} onSubmit={(n) => { close(); void newFolder(dir(), n).catch(fail); }} />;
            case "newCourse":
              return (
                <NewCourseDialog
                  subjects={subjects()}
                  initialSubject={section() === "mine" && split(folder()).length >= 1 ? split(folder())[0] : ""}
                  onClose={close}
                  onCreate={(subject, course, chapters) => {
                    close();
                    void newCourse(subject, course, chapters).then((p) => { go("mine", p); showToast(`Created “${basename(p)}”`); }).catch(fail);
                  }}
                />
              );
            case "rename": {
              const e = (d() as { entry: Entry }).entry;
              return (
                <NameDialog icon="rename" title={`Rename ${e.kind === "folder" ? "folder" : "lesson"}`} label="Name" initial={e.name} action="Rename" onClose={close}
                  onSubmit={async (n) => {
                    close();
                    if (await lockedOut([e.path])) return;
                    try {
                      const np = await renameItem(e.path, n);
                      moveOpened(s().id, e.path, np);
                      setSel([]);
                      bump();
                    } catch (err) { fail(err); }
                  }}
                />
              );
            }
            case "move":
              return <MoveDialog store={s()} moving={(d() as { paths: string[] }).paths} onClose={close} onMove={(to) => { const paths = (d() as { paths: string[] }).paths; close(); void moveItems(paths, to); }} />;
            case "confirm": {
              const c = d() as Extract<Dialog, { kind: "confirm" }>;
              return <ConfirmDialog icon={c.icon} title={c.title} body={c.body} action={c.action} danger={c.danger} onClose={close} onConfirm={() => { close(); void c.run(); }} />;
            }
            case "settings":
              return (
                <LibDialog icon="library" title="Library settings" sub="Where your lessons live, and who is using Rupok." width={640} onClose={close}>
                  <LibrarySettings />
                  <div class="actions" style={{ "margin-top": "8px" }}><div class="grow" /><button class="btn solid" onClick={close}>Done</button></div>
                </LibDialog>
              );
          }
        }}
      </Show>

      <Show when={roleChoice()}>
        <RoleDialog onChoose={(r) => { chooseRole(r); setSel([]); }} />
      </Show>
    </div>
  );
}
