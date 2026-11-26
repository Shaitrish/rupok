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
node pen-latency.mjs --4k --wayland wl-4k         # a 4K board: frame rate, full-size frames, screen redrawn, GPU time
```

Start with [Big screens and weak GPUs](#big-screens-and-weak-gpus-v025) if the
complaint is a slow board, and with [the board test](board-test.md) if you
are the one standing at it.

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
4. The frame loop (`requestAnimationFrame`) draws the wet stroke into the
   **wet tile**, a small canvas under the pen, and submits the GPU commands.
   The board's full-size canvas (the cached dry layer) is not drawn or
   presented while the pen writes inside the tile
   (`crates/rupok-render`, [the wet tile](#the-wet-tile-read-this-before-changing-how-frames-are-drawn)).
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
- Ink kept on the page (`engine/clip.rs`): wet ink is cut with `push_wet`,
  dry with `finish_clipped`, both by `clip_points` with the same inset. The OS
  ink trail is not drawn off the page, nor for a stroke that started off it
  (`wetInk.ts` `onPage`). Any new wet path must clip the same way.
- Pen styles (`rupok_ink::Brush`, spec "Brushes") follow it by
  construction: widths come from `Brush::radii` (pressure, the brush's taper
  measured from the ends, the nib's direction measured a stroke width back
  along the path, never between neighbouring points), textures are sampled
  in page space from one cached GPU noise texture, and dashes, dots and the
  rainbow follow the distance along the path, so those strokes are stored
  unsimplified (`Brush::keeps_every_point`). Magic ink is never saved: after
  pen-up its last wet frame stays on screen, then fades.
- Guarded by `crates/rupok-render/tests/wet_dry.rs` (GPU, every brush), the
  bench budgets "wet vs dry pixels differ" (plain pen and worst brush), and
  the latency harness's "pen-up changed pixels" (`--brush NAME`).

### The wet tile (read this before changing how frames are drawn)

While the pen writes, nothing the board does per frame may grow with the
screen. The ink that is changing is drawn in a second, small canvas
(`canvas.ink-wet`, about a tenth of a 4K screen) that the renderer places
under the pen; the main canvas (`canvas.ink`) keeps showing what it showed.
`InkRenderer::plan` decides each frame:

- **Tile only** (the usual frame while writing): the wet stroke, and
  strokes finished since the main canvas was last presented ("pending"),
  all fit in the tile. Only the tile is drawn and presented.
- **Main canvas too**, when the tile cannot stand in for the difference:
  dry ink changed other than by a stroke added on top (erase, move, undo),
  the view changed, the wet ink left the tile, or the pen has rested for
  220 ms with pending strokes. A long stroke that leaves the tile is
  *baked*: the main canvas draws it with the tile's rectangle cut out (a
  depth mask), and the tile keeps drawing the part still changing, placed a
  quarter-tile ahead of the pen. So no pixel is ever drawn by both.
- A finished stroke is drawn **over** the cached dry layer; the layer is
  only redrawn from scratch when something under it changed.

Rules that keep it correct:

- The tile and the main canvas composited must equal one canvas drawing
  everything, for every brush: `crates/rupok-render/tests/wet_tile.rs`
  (GPU; also checks that writing inside the tile presents no main frame).
- The tile is always in the main canvas's own pixels (same render scale),
  placed on whole pixels; both are presented in the same animation frame,
  so the browser shows them together.
- A canvas keeps its pixels only for the frame that drew it. Anything that
  reads the ink canvases back (a test, a screenshot of the canvas) asks for
  a full frame first (`Board.flush_ink`), or reads both.
- New transient ink (a preview, a pointer trail) needs nothing special:
  whatever differs from what the main canvas shows is found by comparing
  the wet vertex lists, and goes to the tile if it fits.

### Page design and the pen

Page backgrounds and branding (`components/PageBackdrop.tsx`) are DOM under
the ink canvas. They change only when the design does, so the browser
rasterises them once (per zoom level) and pen frames never touch them: a
trace of six strokes shows the same paint and raster work with and without a
tiled background, a logo and a footer (8 paints, about 1 ms of raster in
all). Agent SVG backgrounds may not animate (`rupok-live` refuses it). To
check after a change, write over a designed lesson:
`node pen-latency.mjs --lesson designed.rpk`.

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
| **Autosave ran mid-stroke.** Every 3 s, whatever the pen was doing, the whole lesson was serialised to an `.rpk` (`rupok-bench`: package write ≈ 1 s for a 100k-stroke lesson; it scales with strokes and imported PDFs and images). | `setInterval(autosave, 3000)`; profile shows `Package::write` (zip, crc32) inside the writing session. | Autosave waits for a 1 s pause in the writing (0.4 s once 30 s overdue, any gap after 2 min) and never runs while a pointer is down. |
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

## Big screens and weak GPUs (v0.2.5)

v0.2.4 on a real board: an 86-inch 4K panel, Windows OPS module, Intel UHD
graphics (12th-gen integrated), the desktop app (WebView2, ANGLE D3D11). The
overlay read Pen → frame 16–18 ms (p95 47–83 ms) with WebGPU and 44 ms (p95
124 ms) with WebGL2, with **0.6–2 ms of our own CPU time per frame**. The
board was waiting for the GPU, not for us.

### Where the frame went

Reproduced on the Linux dev machine (integrated Radeon, a similar class) at
3840×2160 on a virtual monitor (`pen-latency.mjs --4k --wayland`), with a
trace (`viz`, `cc`, `gpu`) and the GPU time of Chrome's GPU process:

| Per frame while writing, v0.2.4 | Evidence |
|---|---|
| The board cleared a 4K target and a 4K depth buffer, and copied the 4K dry layer to it, to add a few pixels of wet ink. | `ink.rs` `render()`; 100 % of writing frames presented the full canvas. |
| The compositor redrew the whole screen under it: a canvas that presents damages its whole area, and the ink canvas covered the board. | Trace: `root_damage_rect 0,0 3840x2160` in 288 of 352 frames; 86.5 % of the screen redrawn per frame on average. |
| The "Saving…" dot in the top bar faded in and out for as long as the lesson was unsaved, which while writing is always. The compositor redraws one rectangle around everything that changed, so a small tile under the pen and a 6 px dot in the top bar became most of the screen. | Trace: the damage rectangle ran from the top bar to the tile in 60 of 242 frames, after the tile alone had fixed the rest. |
| WebGL2 (the default until now) pays one more full-canvas copy per frame: wgpu draws into its own texture and copies it to the canvas at present. | `wgpu-hal` `gles/web.rs` `present`; 12 fps against 22–26 on the board. |
| The system compositor copies the window to the screen once more. | Rig: the virtual monitor's compositor used as much GPU time as Chrome. On Windows this is DWM; it is outside the app. |

Chrome's GPU process used **558 ms of GPU time per second** of writing: on
this laptop that is the edge of 60 fps, on a GPU half as fast it is 30.

One more finding, about the overlay itself: its "Frames" number was frames
per second of wall time, so the pauses between strokes counted as slow
frames (a mouse at 34 Hz "drew" at 34 fps). It now reports the rate between
frames while a pointer is down.

### What changed

| Change | Effect |
|---|---|
| **The wet tile** (above): writing draws and presents a 1024×768 canvas on a 4K board, not 3840×2160. | 3 % of writing frames present the full canvas (was 100 %); the compositor redraws 14 % of the screen per frame (was 86 %). |
| **Strokes added on top are drawn over the dry layer**, and pen-up no longer redraws every stroke on the page. | The pen-up frame costs one copy, not a copy plus the page. |
| **No full-size depth buffer in the frame pass**; depth is cleared only where wet ink is drawn. | One 4K clear less in every full frame. |
| **Render scale** (Settings → About → Performance lab → Ink sharpness): Auto / 100 / 75 / 50 %. Auto starts at the sharpest step with at most 5 MP (a 4K board: 75 %, 2880×1620) and steps down when more than 30 % of 90 consecutive frames come late, between strokes; one step back up after a long clean run, once. | The frames that do need the whole canvas (pan, zoom, erase, pen-up, a stroke leaving the tile) fill 44 % fewer pixels at 75 %. |
| **Renderer: Auto** is WebGPU in Chromium on a real GPU (the Windows app), WebGL2 otherwise (other engines, software adapters), with a fallback when WebGPU fails to start. | No extra full-canvas copy per frame where WebGPU works. |
| **The "Saving…" dot steps** twice a second instead of fading. | Nothing in the chrome changes on every refresh while writing. |
| **Flat panels** (Performance lab, off by default): no backdrop blur behind the dock and bars. | Less to redraw when ink changes under them; small on the rig (4 points of GPU load with the old full-canvas path). |

Idle costs nothing: with no change, the loop draws nothing and the
compositor produces no frame (trace: `SKIPPED_REASON_NO_DAMAGE`).

What we evaluated and left out: a CSS clip around a full-size wet canvas
(the compositor's damage did shrink, but WebGPU still presented the full
canvas: no gain); `desynchronized` low-latency canvases (not available for
WebGPU, and the tile and the main canvas must appear in the same frame);
WebView2 browser arguments (below).

### Numbers at 4K

**Linux dev machine, not a board:** Ryzen 5 5625U, integrated Radeon (RADV
RENOIR, Vulkan), Chrome 154 full screen on a 3840×2160 virtual monitor of a
headless GNOME (a real swap chain and a system compositor, but not Windows,
not DWM, not Intel), WebGPU, synthetic pen 240 Hz, 8 s of handwriting with
a long stroke every sixth. The machine was shared and busy; frame rates
moved by ±10 fps between runs, the other rows did not.

| | v0.2.4 | This change (100 %, tile 1024×768) |
|---|---|---|
| Frames while writing: mean / median / slowest 5 % (fps) | 43.8 / 56.8 / 20.0 | 57.3 / 59.9 / 41.8 |
| Pen → frame submitted, p50 / p95 (ms) | 14.1 / 57.6 | 10.1 / 20.9 |
| Writing frames that presented the full canvas | 100 % | 3.2 % |
| Screen redrawn by the compositor per frame | 86.5 % | 14.1 % |
| GPU time used by Chrome per second of writing | 558 ms | 183 ms |

GPU time follows the tile's area (same rig, 100 %): 2048×1152 332 ms/s,
1536×1024 260, 1024×768 189, 768×512 149, 512×384 139; too small a tile
(256×256: 370) costs more, because every letter leaves it. Medium
(1024×768 at 4K) is the default; Small and Large are in the Performance lab.

Without a GPU (SwiftShader, 1600×900, the CI configuration) a pen sample
reached its frame in 30–75 ms p50, against 179 ms before.

`rupok-bench` (one target, as on the page before the tile; the tile path is
measured by the harness above): see the pull request for the two tables.

**Not measured:** Intel UHD, Windows, DWM, WebView2. The model says the
change removes about two thirds of the browser's GPU work per frame; whether
that is enough for 60 fps on the board is what
[the board test](board-test.md) answers.

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
- **Frames while writing**: the rate between frames while a pointer is down,
  of the display's rate (`55 of 60 fps`), and how many of them drew only the
  small wet-ink layer or the full canvas (`52 small, 3 full`). Small frames
  should be nearly all of them while writing. It keeps the last stroke's
  numbers between strokes.
- **Board CPU per frame**: our own work. Under 2 ms; if frames are slow and
  this is small, the GPU is the limit.
- **Canvas**: the ink canvas in pixels, the ink sharpness (`Auto (75 %)`
  means the governor chose 75 %) and the wet-ink layer's size.
- **OS ink trail**: `on` means Windows is drawing the trail ahead of our frames.
  `ready` means it starts on the first stroke. `not on this system` outside
  Windows; `off` when switched off in the Performance lab or while the
  native wet-ink layer leads.
- **Native wet ink** (Windows app): `off`, `ready`, `drawing` or `failed`.
- **Renderer** and the graphics adapter. Red "Software rendering" (SwiftShader,
  Microsoft Basic Render Driver, WARP, llvmpipe) means no GPU: everything will
  lag. Update the graphics driver; Settings → About → Graphics shows the same.

## The board speed test

Settings → About → Troubleshooting → **Run board speed test**
(`apps/board/src/speedTest.ts`). What to run on a slow board instead of
filming it; the steps for a teacher are in [board-test.md](board-test.md).

It writes on the board by itself for 10 s in each mode (the old path,
three wet-layer sizes, three sharpness levels), asks the teacher to write
for 10 s in three modes and, in the Windows app, with the native layer,
reloads to repeat the scripted part with the other renderer, and ends with
one text report: per mode, frames while writing (median, slowest 5 %,
slowest), the share of frames on time and late, Pen → frame p50 and p95,
small and full frames, the input rate; above it the graphics card,
renderer, screen, refresh rate, browser and OS. The desktop app saves it
to `Documents/Rupok/speed-test-<date>.txt`.

The scripted pen calls the engine directly with 240 Hz samples handed over
just before each frame, as the browser hands over a real pen's. It
measures drawing and presenting, not the digitiser, the OS or the browser's
event delivery, and it cannot use the native layer; the hand-written runs
cover those. The lesson is saved to memory first and loaded back after.

## Native wet ink

A prototype, in the Windows app, **off by default** (Settings → About →
Performance lab → Native wet ink). It exists to answer one question on a
real board: is the browser's frame pipeline still what the teacher feels
once the web path draws a tenth of the screen?

How it works (`apps/desktop/src/native_ink.rs`, `apps/board/src/nativeInk.ts`):

- A transparent, click-through window owned by the board window, over its
  client area, with a DirectComposition surface drawn by Direct2D. It is
  shown only while a stroke is being written.
- Input stays with the webview (the window never takes a pointer), so every
  tool, gesture and palm rule works as before. The board forwards each pen
  sample from `pointerrawupdate`, which fires when the browser receives it,
  not at the next frame.
- Each batch of samples redraws only the rectangle around its new
  segments; DWM recomposes that rectangle. Nothing in it scales with the
  screen, and nothing waits for the webview's GPU work.
- The engine receives every sample as always and builds the stroke; the
  renderer just does not draw it wet (`Board.set_wet_hidden`). At pen-up
  the dry stroke appears in the board's own frame, and two frames later the
  native copy is removed. Ink kept on the page is clipped to the page.
- It fails safe: any error from the native side destroys the layer, tells
  the board, and the board draws wet ink itself again for the rest of the
  session.

Known limits of the prototype: a plain solid line, so only the pen and
ballpoint styles in an opaque colour use it; the raw pen samples, so the
board's smoothing shows as a small settle at the hand-off; one pen at a
time; hold-to-snap shapes show at pen-up; it has compiled on Windows in CI
and has **not been run on Windows** by us.

What decides its future is the board test: if the web path with the wet
tile holds the display's rate with Pen → frame under ~20 ms, the native
layer stays a prototype (two renderers that must look identical is the
cost that produced the pen-up bugs above). If the board is still behind
with small frames at 60 fps, or cannot reach 60, the native layer is the
way, and the next steps are: the engine's smoothed points instead of raw
samples, the brush radii from `rupok_ink`, and then the styles.

## WebView2 notes

- The renderer is Auto (Settings → About → Renderer): WebGPU in WebView2 on
  a real GPU, WebGL2 on a software adapter or when WebGPU does not start.
  The first 4K board measured twice the frame rate with WebGPU. The adapter
  is shown there and in the overlay, and a board without a GPU says so at
  start-up.
- No extra `additionalBrowserArgs`: nothing we measured needs one, and the
  ones that cut latency in benchmarks (`--disable-gpu-vsync`,
  `--disable-frame-rate-limit`) tear or burn power. GPU rasterisation and
  direct composition are WebView2's defaults and Tauri passes nothing that
  turns them off; the board in the video composited on the Intel GPU
  (ANGLE D3D11), not in software. To try a flag on a board without a new
  build, set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` before starting the app. The GPU blocklist can force
  software rendering on old drivers; the overlay now makes that visible
  instead of guessing.
- If a mark still follows the pen with the board's cursor gone, it is Windows'
  own pen feedback: Settings → Bluetooth & devices → Pen & Windows Ink →
  "Show visual effects" and "Show cursor". Not verified on a real board yet.
