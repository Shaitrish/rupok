# Changelog

What changed in each Rupok release, written for teachers first. Developer
notes come after each list. Downloads are on
[rupok.app/download](https://rupok.app/download/) and on
[GitHub Releases](https://github.com/Shaitrish/rupok/releases).

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
