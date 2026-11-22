# PDF import (native)

`Status: 📋 planned · Phase 1 · Difficulty: advanced`

Turn every page of a PDF (textbook chapter, worksheet, old slides) into a Rupok slide the teacher can write on, from the desktop app, the CLI and the MCP server.

## Goal

Teachers live in PDFs. Importing one must be a single action that produces one
slide per PDF page, with the PDF page as a fixed background and the pen on top,
and it must work offline on a low-end school PC.

The browser board already depends on `pdfjs-dist` (`apps/board/package.json`),
so the in-browser path renders with pdf.js. This plan adds the **native** path
(Rust, used by the Tauri app, `rupok` CLI and MCP server) and defines the
**shared page model** both paths must produce, so a lesson imported in the
browser and one imported by an agent look the same.

## User story

- *As a teacher*, I drop `chapter-4.pdf` onto the board and get 18 slides in a
  few seconds, each showing one page, ready to annotate.
- *As a teacher*, I ask my AI agent "make a lesson from pages 3–7 of this PDF
  and add a quiz at the end"; the agent calls an MCP tool and the slides appear.
- *As a teacher*, I zoom into a small diagram on a PDF page and it is still
  sharp enough to read on the projector.
- *As a teacher*, my pen strokes never move the PDF page by accident.

## Scope

- Rasterise PDF pages natively and add them as slides (Phase 1).
- Page range selection (`3-7,10`).
- Background "lock" so the PDF image cannot be selected, moved or erased.
- CLI subcommand and MCP tool (`import_pdf`) on top of the same Rust API.
- The same page model from the browser (pdf.js) path.

## Non-goals

- Editing PDF text or annotations. PDF form filling.
- Exporting back to PDF (separate future plan).
- Vector (resolution-independent) PDF backgrounds. Listed below as a Phase 2
  follow-up because it needs a format change.
- OCR of scanned PDFs (see [handwriting-recognition](./handwriting-recognition.md)
  for the recognition stack).

## Design

### Page model (shared by both paths, no format change)

For each imported PDF page *k* with size `pw × ph` (PDF points):

1. Add a slide page with `frame_w = 1920`, `frame_h = round(1920 × ph / pw)`
   (portrait pages give tall frames; the presenter fits the frame to the screen).
2. Rasterise the page at `2 × frame_w` = 3840 px wide (sharp at 200 % zoom on a
   1080p projector), encode as WebP (fallback PNG).
3. Add one `image` item at `x = 0, y = 0, w = frame_w, h = frame_h`, `rot = 0`,
   with the lowest `z` on the page, via `Package::add_asset` (content-addressed,
   so re-importing the same PDF dedupes).
4. Set the page title to `"<pdf title or file stem> – p. <k>"`.

Everything above uses existing keys (`spec/rupok-format.md#pages`,
`#kind-image`). No RFC is needed for the raster model.

### Background lock (format change, requires an RFC)

Add an optional item key `locked: bool` (default `false`) to
`spec/rupok-format.md#items`. Locked items are skipped by hit-testing, lasso,
eraser and "select all", and are drawn normally. This is additive under the
forward-compatibility rules (old readers ignore the key) but it is a spec change,
so it **requires an RFC**. Until it lands, the importer still works; the image is
merely selectable.

Code touch points once accepted:

- `crates/rupok-core/src/model.rs`: `Item { locked: bool }` (new field).
- `crates/rupok-core/src/doc.rs`: read/write `locked` in `read_item` /
  `add_item`; new `Lesson::set_locked(page, id, bool)` (one commit).
- `crates/rupok-core/src/scene.rs`: `hit_test`, `lasso`, `erase_hits` skip locked.
- `crates/rupok-wasm/src/engine.rs`: `select_all`, `clear_page` skip locked;
  new `Engine::set_locked`.

### Native rasteriser (new crate)

```text
crates/rupok-pdf/            (new, AGPL-3.0-or-later, native only)
  src/lib.rs               PdfDocument, PageInfo, render_page(), PdfError
  src/import.rs            import_into(lesson, pkg, pdf_bytes, opts, now_ms)
```

```rust
pub struct ImportOptions { pub pages: Option<Vec<u32>>, pub width_px: u32, pub format: ImageFormat }
pub struct Imported { pub page_ids: Vec<PageId>, pub skipped: Vec<(u32, String)> }

/// Adds one slide per selected PDF page after `after_index`.
pub fn import_into(
    lesson: &mut rupok_core::Lesson,
    pkg: &mut rupok_format::Package,
    pdf: &[u8],
    after_index: Option<usize>,
    opts: &ImportOptions,
    now_ms: f64,
) -> Result<Imported, PdfError>;
```

- Backend: `pdfium-render` (binds a PDFium shared library loaded at run time).
  The Tauri app ships the PDFium binary per platform; the CLI looks for it next
  to the executable or via `RUPOK_PDFIUM`. A pure-Rust backend (e.g. `hayro`) can
  be added behind the same trait later.
- Keep the crate out of `wasm32` builds: the browser uses pdf.js.
- `now_ms` is a parameter; the crate never reads the clock.
- Each page is added with `Lesson::add_page` + `Lesson::add_item`: one commit
  each, so undo removes one page at a time.

### Front ends

| Surface | Change |
|---|---|
| CLI (`crates/rupok-cli/src/cli.rs`) | New `rupok import-pdf <lesson.rpk> <file.pdf> [--pages 3-7] [--after N]` |
| MCP (`crates/rupok-cli/src/mcp.rs`, `tools.rs`) | New tool `import_pdf { lesson, pdf, pages?, after_page? }`; the PDF path goes through `workspace.rs` checks |
| Desktop (`apps/desktop`) | Tauri command `import_pdf(path)`, calls `rupok-pdf`, then hands the updated `.rpk` bytes to the webview's `Board::load`, or (better) Loro updates to `Board` |
| Browser (`apps/board`) | pdf.js renders pages to canvas, then calls a new `Board::add_background_page(bytes, ext, w, h)` |

New engine API for both UI paths (`crates/rupok-wasm/src/engine.rs`):

```rust
/// Append a slide whose background is the given image, sized to the frame.
pub fn add_background_page(&mut self, bytes: Vec<u8>, ext: &str, frame_w: f64, frame_h: f64, title: &str, now_ms: f64) -> usize;
```

`Engine::add_image` centres the image on the camera, which is wrong for
backgrounds; the new method places it at `(0, 0)` with the lowest `z`.

### Sequence (desktop)

```text
teacher drops PDF ─▶ webview ─▶ invoke("import_pdf", path)
                                   │  rupok-pdf: open, render pages (threads)
                                   ▼
                     add pages + images into Lesson (one commit per page)
                                   │
                     export_updates(vv_before) ─▶ webview ─▶ Board import
```

## Implementation tasks

1. **RFC: `locked` item key** — write the RFC with spec text for
   `spec/rupok-format.md#items`. *(intermediate)*
2. **`Engine::add_background_page`** + `Board` binding + engine unit test with
   `MockSink`. *(good-first-issue)*
3. **Browser import with pdf.js** in `apps/board`: file picker and drop target,
   page range dialog, progress toast, calls `add_background_page`. *(intermediate)*
4. **`crates/rupok-pdf` skeleton**: `PdfDocument::open`, `page_count`,
   `page_size`, errors; PDFium discovery; CI job that downloads PDFium.
   *(intermediate)*
5. **`render_page` to RGBA + WebP/PNG encoding** with a size cap
   (3840 px wide, 8192 px max side). *(intermediate)*
6. **`import_into`** with page ranges and `now_ms`; integration test with a
   generated 3-page PDF fixture in `crates/rupok-pdf/tests/`. *(intermediate)*
7. **Parallel rendering** (one PDFium document per worker thread) and a
   progress callback. *(advanced)*
8. **CLI `import-pdf`** subcommand. *(good-first-issue, after 6)*
9. **MCP `import_pdf`** tool with workspace path checks and a JSON summary.
   *(intermediate)*
10. **Implement `locked`** in core, scene and engine once the RFC is
    accepted. *(intermediate)*
11. **Desktop wiring**: Tauri command, drag-and-drop of `.pdf` onto the
    window. *(intermediate, depends on [tauri-desktop](./tauri-desktop.md))*

## Acceptance criteria and tests

- `cargo test -p rupok-pdf`: importing the 3-page fixture yields 3 new pages,
  each with exactly one `image` item at `(0, 0, frame_w, frame_h)` and the
  lowest `z`; page 2 of a landscape/portrait mixed fixture has the right
  `frame_h`.
- Importing the same PDF twice adds pages but **no new assets**
  (`pkg.assets.len()` unchanged).
- Undo after an import of *n* pages removes the last page only.
- `rupok import-pdf` then `rupok info` lists the new pages; the result opens
  in the browser board and in `Package::read`.
- MCP: an agent call with a path outside the workspace is rejected with the
  `workspace.rs` error text.
- With `locked`: lasso, eraser and select-all never touch the background
  (unit tests in `scene.rs` and `engine.rs`).
- Browser and native imports of the same PDF produce the same page sizes and
  item geometry (compare `get_lesson_outline` output).

## Performance budget

| Metric | Budget (mid-range laptop, 4 cores) |
|---|---|
| 30-page A4 PDF, native import | ≤ 10 s total, first page visible ≤ 1 s |
| Peak extra memory during import | ≤ 300 MB |
| Asset size per page (3840 px WebP, text-heavy page) | ≤ 400 KB typical |
| Board frame time with 30 imported pages open | unchanged (`cargo run -p rupok-bench --release`) |

## Open questions

- PDFium binary distribution: bundle per platform (size ~5–6 MB each) or
  download on first use? Licence notice placement in the app's About box.
- WebP encoding in pure Rust (`image` crate, lossless only) vs `libwebp`
  (lossy, much smaller). Is lossless acceptable for scanned pages?
- Phase 2: vector backgrounds. Store the original PDF as an asset and add an
  item kind `pdf_page { asset, page }` rendered lazily as tiles (pairs with
  [renderer-lod](./renderer-lod.md)). Needs an RFC for a new item kind in
  `spec/rupok-format.md#items`.
- Should imported pages carry the PDF's outline (bookmarks) as page titles?

## References

- `spec/rupok-format.md` — `#pages`, `#kind-image`, `#assets`,
  `#versioning-and-forward-compatibility`
- `crates/rupok-wasm/src/engine.rs` — `Engine::add_image`, `Engine::add_page`
- `crates/rupok-format/src/package.rs` — `Package::add_asset`
- Related: [tauri-desktop](./tauri-desktop.md), [knowledge-base](./knowledge-base.md)
  (shares PDF text extraction), [renderer-lod](./renderer-lod.md)
