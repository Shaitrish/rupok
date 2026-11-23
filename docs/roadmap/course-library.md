# Course library

`Status: 🚧 first version shipped (local Library) · Phase 2 · Difficulty: intermediate`

Organise lessons into subjects, courses and chapters, with thumbnails, search and "next lesson", stored as plain folders and `.rpk` files so it syncs anywhere.

## What shipped (first version)

The **Library** is the board's home screen. How it works, where files live
and how agents and sync clients use it: [docs/library.md](../library.md).

- **Storage abstraction** `LibraryStore` (`apps/board/src/library/types.ts`),
  implemented once (`FileLibrary`, `store.ts`) over small file-system drivers:
  - desktop: a real folder, default `Documents/Rupok`, changeable in Settings
    (Rust: `apps/desktop/src/library.rs`, sandboxed paths, validated atomic
    writes; Tauri commands and a `notify` folder watcher in `library_cmd.rs`);
  - browser: the Origin Private File System, or (Chrome, Edge) "Use a folder on
    this computer" through the File System Access API, with the handle kept in
    IndexedDB and permission asked again after a restart; IndexedDB when there
    is no OPFS. Persistent storage is requested and usage shown against quota.
  - The layout is `Subject/Course/Chapter/lesson.rpk`, the same plain folders
    `rupok mcp` works in. `rupok mcp` now defaults to the library folder when
    it exists (`$RUPOK_LIBRARY` overrides).
- **Library UI** (`components/Library*.tsx`): Recent, My lessons, Shared with
  me, My copies (students), Trash; Cloud is stubbed and hidden. Breadcrumbs,
  search, sort, grid and list, multi-select, drag to move (mouse, pen, touch),
  rename, duplicate, move, trash/restore/delete forever, new lesson from
  templates (blank, 16:9 slides, lined, grid), new folder, new course (subject +
  course + numbered chapters), import (`.rpk`, PDF → new lesson, drag and
  drop), export (`.rpk`, folder as zip). Light and dark, large touch targets.
- **Thumbnails** are generated on save from the engine's page preview and
  stored as `thumbs/0.png` (in TypeScript, `src/library/thumbs.ts`, not yet
  `Engine::set_thumbnail`).
- **Fast summaries:** `src/library/zip.ts` reads only the central directory,
  `manifest.json` and `thumbs/0.png` with range reads (the `read_summary` idea
  below, done in TypeScript); summaries are cached in IndexedDB.
- **Saving:** a lesson opened from the Library autosaves into its own file
  (crash-safe), with the folder written back as manifest `course` and library
  metadata (`library.id`, `author`, `copiedFrom`) in the manifest. One tab
  edits a lesson at a time (BroadcastChannel). Lessons changed on disk by an
  agent or a sync client reload, or ask when there are local changes.
- **Roles:** Teacher/Student in Settings and a first-run choice. Students land
  on Shared with me, read received lessons in the Student view, and "Save a
  copy to my library" (also in `/board/?view`) makes an independent copy that
  records `copiedFrom`.
- **Migration:** the old single autosave becomes a library lesson on first
  run; the autosave itself is kept as the recovery copy of the open lesson.

### Not yet (still planned below)

- `spec/library.md` and the RFC for the `library` manifest key, `course.json`
  and `chapter.json` (order, colours, notes). Order is by numeric prefix only.
- `rupok_format::read_summary` in Rust, `Engine::set_thumbnail`, an index cache
  for desktop in Rust (the browser-side IndexedDB cache is used everywhere).
- "Next lesson" button, page-title search, "Reveal in file manager".
- MCP tools `list_courses`, `create_chapter`, `move_lesson`.
- Cloud provider (Rupok Cloud) and sharing links.

## Goal

A teacher's year is hundreds of lessons across several classes. Give them a
library view that answers "what do I teach next in Grade 9 ICT?" in one tap,
without a database server and without locking files into an app-specific
store.

## User story

- *As a teacher*, I open Rupok and see my courses ("Grade 9 ICT",
  "Grade 10 Physics"); I tap one and see chapters with lesson thumbnails in
  teaching order.
- *As a teacher*, I tap "Next lesson" after class and the next lesson in the
  chapter opens.
- *As a teacher*, I duplicate last year's lesson as the start of this year's.
- *As a teacher*, my AI agent can see the course structure and put a new lesson
  into the right chapter.

## Scope

- Library root folder (default `~/Documents/Rupok`, configurable; shipped).
- Folder convention `Subject/Course/Chapter/lesson.rpk` (shipped), mirrored in each
  lesson's `manifest.json` `course` field (`CourseRef { subject, course, chapter }`
  already exists in `crates/rupok-format/src/package.rs`).
- Optional per-course and per-chapter metadata files (order, colours, notes).
- Library index cache for fast browsing; page thumbnails.
- Library UI in `apps/board` (desktop first; browser via File System Access API).
- MCP tools for course structure.

## Non-goals

- Multi-teacher sharing and permissions (sync handles devices, not people).
- Student rosters (Phase 3).
- The chapter knowledge base (see [knowledge-base](./knowledge-base.md)); this
  plan only reserves its folder.

## Design

### On-disk layout (new spec, requires an RFC)

> The first version ships `Subject/Course/Chapter/lesson.rpk` (the subject is a
> folder, not a `course.json` key) plus `My copies/`, `Shared with me/` and
> `.trash/`; see [docs/library.md](../library.md#folder-layout). The proposal
> below is kept for the RFC.

```text
Library/
  Grade 9 ICT/                         course folder (subject from course.json)
    course.json                        optional
    01 Number systems/                 chapter folder
      chapter.json                     optional
      01 Binary numbers.rpk
      02 Hexadecimal.rpk
      kb/                              reserved for the knowledge base
    02 Networks/
      ...
```

- Ordering: a numeric prefix (`01 `) sorts naturally; `chapter.json`
  `"order": ["Binary numbers.rpk", ...]` overrides.
- The folder path is the source of truth; on save, the app writes the matching
  `course` field into the lesson manifest so a lesson e-mailed alone still
  knows where it belongs.

`course.json` / `chapter.json` (proposed):

```json
{ "schema": 1, "title": "Grade 9 ICT", "subject": "ICT", "color": "#2563eb",
  "order": ["01 Number systems", "02 Networks"], "notes": "Mon/Wed 10:00" }
```

This layout is a new document, `spec/library.md` (Apache-2.0), and **requires
an RFC**. Unknown keys must be ignored, like the rest of the format.

### Fast manifest reads (new API)

`Package::read` inflates every entry, including `doc.loro`, which is too slow
for scanning hundreds of files. Add:

```rust
// crates/rupok-format/src/package.rs (new)
pub struct Summary { pub manifest: Manifest, pub thumb0: Option<Vec<u8>>, pub objects: usize, pub assets: usize }
/// Read only the central directory, `mimetype`, `manifest.json` and `thumbs/0.png`.
pub fn read_summary(bytes: &[u8]) -> Result<Summary>;
```

It must apply the same path rules and size limits as `Package::read`
(`spec/rupok-format.md#reader-limits-and-safety`).

### Thumbnails

`Package.thumbs` is read and written today but nothing generates it. On save,
the UI renders each page small and passes the PNGs to the engine:

```rust
// crates/rupok-wasm/src/engine.rs (new)
pub fn set_thumbnail(&mut self, page_index: usize, png: Vec<u8>);   // stored as thumbs/<n>.png
```

- Size 480 × 270 (16:9) or frame aspect, PNG; regenerate only for pages whose
  content changed since the last save (track per-page revision).
- Thumbnails include DOM items (objects, text, images) by compositing the
  canvas with a snapshot of the DOM layer; first version may show ink and
  images only.

### Index cache

`<app data>/library-index.json`: path, mtime, size, title, pages, course ref,
thumb file. Rebuilt incrementally (stat every file, re-read only changed ones).
Never authoritative; delete it and it is rebuilt.

### UI (`apps/board`)

- Library home: courses as cards → chapters as sections → lesson thumbnails.
- Search box: title and page titles (from the index); filter by course.
- Actions: New lesson in chapter, Duplicate, Rename, Move to chapter, Reveal in
  file manager (desktop), Delete (to OS trash).
- "Next lesson" button in the board's top bar when the open lesson is in the
  library.

### MCP (`crates/rupok-cli`)

`list_lessons` exists. Proposed additions, all inside the workspace sandbox
(`workspace.rs`):

| Tool | Purpose |
|---|---|
| `list_courses` | Courses → chapters → lessons with titles and order |
| `create_chapter { course, title }` | Make a chapter folder (+ `chapter.json`) |
| `create_lesson` (extend) | Optional `course`/`chapter` to place the file and fill the manifest |
| `move_lesson { lesson, chapter, position? }` | Move and reorder |

## Implementation tasks

1. **RFC: `spec/library.md`** (folders, `course.json`, `chapter.json`,
   ordering, `kb/` reservation). *(intermediate)*
2. **`rupok_format::read_summary`** with limits and tests against
   `Package::write` output. *(intermediate)*
3. **Thumbnail generation**: `Engine::set_thumbnail` + `Board` binding + UI
   capture on save. *(intermediate)*
4. **Index cache** module (desktop, Rust side in `apps/desktop`): scan,
   incremental update, file watcher. *(intermediate)*
5. **Library home UI**: courses, chapters, lesson grid with thumbnails.
   *(intermediate)*
6. **Lesson actions**: new, duplicate, rename, move, delete to trash.
   *(good-first-issue each)*
7. **"Next lesson"** button and keyboard shortcut. *(good-first-issue)*
8. **Search** over titles and page titles. *(good-first-issue)*
9. **Manifest `course` write-back** on save from the folder path.
   *(good-first-issue)*
10. **MCP tools** `list_courses`, `create_chapter`, `move_lesson`, and
    `create_lesson` placement. *(intermediate)*
11. **Browser support** via File System Access API (Chromium) with a fallback
    message elsewhere. *(intermediate)*

## Acceptance criteria and tests

- `read_summary` on a 50 MB lesson reads < 1 MB of compressed data (test with
  a counting reader) and returns the same manifest as `Package::read`.
- `read_summary` rejects the same unsafe packages as `Package::read`
  (reuse the `rejects_non_packages_and_unsafe_paths` fixture).
- Library of 500 lessons: first index build completes, second launch shows the
  library from cache.
- Moving a lesson between chapters updates its manifest `course` on next save.
- MCP: `list_courses` output matches the folder tree; paths outside the
  workspace are refused.

## Performance budget

| Metric | Budget |
|---|---|
| Library home with 500 lessons (warm cache) | visible ≤ 300 ms |
| Cold index build, 500 lessons on SSD | ≤ 5 s, UI usable while it runs |
| Thumbnail generation on save | ≤ 30 ms per changed page, off the pen path |

## Open questions

- Should a course be one folder per *class* (Grade 9A, 9B) or per *syllabus*
  with classes as tags? Survey teachers.
- How to handle the same lesson used in two courses (symlink, copy, or a
  reference file)?
- Is a numeric filename prefix friendly enough, or should order live only in
  `chapter.json`?

## References

- `crates/rupok-format/src/package.rs` — `Manifest`, `CourseRef`, `Package.thumbs`
- `spec/rupok-format.md` — `#manifestjson`, `#thumbnails-and-layers`
- `crates/rupok-cli/src/mcp.rs` — `list_lessons`, `create_lesson`
- Related: [knowledge-base](./knowledge-base.md), [google-drive-sync](./google-drive-sync.md),
  [tauri-desktop](./tauri-desktop.md)
