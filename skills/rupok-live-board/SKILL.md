---
name: rupok-live-board
description: Work on the Rupok board a teacher has open, live, during a class: read the current page and the teacher's handwriting, add quizzes, practice problems, Smart Object simulations and clean diagrams, convert ink to text, translate a slide, step through objects and point at things, rearrange pages and items, restyle ink, make and place custom stamps (including Smart stamps with logic), switch modes, export PDFs, manage the Library, change settings and customise the Smart Dock. Use whenever the teacher talks about "the board", "this page", "what I just wrote", "this slide", gives a Rupok pairing code, or asks for something to appear in Rupok while teaching.
---

# Live on the Rupok board

The teacher has Rupok open (browser or desktop) and is teaching. With the
`rupok` MCP server running with `--live`, you can see the open lesson and
change it; the class sees every change at once. Treat it like writing on
someone else's whiteboard in front of their students: be quick, be tidy, and
never wreck their work.

For writing Smart Object HTML, follow the `rupok-smart-objects` skill
(`read_object_spec` first, validate, fix, place). This skill is about working
live.

## Connect

1. `get_board_status`. If `paired` is false, relay its `hint`: the teacher
   clicks **Connect AI** at the top right of the board and reads you a
   6-digit code.
2. `connect_board(code)` with the code they give you ("482 913" is fine).
   Never guess codes. If it fails, say so and ask them to read it again.
3. Once paired, the lesson tools take `path: "live"`; the live tools
   (`get_current_page`, `get_page_snapshot`, …) need no path.

If a call says the teacher turned off **Allow AI to edit live**, you may still
read the board; ask before editing. If the board is in Student view, editing
is refused: ask the teacher to switch to Prepare or Teach.

## In Rupok's chat panel

When the teacher chats with you from Rupok's own chat panel (`rupok agent`
or the desktop app started you), you are already paired: skip `connect_board`.
Messages often carry the page as a `rupok://board/page/N` resource (the
`get_current_page` JSON) and sometimes a picture of it; "this" means that
page, or the selection when the URI ends in `/selection`. Keep replies short:
the teacher reads them on a projector panel, mid-class. Say what you changed
in one line; the panel already shows a card per board change.
Mentions arrive as resources: `rupok://board/page/N`, `…/selection`,
`…/object/<id>` (board JSON, often with a picture), `rupok://library/<name>`
(a lesson's outline) and `file:///<name>` (the teacher's files). `/quiz`,
`/explain`, `/translate`, `/design` and `/solve` reach you as plain requests about the
attached page.

## Act, don't ask

In class, routine requests are yours to just do: a page, text, a diagram, a
Smart Object, a quiz, a solution, a translation. Pick sensible defaults (new
page, readable sizes, themed colours) instead of asking clarifying questions
or "shall I?". Every change is one undo step and the teacher has an Undo
button. Ask first only before deleting or clearing anything, replacing the
teacher's ink, or changing settings or the mode.

## In action: let the class watch

1. **Place first.** For anything slower than a few seconds, call
   `show_progress` with the box where the result will go (`x, y, w, h`,
   `page`) and its `kind` (`object`, `handwriting`, `text`, `shape`). A
   full-frame object: `x: 320, y: 180, w: 1280, h: 720`.
2. **Stream.** Update the same card (`id`) with your `step` ("Laying out the
   atoms…", "Testing…"), a short `plan` and `percent`; send `preview_html`
   (the partial Smart Object) every few seconds while you build.
3. **Land.** Place the result in exactly that box: the card turns into it
   with a short animation. If it fails validation, fix it and place it again
   (the card says it is fixing); `state: "failed"` with a friendly `error` if
   you give up.
4. **Size.** Text 32–48 px in short lines (Rupok wraps, shrinks and keeps all
   text inside the page anyway); titles 56–72 px; objects inside the frame.
5. **Solutions line by line.** Call `write_handwriting` once per line with
   `anchor: {"group": <the group the first call returned>}`, or, in Rupok's
   chat when the Solve this prompt asks for it, write the steps in a
   ```` ```board ```` block: each line appears on the board as you finish it.
6. **Celebrate briefly.** The board adds a small sparkle when a result lands;
   say what you made in one line, no fanfare.

## Look before you act

- `get_current_page`: every item with its id, kind, geometry, text and
  `age_s` (seconds since it was made). "What I just wrote" is the strokes with
  the smallest `age_s`. Also returns the selection and the page `scheme`
  (light or dark).
- `get_selection` when they say "this", "these", "the selected".
- `get_page_snapshot` to *see* the page: handwriting, sketches, layout. Pass
  `item_ids` (e.g. the newest strokes) to zoom in on handwriting. Check your
  own result with a snapshot after a big change.

## Classroom rules

1. **New content goes on a new page** by default:
   `add_page_after_current`, fill it with `path: "live"` and the returned
   page number, then `go_to_page` when it is ready (the class should not watch
   half-built slides). Change the current page only when asked ("add it
   here", "fix this slide").
2. **Confirm before anything destructive**: deleting items or pages,
   clearing a page, replacing the teacher's ink (`replace_items_with_text`),
   changing settings or the mode. One short question: "Replace your
   handwriting with typed text?" Then act. Everything else: just do it (each
   change is one undo step).
3. **Keep it themed.** Match the page `scheme`: on chalkboard or blackboard
   pages use light text (the default text colour already adapts) and Smart
   Objects that use the theme CSS variables with fallbacks. New pages inherit
   the current page's look.
4. **Classroom legibility.** Text ≥ 32px (titles 56–72px), short lines, high
   contrast, leave room for the teacher's pen. Keep objects inside the
   1920 × 1080 frame.
5. **Small, fast steps.** Each call is one undo step for the teacher. Prefer a
   few clear additions over one giant change; report what you did in one line
   ("Added 3 practice problems on page 4, answers on Next").
6. **Don't navigate away uninvited.** `go_to_page`, `point_at` and
   `set_board_theme` change what the class sees: use them when asked or when
   presenting something you built.
7. **Answers behind steps.** For quizzes and problems, hide answers behind
   Smart Object build steps so the teacher reveals them with Next
   (`step_object` if they ask you to).

## Solve this (handwriting solutions)

The teacher lassos a problem and taps **Solve this** (or asks "solve this").
Rupok's prompt gives you a picture of the problem, its item ids and what its
offline recogniser read (n-best LaTeX). Then:

1. **Read and solve.** Trust the picture over the recogniser's guesses. Work
   the problem out fully, then **verify every step and all arithmetic**
   (substitute the answer back) before you write anything.
2. **Write it** with one `write_handwriting(lines, anchor: {item_ids})` call.
   The board writes the lines below the problem in handwriting, animated, in
   the problem's ink colour and size (the teacher's own hand if they taught
   it), as one undo step.
   - One short step per line, under ~30 characters: `"Subtract 3 from both
     sides:"`, `"$2x = 4$"`, `"$x = \frac{4}{2} = 2$"`. Maths in `$…$`;
     fractions, powers, subscripts, `\sqrt`, Greek and the usual operators
     are written properly.
   - Don't copy the problem out again; end with the answer on its own line.
   - Match the class level: the method a student at that level would use.
3. **Preview.** If the result says `"status": "preview"`, the teacher is
   checking it before the class sees it. **Never** call `write_handwriting`
   again or write the solution any other way; explain in chat and wait. If
   they ask for a change, write the corrected lines once more.
4. **Explain** the steps in chat in 2–4 short sentences. You may `point_at`
   the part of the problem you are talking about.
5. In Teach the teacher may reveal one line per Next press; you don't need
   to do anything for that.

## Recipes

- **Quiz from what I wrote**: `get_current_page` → `get_page_snapshot` with
  the newest strokes → write questions → `add_page_after_current` →
  `add_smart_object(path:"live", page:N, steps:K)` → `go_to_page(N)`.
- **Simulation for a student's question**: plan in one sentence, build the
  object (validate first), place it full-frame on a new page, show it,
  `point_at` the control to try first.
- **Sketch to clean diagram**: snapshot the sketch's strokes, rebuild it as an
  SVG Smart Object or text, place it next to (not over) the sketch, then offer
  to remove the sketch.
- **Ink to text**: snapshot the strokes, read them, confirm, then
  `replace_items_with_text(item_ids, text)` (one undo restores the ink). Use
  Unicode maths (x², √, π, ½) in plain text.
- **Translate this slide to Bangla**: read texts from `get_current_page`; put
  the translation on a new page with the same layout (`add_text` at the same
  coordinates), unless asked to replace in place.
- **Diagram from shapes**: a flowchart is `add_shape` calls (`terminator`,
  `process`, `decision`, `data`, each with `text`) joined by `arrow`s; a
  sorting activity is a `venn2` / `venn3`; a comparison is `add_table` with
  `cells` and a `header_fill`. Leave 40px or more between shapes, keep text
  ≥ 24px, and use soft fills (`#rrggbb33`) so ink stays readable on top.
- **Harder practice problems**: read the example, make 3 graded problems on a
  new page, answers revealed on Next.

## Page design

Teachers ask for a look: "a chalkboard background with our school logo and a
physics theme for Class 9". Use `set_page_design` and `set_branding` (both
take `path: "live"` or a lesson file; each call is one undo step).

1. **Read first.** Call each tool with only `path` to get the current design
   (colour, pattern, background, branding).
2. **Background** (`set_page_design`): a colour (`white`, `paper`,
   `chalkboard`, `blackboard`, `#rrggbb`), a pattern, and an image drawn as
   SVG for the 1920×1080 page (`viewBox="0 0 1920 1080"`, default fit
   `stretch`) or a PNG/JPEG as base64. `all_pages: true` gives every page the
   look; ask first unless the teacher said "all".
3. **Branding** (`set_branding`): a logo (`logo_svg` or `logo_image`) in a
   `logo_corner`, and a header or footer line (`school`, `teacher`, `class`,
   `date`, `text_position`). Text can fill itself in: `{lesson}`, `{page}`,
   `{pages}`, `{date}`, `{teacher}`, `{school}`, `{subject}`, `{course}`,
   `{chapter}`, so one design fits every lesson ("Class 9 · {subject} ·
   {lesson}", "{page} / {pages}"). `scope: "page"` styles one page; `page`
   with `hidden: true` keeps a title slide clean.

House rules (projectors in bright rooms, chalk-dusted fingers):

- **Ink must stay readable.** On a dark page the teacher writes in white or
  yellow; on a light page in dark colours. Keep the background's contrast
  low: big soft shapes, at most ~8 % luminance difference in the middle.
- **Keep the content area calm.** Decoration (a theme motif, a border, a
  formula watermark) goes to the edges and corners, never behind the middle
  of the slide. No text in the background image; text goes in the branding
  line.
- **Logo small, in a corner**: `logo_width` 0.08–0.12, away from the text
  line. Opacity 0.8–1.
- **Photos need dimming**: `background_dim: true` or `background_opacity`
  0.3–0.5 so ink shows over them.
- SVG is checked: no scripts, event handlers, external references
  (`href` only to `#id` or embedded PNG/JPEG data) or animation. A refused
  SVG changes nothing; fix it and call again.

## Everything the teacher can do, you can do

Rupok is agent-first. Besides building content you can run the board for
the teacher (full list: `docs/agent-capabilities.md`; document tools take
`path`, omitted = the board):

- **Pages**: `list_pages`, `move_pages(pages, to)`, `duplicate_pages`,
  `delete_pages` (confirm first), `set_page_title`.
- **Items**: `select_items` (show the teacher what you mean), `move_items`,
  `align_items` (align and distribute; groups move as one), `restyle_items`
  (colour, width, pen style, shape fill, opacity, text size), `scale_items`,
  `group_items` / `ungroup_items`, `bring_to_front` / `send_to_back`,
  `delete_items` (confirm first), `draw_stroke` (underline, circle, tick),
  `update_text`.
- **Presenting**: `set_mode` (prepare, teach, student; never out of Student
  view), `zoom_to` / `fit_page`, `start_replay`, `celebrate`, `set_tool`
  ("give me a red chalk pen"), `undo_last_change` (only your own newest change).
- **Files**: `export_pdf` (saved in the workspace), `library_list`,
  `library_open`, `library_create_lesson`, `library_move`, `library_rename`,
  `library_trash` (ask first).
- **Settings**: `get_settings`, then `set_setting(key, value)`. Only the
  listed keys; AI keys, accounts, payments, storage and permissions are the
  teacher's alone, so don't try.

Settings, dock and mode changes ask the teacher's permission like edits;
the AI chip's Undo puts a settings change back. Say what you changed in one line.

## Stamps

"Put a star on that answer": `add_stamp(stamp: "star", x, y)` (built-in:
star, tick, cross, heart, thumbs-up, hundred, trophy, excellent, good-job,
try-again, important, question, badge (numbers itself), arrows, `emoji`
with `text`). `list_stamps` shows the teacher's own too.

"Make me a stamp…" (`/stamp` in Rupok's chat): `create_stamp`, then place
one so they see it.

- Artwork: bold, simple SVG (viewBox ~100×100, `fill="currentColor"`, no
  text inside, no scripts or SMIL: refused), or an emoji, or a 1–3 word
  label (`art: "text"`).
- Behaviour: `animation` pop/sparkle/bounce/spin/fade-in, `burst`
  stars/confetti/hearts/sparkles, `counter`, `sound`
  chime/pop/ding/tada/clap/whoosh, `fade_after` seconds. Celebratory but
  brief: one animation, at most one burst and one short sound.
- Logic (a dice that rolls on tap, a timer, a tap counter): `art: "smart"`
  with `html`, a tiny Smart Object (follow `rupok-smart-objects`; the whole
  body is the button; offline; `rupok.state` to remember counts). It runs
  in the Smart Object sandbox, never elsewhere.
- Readable from the back row at 96 px; `update_stamp` changes only the given
  fields; `delete_stamp` only when asked.

## The Smart Dock

"Ask Claude: put pen, highlighter and eraser on the left, undo and next
page on the right, bigger buttons." (`/dock` in Rupok's chat):

1. `list_dock_items` and `get_dock_layout(mode)` (teach or prepare; both
   when the teacher didn't say).
2. `set_dock_layout(mode, layout: {position: "left", size: "l", groups:
   [{items: ["pen", "highlighter", "eraser"]}], split: {position: "right",
   groups: [{items: ["undo"]}, {items: ["pager"]}]}})`. Groups are separated
   on the dock; `split` is a second dock; `sizes` sets one button; `large`
   is large icons for low vision.
3. Pen, highlighter, eraser, laser and undo always stay. Relay any
   `warnings`. Offer `save_dock_preset(name)`; `apply_dock_preset` switches
   to Classic, Left-handed, Minimal teaching, Maths class, Default or a saved one.

## Layouts: Modern and Classic

Rupok has two layouts. **Modern**: a top bar and the Smart Dock. **Classic**:
no top bar, big buttons in three islands along the bottom (☰ menu, mode,
tools, share, zoom · select, pen, eraser, shapes, pan, recognise ink,
stamps, undo, redo · new page, back, page button, next), familiar to
teachers coming from classroom whiteboard software.

- Switch: `set_setting(key: "appearance.layout", value: "classic")` (or
  `"modern"`, which brings back the teacher's own Modern dock), or
  `apply_dock_preset(name: "Classic")`.
- Classic is a dock style, so the dock tools arrange it: in
  `set_dock_layout`, `style: "islands"` makes each group an island (the
  first at the left, the last at the right, the others in the middle),
  `position: "top"` moves them to the top edge, `size` and `sizes` set the
  button size (`l` = 56 px buttons, 32 px icons), `large` makes icons
  bigger still. Pen, eraser and undo always stay on an island.
- Its own buttons: `menu`, `mode`, `toolbox`, `share`, `zoom`, `addpage`,
  `recognize`, `ai` (Ask AI) (they also fit on a Modern dock).
- "Put the menu on the right": `swap_islands(swapped: true)` (left out, it
  toggles; `false` puts it back on the left).
- "I need to write at the bottom": `move_island(island: "all", edge: "top")`,
  and `edge: "bottom"` to bring them back. One island: `move_island(island:
  "right", place: "top-right")`; fold one: `collapsed: true`; everything
  back: `reset: true`. `get_dock_layout` lists each island's place.

```text
set_dock_layout(mode: "teach", layout: {style: "islands", size: "l", groups: [
  {label: "Menu", items: ["menu", "zoom"]},
  {label: "Tools", items: ["pen", "eraser", "shapes", "recognize", "undo"]},
  {label: "Pages", items: ["addpage", "pager"]}]})
```

## Tools

| Tool | Use |
|---|---|
| `get_board_status`, `connect_board(code)` | Pairing. |
| `get_current_page(page?)`, `get_selection()`, `get_page_snapshot(page?, item_ids?, region?, width?)` | Read and see. |
| `add_page_after_current(title?, pattern?, color?, show?)`, `go_to_page(page)` | Pages. |
| `add_text`, `add_smart_object`, `update_smart_object`, `delete_item`, `set_page_background`, `add_page`, `get_lesson_outline` with `path: "live"` | Edit the open lesson (`add_text` with `w` wraps inside `w` on the board). |
| `add_youtube`, `add_media`, `add_3d_model` with `path: "live"` | A YouTube video or trusted web embed by link, a video/audio file, or a `.glb` 3D model from the workspace (files are sent to the board). |
| `replace_items_with_text(item_ids, text, …)` | Ink to text (confirm first). |
| `step_object(item_id?, step?, delta?)` | Next / back inside a Smart Object. |
| `add_shape(shape, x, y, w, h, color?, fill?, width?, dashed?, text?, page?)` | A clean shape: rect, rrect, square, ellipse, circle, triangle, right-triangle, diamond, pentagon, hexagon, star, parallelogram, trapezoid, line, arrow, double-arrow, dashed-line, curved-arrow, connector, numberline, axes, venn2, venn3, terminator, process, decision, data, callout, brace, brace-down. Lines go from (x, y) to (x + w, y + h). |
| `add_table(x, y, rows, cols, w?, h?, cells?, color?, header_fill?, text_size?, page?)` | A table (1–10 × 1–10) with cell text, grouped. |
| `point_at(x, y, w?, h?, label?, seconds?)` | Show the class where to look. |
| `show_progress(id?, title?, step?, plan?, percent?, state?, kind?, x?, y?, w?, h?, page?, preview_html?)` | A working card where your result will land (see "In action"). |
| `write_handwriting(lines, anchor?, style?, speed?, color?, size?, step_mode?)` | Write a solution in handwriting next to the problem (see Solve this); `anchor.group` continues earlier writing; `style: "joined"` joins letters. |
| `set_board_theme(page_color?, pattern?, ui?)` | Chalkboard, grid, dark mode. |
| `start_voice_text(x?, y?, w?, lang?, mode?, style?)`, `stop_voice_text(cancel?)` | Live only: the teacher speaks and the words appear as text in a box on the page (`lang` `en-US` or `bn-BD`; `mode: "ask"` makes them a chat prompt). Only when the teacher asks; stop puts the words on the board as one text item. |
| `set_page_design(path, page?, all_pages?, color?, pattern?, background_svg?, background_image?, background_fit?, background_opacity?, background_dim?, remove_background?)` | Page colour, pattern and background image (see Page design). |
| `set_branding(path, scope?, page?, logo_svg?, logo_image?, logo_corner?, logo_width?, …, school?, teacher?, class?, date?, text_position?, text_align?, hidden?, remove?)` | Logo and header/footer on every page or one. |
| `list_pages`, `move_pages`, `duplicate_pages`, `delete_pages`, `set_page_title` | Pages (file or live). |
| `select_items`, `move_items`, `align_items`, `restyle_items`, `scale_items`, `group_items`, `ungroup_items`, `bring_to_front`, `send_to_back`, `delete_items`, `draw_stroke`, `update_text` | Items (file or live; `select_items` live). |
| `list_stamps`, `add_stamp`, `create_stamp`, `update_stamp`, `delete_stamp`, `celebrate` | Stamps and celebrations. |
| `set_mode`, `zoom_to`, `fit_page`, `start_replay`, `stop_replay`, `set_tool`, `undo_last_change` | Presenting (live). |
| `export_pdf`, `library_list`, `library_open`, `library_create_lesson`, `library_move`, `library_rename`, `library_trash` | Files and the Library (live). |
| `get_settings`, `set_setting`, `list_dock_items`, `get_dock_layout`, `set_dock_layout`, `save_dock_preset`, `apply_dock_preset` | Settings and the Smart Dock (live). |

Setup for each agent, and the one-line setting that pre-approves Rupok's
tools (Claude Code `permissions.allow: ["mcp__rupok__*"]`, Codex
`default_tools_approval_mode = "approve"`, OpenCode `"rupok_*": "allow"`):
`docs/ai-agents.md` in the Rupok repository.
