# Rupok roadmap

Rupok ships in phases. Phase 0 (the engine) is done. Phase 1 makes it usable
in a real class. Every planned feature (📋) has a full plan in
[`docs/roadmap/`](docs/roadmap/), written so a contributor can pick it up. Each
plan covers the teacher's story, scope, design, PR-sized tasks, tests and open
questions.

| Legend | Meaning |
|---|---|
| ✅ | Done |
| 🚧 | In progress |
| 📋 | Planned: has a plan doc you can pick up |
| 💡 | Idea: no plan yet. Writing one is a great contribution |

**Want to help?** Pick a 📋 item, read its plan, and comment on (or open) the
matching issue to claim a task. Tasks tagged `good-first-issue` in the plans
are a gentle start. Also see [Known issues](#known-issues-good-first-fixes)
below. Changes to the file format or the Smart Object protocol need an RFC
first (see [CONTRIBUTING.md](CONTRIBUTING.md#rfcs)).

## Phase 0: foundations (pre-alpha)

| Status | Item | Where |
|---|---|---|
| ✅ | Ink pipeline: One-Euro smoothing, pressure curve, simplification, point codec v1 | `crates/rupok-ink` |
| ✅ | Lesson document on a Loro CRDT: pages, items, undo/redo, merge, replay timeline, spatial index | `crates/rupok-core` |
| ✅ | `.rpk` package reader/writer with safety limits; Smart Object bundles and validator | `crates/rupok-format` |
| ✅ | GPU ink renderer (wgpu: WebGPU/WebGL2 in the browser, Vulkan/Metal/DX12 natively), cached dry layer, chunk culling | `crates/rupok-render` |
| ✅ | Board engine and wasm API: tools, palm rejection, pinch zoom, pages, lasso, text, images, objects, save/load, replay | `crates/rupok-wasm` |
| ✅ | Performance harness with enforced budgets | `bench/` |
| ✅ | `rupok` CLI (`new`, `info`, `validate`, `add-object`) and MCP server with 11 lesson tools | `crates/rupok-cli` |
| ✅ | Specs: `.rpk` format v1 and Smart Object v1 (drafts) | [`spec/`](spec/) |
| ✅ | Smart Object SDK, example objects, AI-agent skill pack | `packages/object-sdk`, `examples/objects`, `skills/` |

## Phase 1: MVP (usable in a real class)

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 🚧 | Board UI in the browser (SolidJS): toolbar, pages, dialogs, design system | `apps/board`, [DESIGN.md](DESIGN.md) | |
| 🚧 | PDF import in the browser: every PDF page becomes a slide you can write on (pdf.js) | `apps/board/src/pdf.ts` | |
| 📋 | PDF import, native (CLI, MCP, desktop) | [pdf-import-native](docs/roadmap/pdf-import-native.md) | advanced |
| 🚧 | Smart Object host: sandboxed iframes, CSP, SDK injection, bridge | `apps/board/src/objects.ts` | |
| 🚧 | Save and open `.rpk`, autosave | `apps/board`, `crates/rupok-wasm` | |
| 🚧 | Replay a page stroke by stroke (play, seek, speed) | `crates/rupok-wasm/src/replay.rs` | |
| 📋 | Desktop app (Tauri 2) for Windows and Linux | [tauri-desktop](docs/roadmap/tauri-desktop.md) | intermediate |
| 📋 | Desktop overlay: draw over any app on the screen | [desktop-overlay](docs/roadmap/desktop-overlay.md) | advanced |
| 🚧 | Touch and pen settings: palm rejection modes, smoothing, pressure curve | engine done; UI in `apps/board` | |
| 🚧 | Themes: light/dark, accent colour, board colours | [DESIGN.md](DESIGN.md) | |

## Phase 2: the teacher's workflow

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 📋 | Course library: Subject → Course → Chapter → Lesson | [course-library](docs/roadmap/course-library.md) | intermediate |
| 📋 | Chapter knowledge base: sources your agent can cite | [knowledge-base](docs/roadmap/knowledge-base.md) | advanced |
| 💡 | Brand kit: school colours, fonts and logo as theme tokens for every object | | |
| 📋 | Agent panel (ACP): chat with your own AI agent inside Rupok | [acp-agent-panel](docs/roadmap/acp-agent-panel.md) | advanced |
| 📋 | Live agent edits during class, merged into the open lesson | [acp-agent-panel § Live edits](docs/roadmap/acp-agent-panel.md#live-edits) | advanced |
| 📋 | Replay with audio | [replay-audio](docs/roadmap/replay-audio.md) | intermediate |
| 📋 | Google Drive sync (Apache OpenDAL, `drive.file` scope, per-device Loro update logs) | [google-drive-sync](docs/roadmap/google-drive-sync.md) | advanced |
| 📋 | LAN "send to board" from laptop or phone | [lan-send](docs/roadmap/lan-send.md) | intermediate |
| 📋 | Web viewer (PWA): open any `.rpk` in a browser, offline | [web-viewer](docs/roadmap/web-viewer.md) | intermediate |
| 📋 | Student annotation layers | [student-layers](docs/roadmap/student-layers.md) | advanced |
| 📋 | Browser panel with clip-to-board | [browser-panel](docs/roadmap/browser-panel.md) | intermediate |
| 📋 | Standalone HTML export: one file students can open anywhere | [standalone-html-export](docs/roadmap/standalone-html-export.md) | intermediate |

## Phase 3: the classroom

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 💡 | Rosters and data export | | |
| 📋 | Self-hostable live broadcast server (students follow the board live) | [live-broadcast](docs/roadmap/live-broadcast.md) | advanced |
| 📋 | Live quizzes with a results panel | [live-quiz](docs/roadmap/live-quiz.md) | intermediate |
| 💡 | Submissions (students hand in annotated pages) | | |
| 📋 | Subject tools: ruler, protractor, compass, graphing, periodic table | [subject-tools](docs/roadmap/subject-tools.md) | intermediate |
| 📋 | Handwriting to text and math | [handwriting-recognition](docs/roadmap/handwriting-recognition.md) | advanced |
| 📋 | OneDrive, Dropbox, WebDAV (through OpenDAL) | [google-drive-sync § Other backends](docs/roadmap/google-drive-sync.md#other-backends-phase-3) | intermediate |
| 💡 | MP4 export of a replay | | |
| 💡 | Plugin API | | |
| 📋 | Android app and overlay | [android](docs/roadmap/android.md) | advanced |
| 📋 | Renderer level of detail and tile pyramid for zoomed-out boards | [renderer-lod](docs/roadmap/renderer-lod.md) | advanced |

## Phase 4: community

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 💡 | Smart Object gallery: share and remix objects | | |
| 💡 | Community themes | | |
| 💡 | Accessibility: screen readers, keyboard-only use, high-contrast mode | | |
| 📋 | Internationalisation, Bangla first | [i18n](docs/roadmap/i18n.md) | intermediate |

## Known issues (good first fixes)

Gaps between the specs and the code, found while writing the specs. Each is
small and well-defined. The specs list them under "Conformance notes".

| Issue | Where | Spec |
|---|---|---|
| `validate()` panics when a `//` follows non-ASCII text within 48 bytes (byte slicing in `remote_resources`). Use a char-boundary-safe window. | `crates/rupok-format/src/object.rs` | [smart-object § Conformance notes](spec/smart-object.md#conformance-notes) |
| An item with an unknown `kind` (or undecodable `pts`) fails the whole page in `Lesson::items()`; it should be skipped. | `crates/rupok-core/src/doc.rs` | [rupok-format § Versioning](spec/rupok-format.md#versioning-and-forward-compatibility) |
| `Package::read` accepts duplicate ZIP entry names (last wins); should reject. | `crates/rupok-format/src/package.rs` | [rupok-format § Size and count limits](spec/rupok-format.md#size-and-count-limits) |
| `Package::read` does not check that `mimetype` is first or that `manifest.format == "rupok-lesson"`. | `crates/rupok-format/src/package.rs` | [rupok-format § Package layout](spec/rupok-format.md#package-layout) |
| Unknown ZIP entries and manifest keys are dropped on rewrite; should be preserved. | `crates/rupok-format/src/package.rs` | [rupok-format § Versioning](spec/rupok-format.md#versioning-and-forward-compatibility) |
| A bundle with a bad `manifest.json` fails the whole package; should be treated as missing. | `crates/rupok-format/src/package.rs` | [rupok-format § Smart Object bundles](spec/rupok-format.md#smart-object-bundles) |
| A stroke's `created` is the pen-up time; the spec says pen-down. | `crates/rupok-wasm/src/engine.rs` | [rupok-format § Replay](spec/rupok-format.md#replay) |
| Host: on `steps`, clamp the stored step and send `step`; ignore out-of-range `resize`; count state size in UTF-8 bytes; send `state: null` when empty. | `apps/board/src/objects.ts` | [smart-object § Host behaviour](spec/smart-object.md#host-behaviour) |

## Proposed format and protocol changes

The plans propose these changes. Each needs an RFC before it is implemented.
Some overlap and must be reconciled in the RFCs.

| Change | Proposed by |
|---|---|
| Stable lesson id in `meta` (`meta.id` / `meta.lesson_id`) | [google-drive-sync](docs/roadmap/google-drive-sync.md), [student-layers](docs/roadmap/student-layers.md) |
| `spec/sync.md`: per-device update logs, compaction | [google-drive-sync](docs/roadmap/google-drive-sync.md) |
| `spec/library.md`: course, chapter and `kb/` layout | [course-library](docs/roadmap/course-library.md), [knowledge-base](docs/roadmap/knowledge-base.md) |
| `layers/<peer>.loro` layout | [student-layers](docs/roadmap/student-layers.md) |
| `locked` item key (background slides from PDFs) | [pdf-import-native](docs/roadmap/pdf-import-native.md) |
| `author` item key (who or which agent made an item) | [acp-agent-panel](docs/roadmap/acp-agent-panel.md) |
| `source_url` / `source_title` on images | [browser-panel](docs/roadmap/browser-panel.md) |
| `recordings` root container (audio) | [replay-audio](docs/roadmap/replay-audio.md) |
| `shape: "arc"` for compass strokes | [subject-tools](docs/roadmap/subject-tools.md) |
| `meta.lang` | [i18n](docs/roadmap/i18n.md) |
| Smart Object manifest `sources` | [knowledge-base](docs/roadmap/knowledge-base.md) |
| `spec/broadcast.md`; quiz `answer` frames; `results/` entry; host → object `results` message | [live-broadcast](docs/roadmap/live-broadcast.md), [live-quiz](docs/roadmap/live-quiz.md) |
