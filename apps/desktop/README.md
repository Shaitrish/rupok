# Rupok desktop (Tauri 2)

The installable Rupok app for **Windows and Linux**. It wraps the board from
[`apps/board`](../board) (SolidJS + the `rupok-wasm` engine, unchanged) in a
Tauri 2 shell and adds what a browser cannot do:

- native Open/Save dialogs and **crash-safe saving** (validate → temp file → fsync → rename);
- **`.rpk` file association**, open from the command line, and a **single instance**
  (double-clicking a second lesson opens it in the running window);
- remembered window size and position;
- **draw over any app**: a transparent, always-on-top ink layer over the whole
  screen with a floating toolbar, click-through, and "Save to lesson".

Plans: [tauri-desktop](../../docs/roadmap/tauri-desktop.md) ·
[desktop-overlay](../../docs/roadmap/desktop-overlay.md).

## Layout

| Path | What |
|---|---|
| `tauri.conf.json` | Windows, CSP, bundle targets, file association. `frontendDist` is `../board/dist`; `devUrl` is `http://127.0.0.1:3737/board/` |
| `capabilities/` | Least-privilege permissions: `main.json` (board window), `overlay.json` (overlay windows) |
| `src/app.rs` | Builder, plugins, `save_lesson`, files opened by the OS |
| `src/overlay.rs` | Overlay windows, click-through, global shortcut, capture, snapshot hand-off |
| `src/files.rs` | Atomic save and argv parsing (plain Rust, unit-tested) |
| `src/capture.rs` | Screen capture: `xcap` on Windows, pure-Rust X11 on Linux |
| `src/geometry.rs` | Toolbar placement and the screenshot wire format (unit-tested) |
| `linux/` | `.desktop` template (`Exec=… %F`) and the shared-mime-info entry for `.rpk` |
| `icons/` | Generated app icons (see below) |

On the board side: [`src/platform.ts`](../board/src/platform.ts) is the file
bridge (Tauri dialogs and fs in the app; File System Access API or
download/`<input type=file>` in a browser) and [`src/overlay/`](../board/src/overlay)
is the overlay UI.

## Prerequisites

- Rust (pinned by `rust-toolchain.toml`), bun, and the wasm-bindgen CLI
  (`cargo install wasm-bindgen-cli --version 0.2.130 --locked`), as for the board.
- **Linux**: the WebKitGTK development packages:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev
  # for bundling an AppImage as well:
  sudo apt install patchelf libfuse2
  ```

- **Windows**: WebView2 (preinstalled on Windows 10/11) and the MSVC build tools.

## Develop

```sh
cd apps/desktop
bun install
bun run dev        # = tauri dev: builds the wasm engine, starts the board dev
                   #   server on :3737 (scripts/dev.sh), opens the app window
```

The Tauri code is behind the crate's `app` feature, which `tauri.conf.json`
(`build.features`) turns on for `tauri dev`/`tauri build`. Plain
`cargo test --workspace` and `cargo clippy --workspace` therefore build this
crate **without** Tauri and need no WebKitGTK; they run the unit tests of
`files.rs` and `geometry.rs`. To lint the real app:

```sh
cargo clippy -p rupok-desktop --features app -- -D warnings      # needs WebKitGTK on Linux
cargo test   -p rupok-desktop --features capture                 # capture code, no WebKitGTK needed
```

## Build installers

```sh
cd apps/desktop
bun run build      # builds the wasm engine and the board, then `tauri build`
```

Output in `target/release/bundle/` at the repo root: `nsis/*.exe` and
`msi/*.msi` on Windows, `deb/*.deb` and `appimage/*.AppImage` on Linux. If the
board is already built, `bun run bundle` runs only `tauri build`.

CI ([`.github/workflows/desktop.yml`](../../.github/workflows/desktop.yml))
builds both platforms on every push to `main` (installers as workflow
artifacts) and attaches them to a **draft GitHub Release** for `v*` tags.

## Icons

The icons are generated from the brand artwork with one command, so a new logo
is one re-run away:

```sh
cd apps/desktop && bun run icons
# = bunx tauri icon ../../brand/rupok-app-icon.svg && rm -rf icons/android icons/ios
```

Any square SVG or PNG works as input (for example `apps/board/public/icon-512.png`,
or `brand/rupok-mark.svg` for rounded corners on the taskbar).

## The overlay: draw over any app

- **Ctrl+Shift+D** (global) shows or hides the overlay on the monitor under the
  mouse. `platform.openOverlay()` / `closeOverlay()` do the same from the board.
- The **ink window** (`/board/?overlay`) is transparent, undecorated, always on
  top, skipped from the taskbar and covers the monitor. It shows only ink: no
  page frame, no canvas colour. It is a separate small lesson and never touches
  the teacher's lesson or its autosave.
- The **toolbar window** (`/board/?overlay=toolbar`) is a small always-on-top
  window owned by the ink window: pen colours, highlighter, eraser, laser, undo,
  clear, click-through, Save to lesson, exit. Drag it by the grip.
- **Click-through** calls `set_ignore_cursor_events(true)` on the ink window:
  clicks go to the apps underneath and the ink stays visible. The toolbar is a
  different window, so it keeps working. Picking a pen turns drawing back on.
- **Esc** in the overlay (or the × button) hides it. The ink is kept until you
  clear it, so toggling the overlay off and on does not lose anything.
- **Save to lesson** hides the overlay for a moment, captures the monitor,
  composites the ink on top in the webview and adds the image as a new page in
  the main window's lesson.
- In a browser, `/board/?overlay` is a plain page with the toolbar inline (handy
  for development; Save to lesson needs the desktop app).

## Known platform limits

| Platform | Limit |
|---|---|
| Linux, Wayland | Apps cannot place windows, keep them on top or grab global shortcuts. The app therefore runs under **XWayland** on Wayland sessions (it sets `GDK_BACKEND=x11` unless you set it, or `RUPOK_WAYLAND=1`). Even so, Ctrl+Shift+D only fires while a Rupok or other X11 window has focus, and **screen capture is not possible**, so Save to lesson saves **the ink only** (it says so). |
| Linux, X11 | Transparency needs a compositor (all mainstream desktops have one); without it the overlay is black. Some window managers keep undecorated windows out of panel areas. |
| Linux, AppImage | The file association is not registered system-wide (that needs AppImageLauncher or the `.deb`). |
| Windows | Full-screen *exclusive* apps (some games and video players) can cover the overlay; PowerPoint slideshows and full-screen browsers work. Installers are unsigned for now, so SmartScreen warns on first run. |
| Global shortcut | Ctrl+Shift+D is skipped (with a log line) if another app already owns it. A settings editor for shortcuts is still to do. |
| Multi-monitor | One overlay at a time, on the monitor under the mouse when it opens. |
| Smart Objects | Objects run as in the browser (`srcdoc` iframes). Objects that `import` ES modules from `/rupoklib/` are untested in the app: the opaque-origin iframe may need CORS headers that only the planned `rupokobj://` protocol will add. |

## Security notes

- The CSP allows `'wasm-unsafe-eval'` for the engine. It also allows
  `'unsafe-inline'`/`'unsafe-eval'` scripts because Smart Objects are `srcdoc`
  iframes, and a `srcdoc` document **inherits** the board's CSP: without them no
  object could run. The objects stay isolated by `sandbox="allow-scripts"`
  (opaque origin, no Tauri IPC) and their own stricter CSP. For the same reason
  `dangerousDisableAssetCspModification` keeps Tauri from adding nonces to
  `script-src`/`style-src`, which would switch `'unsafe-inline'` off.
- `save_lesson` only writes `.rpk` files the teacher chose in a dialog or
  opened from the OS (the fs scope), and refuses bytes that do not parse as a
  lesson package, so a broken page can never overwrite a good lesson.
- Each window gets only the commands it needs (`build.rs` generates
  `allow-<command>` permissions; see `capabilities/`).

## Not done yet

From the plan: autosave to the app data folder with restore, recent files, the
`rupokobj://` Smart Object protocol with vendored `rupoklib://` libraries, the
MCP sidecar, the updater and code signing; for the overlay: freeze-frame,
per-monitor overlays and a shortcut editor. The board's mode switcher opens the
overlay through `platform.openOverlay()`.
