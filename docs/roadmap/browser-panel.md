# Browser panel with clip-to-board

`Status: 📋 planned · Phase 2 · Difficulty: intermediate`

A web browser docked beside the board in the desktop app, with one-tap clipping of a region, an image or a link onto the current page.

## Goal

Teachers constantly pull material from the web during class: a diagram from
Wikipedia, a map, a news photo. Today that means alt-tab, screenshot tool,
paste. The browser panel puts a real browser next to the board and a "Clip"
button that drops the selected region onto the page as an image item, with its
source URL remembered.

## User story

- *As a teacher*, I open the browser panel, search for "ENIAC", drag a box
  around the photo and tap **Clip**. The photo appears on my slide and I start
  annotating it immediately.
- *As a teacher*, I clip a link to a video as a card on the slide, so I can open
  it again next year.
- *As a teacher*, I never worry that a website can see or change my lesson: the
  panel cannot talk to the app.

## Scope

- Desktop only (Tauri 2, `apps/desktop`): a docked, resizable panel with a URL
  bar, back/forward, reload, and a "Clip" mode.
- Clip types: **region** (drag a rectangle over the page), **whole visible
  page**, **link card** (title + URL as a text item).
- Clipped images become ordinary image items through the existing
  `Engine::add_image(bytes, ext, w, h, now_ms)` path (content-addressed asset).
- Source attribution: the clip remembers its URL and page title.

## Non-goals

- A browser in the web build (`apps/board` in a normal browser): embedding
  arbitrary sites in an `<iframe>` is blocked by most sites
  (`X-Frame-Options`, `frame-ancestors`). The web build keeps paste and drag
  and drop.
- Tabs, bookmarks sync, extensions, password manager.
- Turning a live web page into a Smart Object (network access and licensing
  problems; see `spec/smart-object.md#sandbox`).

## Design

### Desktop shell (`apps/desktop`, new code)

- The panel is a **separate child webview** created with Tauri 2's
  multi-webview API (`Window::add_child` / `WebviewBuilder`). It has **no IPC
  capability**: the Tauri capability file grants commands only to the main
  board webview's label. The browser webview cannot call any app command.
- New Rust module `apps/desktop/src-tauri/src/browser.rs`:

```rust
#[tauri::command] fn browser_open(url: String) -> Result<(), String>;
#[tauri::command] fn browser_nav(action: String) -> Result<(), String>;      // "back" | "forward" | "reload"
#[tauri::command] fn browser_set_bounds(x: f64, y: f64, w: f64, h: f64);
#[tauri::command] async fn browser_capture(rect: Option<[f64; 4]>) -> Result<Clip, String>;

#[derive(Serialize)] struct Clip { png: Vec<u8>, w: u32, h: u32, url: String, title: String }
```

- `browser_capture` uses the platform webview's snapshot API, wrapped in a
  small internal crate or module per OS:
  - Windows WebView2: `ICoreWebView2::CapturePreview` (PNG).
  - Linux WebKitGTK: `webkit_web_view_get_snapshot`.
  - macOS WKWebView: `takeSnapshot(with:)` (macOS is not a release target yet;
    keep the code path compiling).
  - Fallback: OS screen capture of the panel's screen rectangle (for example
    the `xcap` crate), used when a snapshot API fails.
- Region clips crop the snapshot in Rust (`image` crate) before returning.
  Device pixel ratio is applied so a 400×300 CSS region on a 2× screen gives an
  800×600 PNG, then downscaled to at most 2048 px on the long side.

### Board side (`apps/board`)

- A `BrowserPanel` component that owns the panel's screen rectangle and calls
  `browser_set_bounds` on layout changes; a toolbar; a clip overlay that
  captures the drag rectangle in panel coordinates.
- On clip: `board.add_image(png, "png", w, h)` (exists in
  `crates/rupok-wasm/src/lib.rs`), then place the item at the page centre (what
  `Engine::centered_geom` already does) and select it.
- Link card: `board.add_text(x, y, "<title>\n<url>", 28)`.

### Source attribution (proposed; requires an RFC)

Add optional keys to image items: `source_url: String`, `source_title: String`.
This is additive (`spec/rupok-format.md`, section "Items", `kind: "image"`) and
needs no version bump, but **requires an RFC** because it changes the spec.
Until then, the UI keeps attribution in a text item placed under the image.

Engine change after the RFC: `ImageData { geom, asset, source: Option<ImageSource> }`
in `crates/rupok-core/src/model.rs`, written/read in `write_item_data`/`read_item`.

### Security

- The panel never receives cookies or storage from the board webview (separate
  webview, separate data directory: `WebviewBuilder::data_directory`).
- Downloads are disabled; `window.open` opens inside the panel, never a new
  app window.
- Clipped PNGs are re-encoded by the `image` crate, which strips metadata and
  rejects malformed data before it enters the package.

## Implementation tasks

1. Tauri capability split: main webview gets app commands, a `browser` label gets none; test that a page in the panel cannot invoke a command. (intermediate)
2. `browser.rs`: open, navigate, bounds, URL/title change events to the board. (intermediate)
3. Board `BrowserPanel` component with URL bar and back/forward/reload, docked left or right, resizable. (good-first-issue)
4. Windows snapshot via WebView2 `CapturePreview`. (advanced)
5. Linux snapshot via WebKitGTK `webkit_web_view_get_snapshot`. (advanced)
6. Screen-capture fallback and DPR-correct cropping/downscaling. (intermediate)
7. Clip overlay (drag rectangle, Esc to cancel, touch-friendly handles) and insertion via `add_image`. (intermediate)
8. Link-card clip as a text item. (good-first-issue)
9. RFC + core change for `source_url`/`source_title` on image items; show the source in the item's context menu. (intermediate)
10. Docs: user guide section and a short licensing note for teachers about reusing web images. (good-first-issue)

## Acceptance criteria and tests

- Clicking **Clip** over a 400×300 CSS region inserts an image whose pixel size
  matches the region × DPR (capped at 2048 px) on Windows and Linux.
- Undo removes the clip in one step (it is one `add_item` commit).
- The clipped image survives save/load (`Package::write`/`read`) and is stored
  once even if clipped twice (content addressing).
- A test page in the panel calling `window.__TAURI__` or any IPC fails.
- Opening the panel does not change pen latency: `rupok-bench` wet frame p99
  stays ≤ 4 ms, and a manual check with the panel open shows no extra dropped
  frames while writing.

## Performance budget

- Region clip, tap to item on page: ≤ 500 ms for a 1080p panel.
- Panel idle cost: no animation frames requested by the board while the
  teacher only browses.

## Open questions

- Is a child webview stable enough on Linux (WebKitGTK) inside one window, or
  should the panel be a separate, docked window?
- Should clips keep the full-resolution original as a second asset for later
  zoom-in, or only the downscaled image?
- Do we want "clip text" (selected text to a text item)? It needs a script in
  the panel, which conflicts with "no bridge to the panel".

## References

- `spec/rupok-format.md` — sections "Items" (`kind: "image"`), "Assets", "Change process"
- `crates/rupok-wasm/src/engine.rs` — `Engine::add_image`, `Engine::centered_geom`
- [tauri-desktop](./tauri-desktop.md), [desktop-overlay](./desktop-overlay.md), [standalone-html-export](./standalone-html-export.md)
