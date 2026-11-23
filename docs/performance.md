# Performance: pen latency and ink smoothness

How Rupok gets a pen stroke onto the screen, how we measure it, and the
numbers. Two harnesses:

- `cargo run -p rupok-bench --release [-- --check]`: the Rust budgets (document,
  GPU frames, the engine's pen path, wet-versus-dry pixels). Needs a GPU for the
  render budgets.
- `node bench/latency/pen-latency.mjs`: pen-to-pixel in a real browser. It drives
  the production build (`apps/board/dist`) in Chrome with synthetic pen input
  through the DevTools protocol (`Input.dispatchMouseEvent`, `pointerType: "pen"`,
  240 Hz) and measures what the page produces. CI runs it with SwiftShader and
  `--check` (job "Pen latency" in `.github/workflows/ci.yml`).

```sh
cd apps/board && bun run build                    # dist/ (wasm built first, see AGENTS.md)
cd ../../bench/latency && npm install             # playwright-core only
node pen-latency.mjs --chrome /usr/bin/google-chrome            # real GPU (Vulkan), headless
node pen-latency.mjs --smoothing 0 --speed 150 --shot wetdry.png  # slow writing; wet/dry picture
node pen-latency.mjs --swiftshader --check        # the CI budgets
node pen-latency.mjs --headed --present           # adds the compositor estimate (needs a display)
```

## The pen's path

1. The OS hands a pointer event to Chromium (WebView2 in the Windows app). Its
   `timeStamp` is our time zero; the digitiser and OS before it are outside any
   app's reach.
2. Chromium delivers `pointermove` once per frame, just before
   `requestAnimationFrame`, with every sample since the last frame in
   `getCoalescedEvents()` (240 Hz pens give 4 per 60 Hz frame).
3. `controller.ts` hands the samples to the engine (`Board.pointer_move`). The
   `StrokeBuilder` (`crates/rupok-ink/src/builder.rs`) smooths them (One-Euro
   filter; Settings → Pen → Smoothing) and the engine rebuilds the wet stroke:
   **smoothed points + the raw tip + at most 12 ms of the browser's prediction**.
4. The frame loop (`requestAnimationFrame`) draws: the cached dry layer is
   blitted, the wet stroke drawn on top (`crates/rupok-render`), and the GPU
   commands are submitted.
5. Chromium's compositor presents the frame, usually on the next refresh.
6. **On Windows**, the OS draws the last stretch from our ink to the pen at the
   next refresh, ahead of steps 4 and 5: the *delegated ink trail*
   (`navigator.ink`, `apps/board/src/wetInk.ts`).
7. Pen-up: the dry stroke is the same points, ending at the same raw tip, then
   simplified by 0.2 screen px. It must look exactly like the last wet frame.

### The wet/dry contract (read this before changing how ink looks)

- Wet ink = `builder.points()` + `builder.tip()` + prediction
  (`Engine::update_wet`). Dry ink = `builder.finish()` = points + tip,
  simplified. Anything that changes how a wet stroke looks (pen styles,
  textures, widths) must change the dry stroke identically.
- The renderer draws both with the same shader. A segment's quad must not
  reach ahead of its end point (`ink.wgsl`), or dense wet strokes render
  thinner than their simplified dry copies.
- Guarded by `crates/rupok-render/tests/wet_dry.rs` (GPU), the bench budget
  "wet vs dry pixels differ", and the latency harness's "pen-up changed pixels".

## What we found (v0.2.1)

Measured on the Linux dev machine below, synthetic pen, 240 Hz, 400 px/s.

| Cause | Evidence | Fix |
|---|---|---|
| **Crosshair cursor over the ink.** The board's CSS cursor for pen, highlighter and shapes was `crosshair`; Chromium draws the CSS cursor under a hovering or touching pen. This is the "+" on the smartboard. | `App.tsx` `cursorFor`: `default: return "crosshair"` | No cursor for pen and touch, and none at all while drawing; a 5 px dot for a mouse when not drawing (`wetInk.ts` `drawCursor`). Same in the draw-over-apps overlay. |
| **Wet ink thinner than dry: the stroke "thickens" at pen-up.** Each segment's quad reached `radius + 1.5 px` past its end point. Segments of one stroke share a depth and the first fragment wins, so with a sample every ~0.5 px a quad claimed pixels belonging to segments 2+ steps ahead, which it cannot see, and gave them too little coverage. The simplified dry stroke has long segments and did not suffer. | GPU test: 733 of 1543 ink pixels (47 %) differ between the same stroke wet and dry. Harness at 150 px/s: 4–6 % of the stroke's pixels change at pen-up. | `ink.wgsl`: a quad reaches back to cover the joint but not ahead, except the last segment's cap. 0 % differ. |
| **The ink trailed the pen, then jumped to it at pen-up.** The One-Euro filter lags (≈ 11 ms at 400 px/s with the default smoothing, ≈ 50 ms when slow), and only `finish()` snapped the tail to the pen. | Zig-zag: dry ink misses pen samples by p95 5 px (default) and 2.8 px (**smoothing 0**). | Raw tip: wet ink always ends at the newest sample (`StrokeBuilder::tip`), dry ends at the same point. |
| **Smoothing 0 was not raw.** `from_smoothing(0)` still filtered at 6 Hz. | Zig-zag p95 2.8 px at smoothing 0. | Smoothing 0 bypasses the filter (`OneEuroParams::RAW`): 0.0 px. |
| **Prediction overshot by ~2 frames.** All of Chromium's predicted events were drawn: the tip ran ~30 ms of travel ahead of the pen, overshot corners and retracted at pen-up. | Tip 12 px (30 ms) ahead of the newest sample, p50. | Predicted points ≤ 12 ms ahead; none when the OS ink trail is on. |
| **Default smoothing rounded corners.** 50 % smoothing cuts a handwriting corner by ~5 px at writing speed. | Zig-zag p95 5 px; a simulation of the filter agrees (5.9 px). | Default 25 % (existing users keep their setting). 0 % is raw, as in Windows Ink. |
| **Autosave ran mid-stroke.** Every 3 s, whatever the pen was doing, the whole lesson was serialised to an `.rpk` (`rupok-bench`: package write ≈ 1 s for a 100k-stroke lesson; it scales with strokes and imported PDFs and images). | `setInterval(autosave, 3000)`; profile shows `Package::write` (zip, crc32) inside the writing session. | Autosave waits for a 1 s pause in the writing (at most ~30 s, then the next gap between strokes) and never runs while a pointer is down. |
| **Maths preview could recognise mid-stroke.** It ran 160 ms after each stroke, often inside the next one. | `MathPreview.tsx` timer. | Waits until the pen has rested 120 ms. |
| Style thrash in the pointer handler: the eraser ring's style was written on every move for every tool, and the board's rect read per event. | Profile: `getBoundingClientRect` and `hover` among the top JS self-time. | Ring updated only for the eraser; rect read once per frame. |

Checked and **not** a cause:

- **Re-uploading the whole wet stroke every move (O(n²)).** It does, but a 10 s,
  2400-sample stroke costs 0.04 ms per move (p99) and its frame 0.9 ms
  (`rupok-bench` "pen path"). Not worth the complexity of an incremental
  buffer; the budget will flag it if that changes.
- **Smart Ink at pen-up** with straight strokes: the next stroke's first frame
  was no slower than any other (harness "pen-down → first frame").
- **Dropped samples:** 0.2 %: one per stroke, the harness's last move racing
  its own pen-up.
- **`pointerrawupdate`:** fires before the frame, but we only draw in the frame,
  so it cannot shorten pen-to-frame; the OS ink trail covers the time it could.

## Numbers

Hardware: **Linux dev machine**, AMD Ryzen 5 5625U laptop, integrated Radeon
(Vega 7, RADV RENOIR) through Vulkan, Chrome 154 headless, 1600×900 at 1×,
synthetic pen 240 Hz, 400 px/s straight strokes and a zig-zag. The machine was
also running other work; runs before and after were interleaved.

**default smoothing (before 50 %, after 25 %)**

| | Before (v0.2.1) | After |
|---|---|---|
| Pen → frame submitted, p50 / p95 (ms) | 10.8 / 31.1 | 9.9 / 17.9 |
| Pointer handler, p95 (ms) | 0.30 | 0.30 |
| Ink tip vs newest pen sample, p50 (ms of pen travel) | 30 ms ahead (prediction) | 20 ms ahead (prediction) |
| Ink tip behind newest sample, p95 (px) | -8.4 | 0.2 |
| Pixels changed at pen-up (% of stroke) | 0.28 | 0.00 |
| Zig-zag: pen sample → dry ink, p95 (px) | 5.0 | 3.6 |
| Samples dropped (%) | 0.19 | 0.19 |

**smoothing 50 %**

| | Before (v0.2.1) | After |
|---|---|---|
| Pen → frame submitted, p50 / p95 (ms) | 10.7 / 20.3 | 9.5 / 16.9 |
| Pointer handler, p95 (ms) | 0.30 | 0.20 |
| Ink tip vs newest pen sample, p50 (ms of pen travel) | 29 ms ahead (prediction) | 9 ms ahead (prediction) |
| Ink tip behind newest sample, p95 (px) | -1.7 | -2.8 |
| Pixels changed at pen-up (% of stroke) | 0.27 | 0.04 |
| Zig-zag: pen sample → dry ink, p95 (px) | 5.0 | 5.0 |
| Samples dropped (%) | 0.19 | 0.19 |

**smoothing 0**

| | Before (v0.2.1) | After |
|---|---|---|
| Pen → frame submitted, p50 / p95 (ms) | 10.0 / 19.8 | 9.4 / 17.4 |
| Pointer handler, p95 (ms) | 0.30 | 0.30 |
| Ink tip vs newest pen sample, p50 (ms of pen travel) | 28 ms ahead (prediction) | 9 ms ahead (prediction) |
| Ink tip behind newest sample, p95 (px) | 2.2 | 0.0 |
| Pixels changed at pen-up (% of stroke) | 0.22 | 0.04 |
| Zig-zag: pen sample → dry ink, p95 (px) | 2.8 | 0.0 |
| Samples dropped (%) | 0.19 | 0.19 |

**slow writing, 150 px/s, default smoothing**

| | Before (v0.2.1) | After |
|---|---|---|
| Pen → frame submitted, p50 / p95 (ms) | 9.2 / 16.8 | 9.3 / 16.4 |
| Pointer handler, p95 (ms) | 0.20 | 0.20 |
| Ink tip vs newest pen sample, p50 (ms of pen travel) | 19 ms ahead (prediction) | 11 ms ahead (prediction) |
| Ink tip behind newest sample, p95 (px) | -0.8 | -0.7 |
| Pixels changed at pen-up (% of stroke) | 4.39 | 0.01 |
| Zig-zag: pen sample → dry ink, p95 (px) | 1.4 | 1.0 |
| Samples dropped (%) | 0.07 | 0.07 |

Before is the v0.2.1 build, after is this change, same machine, runs
interleaved. "Ahead" is the browser's prediction drawn past the newest real
sample (the round cap adds ~1.5 px); "behind" is the ink trailing the pen.
The zig-zag error at 25–50 % smoothing is the smoothing itself (corners are
rounded on purpose); at 0 it is raw.

`rupok-bench` on the same machine, before and after built and run back to back:

| | Before | After |
|---|---|---|
| Wet frame p99 (budget 4 ms) | 0.90 ms | 1.19 ms |
| Pan frame p99 @100 % (budget 8.3 ms) | 3.16 ms | 3.07 ms |
| Pan, whole board (1.7 M segments), p50 / p99 | 19.6 / 24.3 ms | 15.1 / 18.3 ms |
| Open lesson, 100k strokes (budget 1000 ms) | 755 ms | 730 ms |
| Pen move → wet update p99, 10 s stroke (2400 samples; budget 0.5 ms) | — | 0.03 ms |
| Wet frame at the end of that stroke, p99 | — | 0.87 ms |
| Wet vs dry pixels differ (budget 0.5 %) | 47 % (dense stroke, GPU test) | 0.00 % |

The whole-board pan got faster because segment quads no longer reach ahead
(less overdraw). Wet frames are within noise.

"Pen → frame" is the event timestamp to the end of the frame that drew the
sample (GPU commands submitted). On top of that comes the compositor (about
one refresh, ~17 ms at 60 Hz) and the display. On Windows the OS ink trail
covers that remaining stretch from the pen's side.

These are not Windows numbers. The founder's smartboard and a Windows laptop
with a pen are the next measurements, with the overlay below.

## Reading the overlay

Settings → About → Troubleshooting → **Performance and pen latency overlay**.
Write for a few seconds; it updates every second.

- **Pen → frame** (big number, and p95): from the moment the OS stamped a pen
  sample to the moment our frame with it went to the GPU. Good: under 20 ms
  p50. Over 30 ms: the board is busy or the GPU is slow.
- **Pen-down → screen**: the browser's own measure of a pen-down until its frame
  was on screen, compositor included (Chrome only reports events of 16 ms or
  more, so it reads high). Good: 20–35 ms at 60 Hz.
- **Input**: samples per second while writing and the pointer kind. A pen
  should say `pen` and 100–240 Hz. `mouse` on a smartboard means the board
  reports itself as a mouse (no pressure, and Windows may smooth it).
- **Frames**: frames drawn per second while writing (should reach the display's
  rate, e.g. 60) and our CPU time per frame.
- **OS ink trail**: `on` means Windows is drawing the trail ahead of our frames.
  `ready` means it starts on the first stroke. `not on this system` outside
  Windows.
- **Renderer** and the graphics adapter. Red "Software rendering" (SwiftShader,
  Microsoft Basic Render Driver, WARP, llvmpipe) means no GPU: everything will
  lag. Update the graphics driver; Settings → About → Graphics shows the same.

## Native wet ink: when, and how

The remaining gap to a native inking app is the browser's frame pipeline: a
sample waits for the next frame (0–17 ms), then the compositor (~1 refresh).
Native Windows Ink draws wet ink on its own low-latency composition path.

On Windows the delegated ink trail closes most of that gap from the pen's
side, inside the web stack, and it is the path Chromium and Edge built for this.
So: **ship the web pipeline with the trail, measure on the smartboard with the
overlay, and only go native if the numbers say so.** Go native if, on the
board, the trail reads "on" and writing still feels behind, or the trail can't
run in WebView2 (overlay says "failed").

Design, if needed (Windows only, `apps/desktop`):

- A child `DirectComposition` visual above the WebView2 child window, hosting
  `InkPresenterDesktop` (`Windows.UI.Input.Inking`, through the `windows`
  crate). It takes pen input only (`InkInputProcessingMode`), touch and mouse
  pass to the webview.
- Wet strokes are drawn natively. On pen-up the points (with timestamps and
  pressure) go to the board through a Tauri event; the board commits them as a
  normal stroke and, once its frame with the stroke is presented, tells the
  native layer to drop it (no flicker: the native stroke stays until the web
  one is on screen).
- The native layer must mirror the board camera (pan and zoom), pen colour,
  width and pressure curve, palm rejection, and stay out of the way of every
  tool that is not the pen.

Costs: Windows only; two renderers that must look identical (exactly what the
pen-up bugs above were about); input routing over a webview (airspace,
focus, gestures); a hand-off on every stroke. Weeks of work, and every pen
style later doubles. Not justified until the smartboard numbers say so.

## WebView2 notes

- The desktop app keeps WebGL2 as the default renderer (Settings → About →
  Renderer); the adapter is shown there and in the overlay.
- No extra `additionalBrowserArgs`: nothing we measured needs one, and the
  ones that cut latency in benchmarks (`--disable-gpu-vsync`,
  `--disable-frame-rate-limit`) tear or burn power. The GPU blocklist can force
  software rendering on old drivers; the overlay now makes that visible
  instead of guessing.
- If a mark still follows the pen with the board's cursor gone, it is Windows'
  own pen feedback: Settings → Bluetooth & devices → Pen & Windows Ink →
  "Show visual effects" and "Show cursor". Not verified on a real board yet.
