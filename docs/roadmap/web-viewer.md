# Web viewer (PWA)

`Status: 📋 planned · Phase 2 · Difficulty: intermediate`

A static, installable web app that opens any `.rpk` file in the browser: flip through pages, run Smart Objects, play the class replay and present, offline, with nothing to install.

## Goal

Lessons must be viewable by anyone: a student at home on a phone, a parent, a
colleague on a Chromebook, a teacher on a borrowed PC. Build a read-only
viewer from the same `apps/board` + `rupok-wasm` code, host it as static files
(rupok.app on Cloudflare), and make it work offline as a PWA.

## User story

- *As a teacher*, I share last week's lesson file in the class group; students
  open it on their phones in the browser and replay how I solved the problem.
- *As a teacher*, I open a lesson on the school's Chromebook in present mode
  without installing anything.
- *As a teacher*, I install the viewer on my phone; it opens `.rpk` files from
  the Files app even with no signal.
- *As a teacher*, a student can try the quiz object in my lesson at home, but
  cannot change my lesson.

## Scope

- Viewer build of `apps/board` (`VITE_MODE=viewer`): no editing tools; page
  list, zoom/pan, present mode, replay controls, Smart Objects.
- Open from: file picker, drag-and-drop, File Handling API (`.rpk`
  association for installed PWA), Web Share Target, `?src=<url>` for public
  links (CORS permitting).
- Offline via a service worker (app shell + vendored `rupoklib` libraries).
- Smart Object host in a pure browser (sandbox, SDK injection, `rupoklib://`).
- Mobile-friendly layout and touch navigation.

## Non-goals

- Editing and saving (that is the full board; "Open in Rupok" link instead).
- Accounts, hosting user files, analytics.
- Student annotation (see [student-layers](./student-layers.md); the viewer
  becomes its client later).

## Design

### Build

- Same SolidJS app, a viewer flag removes editing UI and never calls mutating
  `Board` methods except `set_object_step` / `set_object_state` on a
  **throwaway in-memory copy** (so steps and quiz state work, but nothing is
  saved back unless the user exports).
- `rupok-wasm` is unchanged; `Board::load(bytes)`, `goto_page`, `fit_page`,
  `replay_*`, `dom_items_json`, `object_html`, `asset` cover the viewer.
- Output: `apps/board/dist-viewer/`, deployed to Cloudflare by `.github/workflows/deploy.yml`.

### PWA

`manifest.webmanifest` (new):

```json
{
  "name": "rupok Viewer", "short_name": "rupok", "display": "standalone",
  "start_url": "./", "icons": [ … ],
  "file_handlers": [{ "action": "./", "accept": { "application/vnd.rupok.lesson+zip": [".rpk"] } }],
  "share_target": { "action": "./share", "method": "POST", "enctype": "multipart/form-data",
                    "params": { "files": [{ "name": "lesson", "accept": [".rpk"] }] } }
}
```

- Service worker precaches the app shell, wasm, fonts and `rupoklib` files
  (versioned cache name per release). `launchQueue` handles File Handling.
- The wasm and libraries are large; precache in the background after first
  load and show "Ready offline" when done.

### Smart Objects in a plain browser (spike needed)

The desktop serves bundles through a custom scheme
([tauri-desktop](./tauri-desktop.md)); a static site cannot. Options to
evaluate in task 1:

| Option | How | Concern |
|---|---|---|
| A. `srcdoc` | Inline the entry HTML with the SDK and CSP `<meta>` injected; turn other bundle files into `data:` URLs referenced by a rewritten import map | Large bundles inflate memory; CSS `url()` rewriting |
| B. Service worker | SW serves `/__obj/<doc>/<bundle>/<path>` from memory | Requests from an opaque-origin (sandboxed) frame may not be controlled by the SW; needs testing per browser |
| C. Separate origin | Serve objects from `objects.<viewer-domain>` with a tiny SW there, fed by `postMessage` | Two deployments, but strongest isolation |

Whatever wins must satisfy `spec/smart-object.md#sandbox` (exactly
`sandbox="allow-scripts"`, network blocked by CSP unless
`permissions.network` allows hosts). `rupoklib://` is a *relative* URL to the
browser (schemes must start with a letter), so the viewer rewrites
`rupoklib://<name>/<file>` references and injects an import map
`{"imports": {"rupoklib://": "<viewer>/libs/"}}`, as the desktop host does.

### Opening remote files

`?src=https://…/lesson.rpk` fetches with CORS; failures explain how to share
(e.g. "Download, then open"). Google Drive share links need a Drive-aware
fetch (later, with [google-drive-sync](./google-drive-sync.md)).

All reads go through `Package::read` inside wasm, so reader limits and path
rules apply (`spec/rupok-format.md#reader-limits-and-safety`).

## Implementation tasks

1. **Spike: Smart Object hosting in a static PWA** (options A–C in Chromium,
   Firefox, Safari); write results into this plan. *(advanced)*
2. **Viewer build flag** in `apps/board` hiding editing UI; throwaway copy for
   object steps/state. *(intermediate)*
3. **Open flows**: picker, drag-and-drop, `?src=` with CORS errors explained.
   *(good-first-issue)*
4. **PWA manifest + service worker** with versioned precache. *(intermediate)*
5. **File Handling** (`launchQueue`) and **Share Target**. *(intermediate)*
6. **Object host** implementation from the spike, incl. SDK injection,
   CSP, `rupoklib://` rewriting. *(advanced)*
7. **Mobile layout**: bottom page strip, swipe between pages, replay bar.
   *(intermediate)*
8. **Present mode** in the viewer (full screen, clicker keys, object steps).
   *(good-first-issue)*
9. **Cloudflare deploy** of `dist-viewer` (extend `.github/workflows/deploy.yml`).
   *(good-first-issue)*
10. **"Export my answers"**: download the lesson with the viewer's object state
    as a new `.rpk`. *(intermediate)*

## Acceptance criteria and tests

- The three `examples/objects/` objects run in the viewer with steps, state
  and theme; a test object that fetches a remote URL is blocked.
- After one online visit, the viewer opens a local `.rpk` in airplane mode.
- Installed PWA (Chromium) opens `.rpk` files from the OS file manager.
- A malformed package (unsafe path, oversized entry) shows an error and does not
  crash the page.
- Lighthouse PWA checks pass; viewer works on a 2 GB RAM Android phone with a
  20-page lesson (manual test).
- No mutating `Board` call reaches the loaded bytes: exporting without changes
  produces a byte-identical package (unit test on the viewer store).

## Performance budget

| Metric | Budget |
|---|---|
| First load on 4G (shell + wasm, compressed) | ≤ 3 MB, interactive ≤ 4 s |
| Open a 20 MB lesson on a mid-range phone | ≤ 2 s |
| Replay playback frame time on a mid-range phone | p99 ≤ 16 ms |

## Open questions

- Domain and hosting: rupok.app (Cloudflare); the viewer can live at a path or subdomain
  for QR codes?
- Should the viewer allow light editing (pen only, no save) for students?
  That overlaps with [student-layers](./student-layers.md).
- iOS Safari: no File Handling API; rely on Share Target alternatives?

## References

- `spec/rupok-format.md` — `#reader-limits-and-safety`, `#replay`
- `spec/smart-object.md` — `#sandbox`, `#host-duties`, `#offline-libraries-rupoklib`
- `crates/rupok-wasm/src/lib.rs` — `Board::load`, `Board::replay_start`, `Board::object_html`
- Related: [tauri-desktop](./tauri-desktop.md), [standalone-html-export](./standalone-html-export.md),
  [student-layers](./student-layers.md)
