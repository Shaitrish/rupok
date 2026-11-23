# Desktop overlay annotation

`Status: 🚧 in progress · Phase 1 · Difficulty: advanced`

> **Status (2026-11):** a first version ships in `apps/desktop`
> (`src/overlay.rs`, `src/capture.rs`) and `apps/board/src/overlay/`. Done:
> tasks 1–4 (transparent always-on-top ink window loading `/board/?overlay`,
> pinned camera, draw / click-through modes, a separate toolbar window owned by
> the ink window), the Ctrl+Shift+D toggle and Esc (part of 6), the screenshot
> (part of 7) and Save to lesson (9). Differences from this plan: no
> `overlay.html` entry, the board renders the overlay for `?overlay`; Save to
> lesson composites screenshot + ink into one image in the webview and adds it
> as a new page (no `Engine::add_page_from` yet, so strokes are not copied as
> vectors); Linux capture uses a pure-Rust X11 `GetImage` (no PipeWire), and
> on Wayland the app runs under XWayland and saves the ink only. Open: 5, 6
> (editor, more shortcuts), 7 (freeze), 8, 10. See `apps/desktop/README.md`.

Draw with the Rupok pen on top of anything on screen (PowerPoint, a browser, a video, a simulator), then click through to the app underneath or save the annotated screen as a slide.

## Goal

Note3-style "annotate the desktop": a transparent, always-on-top layer over
the whole screen with the same low-latency ink as the board, a small floating
toolbar, a pass-through mode, and "save to lesson".

## User story

- *As a teacher*, I am showing a YouTube video; I press the pen button on my
  board, circle something on the paused frame, and keep teaching.
- *As a teacher*, I switch to "mouse" on the floating toolbar and my clicks go
  to PowerPoint underneath while my ink stays visible.
- *As a teacher*, I tap "Save to lesson" and get a new slide with a screenshot
  of the screen and my ink on top, so I can replay it later.
- *As a teacher* with two screens, I choose which screen to draw on.

## Scope

- Overlay windows (one per monitor) in the Tauri app: transparent, borderless,
  always on top, covering the monitor.
- Modes: **draw** (overlay takes input), **pass-through** (input goes to apps
  below, ink stays visible), **hidden**.
- Floating toolbar window: pen, highlighter, eraser, colours, clear, undo,
  mode toggle, save, close.
- Global shortcuts: toggle overlay, toggle draw/pass-through, clear.
- "Freeze": take a screenshot and draw on the frozen image (useful over video).
- "Save to lesson": screenshot + ink become a new page in the open lesson.
- Windows (WebView2) and Linux X11; Linux Wayland best effort.

## Non-goals

- Recording the screen as video (see [replay-audio](./replay-audio.md) for
  recording; MP4 export is Phase 3).
- Android overlay (see [android](./android.md)).
- Interacting with the content underneath (e.g. auto-detect PowerPoint slides).

## Design

### Windows and input

```text
apps/desktop/src-tauri/src/overlay.rs      (new)  window management, modes, capture
apps/board/src/overlay/                     (new)  overlay entry: canvas + toolbar UI
```

- Each overlay window loads `overlay.html` (a second Vite entry in
  `apps/board`) with a full-window canvas and a `Board` created by
  `Board::create`. The board already picks
  `CompositeAlphaMode::PreMultiplied` when available
  (`crates/rupok-wasm/src/lib.rs`) and the ink renderer clears to transparent
  (`bench/src/main.rs` asserts transparent pixels outside strokes), so ink
  composites over the desktop.
- The overlay page is an **infinite board page** (no `frame_w/frame_h`) with
  `bg_color = 0x00000000` and the CSS background transparent.
- Window flags (Tauri `WebviewWindowBuilder`): `transparent(true)`,
  `decorations(false)`, `always_on_top(true)`, `skip_taskbar(true)`,
  position/size = monitor bounds, `shadow(false)`.
- **Pass-through**: `window.set_ignore_cursor_events(true)`. The toolbar is a
  separate small window that is never click-through.
- Camera: fixed at `Camera { x: 0, y: 0, zoom: 1 }` so world units equal CSS
  pixels of the monitor; pan/zoom gestures are disabled in overlay mode
  (`Board::set_touch_mode("draw")`, hand tool hidden).

### Platform notes

| Platform | Transparency | Click-through | Always on top |
|---|---|---|---|
| Windows 10/11 | WebView2 transparent background | `WS_EX_TRANSPARENT` via Tauri | yes |
| Linux X11 | needs a compositor (most desktops) | input shape via Tauri/GTK | yes |
| Linux Wayland | yes | input region | compositor-dependent; GNOME does not honour app requests. Document the limitation; investigate `wlr-layer-shell` on wlroots compositors |

Full-screen exclusive apps (some games, some video players) may cover the
overlay; PowerPoint slideshow and browsers in full screen work on Windows.

### Screenshots

- Capture with the `xcap` crate (Windows, X11; Wayland via the desktop portal),
  hiding the overlay windows for one frame first.
- Freeze: show the capture as a CSS background image under the canvas; ink
  stays in the same coordinate space.

### Save to lesson

```rust
#[tauri::command]
async fn overlay_save(monitor: u32) -> Result<(), String>;
```

1. Capture the monitor (without the overlay), encode WebP.
2. Ask the overlay webview for its strokes: new `Board::export_page_updates()`
   or simply `Board::save()` of the overlay lesson (small).
3. In the main window's board, call a new
   `Engine::add_page_from(lesson_bytes, page_index, background: Option<(bytes, ext)>, now_ms)`
   that copies stroke items (same points, same `created`, same timing) into a
   new slide page with `frame = monitor size`, adding the screenshot as the
   lowest image item (same model as [pdf-import-native](./pdf-import-native.md)).

Copying preserves each stroke's `created` and per-point `dt`, so the saved
page replays exactly as it was drawn. No format change is needed.

### Shortcuts

`tauri-plugin-global-shortcut`, user-configurable in Settings:

| Default | Action |
|---|---|
| `Ctrl+Shift+D` | toggle overlay |
| `Ctrl+Shift+M` | toggle draw / pass-through |
| `Ctrl+Shift+X` | clear overlay ink |
| `Esc` (overlay focused) | pass-through |

Interactive panels often expose pen buttons as keys; the shortcut editor
records any key combo.

## Implementation tasks

1. **Spike**: transparent always-on-top Tauri window with a `Board` drawing
   over the desktop on Windows and X11; document findings in this file.
   *(advanced)*
2. **Overlay Vite entry** (`overlay.html`) that creates a `Board` with a
   transparent infinite page and fixed camera. *(intermediate)*
3. **Mode switching** (draw / pass-through / hidden) with
   `set_ignore_cursor_events`. *(intermediate)*
4. **Floating toolbar window** (drag to move, remembers position, big touch
   targets ≥ 48 px). *(intermediate)*
5. **Multi-monitor**: one overlay per monitor or a monitor picker; handle
   monitors being added/removed and DPI changes. *(advanced)*
6. **Global shortcuts** + Settings editor. *(good-first-issue, after 3)*
7. **Screenshot + freeze** with `xcap`. *(intermediate)*
8. **`Engine::add_page_from`** copying strokes with timing; unit test with
   `MockSink`. *(intermediate)*
9. **Save to lesson** command wiring both windows. *(intermediate)*
10. **Wayland investigation** and documented limitations. *(advanced)*

## Acceptance criteria and tests

- Drawing on the overlay over a playing video shows ink with no visible extra
  latency compared to the board (manual check plus the bench budget).
- In pass-through mode, clicks reach the app below and the ink stays visible.
- "Save to lesson" adds one page whose items are: one image (the screenshot)
  at the bottom and the strokes with unchanged `created`/`dt`; the page replays
  stroke by stroke (`replay_start` → duration > 0).
- Unit test: `add_page_from` with 3 strokes produces 3 stroke items with equal
  point arrays and `created` values.
- Overlay windows never appear in the taskbar or alt-tab list (Windows).

## Performance budget

| Metric | Budget |
|---|---|
| Wet ink frame on a 4K monitor | p99 ≤ 6 ms (1080p: ≤ 4 ms, as the board) |
| Idle CPU with overlay visible and no input | ≤ 1 % (no frames when nothing changes; `Board::frame` returns false) |
| Toggle draw ↔ pass-through | ≤ 50 ms |

## Open questions

- Wayland: is a layer-shell helper process worth it, or do we document "use
  X11 session for overlay"?
- Should overlay ink persist per monitor between sessions?
- Laser pointer in pass-through mode (draw without capturing input) — possible
  with raw input hooks, but intrusive. Worth it?

## References

- `crates/rupok-wasm/src/lib.rs` — `Board::create` (alpha mode), `Board::frame`
- `crates/rupok-wasm/src/engine.rs` — `TouchMode`, `Engine::add_image`
- `spec/rupok-format.md` — `#pages` (infinite boards), `#replay`
- Related: [tauri-desktop](./tauri-desktop.md), [pdf-import-native](./pdf-import-native.md),
  [android](./android.md)
