---
name: rupok-live-board
description: Work on the Rupok board a teacher has open, live, during a class: read the current page and the teacher's handwriting, add quizzes, practice problems, Smart Object simulations and clean diagrams, convert ink to text, translate a slide, step through objects and point at things. Use whenever the teacher talks about "the board", "this page", "what I just wrote", "this slide", gives a Rupok pairing code, or asks for something to appear in Rupok while teaching.
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
`/explain`, `/translate` and `/solve` reach you as plain requests about the
attached page.

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
2. **Confirm before anything destructive**: deleting items, replacing the
   teacher's ink (`replace_items_with_text`), replacing an object's HTML,
   changing the page colour of an existing page. One short question: "Replace
   your handwriting with typed text?" Then act.
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
- **Harder practice problems**: read the example, make 3 graded problems on a
  new page, answers revealed on Next.

## Tools

| Tool | Use |
|---|---|
| `get_board_status`, `connect_board(code)` | Pairing. |
| `get_current_page(page?)`, `get_selection()`, `get_page_snapshot(page?, item_ids?, region?, width?)` | Read and see. |
| `add_page_after_current(title?, pattern?, color?, show?)`, `go_to_page(page)` | Pages. |
| `add_text`, `add_smart_object`, `update_smart_object`, `delete_item`, `set_page_background`, `add_page`, `get_lesson_outline` with `path: "live"` | Edit the open lesson. |
| `replace_items_with_text(item_ids, text, …)` | Ink to text (confirm first). |
| `step_object(item_id?, step?, delta?)` | Next / back inside a Smart Object. |
| `point_at(x, y, w?, h?, label?, seconds?)` | Show the class where to look. |
| `write_handwriting(lines, anchor?, style?, speed?, color?, size?, step_mode?)` | Write a solution in handwriting next to the problem (see Solve this). |
| `set_board_theme(page_color?, pattern?, ui?)` | Chalkboard, grid, dark mode. |

Setup for each agent: `docs/ai-agents.md` in the Rupok repository.
