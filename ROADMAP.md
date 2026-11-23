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
| ✅ | Board UI in the browser (SolidJS): toolbar, pages, dialogs, design system | `apps/board`, [DESIGN.md](DESIGN.md) | |
| ✅ | Selection you can manipulate: frame, resize handles, one-gesture press-and-drag (mouse, pen, touch), floating toolbar | `apps/board/src/components/Selection.tsx` | |
| ✅ | Copy, cut, paste, duplicate across lessons and tabs; group, ungroup, merge ink | `crates/rupok-wasm` (`export_items_json`, `import_items_json`, `group_selection`) | |
| ✅ | Real page thumbnails; duplicate, reorder (drag) and delete pages | `crates/rupok-wasm` (`page_preview_json`, `duplicate_page`), `apps/board` | |
| ✅ | Modes: Prepare, Teach (always-fitted slide, minimal chrome), Student view; per-mode chrome and toolbar, importable UI config | `apps/board/src/modes.ts`, [docs/ui-config.md](docs/ui-config.md) | |
| 🚧 | PDF import in the browser: every PDF page becomes a slide you can write on (pdf.js) | `apps/board/src/pdf.ts` | |
| 📋 | PDF import, native (CLI, MCP, desktop) | [pdf-import-native](docs/roadmap/pdf-import-native.md) | advanced |
| 🚧 | Smart Object host: sandboxed iframes, CSP, SDK injection, bridge | `apps/board/src/objects.ts` | |
| 🚧 | Save and open `.rpk`, autosave | `apps/board`, `crates/rupok-wasm` | |
| 🚧 | Replay a page stroke by stroke (play, seek, speed) | `crates/rupok-wasm/src/replay.rs` | |
| 🚧 | Desktop app (Tauri 2) for Windows and Linux: shell, `.rpk` association, single instance, atomic save, installers in CI | [tauri-desktop](docs/roadmap/tauri-desktop.md), `apps/desktop` | intermediate |
| 🚧 | Desktop overlay: draw over any app on the screen (ink, toolbar, click-through, save to lesson) | [desktop-overlay](docs/roadmap/desktop-overlay.md), `apps/desktop/src/overlay.rs` | advanced |
| 🚧 | Touch and pen settings: palm rejection modes, smoothing, pressure curve | engine done; UI in `apps/board` | |
| 🚧 | Themes: light/dark, accent colour, board colours | [DESIGN.md](DESIGN.md) | |
| ✅ | Website: landing page, RPK viewer page (`/open/`, `/bn/open/`), smartboard / AI / online-class / teacher pages, honest comparison pages, Bangla pages, `llms.txt`, sitemap and JSON-LD | `apps/board/site/` (`node site/build.mjs`), [docs/seo](docs/seo/keywords.md) | |

## Phase 2: the teacher's workflow

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 📋 | Course library: Subject → Course → Chapter → Lesson | [course-library](docs/roadmap/course-library.md) | intermediate |
| 📋 | Chapter knowledge base: sources your agent can cite | [knowledge-base](docs/roadmap/knowledge-base.md) | advanced |
| 💡 | Brand kit: school colours, fonts and logo as theme tokens for every object | | |
| 📋 | Agent panel (ACP): chat with your own AI agent inside Rupok | [acp-agent-panel](docs/roadmap/acp-agent-panel.md) | advanced |
| 📋 | Live agent edits during class, merged into the open lesson | [acp-agent-panel § Live edits](docs/roadmap/acp-agent-panel.md#live-edits) | advanced |
| 📋 | Replay with audio | [replay-audio](docs/roadmap/replay-audio.md) | intermediate |
| 📋 | LAN "send to board" from laptop or phone | [lan-send](docs/roadmap/lan-send.md) | intermediate |
| 🚧 | Web viewer (PWA): open any `.rpk` in a browser, offline. Read-only student view at `/board/?view` done; the `/open/` page hands a picked file to it; installed-PWA "Open with" (`launchQueue`) done; offline install to do | [web-viewer](docs/roadmap/web-viewer.md) | intermediate |
| 📋 | Android share target: share a `.rpk` from WhatsApp straight into Rupok (Web Share Target + service worker) | [rupok-cloud § Android](docs/roadmap/rupok-cloud.md#android-web-share-target) | intermediate |
| 📋 | Rupok Cloud: Google sign-in, 500 MB free per teacher, share links (`rupok.app/s/<id>`) students open with no account, Pro and School plans (Paddle, then bKash) | [rupok-cloud](docs/roadmap/rupok-cloud.md) | advanced |
| 📋 | Claude connector: remote MCP server on Workers with OAuth, so teachers manage lessons from their own Claude | [rupok-cloud § Claude connector](docs/roadmap/rupok-cloud.md#claude-connector-remote-mcp) | advanced |
| 📋 | Google Drive "Open with → Rupok" (Workspace Marketplace): a `.rpk` tapped in Drive opens in the Rupok viewer | [rupok-cloud § Google Drive](docs/roadmap/rupok-cloud.md#google-drive) | intermediate |
| 📋 | Student annotation layers | [student-layers](docs/roadmap/student-layers.md) | advanced |
| 📋 | Browser panel with clip-to-board | [browser-panel](docs/roadmap/browser-panel.md) | intermediate |
| 📋 | Standalone HTML export: one file students can open anywhere | [standalone-html-export](docs/roadmap/standalone-html-export.md) | intermediate |

## Phase 3: the classroom

| Status | Item | Plan | Difficulty |
|---|---|---|---|
| 💡 | Rosters and data export | | |
| 📋 | Self-hostable live broadcast server (students follow the board live) | [live-broadcast](docs/roadmap/live-broadcast.md) | advanced |
| 📋 | Hosted live classes (Durable Objects) and class spaces, School admin, in Rupok Cloud | [rupok-cloud § Phases](docs/roadmap/rupok-cloud.md#phases-rough-sizes) | advanced |
| 📋 | Live quizzes with a results panel | [live-quiz](docs/roadmap/live-quiz.md) | intermediate |
| 💡 | Submissions (students hand in annotated pages) | | |
| 📋 | Subject tools: ruler, protractor, compass, graphing, periodic table | [subject-tools](docs/roadmap/subject-tools.md) | intermediate |
| 📋 | Handwriting to text and math | [handwriting-recognition](docs/roadmap/handwriting-recognition.md) | advanced |
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
| ✅ Fixed: an item with an unknown `kind` (or undecodable `pts`) failed the whole page in `Lesson::items()`; it is now skipped. | `crates/rupok-core/src/doc.rs` | [rupok-format § Versioning](spec/rupok-format.md#versioning-and-forward-compatibility) |
| `Package::read` accepts duplicate ZIP entry names (last wins); should reject. | `crates/rupok-format/src/package.rs` | [rupok-format § Size and count limits](spec/rupok-format.md#size-and-count-limits) |
| `Package::read` does not check that `mimetype` is first or that `manifest.format == "rupok-lesson"`. | `crates/rupok-format/src/package.rs` | [rupok-format § Package layout](spec/rupok-format.md#package-layout) |
| Unknown ZIP entries and manifest keys are dropped on rewrite; should be preserved. | `crates/rupok-format/src/package.rs` | [rupok-format § Versioning](spec/rupok-format.md#versioning-and-forward-compatibility) |
| A bundle with a bad `manifest.json` fails the whole package; should be treated as missing. | `crates/rupok-format/src/package.rs` | [rupok-format § Smart Object bundles](spec/rupok-format.md#smart-object-bundles) |
| ✅ Fixed: a stroke's `created` was the pen-up time; it is now pen-down, as the spec says. | `crates/rupok-wasm/src/engine.rs` | [rupok-format § Replay](spec/rupok-format.md#replay) |
| Host: on `steps`, clamp the stored step and send `step`; ignore out-of-range `resize`; count state size in UTF-8 bytes; send `state: null` when empty. | `apps/board/src/objects.ts` | [smart-object § Host behaviour](spec/smart-object.md#host-behaviour) |

## Proposed format and protocol changes

The plans propose these changes. Each needs an RFC before it is implemented.
Some overlap and must be reconciled in the RFCs.

| Change | Proposed by |
|---|---|
| Stable lesson id in `meta` (`meta.id` / `meta.lesson_id`) | [rupok-cloud](docs/roadmap/rupok-cloud.md), [student-layers](docs/roadmap/student-layers.md) |
| `spec/sync.md`: Loro update exchange and compaction for Rupok Cloud sync | [rupok-cloud](docs/roadmap/rupok-cloud.md) |
| `spec/library.md`: course, chapter and `kb/` layout | [course-library](docs/roadmap/course-library.md), [knowledge-base](docs/roadmap/knowledge-base.md) |
| `layers/<peer>.loro` layout | [student-layers](docs/roadmap/student-layers.md) |
| `locked` item key (background slides from PDFs) | [pdf-import-native](docs/roadmap/pdf-import-native.md) |
| `group` item key (Group, Ungroup and Merge in the board). Implemented in `rupok-core` as an optional, forward-compatible key; `spec/rupok-format.md` still needs the row | board selection work |
| `author` item key (who or which agent made an item) | [acp-agent-panel](docs/roadmap/acp-agent-panel.md) |
| `geom` key on shape strokes (kind, box, rotation, normalised polygon vertices) so shapes resize at a constant line width, and new `shape` values (`circle`, `square`, `triangle`, `diamond`, `pentagon`, `hexagon`, `star`). Implemented in `rupok-core` as an optional, forward-compatible key and documented in `spec/rupok-format.md` (Shape geometry); needs the RFC | Smart Ink (board) |
| `source_url` / `source_title` on images | [browser-panel](docs/roadmap/browser-panel.md) |
| `recordings` root container (audio) | [replay-audio](docs/roadmap/replay-audio.md) |
| `shape: "arc"` for compass strokes | [subject-tools](docs/roadmap/subject-tools.md) |
| `meta.lang` | [i18n](docs/roadmap/i18n.md) |
| Smart Object manifest `sources` | [knowledge-base](docs/roadmap/knowledge-base.md) |
| `spec/broadcast.md`; quiz `answer` frames; `results/` entry; host → object `results` message | [live-broadcast](docs/roadmap/live-broadcast.md), [live-quiz](docs/roadmap/live-quiz.md) |
