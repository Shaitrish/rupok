# Agent capabilities: what your AI can do in Rupok

Rupok is **agent-first**: anything a teacher can do, their own AI agent
(Claude Code, Codex, OpenCode, Gemini CLI, or any MCP client) can do on
their behalf, through the live bridge and the MCP tools
([ai-agents.md](ai-agents.md)). This file is the checked list. A CI test
(`crates/rupok-cli/tests/agent_capabilities.rs`) fails when a setting,
a dock button or a live operation is missing here, or when a setting
marked *agent* is not in the board's allowlist
(`apps/board/src/agentSettings.ts`), or the other way round.

How to read the tables:

- **Before** is what an agent could do in v0.2.3; **Now** is this
  version. ✅ yes, ◐ partly, ✗ no, 🔒 teacher only (with the reason).
- **How** names the MCP tool (and the live op, when its name differs).
- **File** means the tool also edits a `.rpk` file in the workspace
  (`path: "lesson.rpk"`); **Live** means it works on the paired board
  (`path: "live"`, or no path).

Every change an agent makes to the lesson is **one undo step**, tagged with
the agent's name (`rupok-agent/1 {"agent": …, "op": …}`). The board refuses
everything but reads when the teacher turns off **Allow AI to edit live**,
refuses lesson changes, settings and mode changes in **Student view**, and
in the in-app chat the agent asks permission for edits, settings and mode
changes alike (Auto-approve covers them the same way).

## Summary

| Area | Before | Now |
|---|---|---|
| Pages | ◐ add, go to, background, design | ✅ list, insert, delete, reorder, duplicate, rename, navigate, design |
| Ink | ◐ handwriting solutions only | ✅ draw, erase (delete), restyle, recolour, scale, pen styles, highlighter |
| Shapes and tables | ✅ add (live) | ✅ add, fill, dash, restyle, scale |
| Text | ◐ add | ✅ add, edit, style, ink to text |
| Stamps | ✗ | ✅ built-in and custom stamps, Smart stamps, celebrations |
| Images, PDF, media, Smart Objects | ◐ | ◐ media and objects ✅; PDF import and pictures 🔒 (files from the teacher's computer) |
| Selection | ◐ read | ✅ select, group, move, align, distribute, reorder, delete |
| Undo | ✗ | ✅ the agent's own last change; teacher's history 🔒 |
| Modes, view, replay | ◐ go to page, point | ✅ Prepare/Teach/Student, zoom, fit, replay, tool |
| Library, export, save | ◐ workspace files | ✅ list, open, create, move, rename, trash, export PDF |
| Settings | ◐ theme | ✅ allowlisted settings (56), dock layout; secrets 🔒 |
| Smart Dock | ✗ | ✅ position, groups, split dock, sizes, named layouts |
| Solve this, To text, maths, voice | ◐ | ✅ write handwriting, ink to text, Voice on the board (start, stop); recognition and dictation 🔒 (local input) |
| Cloud and payments | ✗ | 🔒 account, sharing links, payments |

## Pages

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| List pages | ◐ outline | ✅ | `list_pages` | ✅ | ✅ |
| Add / insert a page | ✅ | ✅ | `add_page` (index), `add_page_after_current` | ✅ / – | ✅ |
| Delete pages | ✗ | ✅ | `delete_pages` | ✅ | ✅ |
| Reorder pages | ✗ | ✅ | `move_pages` | ✅ | ✅ |
| Duplicate pages | ✗ | ✅ | `duplicate_pages` | ✅ | ✅ |
| Rename a page | ✗ | ✅ | `set_page_title` | ✅ | ✅ |
| Navigate | ✅ | ✅ | `go_to_page` | – | ✅ |
| Colour and pattern | ✅ | ✅ | `set_page_background`, `set_board_theme` | ✅ | ✅ |
| Page design, branding | ✅ | ✅ | `set_page_design`, `set_branding` | ✅ | ✅ |
| Clear a page | ✗ | ✅ | `delete_items` with every id from `get_current_page` | ✅ | ✅ |
| Save my design as the default for new lessons | ✗ | 🔒 | Teacher only: it changes every future lesson on this device. | | |

## Ink, pens and erasing

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Draw ink (pen, highlighter, any pen style) | ✗ | ✅ | `draw_stroke` (points, colour, width, brush, opacity, tool) | ✅ | ✅ |
| Write a solution in handwriting | ✅ | ✅ | `write_handwriting` | – | ✅ |
| Erase strokes | ✗ | ✅ | `delete_items` (whole strokes) | ✅ | ✅ |
| Partial erasing, the palm duster | ✗ | 🔒 | Teacher only: physical input (a rub on the board); agents delete whole strokes. | | |
| Recolour, re-thicken, change the pen style | ✗ | ✅ | `restyle_items` (color, width, brush, opacity) | ✅ | ✅ |
| Scale | ✗ | ✅ | `scale_items` | ✅ | ✅ |
| The teacher's current tool, colour, width, style | ✗ | ✅ | `set_tool` | – | ✅ |
| Smart ink snapping while drawing | ✗ | ◐ | Setting `pen.smartShapes` via `set_setting`; the snap itself needs a pen. | | |
| Laser pointer | ◐ | ✅ | `point_at` (ring or box, with a label) | – | ✅ |

## Shapes, tables, text

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Shapes, lines, arrows, diagrams | ✅ | ✅ | `add_shape` | – | ✅ |
| Tables | ✅ | ✅ | `add_table` | – | ✅ |
| Shape fill, dashed outline | ◐ at creation | ✅ | `restyle_items` (fill, dashed) | ✅ | ✅ |
| Add text | ✅ | ✅ | `add_text` | ✅ | ✅ |
| Edit text, size, colour, position | ✗ | ✅ | `update_text`, `restyle_items` (text_size, color) | ✅ | ✅ |
| Ink to text | ✅ | ✅ | `replace_items_with_text`, `replace_strokes_with_text` | ✅ | ✅ |
| Maths (LaTeX) text | ◐ | ◐ | `write_handwriting` with `$…$`; typeset LaTeX text boxes are made by the recogniser (below). | | |

## Stamps and celebrations

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Place a built-in stamp (star, tick, badge, emoji…) | ✗ | ✅ | `add_stamp` | ✅ | ✅ |
| List stamps | ✗ | ✅ | `list_stamps` (built-in, lesson, My stamps) | ✅ | ✅ |
| Make a custom stamp (SVG, emoji, label; behaviour) | ✗ | ✅ | `create_stamp` | ✅ | ✅ |
| Make a Smart stamp (agent-written logic, sandboxed) | ✗ | ✅ | `create_stamp` with `art: "smart"`, `html` | ✅ | ✅ |
| Change, delete a custom stamp | ✗ | ✅ | `update_stamp`, `delete_stamp` | ✅ | ✅ |
| Move, resize, turn, delete a placed stamp | ✗ | ✅ | `move_items`, `scale_items`, `delete_items` | ✅ | ✅ |
| Celebrate | ✗ | ✅ | `celebrate` (stars, confetti, hearts) | – | ✅ |
| The quick-menu favourites | ✗ | 🔒 | Teacher only: a personal picker; agents place stamps directly. | | |
| Import / export a stamp pack | ✗ | 🔒 | Teacher only: picks a file on the teacher's computer. Agents make stamps with `create_stamp`. | | |

## Images, PDF, media, Smart Objects

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| YouTube, trusted embeds, video and audio files | ✅ | ✅ | `add_youtube`, `add_media` | ✅ | ✅ |
| 3D models | ✅ | ✅ | `add_3d_model` (`add_model`) | ✅ | ✅ |
| Smart Objects: add, update, step | ✅ | ✅ | `add_smart_object`, `update_smart_object`, `step_object` | ✅ | ✅ / ✅ / live |
| Smart Object presets | ◐ | ◐ | Agents write their own objects (`read_object_spec`). | | |
| Insert a picture, import a PDF | ✗ | 🔒 | Teacher only: files from the teacher's computer or clipboard. (Agents draw diagrams as SVG Smart Objects or stamps.) | | |
| Move, resize, delete media and objects | ◐ delete | ✅ | `move_items`, `scale_items`, `delete_items` | ✅ | ✅ |

## Selection, arranging, undo

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Read the selection | ✅ | ✅ | `get_selection` | – | ✅ |
| Select items | ✗ | ✅ | `select_items` | – | ✅ |
| Group, ungroup | ✗ | ✅ | `group_items`, `ungroup_items` | ✅ | ✅ |
| Move | ✗ | ✅ | `move_items` | ✅ | ✅ |
| Align, distribute | ✗ | ✅ | `align_items` (also new for teachers' agents; no button yet) | ✅ | ✅ |
| Bring to front, send to back | ✗ | ✅ | `bring_to_front`, `send_to_back` (`reorder_items`) | ✅ | ✅ |
| Delete | ◐ one | ✅ | `delete_items`, `delete_item` | ✅ | ✅ |
| Copy and paste, duplicate items | ✗ | ◐ | Agents read items and add them again; the clipboard is the teacher's. | | |
| Undo | ✗ | ✅ | `undo_last_change` (`undo`): only the agent's own newest change | – | ✅ |
| Undo / redo the teacher's history | ✗ | 🔒 | Teacher only: an agent must never take back the teacher's own work. | | |

## Modes, view, presenting

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Prepare, Teach, Student view | ✗ | ✅ | `set_mode` (leaving Student view is refused) | – | ✅ |
| Zoom, fit | ✗ | ✅ | `zoom_to`, `fit_page` | – | ✅ |
| Replay a page | ✗ | ✅ | `start_replay`, `stop_replay` | – | ✅ |
| Theme (light, dark) | ✅ | ✅ | `set_board_theme` (ui), `set_setting` | – | ✅ |
| Fullscreen, the overlay over other apps | ✗ | 🔒 | Teacher only: the browser and the OS allow it only from a user gesture. | | |
| Pages drawer | ✗ | ✅ | `set_setting` `appearance.pagesPanel`, `appearance.pagesPinned` | – | ✅ |

## Library, export, save, sharing

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Workspace lessons (list, create, read) | ✅ | ✅ | `list_lessons`, `create_lesson`, `get_lesson_outline` | ✅ | – |
| The board's Library: list | ✗ | ✅ | `library_list` | – | ✅ |
| Open a lesson | ✗ | ✅ | `library_open` | – | ✅ |
| Create a lesson | ✗ | ✅ | `library_create_lesson` | – | ✅ |
| Move, rename | ✗ | ✅ | `library_move`, `library_rename` | – | ✅ |
| Move to the trash | ✗ | ✅ | `library_trash` (restorable) | – | ✅ |
| Delete forever, empty the trash | ✗ | 🔒 | Teacher only: cannot be undone. | | |
| Export PDF | ✗ | ✅ | `export_pdf` (saved in the workspace; `download` also on the board) | – | ✅ |
| Export `.rpk` | ◐ | ✅ | Lesson files in the workspace are `.rpk`; live lessons save themselves (autosave). | ✅ | ✅ |
| Choose the storage folder or browser storage | ✗ | 🔒 | Teacher only: storage location and file-system permission. | | |
| Share links (Rupok Cloud), QR codes | ✗ | 🔒 | Teacher only: the teacher's cloud account. | | |
| Plans, payments, receipts | ✗ | 🔒 | Teacher only: account and payment actions. | | |

## Solve this, To text, maths, voice

| Feature | Before | Now | How | File | Live |
|---|---|---|---|---|---|
| Solve this | ✅ | ✅ | `write_handwriting` (the agent is the solver) | – | ✅ |
| Show the AI working (a live card where the result will land) | ✗ | ✅ | `show_progress` | – | ✅ |
| To text (ink to text) | ✅ | ✅ | `replace_items_with_text` | ✅ | ✅ |
| Handwriting maths recognition | ✗ | 🔒 | Teacher only: it reads the pen as it writes; agents read handwriting with `get_page_snapshot`. | | |
| Teach Rupok my handwriting | ✗ | 🔒 | Teacher only: needs the teacher's own writing (physical input). | | |
| Voice dictation | ✗ | 🔒 | Teacher only: the microphone. Language and style are settings (below). | | |
| Voice on the board (speech becomes text or an AI question) | ✗ | ✅ | `start_voice_text`, `stop_voice_text` (the board's own microphone, which the teacher allows) | – | ✅ |

## Live operations

Every operation of the live protocol (`crates/rupok-live/src/op.rs`), by
where it runs. **Document** operations run in `rupok_live::apply` on the
board and on files; **board** operations run in the board app
(`apps/board/src/agentControl.ts`, `live.ts`, `snapshot.ts`).

| Operation | Runs in | Access |
|---|---|---|
| `get_page`, `get_outline`, `get_selection` | document | read |
| `snapshot` | board | read |
| `go_to_page`, `point_at` | board / document | control |
| `show_progress` | board (`aiAction/`, the working card; not saved) | control |
| `set_board_theme` | document + board | control or write |
| `add_page`, `add_page_after_current`, `add_text`, `add_smart_object`, `update_smart_object`, `add_media`, `add_model`, `delete_item`, `set_page_background`, `set_page_design`, `set_branding`, `replace_items_with_text`, `write_handwriting`, `step_object` | document | write |
| `add_shape`, `add_table` | board engine | write |
| `list_pages` | document | read |
| `move_pages`, `delete_pages`, `duplicate_pages`, `set_page_title` | document | write |
| `select_items` | document (the board selects) | control |
| `move_items`, `align_items`, `restyle_items`, `scale_items`, `group_items`, `ungroup_items`, `reorder_items`, `delete_items`, `draw_stroke`, `update_text` | document | write |
| `list_stamps` | document + board library | read |
| `create_stamp`, `update_stamp`, `delete_stamp`, `add_stamp` | document + board library | write |
| `set_mode`, `zoom_to`, `fit_page`, `start_replay`, `stop_replay`, `celebrate`, `set_tool`, `export_pdf`, `library_open` | board | control |
| `undo` | board | write |
| `start_voice_text`, `stop_voice_text` | board (`voice/board.ts`, the microphone) | write |
| `library_list`, `get_settings`, `get_dock_layout`, `list_dock_items` | board | read |
| `library_create_lesson`, `library_move`, `library_rename`, `library_trash`, `set_setting`, `set_dock_layout`, `save_dock_preset`, `apply_dock_preset` | board | write |

## Settings

Every entry of the settings registry (`apps/board/src/settings/defs/`),
except the read-only keyboard shortcut list (`keyboard.*`). *agent*: an
agent reads it with `get_settings` and changes it with `set_setting`;
the AI chip's Undo puts it back.

| Setting | Who | How, or why teacher only |
|---|---|---|
| `appearance.theme` | agent | `set_setting` |
| `appearance.accent` | agent | `set_setting` |
| `appearance.density` | agent | `set_setting` |
| `appearance.dockSide` | agent | `set_setting` |
| `appearance.iconStyle` | agent | `set_setting` |
| `appearance.pagesPanel` | agent | `set_setting` |
| `appearance.pagesPinned` | agent | `set_setting` |
| `appearance.pageDesign` | agent | `set_page_design`, `set_branding` (this entry only opens the sheet) |
| `appearance.pageDesignDefault` | teacher only | Changes every future lesson on this device; agents design each lesson. |
| `pen.style` | agent | `set_setting`, or `set_tool` with `brush` |
| `pen.markerOpacity` | agent | `set_setting` |
| `pen.pressure` | agent | `set_setting` |
| `pen.smoothing` | agent | `set_setting` |
| `pen.hand` | agent | `set_setting` |
| `pen.eraserMode` | agent | `set_setting` |
| `pen.keepInkOnPage` | agent | `set_setting` |
| `pen.smartShapes` | agent | `set_setting` |
| `pen.scratchOut` | agent | `set_setting` |
| `pen.stampQuickMenu` | agent | `set_setting` |
| `pen.stampSounds` | agent | `set_setting` |
| `pen.stampFavorites` | teacher only | A personal picker; agents place stamps with `add_stamp`. |
| `pen.teachHandwriting` | teacher only | Needs the teacher's own handwriting (physical input). |
| `modes.shapeAfterTeach` | agent | `set_setting` |
| `modes.shapeAfterPrepare` | agent | `set_setting` |
| `modes.editor` | agent | `get_dock_layout`, `set_dock_layout` (this entry is the editor section) |
| `modes.dockReset` | agent | `apply_dock_preset` with `Default` |
| `touch.fingers` | agent | `set_setting` |
| `touch.palmEraser` | agent | `set_setting` |
| `touch.palmSize` | agent | `set_setting` |
| `touch.eraserScale` | agent | `set_setting` |
| `touch.handCluster` | agent | `set_setting` |
| `touch.thumbScrub` | agent | `set_setting` |
| `touch.swipe` | agent | `set_setting` |
| `touch.swipeFingers` | agent | `set_setting` |
| `touch.flick` | agent | `set_setting` |
| `touch.swipeSensitivity` | agent | `set_setting` |
| `touch.tapUndo` | agent | `set_setting` |
| `touch.tapRedo` | agent | `set_setting` |
| `touch.doubleTapFit` | agent | `set_setting` |
| `touch.longPressDock` | agent | `set_setting` |
| `touch.largeTargets` | agent | `set_setting` |
| `touch.hints` | agent | `set_setting` |
| `touch.calibrate` | teacher only | Needs the teacher's hand on the board (physical input). |
| `touch.showHintsAgain` | teacher only | Tips for the person at the board, not for an agent. |
| `files.pdfLayout` | agent | `set_setting` |
| `files.pdfInk` | agent | `set_setting` |
| `files.pdfPrintFriendly` | agent | `set_setting` |
| `files.exportPdf` | agent | `export_pdf` (this entry only opens the dialog) |
| `files.quizAnswers` | teacher only | Students' answers: privacy. |
| `files.newLesson` | agent | `library_create_lesson` (this entry replaces the board) |
| `general.role` | teacher only | Who uses this device (teacher or student) is the person's choice. |
| `general.author` | teacher only | Personal data (the teacher's name). |
| `general.library` | teacher only | Storage location and file-system permission. |
| `general.saveLocation` | teacher only | Storage location (this device or the cloud account). |
| `general.cloudAccount` | teacher only | The cloud account (sign in, sign out). |
| `general.cloudUsage` | teacher only | The cloud account. |
| `general.cloudPlan` | teacher only | Plans and payments. |
| `general.cloudReceipts` | teacher only | Payments. |
| `general.voiceInput` | teacher only | The microphone: a permission. |
| `general.voiceLang` | agent | `set_setting` |
| `general.voiceProvider` | teacher only | Privacy: where the teacher's voice is sent. |
| `general.voicePrivacy` | teacher only | Privacy: what happens to recordings. |
| `general.voiceModel` | teacher only | Downloads a large speech model to this device. |
| `general.voicePunctuation` | agent | `set_setting` |
| `general.voiceMode` | agent | `set_setting` |
| `general.voiceBoardMode` | agent | `set_setting` |
| `general.voiceBoardTalk` | agent | `set_setting` |
| `general.voiceBoardStyle` | agent | `set_setting` |
| `general.voiceBoardPause` | agent | `set_setting` |
| `general.voiceBoardStudent` | teacher only | Permission: whether students may ask the AI by voice. |
| `ai.connect` | teacher only | Security: pairing with an agent. |
| `ai.allowEdit` | teacher only | Permission: whether the AI may edit live. |
| `ai.handwriting` | teacher only | Privacy and AI keys: which service reads handwriting. |
| `ai.claudeKey` | teacher only | An AI key. |
| `ai.chatOpen` | teacher only | Opens the chat for the person; the agent is already in it. |
| `ai.chatAgent` | teacher only | Security: which program runs on the teacher's computer. |
| `ai.chatPermissions` | teacher only | Permission: whether board edits need approval. |
| `ai.chatAllowNoPrompt` | teacher only | Permission. |
| `ai.chatAttach` | agent | `set_setting` |
| `ai.chatPosition` | agent | `set_setting` |
| `ai.solveStyle` | agent | `set_setting` |
| `ai.solveCalibrate` | teacher only | Needs the teacher's own handwriting (physical input). |
| `ai.solveSpeed` | agent | `set_setting` |
| `ai.solvePreview` | teacher only | The teacher's check on AI output: an agent never turns off its own review. |
| `ai.solveTeachStep` | agent | `set_setting` |
| `ai.solveStudents` | teacher only | Permission: whether students may use the AI. |
| `ai.classMode` | teacher only | Permission: which AI edits go through without asking. |
| `ai.preapprove` | teacher only | Permission: pre-approving tools in the teacher's own agent settings. |
| `ai.progress` | agent | `set_setting` |
| `ai.celebrate` | agent | `set_setting` |
| `ai.sound` | agent | `set_setting` |
| `ai.streamSolve` | agent | `set_setting` |
| `ai.studentsAsk` | teacher only | Permission: whether students may ask the AI. |
| `ai.studentLimit` | teacher only | Permission: how much students may ask the AI. |
| `about.info` | teacher only | Read-only information. |
| `about.updates` | teacher only | Installing app updates. |
| `about.tour` | teacher only | A walkthrough for the person at the board. |
| `about.support` | teacher only | Donations and payments. |
| `about.renderer` | teacher only | Needs a restart and can leave the board blank on some hardware. |
| `about.stats` | agent | `set_setting` |
| `about.speedTest` | teacher only | Takes over the board for a few minutes and needs the teacher to write on it. |
| `about.renderScale` | agent | `set_setting` |
| `about.wetLayer` | agent | `set_setting` |
| `about.inkTrail` | agent | `set_setting` |
| `about.flatPanels` | agent | `set_setting` |
| `about.nativeWetInk` | teacher only | An experimental native layer of the Windows app: a failure needs someone at the board. |
| `about.graphics` | teacher only | Read-only information. |

## Dock buttons

Everything that can sit on the Smart Dock (`modes.ts → TOOLBAR_ITEMS`, plus
the Shapes button). Agents place any of them with `set_dock_layout`
(`list_dock_items` lists them); the teacher does it in **Customize dock**.
What each button does is in the tables above.

| Button | Agent path for what it does |
|---|---|
| `pen` | `set_tool`, `draw_stroke` |
| `highlighter` | `set_tool`, `draw_stroke` (tool highlighter) |
| `eraser` | `delete_items` |
| `lasso` | `select_items` |
| `laser` | `point_at` |
| `text` | `add_text`, `update_text` |
| `voice` | `start_voice_text`, `stop_voice_text` |
| `shape` | `add_shape`, `add_table` |
| `stamp` | `add_stamp`, `create_stamp` |
| `rect` | `add_shape` |
| `ellipse` | `add_shape` |
| `line` | `add_shape` |
| `arrow` | `add_shape` |
| `hand` | `zoom_to`, `fit_page` |
| `interact` | `step_object` |
| `smartink` | `set_setting` `pen.smartShapes` |
| `undo` | `undo_last_change` (the agent's own) |
| `redo` | teacher only: the teacher's history |
| `replay` | `start_replay`, `stop_replay` |
| `pager` | `go_to_page` |
| `exit` | `set_mode` |
| `celebrate` | `celebrate` |
| `pages` | `list_pages`, `set_setting` `appearance.pagesPinned` |
| `solve` | `write_handwriting` |
| `settings` | `get_settings`, `set_setting` |
| `shapes` | `add_shape`, `add_table` |

## Adding a feature

A new user-facing feature ships with its agent path in the same pull
request (AGENTS.md, rule 8): a live op and an MCP tool (file and live where
it makes sense), docs in [ai-agents.md](ai-agents.md) and the
`rupok-live-board` skill, tests, and a row here. When an agent must not do
it, add the row as *teacher only* with the reason.
