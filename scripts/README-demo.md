# The hero demo video

The landing page's hero (`apps/board/index.html`, `#hero-video`) is a silent,
looping video of a teacher taking a class on the real board. It's recorded by
`scripts/record-demo.mjs`, so it can be re-recorded with one command whenever
the board changes.

```sh
# The board must be running at http://127.0.0.1:3737/board/ (scripts/dev.sh).
node scripts/record-demo.mjs                 # both themes; about 1 hour on a busy machine
```

Outputs, in `apps/board/public/demo/`:

| File | What |
|---|---|
| `rupok-demo-light.webm` / `.mp4` | White page, light app theme (VP9 / H.264) |
| `rupok-demo-dark.webm` / `.mp4` | Dark app theme, blackboard page |
| `rupok-demo-light.webp` / `-dark.webp` | Posters: the annotated board, just before Teach |

Each video is 1920×1080 at 60 fps, about 30 s, and about 3 MB
(two-pass encodes to a size budget). The last 0.4 s cross-fades into the
first frame, so the loop has no visible seam. `public/hero.webp` (the page's
JSON-LD screenshot) is a copy of the light poster; refresh it after a
re-record:

```sh
cp apps/board/public/demo/rupok-demo-light.webp apps/board/public/hero.webp
```

The page picks the take that matches the site theme (an inline script right
after the `<video>`, and `src/site.ts` when the visitor switches theme). Both
takes follow the same script frame for frame, so a theme switch keeps the
playback position.

## Storyboard

| # | Caption | What happens |
|---|---|---|
| 1 | Write on any slide | The teacher writes "Projectile motion" with the pen and underlines it. |
| 2 | Drop in a live lesson from your AI | Page 2, then **Insert ▾ → Smart Object… → Presets → Projectile motion**. |
| 3 | Step through with your clicker | PageDown twice: the launch, then the velocity split. |
| 4 | Annotate on top | Red pen: a circle round the peak (held still so Smart shapes snaps it), an arrow onto the range tag, then a highlighter swipe. |
| 5 | Present full screen | F5 (Teach), then the laser pointer along the flight path. |
| 6 | Replay every class | Esc, **Replay**, and the dock's scrubber dragged end to end so the ink redraws. |

The captions, keycaps (PgDn, F5) and the "Ms. Rahman" cursor are drawn into
the page by the recorder, in the site's style. The board's own toasts are
hidden during the take because they'd sit under the caption.

## How it records

A real-time screen capture drops frames on a busy machine (Xvfb + x11grab
managed 11–25 fps here while other builds ran). The recorder runs the board on
**virtual time** instead:

- An init script in every frame (the board and each Smart Object iframe)
  replaces `requestAnimationFrame`, timers, `performance.now`, `Date`,
  `Event.timeStamp` and `Math.random` (seeded), and pauses CSS and Web
  Animations, moving them by hand.
- The recorder steps every frame's clock by exactly 1/60 s, then takes a
  screenshot (`Page.captureScreenshot`) and pipes it to ffmpeg. However slow
  the machine, the video is frame-perfect 60 fps and the same every run.
- Pen input is real CDP pen events (`pointerType: "pen"`) with pressure, four
  per frame (240 Hz). Handwriting uses single-stroke Hershey Script glyphs
  (`scripts/demo-hershey.mjs`), joined into cursive, smoothed with a
  centripetal Catmull-Rom spline and timed by the 2/3 power law (slower in
  tight curves), with i-dots and t-bars added after each word.
- Vite's HMR socket is blocked in the recording page, so edits elsewhere in
  the tree can't reload the board mid-take.
- The near-lossless master (x264 4:4:4) goes to `/dev/shm/rupok-demo` (RAM) by
  default and is deleted after encoding. Nothing big touches the disk.

The setup (not recorded) uses the board's own modules through the dev server:
it adds the second page, sets the blackboard colour for the dark take, and
picks the pen and highlighter sizes. Everything in the take is driven like a
user would: clicks, keys and pen strokes.

## Options

```sh
node scripts/record-demo.mjs --theme dark          # one theme (light, dark, both)
node scripts/record-demo.mjs --preview             # 30 fps draft + stills, no final encode
node scripts/record-demo.mjs --preview --until 2   # stop after scene 2 (quick checks)
node scripts/record-demo.mjs --stills --keep       # also save stills and keep the masters
node scripts/record-demo.mjs --encode-only         # re-encode kept masters (after --keep)
node scripts/record-demo.mjs --max-mb 2.5          # size budget per video file
node scripts/record-demo.mjs --url http://127.0.0.1:4173/board/   # another server
node scripts/record-demo.mjs --gpu vulkan          # GPU WebGL instead of SwiftShader
```

`--work <dir>` (or `RUPOK_DEMO_WORK`) moves the masters and stills, and
`--out <dir>` moves the outputs. Stills (`<work>/stills/<theme>-<scene>.png`)
are the quickest way to check a re-record: `written`, `object`, `stepped`,
`annotated`, `teach`, `replayed`.

## Requirements

- Node 22+, ffmpeg with libx264, libvpx-vp9 and libwebp, and Chrome
  (`CHROME=/path/to/chrome`, default `/usr/bin/google-chrome`). No display or
  Xvfb needed: Chrome runs headless.
- `playwright-core`. It isn't a project dependency: it's resolved from
  `PLAYWRIGHT_CORE=/path/to/playwright-core`, then from the current
  directory, then normally.
- The board's dev server (the setup imports `/src/controller.ts`).

## When the board changes

- **New features** (Smart shapes, LaTeX in objects, PDF export…): just
  re-record. Smart shapes are turned on (`smartShapes: "hold"`) when the
  board supports them, and the circle in scene 4 is held still at the end to
  snap.
- **The projectile preset's layout changes:** the annotation spots are
  fractions of the slide in `SPOTS` at the top of `record-demo.mjs`. Run with
  `--preview` and check `stills/light-annotated.png`.
- **UI the take depends on** (find controls by these; update `record()` if
  they change):
  - `.dock` (the Smart Dock) exists once the board is ready (setup waits for it);
  - `.pages .page-thumb` (the second page's thumbnail);
  - header **Insert ▾**: `button[aria-label="Insert"]`, then the
    `.hmenu [role=menuitem]` "Smart Object…";
  - `.preset-card` containing "Projectile" (the presets gallery);
  - `Colour #e5484d` (red swatch; optional, the colour is also set directly);
  - header `button[title^="Replay this page"]`, then the dock's replay
    scrubber `.dock-replay input[type=range]`;
  - keys: PageDown, F5 (Teach), Esc, P, H, L;
  - modules `/src/controller.ts` and `/src/state.ts` (setup and ink sizes:
    `setSizes`, `setWidth`, `setColor`, `setHlColor`, `board.set_width/set_color`).
- **Timing:** each scene is a few lines at the bottom of `record()`; `hold()`
  takes seconds of video time.
