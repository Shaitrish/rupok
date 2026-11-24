# Changelog

What changed in each Rupok release, written for teachers first. Developer
notes come after each list. Downloads are on
[rupok.app/download](https://rupok.app/download/) and on
[GitHub Releases](https://github.com/Shaitrish/rupok/releases).

## Unreleased

### For teachers

- **Show pages anywhere:** press G, tap the page counter on the dock, or
  use ⋯ → Show pages, in Teach, Prepare and Student view. Big thumbnails
  slide in over the slide without moving it; tap one to jump (pin it to
  keep it open).
- **New pages look like the last one:** + adds a page right after the one
  you are on, with the same colour, pattern, background and branding. Insert
  before or after, a + between thumbnails, and a page menu (right-click or
  press and hold) to duplicate, move or delete. Mark several pages at once
  to delete, copy or move them together.
- **Page design:** looks such as chalkboard, lined notebook, graph paper and
  exam paper; your own background image (fit, fill, stretch, tile, centre,
  dimmed for readable ink); and your school's logo with the school, teacher,
  class and date on every page (or one). The text can fill itself in with
  the lesson title, page number and more. Save it as your default and every
  new lesson starts with it. Shown in Teach, Student view, replay,
  thumbnails and the PDF.
- **Design with AI:** ask your AI (or type /design in the chat) for a
  background or theme; it can set colours, draw a background and place your
  logo, and each change is one undo step.
- **Teach is truly full screen:** F5 or the mode switcher fills the whole
  screen, with no title bar in the desktop app. Esc or the exit button on
  the dock leaves Teach and full screen together.
- **The page is the board:** on slide pages, ink that starts outside the
  page does not draw, ink stops at the page edge, and shapes, text and moved
  items stay on the page. Turn it off in Settings → Pen & ink → Keep ink on
  the page. Infinite pages are never limited.
- **A real duster:** the eraser now rubs out only what it covers and splits
  strokes, like a board duster (Whole strokes is still one tap away). It has
  a duster size, a clear circle shows where it rubs, a finger erases with it
  even after you used a pen, and Clear page asks for a second tap.
- **Palm eraser** also works on boards that report every touch as a dot: a
  palm seen as two touches rubbing back and forth now erases.
- **Select in Teach:** Select & move is on the Teach dock. A finger tap
  picks handwriting more easily, and the selection toolbar can recolour,
  make ink thinner or thicker, make anything smaller or bigger (or drag a
  corner), duplicate, group, turn ink into text and delete, with big
  buttons. Each change is one undo step.
- On a touch board the Teach dock keeps finger-sized buttons.
- **Videos, audio and 3D models in class:** paste a YouTube link on the
  board (or Insert ▾ → Video / YouTube…) and it plays from
  youtube-nocookie.com with start and end times, mute, loop and autoplay.
  Drop an mp4, webm, mp3, m4a, ogg or wav file and it is saved in the lesson,
  so it plays offline. Drop a `.glb` 3D model to turn, zoom and animate it,
  and save camera views that the clicker steps through. In Teach, big
  buttons and Space or K play and pause; you can always write over a paused
  video. Offline, a video shows its poster and says it needs internet; PDFs
  show the poster with the link and a QR code.
- **Trusted web embeds:** PhET, GeoGebra, Desmos, published Google Slides
  and Docs, Wikipedia and Khan Academy links embed live. Other links are not
  embedded; Rupok offers to open them in the browser.
- The Smart Object library has a **Media** tab.

### For developers

- Board engine: the partial eraser (`engine/duster.rs`, `set_eraser_mode`),
  ink kept on the page (`engine/clip.rs`, `set_clip_ink`) and selection
  editing (`engine/restyle.rs`: `restyle_selection`, `scale_selection`,
  `scale_selection_by`, `scale_cancel`).
- `rupok_core::NewItem` has a `z` (a cut stroke's pieces keep its place)
  and `Edits` has `text_colors`. The file format does not change.
- The desktop main window may set full screen
  (`core:window:allow-set-fullscreen`).
- Page design in the format (proposed, pending RFC; spec "Page design"):
  page keys `background` and `branding`, and `meta.branding`, additive plain
  map values that older readers ignore. `rupok_core`: `PageStyle`,
  `PageBackground`, `Branding`, `Lesson::{set_page_style, set_branding,
  set_page_branding, branding_for, add_page_like, duplicate_pages,
  remove_pages, move_pages, forget_history}`.
- New live ops and MCP tools `set_page_design` and `set_branding` (live or
  on a file); agent SVG is checked (no scripts, external references or
  animation).
- `bench/latency/pen-latency.mjs --lesson FILE.rpk` writes over a given
  lesson's first page.
- Media items: an optional `media` map on `image` items (proposed, pending
  RFC; spec/rupok-format.md "Media items"). Older readers show the poster.
  `rupok_format::media` is the embed allowlist; `rupok_format::model3d`
  builds the 3D viewer object. New MCP tools `add_youtube`, `add_media`,
  `add_3d_model`; live ops `add_media`, `add_model`. The desktop CSP allows
  frames from the trusted sites only. `rupoklib://three/addons/` adds
  `GLTFLoader`.
- Fixed: saving while an item was undone dropped its Smart Object bundle or
  image from memory, so redo brought it back broken.

## v0.2.1 (23 November 2026)

### For teachers

- **Smart Ink:** a small handwritten "+" stays as your ink instead of
  turning into an arrow.
- **Maths:** chemistry formulas such as H₂O and CO₂ come out upright
  (`\mathrm{H}_2\mathrm{O}`), as in a textbook, not in italics.
- **Website:** a better Bangla home page and corrected copy across the
  site.

### For developers

- Dependency updates in one batch (#12); Dependabot now groups updates
  weekly per ecosystem.

## v0.2.0 (23 November 2026)

The second early-alpha release. It brings a home for your lessons, a new
toolbar and header, AI that works with you on the board, and a Mac app.

### For teachers

- **Library.** All your lessons in one place: folders, search, thumbnails,
  recent lessons and templates. Lessons save themselves as you teach.
- **Smart Dock.** A new drawing toolbar you can drag anywhere. It snaps to
  edges and corners, remembers your recent colours, lets you pin the tools
  you use, opens away from your writing hand, and shrinks to a small bubble.
  Ink drawn next to it passes through.
- **New header.** Lesson title, save, share, export and modes in one tidy
  bar at the top.
- **Settings.** One searchable Settings window for appearance, pen and
  touch, the dock, each mode, files, keyboard, voice, AI and more.
- **Palm eraser and gestures.** Rub with the side of your hand or palm to
  erase; two- and three-finger gestures for undo, redo and moving around.
  You can calibrate them for your board.
- **PDF export with handouts.** Export the lesson as slides, or as A4
  handouts with two or three slides per page and ruled lines for notes.
- **Smart shapes.** Hand-drawn shapes snap clean, and Smart Ink turns
  sketches into graphs, flowcharts, logic gates and electric circuits.
- **Maths preview.** Write maths by hand and Rupok shows the typeset
  version above it. One tap turns it into neat maths; one undo brings your
  ink back. Nothing converts on its own.
- **LaTeX in Smart Objects.** Interactive objects can show proper maths.
- **Presets gallery.** Ready-made interactive objects for physics,
  chemistry, maths, ICT and history, with previews in light and dark.
- **AI chat with your own AI.** Chat inside Rupok with your own Claude,
  Codex or OpenCode. It can read the lesson, make Smart Objects and add
  slides.
- **Live AI editing.** Connect an AI agent and watch it edit the open
  lesson while you teach; every change can be undone.
- **Solve this.** Select a problem on the board and the AI writes a
  checked, step-by-step solution in handwriting below it. By default you
  check it first, before the class sees it.
- **Voice input.** Dictate text instead of typing, in the browser or, on
  the desktop, offline.
- **macOS app.** A universal app for Apple silicon and Intel Macs (macOS 11
  or later), with a menu bar, ⌘ shortcuts and `.rpk` files that open with a
  double-click. It is new: please tell us how it works on your Mac.
- **Lesson files AI can read.** Every `.rpk` now carries a plain-language
  summary of the lesson (`README.md`, `content.md`, `content.json`), so
  ChatGPT, Claude and other AI can read a lesson without Rupok.
- **Light and dark website**, a new download page, and a page about the RPK
  format at [rupok.app/rpk](https://rupok.app/rpk/).

**Rupok Cloud is coming soon:** sync between devices and share links that
students open with no account. Until it launches, the "Sign in with
Google" button in Settings and the Library stays switched off, and
everything works on your device without an account.

### Notes

- The installers are not code-signed yet. **Windows:** if SmartScreen says
  "Windows protected your PC", choose **More info**, then **Run anyway**.
  **Mac:** right-click Rupok and choose **Open**, then **Open** again; or
  open **System Settings → Privacy & Security** and choose **Open Anyway**.
- The macOS app and the overlay (drawing over other apps) are beta. The
  overlay on macOS has not been tried on a real Mac yet.

### For developers

- New crate `rupok-live`: the live agent bridge (WebSocket protocol v1)
  shared by the board and `rupok mcp --live`.
- `rupok agent`: an ACP client that relays the in-app chat to Claude Code,
  Codex or OpenCode, with MCP over loopback HTTP.
- `rupok-ink`: maths recognition (`math`) and text + LaTeX to handwriting
  (`write`); Smart Ink recognisers for graphs, diagrams, logic and science.
- `rupok-format`: readable entries (`readable.rs`, spec section "Readable
  entries", pending RFC). The board's autosave refreshes them at most once
  a minute; explicit saves, closing and exports always do.
- Skills: `rupok-live-board` and `read-rpk` (with `read_rpk.py`).
- The macOS build (universal `.dmg`, optional signing and notarisation) and
  WebKit e2e run in CI. The project now uses a CLA (CLA.md) instead of DCO
  sign-off.

## v0.1.0 (23 November 2026)

The first early-alpha release: the board in the browser at
[rupok.app](https://rupok.app) and a desktop app for Windows and Linux.

### For teachers

- Fast GPU pen and highlighter with pressure and palm rejection, shapes,
  eraser, lasso and laser pointer.
- Slides and PDF import: write on every page of a PDF.
- Teach mode (F5): the slide fills the screen with minimal tools.
- Student mode: open any `.rpk` lesson read-only and replay the class.
- Replay any page stroke by stroke, at 1–8× speed.
- Smart Objects: live simulations and quizzes made by your AI, plus 8
  presets (physics, chemistry, maths, ICT, history).
- Selection with handles, group and merge, copy and paste between lessons,
  page thumbnails.
- Desktop app for Windows (`.exe`, `.msi`) and Linux (`.deb`, AppImage),
  unsigned.

### For developers

- Rust core (Loro CRDT, wgpu) compiled natively and to WebAssembly; SolidJS
  board; Tauri 2 desktop shell.
- The `.rpk` package format and Smart Object specs (Apache-2.0), the Object
  SDK and the agent skill pack.
- `rupok` CLI with an MCP server, so Claude Code, Codex and OpenCode can
  build lessons.
