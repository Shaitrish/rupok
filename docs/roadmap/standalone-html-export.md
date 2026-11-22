# Standalone HTML export

`Status: 📋 planned · Phase 2 · Difficulty: intermediate`

Export a lesson as one self-contained `.html` file that opens offline in any modern browser, with pages, ink, images, working Smart Objects and the step-by-step replay.

## Goal

Some schools block unknown file types, students rarely install apps, and
learning platforms (Moodle, Google Classroom) accept HTML attachments. One
`.html` file that "just opens" is the most portable form of a lesson. The
export is read-only, deterministic and needs no server.

## User story

- *As a teacher*, I click File → Export → Web page and upload the file to
  Google Classroom; students open it on any device and see my slides and ink.
- *As a teacher*, students press Play on the replay bar and watch how I solved
  the equation, stroke by stroke.
- *As a teacher*, the quiz object in the lesson still works in the exported file.
- *As a teacher*, my agent runs `rupok export lesson.rpk --html` and attaches
  the result to an e-mail.

## Scope

- New exporter producing a single HTML string from a `Package`.
- Inline player (vanilla JS, Canvas 2D) that decodes strokes with the
  point codec, renders them per the width rules, shows text/images, runs
  objects, and replays.
- Smart Objects embedded as sandboxed `srcdoc` iframes with the SDK, CSP and
  `rupoklib` libraries inlined; the player is the bridge host.
- Front ends: CLI `export --html`, MCP tool `export_html`, board menu (wasm).

## Non-goals

- Editing in the exported file (use the [web viewer](./web-viewer.md) or the app).
- Pixel-identical rendering with the GPU renderer (close match per the spec's
  width and coverage rules is enough).
- PDF or video export (separate plans; MP4 is Phase 3).

## Design

### Crate

```text
crates/rupok-export/                    (new, AGPL-3.0-or-later; builds native and wasm32)
  src/lib.rs        export_html(pkg, opts, libs) -> Result<String, ExportError>
  src/data.rs       JSON model embedded in the page
  src/objects.rs    srcdoc builder: SDK + CSP + rupoklib inlining
  assets/player.js  the inline player (hand-written, no build step)
  assets/player.css
```

```rust
pub struct ExportOptions { pub include_replay: bool, pub include_objects: bool, pub exported_at_ms: f64, pub title: Option<String> }
pub trait LibResolver { fn lib_file(&self, name: &str, file: &str) -> Option<Vec<u8>>; }
pub fn export_html(pkg: &rupok_format::Package, opts: &ExportOptions, libs: &dyn LibResolver) -> Result<String, ExportError>;
```

- Deterministic: same input and options → identical bytes (sorted maps,
  no random ids, `exported_at_ms` passed in — no clock reads).
- The SDK is embedded from `packages/object-sdk/sdk.js` at build time
  (`include_str!`), so exports always carry the SDK version they were built
  with.

### Embedded data model

```html
<script type="application/json" id="rupok-data">{ "v":1, "title":"…", "pages":[ {
  "title":"…", "frame":[1920,1080], "bg":["plain", 4294967295],
  "items":[ {"k":"s","id":"…","z":1,"t":1763870401000,"tool":"pen","c":488911871,"w":3,"o":1,"off":[0,0],"pts":"AQPAAoAFgAAQF5kIMAf/CA=="},
            {"k":"t", …}, {"k":"i","asset":"a1"}, {"k":"o","obj":"binary-counter","step":0,"state":null, …} ] } ],
  "assets": {"a1":"data:image/webp;base64,…"}, "objects": {"binary-counter": {"srcdoc":"…","steps":10}} }</script>
```

- Stroke points stay in **point codec v1** bytes (base64), not SVG: smaller,
  and they keep per-point timing for replay. The player has a ~40-line decoder
  following `spec/rupok-format.md#stroke-points` (version byte, LEB128 varints,
  zigzag deltas, `/16`, `/255`).
- Items are emitted in stacking order (`z`, then id).

### Player (`player.js`)

- **Rendering**: for each stroke, draw discs at points and the hull between
  consecutive discs (tapered capsule) using
  `radius = 0.5 × w × (0.35 + 0.65 × p)` (`spec/rupok-format.md#stroke-width-and-rendering`).
  A translucent stroke is drawn opaque into an offscreen canvas, then
  composited once with `globalAlpha = A × opacity`, so it never darkens where
  it overlaps itself.
- Text items as positioned `<div>`s; images as `<img>` with data URLs; pages
  scale to fit the window (frame) or fit content (infinite boards).
- Navigation: arrows/PageUp/PageDown/Space, swipe, page strip, full screen.
- **Replay**: the same timeline as `crates/rupok-wasm/src/replay.rs` — items by
  `created` then id, stroke duration = last `dt`, idle gaps capped at 900 ms
  (`MAX_GAP_MS`). Play/pause, speed 0.5–8×, scrubber.

### Smart Objects

For each bundle in `pkg.objects`:

1. Take the entry HTML (`ObjectBundle::entry_html`).
2. Inject, first thing in `<head>`: a CSP `<meta>` (as in
   `spec/smart-object.md#network`, with `data:` added for inlined files) and
   the SDK `<script>`.
3. Replace `rupoklib://<name>/<file>` references with `data:` URLs from the
   `LibResolver`, and add an import map mapping each used
   `rupoklib://…` module specifier to its `data:` URL. Rewriting is required
   because `rupoklib://` is a relative URL to browsers (schemes must start with a
   letter) and the base of a `srcdoc` frame is `about:srcdoc`.
4. Inline other bundle files referenced by relative URL as `data:` URLs
   (best effort; warn on anything unresolved).
5. Emit `<iframe sandbox="allow-scripts" srcdoc="…">` sized to the item
   geometry.

The player implements the host side of the bridge protocol
(`spec/smart-object.md#bridge-protocol`): replies to `hello` with `init`
(`mode: "present"`, default theme, saved `state`), sends `step` on Next,
honours `goto` and `steps`, and keeps `state` in memory. Presenter Next
advances objects to their `count` before turning the page.

### Front ends

| Surface | Change |
|---|---|
| CLI (`crates/rupok-cli/src/cli.rs`) | `rupok export <lesson.rpk> --html <out.html> [--no-replay] [--no-objects]` |
| MCP (`crates/rupok-cli/src/mcp.rs`) | `export_html { lesson, out }` (workspace-checked, writes `.html` only) |
| Board (`crates/rupok-wasm`) | `Board::export_html(include_replay: bool) -> String`; UI downloads it |

Library files: the CLI and desktop app resolve `rupoklib` from their vendored
`libs/` directory; the browser board fetches them from the app origin.

## Implementation tasks

1. **Crate skeleton + data model** (`data.rs`) with a golden-file test for a
   tiny lesson. *(intermediate)*
2. **JS point decoder** in `player.js` plus a Node test comparing it with the
   Rust codec on fixtures (incl. the spec's worked example). *(good-first-issue)*
3. **Stroke rendering** per the width rules, with offscreen compositing for
   translucency. *(intermediate)*
4. **Pages, text, images, navigation, fit-to-window.** *(intermediate)*
5. **Replay timeline and controls** matching `replay.rs`. *(intermediate)*
6. **Object srcdoc builder** (SDK + CSP injection). *(intermediate)*
7. **`rupoklib` inlining** with `LibResolver` and import maps. *(intermediate)*
8. **Bridge host in the player** (`init`, `step`, `goto`, `steps`, `state`).
   *(intermediate)*
9. **CLI and MCP front ends.** *(good-first-issue, after 1)*
10. **`Board::export_html`** + menu item and download. *(good-first-issue)*
11. **Size report**: print per-part sizes and warn above 25 MB. *(good-first-issue)*

## Acceptance criteria and tests

- `cargo test -p rupok-export`: exporting the same package twice gives identical
  strings; golden file for a 2-page fixture.
- Decoder test: the worked example bytes
  `01 03 C0 02 80 05 80 00 10 17 99 08 30 07 FF 08` decode to the spec's three
  points (Node `node --test`).
- Exported file opened from `file://` in Chromium and Firefox (Playwright
  smoke test in CI): pages render, Next steps through `binary-counter` before
  changing page, the quiz records answers, replay plays to the end.
- An exported object cannot fetch the network (CSP) and cannot reach the
  player's DOM (sandbox).
- Visual check: a translucent highlighter stroke crossing itself has uniform
  colour (pixel test in the Playwright run).

## Performance budget

| Metric | Budget |
|---|---|
| Player JS + CSS (unminified) | ≤ 40 KB |
| Export of a 20-page lesson without heavy libraries | ≤ 1 s, output ≤ 3 MB + images |
| Open a 10,000-stroke export on a mid-range laptop | first page ≤ 1 s |
| Replay playback | 60 fps on a mid-range laptop for typical pages |

## Open questions

- Heavy libraries (three.js ≈ 1 MB+) inflate exports by ~33 % as base64. Offer
  "link libraries next to the file" (a folder export) as an alternative?
- Should exports embed the original `.rpk` (so "Open in Rupok" can recover
  the editable lesson)? It doubles the size.
- Multi-file bundles with dynamic imports or `fetch()` of their own files
  cannot be fully inlined; is a warning enough?

## References

- `spec/rupok-format.md` — `#stroke-points`, `#stroke-width-and-rendering`, `#replay`
- `spec/smart-object.md` — `#bridge-protocol`, `#network`, `#offline-libraries-rupoklib`
- `crates/rupok-wasm/src/replay.rs` — `Replay::new` timeline, `MAX_GAP_MS`
- `packages/object-sdk/sdk.js`
- Related: [web-viewer](./web-viewer.md), [tauri-desktop](./tauri-desktop.md)
