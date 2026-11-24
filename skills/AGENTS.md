# Rupok for AI agents (Codex, OpenCode, Cursor, Gemini CLI, …)

Instructions for any coding agent that builds **Rupok lessons and Smart
Objects** for a teacher. Claude Code users can install the skill in
[`rupok-smart-objects/SKILL.md`](rupok-smart-objects/SKILL.md), which is the
long form of this file.

To **read** a `.rpk` someone gives you, follow
[`read-rpk/SKILL.md`](read-rpk/SKILL.md): unzip it and read `content.md`.

## What you are making

- A **lesson** is a `.rpk` file: pages of 1920 × 1080 slides holding ink,
  text, images and Smart Objects.
- A **Smart Object** is one self-contained HTML document that runs live in a
  sandboxed iframe on the classroom board. The teacher writes on top of it with
  a pen, and the presenter's Next button steps through it.
- Specs: `spec/smart-object.md` (or the `read_object_spec` tool) and
  `spec/rupok-format.md`.

## Connect the MCP server

Install the CLI (`cargo install --path crates/rupok-cli` from the Rupok repo,
or a release build). The binary is `rupok`. The server speaks MCP over stdio:

```sh
rupok mcp --workspace ~/Lessons
```

| Agent | Configuration |
|---|---|
| Claude Code | `claude mcp add rupok -- rupok mcp --workspace ~/Lessons` |
| Codex CLI | `~/.codex/config.toml`: `[mcp_servers.rupok]` with `command = "rupok"` and `args = ["mcp", "--workspace", "/home/me/Lessons"]` |
| OpenCode | `opencode.json`: `"mcp": { "rupok": { "type": "local", "command": ["rupok", "mcp", "--workspace", "/home/me/Lessons"] } }` |
| Others | Any MCP client: command `rupok`, args `mcp --workspace <dir>`, transport stdio. |

The agent can only read and write lessons inside the workspace directory.

## Tools

Paths are relative to the workspace; pages are 1-based; coordinates are
slide pixels in the 1920 × 1080 frame.

- `list_lessons()`, `create_lesson(path, title, pages?)`,
  `get_lesson_outline(path)`, `add_page(path, title?, index?, pattern?)`
- `add_smart_object(path, page, html, title, width?, height?, steps?, x?, y?, network_hosts?)`
  validates, then saves; returns `item_id` and `bundle_id`
- `update_smart_object(path, bundle_id, html, steps?, network_hosts?)`
- `add_youtube(path, page, url, title?, start?, end?, muted?, loop?, autoplay?, x?, y?, width?, height?)`,
  `add_media(path, page, url? | file?, …)` (a YouTube link, a page on a trusted
  education site, or an mp4/webm/mp3/m4a/ogg/wav file from the workspace,
  stored in the lesson), `add_3d_model(path, page, file, title?, …)` (a `.glb`;
  becomes a 3D viewer Smart Object that works offline)
- `add_text(path, page, text, x, y, w?, size?, color?)`,
  `delete_item(path, page, item_id)`,
  `replace_strokes_with_text(path, page, stroke_ids, text, size?, color?)`
  (handwriting to text; replay still shows the ink before it became text),
  `set_page_background(path, page, pattern, color?)`
- `validate_smart_object(html, steps?, network_hosts?, width?, height?)`,
  `read_object_spec()`

Without MCP, use the CLI: `rupok new`, `rupok validate file.html --steps N`,
`rupok add-object lesson.rpk file.html --page N --steps N`, `rupok info`.

## Live board (during class)

`rupok mcp --live` (the default) also lets the teacher pair the Rupok board
they have open: they click **Connect AI** and give you a 6-digit code; call
`connect_board(code)` (`get_board_status` says what to do next). Then:

- the lesson tools above take `path: "live"` to edit the open lesson; the
  class sees each change at once and the teacher can undo it with Ctrl+Z;
- `get_current_page`, `get_page_snapshot` (an image: read handwriting) and
  `get_selection` show you the board;
- `add_page_after_current`, `go_to_page`, `step_object`, `point_at`,
  `replace_items_with_text` (ink to text) and `set_board_theme` help you teach.

Rules: put new content on a new page unless asked otherwise, ask before
deleting or replacing the teacher's work, match the page `scheme` (light or
dark), keep text ≥ 32px. Long form: `rupok-live-board/SKILL.md`; setup for
each agent and the protocol: `docs/ai-agents.md`.

## Loop

1. `read_object_spec` once.
2. Plan: one idea per page; tell the teacher the plan.
3. Write the HTML. Then `validate_smart_object`, fix every error, and
   repeat until valid.
4. `add_smart_object` (it validates again; on errors nothing is saved).
5. Add titles with `add_text` (size ≥ 32). Report what you built.

## Rules for object HTML

- Everything inline. **No network**: no CDNs, remote fonts or images. Use
  `rupoklib://<name>/<file>` for `three`, `d3`, `katex`, `mermaid`, `chart.js`,
  `gsap`, `p5`, `matter-js`, `animejs`, `tailwind`, `lucide`.
- No `localStorage`, cookies, `alert()`, popups or `window.top`. Persist with
  `rupok.state.save(value)` after `rupok.ready`.
- The host injects `window.rupok`. Steps:
  `rupok.steps.setCount(n); rupok.steps.onChange(render);`, where
  `render(step)` draws step `step` from scratch (`0` = initial, `n` = fully
  revealed). Pass the same `steps` to `add_smart_object`.
- Theme: CSS variables like `var(--rupok-color-accent, #2563eb)` (always with a
  fallback).
- Fill the frame at any size. Tie `rem` to the viewport:
  `html { font-size: min(calc(100vw * 16 / W), calc(100vh * 16 / H)); }`.
- Projector and touch board: text ≥ 24px at nominal size, contrast ≥ 4.5:1,
  tap targets ≥ 64px, no hover-only interactions, leave empty space for the
  teacher's pen.
- Write JS comments as `/* … */` (a validator bug can trip on `//` after
  non-ASCII text).

Complete, validated examples: `examples/objects/`.
