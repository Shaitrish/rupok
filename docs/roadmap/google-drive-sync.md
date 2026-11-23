# Google Drive sync

`Status: ⛔ not planned (superseded by Rupok Cloud) · kept for reference`

> **Decision (Nov 2026):** Rupok will not build its own two-way Google Drive
> sync engine. Sync and sharing are provided by
> [Rupok Cloud](./rupok-cloud.md). Lessons remain plain `.rpk` files in a
> local folder, so any cloud-drive desktop client (Google Drive, OneDrive,
> Dropbox, Syncthing) can back them up with no Rupok code. A Drive
> "Open with → Rupok" integration is still planned, in the Rupok Cloud plan.
> The design below is kept for reference; because Rupok is open source, the
> community may still build a Drive sync add-on.

Keep lessons in sync between the teacher's laptop, the classroom board and home PC through the teacher's own Google Drive, offline-first and conflict-free, using Apache OpenDAL and per-device Loro update logs.

## Goal

No Rupok servers and no accounts: the teacher's Drive is the sync medium.
Edits made offline on two devices merge automatically (Loro CRDT), sync is
incremental (small update files, not whole `.rpk` uploads) and the app only
asks for the narrow `drive.file` scope.

## User story

- *As a teacher*, I prepare a lesson at home, walk into class, and it is
  already on the board PC.
- *As a teacher*, I wrote on the board during class with no internet; at home
  my laptop shows that ink, and the slide I edited at home in the meantime is
  also there. Nothing was overwritten.
- *As a teacher*, Rupok only sees files it created in my Drive, not my
  personal documents.
- *As a teacher*, I can still copy a normal `.rpk` file to a USB stick.

## Scope

- OAuth (installed-app flow, PKCE) with scope
  `https://www.googleapis.com/auth/drive.file`.
- A remote layout of append-only, single-writer files per device.
- Push (local changes → update files), pull (others' updates → import),
  periodic compaction.
- Assets and Smart Object bundles synced as content-addressed blobs.
- Library structure (courses/chapters) mirrored.
- Desktop app first; status UI (synced / syncing / offline / error).

## Non-goals

- Real-time co-editing between people (see [live-broadcast](./live-broadcast.md)).
- Sharing lessons with other teachers through Drive permissions (later).
- A rupok-hosted relay.

## Design

### Lesson identity (requires an RFC)

`doc.loro` `meta` has no stable id today (`{format, title, created}`). Sync
needs one that survives renames and moves, and that "Duplicate" changes.
Propose an optional `meta.id: String` (UUID v4, set at creation and on
duplicate) in `spec/rupok-format.md#meta` — additive, but a spec change, so it
**requires an RFC**. Lessons without an id get one on first sync (one commit).

### Remote layout (new spec `spec/sync.md`, requires an RFC)

```text
rupok/                                  app-created root folder in My Drive
  devices/<device-id>.json               {name, last_seen_ms}
  lessons/<lesson-id>/
    lesson.json                          {schema:1, title, course?, modified_ms}
    log/<device-id>/<seq:010>.loro       Loro update blob (export_updates), append-only
    snap/<device-id>-<seq:010>.loro      full snapshot written by compaction
    blobs/<blake3>.<ext>                 assets and bundle files
    bundles/<bundle-id>.json             bundle manifest + map path → blob hash
  library/                               course.json / chapter.json mirrors
```

Rules that make it conflict-free on an eventually consistent store:

- **Single writer per path.** A device only creates files under its own
  `log/<device-id>/` and `snap/<device-id>-*`; nobody modifies a file after
  upload.
- **Idempotent import.** Importing an update twice is harmless in Loro, so a
  listing that is stale or repeated causes no damage.
- **Device id** is random per installation, stored in app data. It is not the
  Loro peer id (`Lesson::peer_id`), which changes per session.

### Crate

```text
crates/rupok-sync/                 (new, AGPL-3.0-or-later)
  src/plan.rs      sans-IO planner: (local state, remote listing) → actions
  src/exec.rs      runs actions on an opendal::Operator
  src/state.rs     per-lesson cursor: pushed VV, imported files per device
  src/auth.rs      OAuth PKCE loopback, token refresh, keyring storage (desktop)
```

```rust
pub enum Action { Upload { path: String, bytes: Vec<u8> }, Download { path: String }, Import { lesson: LessonId, bytes: Vec<u8> }, Delete { path: String } }
pub fn plan_push(state: &SyncState, lesson: &rupok_core::Lesson, pkg: &rupok_format::Package, now_ms: f64) -> Vec<Action>;
pub fn plan_pull(state: &SyncState, listing: &[RemoteEntry]) -> Vec<Action>;
```

- The planner is pure (no IO, no clock reads; `now_ms` is a parameter) and
  fully unit-testable with an in-memory listing. `exec.rs` uses OpenDAL's
  `gdrive` service; tests use OpenDAL's `memory` or `fs` service.
- **Push**: `bytes = lesson.export_updates(&state.pushed_vv)`; if non-empty,
  upload `log/<device>/<seq+1>.loro`, then advance `pushed_vv`
  (`Lesson::version`). Upload new blobs/bundle descriptors first.
- **Pull**: list `log/*/` and `snap/`; download files not yet imported
  (newest snapshot first if the local doc is far behind); `Lesson::import`
  each; fetch missing blobs; then write the local `.rpk` atomically.
- **Compaction**: when a lesson has more than 300 log files or 20 MB of logs,
  the device with the most recent pull writes `snap/<device>-<seq>.loro`
  (`Lesson::save`, full history kept). A device deletes **only its own** log
  files, and only those older than 14 days and covered by a snapshot.

### Triggers and UI

- Pull before opening a lesson, on app focus and every 60 s while open; push
  after each save/autosave (debounced 5 s).
- Offline: pushes go to a local outbox and upload later.
- Status chip in the top bar; per-lesson "Last synced" in the library.

### Browser and web viewer

OpenDAL `gdrive` on `wasm32` is unverified. Phase 2 ships desktop sync; the
[web viewer](./web-viewer.md) reads `.rpk` files. Browser sync is an open
question.

### Other backends (Phase 3)

OpenDAL makes extra backends cheap because the layout and planner do not
change, only the `Operator`:

| Backend | OpenDAL service | Notes |
|---|---|---|
| OneDrive | `onedrive` | Microsoft Graph, app folder; common in schools on Microsoft 365 |
| Dropbox | `dropbox` | App-folder permission |
| WebDAV | `webdav` | Nextcloud, ownCloud, school NAS; basic auth or app password |
| S3-compatible | `s3` | Self-hosted MinIO for school IT departments |

Each needs: auth flow, a settings form, and running the shared conformance
test suite against the service.

## Implementation tasks

1. **RFC: `meta.id`** in `spec/rupok-format.md#meta`. *(intermediate)*
2. **RFC: `spec/sync.md`** (layout, single-writer rules, compaction).
   *(intermediate)*
3. **`rupok-sync` planner** (`plan_push`, `plan_pull`) with unit tests on an
   in-memory listing. *(advanced)*
4. **Executor on OpenDAL** with the `fs` service; end-to-end test: two
   replicas, offline edits, sync through a temp dir, identical item lists.
   *(advanced)*
5. **Blob and bundle sync** (content-addressed, dedupe). *(intermediate)*
6. **OAuth PKCE loopback + keyring** for Google (desktop). *(intermediate)*
7. **gdrive executor** + manual test checklist. *(intermediate)*
8. **Compaction** with own-log deletion rules. *(advanced)*
9. **Desktop integration**: triggers, outbox, status chip, settings page
   (connect/disconnect account). *(intermediate)*
10. **Library mirror** (`library/`). *(intermediate)*
11. **Phase 3 backends**: OneDrive, Dropbox, WebDAV — one PR each.
    *(intermediate each)*

## Acceptance criteria and tests

- `cargo test -p rupok-sync`: two `Lesson` replicas edit offline (strokes on the
  same page, a page added on each), sync through the `fs` service in both
  orders; final `items()` and `pages()` are equal on both, with nothing lost.
- Repeating a pull is a no-op (no new commits, no new files).
- A device never uploads to another device's `log/` directory (planner
  property test).
- Compaction: after compaction and log deletion, a fresh device that pulls
  ends with the same document as the others.
- Revoking the Google token shows "Reconnect Drive" and keeps local files
  untouched.
- Only the `drive.file` scope is requested (checked in the auth URL test).

## Performance budget

| Metric | Budget |
|---|---|
| Push after one stroke | 1 upload, ≤ 2 KB |
| Pull of a 2-hour class from another device | ≤ 10 s on a 10 Mbit/s link |
| Background sync CPU | ≤ 2 % average; never on the pen path |

## Open questions

- Snapshot strategy for very long histories: shallow snapshots would break
  replay, so full history is kept; is 14 days the right log retention?
- Google's desktop OAuth client "secret" is not secret; is PKCE-only possible
  for our client type?
- Browser sync: Google Identity Services token client + OpenDAL on wasm, or a
  small TypeScript implementation of the same planner rules?
- How to present a lesson deleted on one device but edited on another?

## References

- `crates/rupok-core/src/doc.rs` — `Lesson::export_updates`, `Lesson::import`, `Lesson::version`, `Lesson::save`
- `spec/rupok-format.md` — `#meta`, `#assets`, `#docloro`
- Apache OpenDAL: <https://opendal.apache.org>
- Related: [course-library](./course-library.md), [lan-send](./lan-send.md),
  [student-layers](./student-layers.md), [web-viewer](./web-viewer.md)
