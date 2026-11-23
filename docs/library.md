# The Library

The Library is Rupok's home screen: every lesson a teacher or student has, in
plain folders of `.rpk` files. It is local-first. Lessons live on the device,
work offline, and sync with whatever already syncs folders (Google Drive or
OneDrive for desktop). Rupok Cloud will plug in later as one more storage
provider.

- **Code:** `apps/board/src/library/` (storage, session, actions) and
  `apps/board/src/components/Library*.tsx` (UI). Desktop side:
  `apps/desktop/src/library.rs` and `library_cmd.rs`.
- **Plan and status:** [roadmap/course-library.md](roadmap/course-library.md).

## Where the files live

| Platform | Location | Notes |
|---|---|---|
| Desktop app (Windows, Linux) | `Documents/Rupok` (`~/Documents/Rupok`, `C:\Users\<you>\Documents\Rupok`) | A real folder. Change it in **Settings → Library → Change folder…**. The choice is stored in `library.json` in the app's config folder (`~/.config/app.rupok.desktop/` on Linux, `%APPDATA%\app.rupok.desktop\` on Windows). If Windows redirects Documents to OneDrive, the library is synced by OneDrive automatically. |
| Browser (default) | The site's Origin Private File System, folder `Rupok/` | Private to the browser profile and the site. Rupok asks the browser to keep it (`navigator.storage.persist()`) and shows the space used against the quota. Browsers without OPFS fall back to IndexedDB (database `rupok-library`). |
| Browser, "Use a folder on this computer" (Chrome, Edge) | Any folder you pick | Uses the File System Access API. The folder handle is kept in IndexedDB; after a restart the browser asks for permission again with one click ("Allow access"). Pick the desktop app's folder, or a folder inside Google Drive for desktop, to share one library between the browser, the app and other devices. |

Rupok never needs a server for any of these.

## Folder layout

The same layout on every platform, and the same one the agent tools use:

```text
Rupok/                                   the library root
  ICT/                                   subject
    Grade 9 ICT/                         course (a class)
      01 Number systems/                 chapter (numbered: teaching order)
        Binary numbers.rpk               lesson
        Hexadecimal.rpk
      02 Networks/
  Untitled lesson.rpk                    lessons can sit at any level
  My copies/                             students' own copies of received lessons
  Shared with me/                        lessons opened from a file or a link
  .trash/<id>/                           the trash: the item plus trash.json
```

- **The file name is the lesson's name.** Renaming in the Library renames the
  file (and the manifest title); changing the title on the board renames the
  file.
- **Folders by depth:** level 1 is a subject, level 2 a course, level 3 a
  chapter. "New course" creates `Subject/Course/` with numbered chapter
  folders (`01 …`, `02 …`) so they sort in teaching order.
- **Shared with me** holds files that came from outside: a `.rpk` opened from
  WhatsApp, e-mail or Drive, a file dropped on the board, or a lesson opened in
  the student viewer (`/board/?view`). A newer copy of the same lesson replaces
  the older one instead of piling up duplicates.
- **My copies** holds students' copies ("Save a copy to my library"). A copy
  is independent, like Google Drive's "Make a copy".
- **The trash** is a dot folder, so `rupok mcp` and most tools skip it. Items
  are deleted for good after 30 days, or with "Delete forever".
- Names are cleaned so they work on Windows, macOS, Linux and sync clients: no
  `\ / : * ? " < > |`, no leading dots, no trailing spaces, at most 120
  characters. Bangla names are fine.

## Lesson metadata

The Library keeps what it knows inside each lesson's `manifest.json`, so a
lesson sent on its own still carries it:

```json
{
  "title": "Binary numbers",
  "course": { "subject": "ICT", "course": "Grade 9 ICT", "chapter": "Number systems" },
  "library": {
    "id": "l_8f3a2c1d9e0b4a77",
    "author": "Ms Rahman",
    "copiedFrom": { "id": "l_1b2c…", "title": "Binary numbers", "author": "Mr Karim", "at": 1795400000000 }
  }
}
```

- `course` is written from the folder on every save (an existing manifest key,
  `spec/rupok-format.md#manifestjson`). Lessons in `My copies` and `Shared with
  me` keep the `course` they arrived with.
- `library.id` is a stable lesson id: kept across renames and moves, new for
  every copy or duplicate. `author` is "Your name" from Settings.
  `copiedFrom` links a copy to its source, so Rupok Cloud can later link back.
- `thumbs/0.png` (480 × 270) is the cover thumbnail, drawn from page 1 when the
  lesson is saved.

> **Spec status.** `library` is a new manifest key. Readers already ignore
> unknown manifest keys (`spec/rupok-format.md#versioning-and-forward-compatibility`),
> so this is additive and needs no version bump, but it is not yet in the
> normative spec: adding it needs an RFC (CONTRIBUTING.md). Note also that the
> engine (`Package::update_lesson`) drops unknown manifest keys when it
> rewrites a package (a known conformance gap), so the Library writes the key
> back on every save.

## Saving

- Opening a lesson from the Library loads it on the board. Changes are saved
  back to **that file** every few seconds (the board's autosave), when going
  back to the Library, and when the tab is hidden.
- Writes are crash-safe: the desktop app writes a temporary file, syncs it and
  renames it over the lesson (and refuses bytes that are not a valid lesson);
  browsers write through `createWritable()`, which replaces the file only when
  the write is complete; IndexedDB writes are one transaction.
- The browser also keeps a recovery copy of the open lesson (IndexedDB
  `rupok`, the board's old autosave). If a save never reached the file (tab
  killed, disk full), the newer recovery copy is used the next time the lesson
  is opened.
- "Save as" / "Download .rpk" (Ctrl+S) and "Export PDF" still save a separate
  file anywhere.
- **One tab at a time.** Tabs talk over a `BroadcastChannel`. Opening a lesson
  that is open in another tab asks first; "Open here instead" tells the other
  tab to stop saving (it shows a notice). Renaming, moving or trashing a
  lesson that is open elsewhere is refused politely.

## Agents and sync clients

**AI agents (`rupok mcp`).** Without `--workspace`, `rupok mcp` uses
`$RUPOK_LIBRARY` if set, else `~/Documents/Rupok` if it exists, else the
current directory. So with the desktop app installed, Claude and the app see
the same files with no configuration:

```sh
claude mcp add rupok -- rupok mcp                          # the library folder
claude mcp add rupok -- rupok mcp --workspace ~/Lessons    # or any folder
```

Agents use library paths directly, for example
`create_lesson { "path": "ICT/Grade 9 ICT/01 Number systems/Binary numbers.rpk" }`.
`list_lessons` skips `.trash`.

**Changes from outside show up.** The desktop app watches the folder
(`notify`); Chromium watches a picked folder with `FileSystemObserver`, or
looks every few seconds; tabs tell each other about their changes. The
Library refreshes, and the open lesson:

- reloads by itself when it has no unsaved changes ("Updated with changes made
  outside Rupok");
- otherwise asks: **Load theirs** or **Keep mine**.

**Sync clients.** Point Google Drive for desktop or OneDrive at the library
folder, or move the library into a synced folder (desktop: Settings → Library
→ Change folder; Chrome/Edge: "Use a folder on this computer"). Rupok writes
whole files atomically, so a sync client never uploads half a lesson. If two
devices change the same lesson at once, the sync client keeps both (for
example `Binary numbers (1).rpk`) and the Library shows both.

## Roles

**Settings → Library → I use Rupok as a Teacher / Student** (also asked once,
the first time the Library opens) changes the defaults only:

- Teachers land on Recent; "Make a copy" goes to My lessons.
- Students land on Shared with me, see My copies in the sidebar, read received
  lessons in the Student view, and copies go to My copies. The student viewer
  (`/board/?view`) offers **Save a copy** for the lesson it shows.

The very first visit to `/board/` (empty library, no choice made yet) opens a
fresh board straight away, so trying Rupok from the website stays one click.
The first change makes it "Untitled lesson" in the Library. From then on,
`/board/` opens the Library; a reload keeps an open lesson open.
`/board/?library` always opens the Library, and `/board/?open=<path>` opens a
lesson.

## Rupok Cloud (optional)

Signing in to Rupok Cloud (Library sidebar → **Sign in with Google**, or
Settings → General → Rupok Cloud) adds a **Rupok Cloud** section to the
Library. Nothing changes for people who never sign in.

- **Account:** your name, plan and space used (500 MB on the free plan).
  Settings also has **Save new lessons to** (this device or Rupok Cloud).
- **Cloud lessons** open and save like local ones. Saving pushes only the
  Loro changes (`POST /api/v1/lessons/<id>/push`; new images first as
  entries); if that fails for any reason other than space or sign-in, the
  whole file is uploaded and the server merges it. An open cloud lesson pulls
  other devices' edits every 15 s and when the window gets focus, so two
  devices never make "conflicted copies".
- **Move between local and cloud:** "Upload to Rupok Cloud" on a local
  lesson, "Download a local copy" on a cloud lesson. The cloud has its own
  trash (Trash → Rupok Cloud) and "Shared with me" shows lessons other
  accounts shared with you.
- **Share a lesson:** header ⋯ → **Share lesson…**: a link that can view,
  view and copy, or edit; copy it, send it on WhatsApp, or show its QR code
  on the board. Links can be turned off later. Signed out or offline, Share
  sends the `.rpk` file as before.
- **Students** open `rupok.app/s/<token>` with no account: the lesson opens
  in the Student view (`/board/?view&src=/api/v1/s/<token>/content&share=…`;
  `src` must be a same-origin `/api/v1/` path) with the title, the teacher
  and **Make a copy** (sign-in needed; the copy opens from the student's own
  cloud). A link that was turned off or has expired shows a friendly page.
- **Errors:** out of space (507) or a lesson too big (413) say what to do
  (empty the cloud trash, delete lessons, or keep it on this device). The
  lesson is always kept on the device.
- **Desktop app:** cookies don't work from the app, so it signs in through
  the system browser: a loopback redirect to `127.0.0.1`, PKCE and a
  one-time code traded for a bearer token (`apps/desktop/src/cloud_auth.rs`,
  `apps/board/src/cloud/desktop.ts`).

Code: `apps/board/src/cloud/` (`api.ts` the client, `library.ts` the
provider and sync, `vv.ts` Loro version vectors, `viewer.ts` share links,
`qr.ts` the QR encoder) and `components/CloudAccount.tsx`,
`ShareDialog.tsx`, `SharedLessonBar.tsx`. The service itself is closed
source; the API it implements (`/api/v1`) is public.

**Local development:** run the cloud service on port 8787; the board's Vite
dev server proxies `/api/` and `/s/` to it (`RUPOK_CLOUD_DEV` overrides the
target). On a dev server the sign-in card also offers a dev sign-in.

## For developers

- `LibraryStore` (`src/library/types.ts`) is the storage abstraction: list,
  walk, stat, read, write, create folder, move, rename, duplicate,
  trash/restore, delete forever, watch. `FileLibrary` (`store.ts`) implements
  it once over a small `FsDriver` (`drivers.ts`): `HandleDriver` (OPFS and a
  picked folder), `IdbDriver`, `MemoryDriver`, `DesktopDriver` (`desktop.ts`).
  A cloud provider can implement `LibraryStore` directly, or `FsDriver`.
- `zip.ts` reads summaries (central directory, `manifest.json`,
  `thumbs/0.png`) with range reads, so browsing never inflates documents; the
  metadata is cached in IndexedDB (`rupok-library`, store `meta`), keyed by
  path, size and mtime.
- Tests:

  ```sh
  cargo test -p rupok-desktop                  # library.rs: sandbox, atomic writes, rename/copy/remove
  cargo check -p rupok-desktop --features app  # the Tauri commands (needs WebKitGTK on Linux)
  ```

  `src/library/store.spec.ts` runs the provider specs (memory, IndexedDB and
  OPFS) in a real browser: open the dev server and run
  `(await import("/src/library/store.spec.ts")).runStoreSpecs()`.
