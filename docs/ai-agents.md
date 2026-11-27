# Your AI on the board: the live agent bridge

Rupok lets a teacher's **own** AI agent (Claude Code, Claude Desktop, Codex,
OpenCode, or any MCP client) see the lesson that is open on the board and
change it **live**, during class:

> "Make a quiz from what I just wrote on this page."
> "Build a simulation for the question a student just asked."
> "Turn my sketch into a clean diagram."
> "Translate this slide to Bangla."
> "Make 3 harder practice problems like this one on the next page."

Each change appears on the board at once, and each one is **one undo step**:
the teacher presses Ctrl+Z to take back anything the AI did. The agent runs
on the teacher's computer under the teacher's own subscription. Rupok never
sees the account, the API key or the chat; only the open lesson goes between
the board and the agent.

```text
 Claude Code / Desktop / Codex / OpenCode
        │  MCP over stdio
        ▼
   rupok mcp --live  ── WebSocket on 127.0.0.1:47373 ──▶  Rupok board (browser or desktop app)
   (MCP tools, Loro mirror)        loopback only                (the lesson, the undo history)
```

- [Chat in Rupok](#chat-in-rupok): chat with your agent inside the board (`rupok agent`, or the desktop app).
- [AI in action](#ai-in-action): working cards, class mode, live solutions, students asking AI.
- [Make AI smoother: pre-approve Rupok tools](#make-ai-smoother-pre-approve-rupok-tools).
- [Setup](#setup): install `rupok`, add it to your agent.
- [In class](#in-class): connect the board, then ask.
- [Prompt library](#prompt-library) for science, maths and ICT teachers.
- [Tools](#tools) the agent gets; [everything an agent can do](agent-capabilities.md).
- [Custom stamps](#custom-stamps) and [the Smart Dock](#the-smart-dock).
- [Pairing and security](#pairing-and-security).
- [Live protocol v1](#live-protocol-v1): the WebSocket protocol, for developers.

## Chat in Rupok

The other way round: instead of adding Rupok to your agent, Rupok starts
your agent and you chat with it in a panel on the board. Say *"make a class on
Ohm's law"* and watch the pages, simulations and practice questions appear.

```text
 Rupok board ── chat panel ── WebSocket ──▶ rupok agent ── ACP (stdio) ──▶ Claude Code / Codex / OpenCode / Gemini CLI
                                             └── Rupok MCP tools (loopback HTTP) ◀── the agent calls them
```

**Desktop app.** Click the AI button in the top bar. The app starts the relay
itself, finds the assistants installed on this computer and opens the chat.
No terminal needed.

**Browser.** Run the relay in a terminal, then click the AI button and type
the code it shows:

```sh
rupok agent                    # uses the first installed assistant
rupok agent --agent claude     # or codex, opencode, gemini
rupok agent --open             # also opens the board in the browser, already paired
```

What each assistant needs (install once, log in once, in a terminal):

| Assistant | Install | Log in |
|---|---|---|
| Claude Code | `npm install -g @anthropic-ai/claude-code @agentclientprotocol/claude-agent-acp` | run `claude` once and log in (your Claude subscription) |
| Codex | `npm install -g @openai/codex @zed-industries/codex-acp` | run `codex` once and sign in |
| OpenCode | `npm install -g opencode-ai` | `opencode auth login` |
| Gemini CLI | `npm install -g @google/gemini-cli` | run `gemini` once and sign in |

If only `claude` (or `codex`) is installed, the relay runs its ACP adapter
with `npx -y`. Any other ACP agent: `rupok agent --agent-command "my-agent
--acp" --agent-name "My agent"`.

In the panel:

- **Assistant, model and mode.** The header picks the assistant on this
  computer. Under it, chips show what that assistant offers, and only that:
  its **models** (an ACP session option of category `model`, or the agent's
  model list) and its **modes**, with plain names: *Ask first*, *Accept
  edits*, *Plan only*, *Never ask* for Claude Code's `default`,
  `acceptEdits`, `plan`, `bypassPermissions`. Teachers signed in to Rupok
  Cloud also see **Rupok AI**, marked *coming soon* (no backend yet; see
  `apps/board/src/chatProviders.ts`).
- **Auto-approve** (the chip on the right): approve Rupok's board edits
  without asking, in this chat. It turns itself on, locked, when the mode
  already edits without asking. Settings → AI & agents → *Board edits in a
  new chat* sets how each chat starts. Other tools always ask.
- **Messages** stream in with Markdown and maths. **Thinking** is folded.
  The agent's **plan** is a checklist that ticks itself off. **Tool cards**
  say what changed on the board ("Added Smart Object 'Ohm's law' on page 3")
  with a *Go to page* link, and *Details* shows the tool's input and output.
  Hover a reply to **Copy** or **Retry** it; **Edit** puts a message you sent
  back in the composer.
- **Permission requests** show a picture of the page the change lands on,
  then *Allow once*, *Always for this session*, *Deny*, or **Deny all &
  stop**.
- **Composer.** Enter sends, Shift+Enter starts a new line, ↑ recalls what
  you sent, Esc stops. While the assistant works, Enter **queues** a
  follow-up (it goes when the turn ends; *Cancel* drops it). The mic
  dictates into it (Alt+V too), in English or বাংলা.
- **@-mentions** attach context: `@page 3` (its items and a picture),
  `@selection`, `@object <name>` (a Smart Object), `@lesson <name>` (a lesson
  from your Library, sent as its outline), `@file` (a picture, PDF or `.rpk`
  from this computer). Pictures and files can also be dropped or pasted.
  **Attach page** (on by default) sends the page you are on, or your lasso
  selection, when you mention nothing on the board.
- **/commands**: the assistant's own (Claude Code's `/review`, `/compact`…)
  and Rupok's: `/solve` (Solve this), `/quiz 5`, `/explain`, `/translate bn`, `/design` (page design), `/stamp` (a custom stamp), `/dock` (arrange the dock),
  `/clear` (clears the view) and `/new`.
- **History**: the clock button lists the chats on this lesson (kept in this
  browser) and the assistant's own saved chats. Reopening one resumes it when
  the assistant can (ACP `session/load`); otherwise the transcript is shown
  and your next message starts a new chat.
- **Usage**: tokens used of the context window, and cost, when the assistant
  reports them.
- **Prepare**: the panel docks on the right (or left, in Settings) and the
  slide re-fits beside it, resizable from its edge. **Teach**: the AI button
  floats top right (always, labelled *Ask AI* until an AI is connected); the
  chat floats under it and tucks into a small chip as soon as you draw; tap
  the chip or the AI button to bring it back. **Student view**: chat is off
  (the AI button says so); see [Students ask AI](#students-ask-ai).

When something is missing the panel says what to do: no relay (start
`rupok agent`), no assistant installed (the install commands), not logged in
("run `claude` once to log in"), the connection lost (it reconnects), or the
assistant stopped (*Restart*).

Every change is still one undo step, tagged with the assistant's name.

## AI in action

The class should *see* the AI work: on the board, live, with nothing to
click. Settings → AI & agents → *AI in action* has the switches.

### Working cards

As soon as the teacher asks (chat, voice *Ask AI*, or Solve this), a card
appears on the board where the result will go: a soft moving outline, the
agent's name, its current step (*Planning…*, *Reading the spec…*, *Building
the Smart Object…*, *Testing…*), its plan as a checklist with a progress
bar, a live preview of a Smart Object being built, and ✕ to stop. In Teach
it is large enough to read from the back of the room.

- **Where it comes from.** The in-app chat makes the card from the agent's
  ACP stream: a prompt starts it, `plan` updates fill the checklist,
  `tool_call` titles become plain steps, and a tool call with `x`, `y`,
  `width`, `height` moves the card to that box at once. Any agent can also
  post it explicitly with [`show_progress`](#show_progress).
- **When the result lands** the card moves onto it and resolves: a Smart
  Object fades and scales into its spot, handwriting writes stroke by stroke,
  shapes and tables are drawn on, and a small mint sparkle marks the spot
  (*Sparkle when a result lands*; a soft chime is available, off by default).
- **When something fails** (a Smart Object that does not pass Rupok's checks)
  the card stays and says it is being fixed; if the turn ends without a
  result it offers *Try again*, which sends the agent what went wrong.

### `show_progress`

The op (and MCP tool) behind the cards, for any agent, including Claude Code
in a terminal (`rupok_live::progress`, `rupok-cli/src/mcp_progress.rs`):

```json
{"op": "show_progress", "id": "caffeine", "title": "Caffeine molecule", "kind": "object",
 "step": "Placing the atoms…", "percent": 40,
 "plan": [{"content": "Atoms", "status": "completed"}, {"content": "Bonds", "status": "in_progress"}],
 "x": 320, "y": 180, "w": 1280, "h": 720, "preview_html": "<!doctype html>…partial object…"}
```

All fields are optional. `id` updates a card (the first call returns it);
`state` is `working` (default), `done`, `failed` (with a friendly `error`)
or `clear`; `kind` is `object`, `handwriting`, `text`, `shape` or `other`;
`preview_html` (≤ 512 KiB) renders in the Smart Object sandbox as a live
thumbnail. Access level **control** (like `point_at`): nothing is saved, no
undo step, no permission needed. A card ends when a result lands in its box,
on `done`/`clear`, or after two minutes without an update. In the in-app
chat the agent's card takes over the chat's own card for the request.

**Guide for agents ("in action"):**

1. **Place first.** Decide the box (a full-frame object: `x: 320, y: 180,
   w: 1280, h: 720`; a solution: below the problem) and call `show_progress`
   with it and the right `kind` before the slow part.
2. **Size.** Objects 1280×720 or full frame; text 32–48 px (titles 56–72);
   handwriting matches the problem. Add text in short lines: Rupok fits any
   text to the page anyway (below).
3. **Stream.** Update `step` (and `percent`) as you go; send `preview_html`
   with the partial object every few seconds while building, or
   `update_smart_object` an object you already placed.
4. **Land.** Place the result in exactly that box (`add_smart_object` with the
   same `page`, `x`, `y`, `width`, `height`): the card turns into it and the
   board celebrates, briefly. Say what you made in one line.

### Text always fits the page

`add_text` and `replace_items_with_text` fit what an agent writes inside the
slide frame (48 px safe margin), on the board and in files alike
(`rupok-live/src/fit.rs`): the asked size (else the size of the page's other
text, else 32 px) is capped for long passages (64 px over 40 characters, 48
over 120, 40 over 400; never more than an eighth of the page height); the
start is pulled inside the page; lines wrap at the room left (Inter's
character widths, estimated generously); a block that is still too tall moves
up into free space, then shrinks step by step to 24 px, then continues on new
pages after this one. The result says what changed (`warnings` with code
`fit`, `size`, `continued`). The board also wraps every text box at the
page's right edge. `write_handwriting` fits the same way: an explicit spot is
pulled inside, letters shrink until the block fits, else it continues on a new
page, scaled to fit.

### Class mode

On by default in Teach (*Class mode*: In Teach / Always / Off):

- The AI's board edits apply at once: permission requests for Rupok's board
  tools are approved automatically (the chat notes *Approved automatically
  (Class mode)* and the header chip says *Class mode*).
- Each edit shows a toast, *"Claude added a Smart Object · Undo"*; Undo takes
  back exactly that step (a streamed solution: all its lines).
- **Destructive actions still ask**: deleting items or pages, clearing a page,
  replacing the teacher's ink with text, and changing settings or the mode
  (including the app theme). If such an op reaches the board without a fresh
  approval (an agent in a no-ask mode, or an outside MCP client), the board
  asks the teacher itself, *Allow* / *Don't allow*, and refuses it after 25 s.
- Solve this writes straight away (no preview check), streaming line by line.

### Solve this, live

With *Write solutions line by line* on (default) and no teacher check (or in
class mode), the Solve this prompt asks the agent to write its steps in its
reply inside a ```` ```board ```` block, one step per line. The card says
*Thinking…* below the problem; each line is written on the board in
handwriting the moment the agent finishes it, below the problem and the lines
before it, in the same group (`write_handwriting` with `anchor.group`), so the
class watches the solution arrive. Writing that arrives while a line is still
being drawn queues behind it. Agents outside the chat do the same by calling
`write_handwriting` once per line with `anchor: {"group": …}`.

### Students ask AI

*Students can ask AI* (off by default) adds **Ask AI to show me** in Student
view, with the asks left (*Asks per student*, default 3 every 10 minutes, per
Student view session). The question goes to the AI connected to this board,
with a prompt to make **one** thing (a Smart Object, or a few lines of
handwriting) and to post a working card. Whatever the AI adds lands on the
**student's own layer**: drawn over the page on this screen only, marked
*Only you see this*, never written into the teacher's lesson, and cleared when
the board leaves Student view (`aiAction/student.ts`; the board answers the
agent `{"status": "student_layer"}`). Only additions are routed there; every
other write is refused as in any Student view.

- **On a classroom board** the AI is the teacher's (`rupok agent` or the
  desktop app), so the teacher's switch decides.
- **At home** a student opens a lesson in their own Rupok and connects their
  own assistant the same way (Setup below); in Prepare it is simply their copy.
- **Rupok AI** (planned, Rupok Cloud) will answer the same prompt the same way.
- With no AI connected the box explains these options instead.

## Make AI smoother: pre-approve Rupok tools

Every Rupok tool declares MCP annotations: `readOnlyHint` for reads
(`get_*`, `get_page_snapshot`, `list_*`, `library_list`,
`validate_smart_object`…), `destructiveHint` only for real deletes and ink
replacement (`delete_item`, `delete_items`, `delete_pages`, `delete_stamp`,
`library_trash`, `replace_items_with_text`, `replace_strokes_with_text`), and `idempotentHint`
where repeating changes nothing (`go_to_page`, `set_page_design`,
`update_smart_object`, `show_progress`…). Clients that honour them stop
asking for safe calls. To stop the prompts for board edits too, pre-approve
the Rupok tools once (Settings → AI & agents → *Make AI smoother* has these
to copy):

| Agent | Where | Add |
|---|---|---|
| Claude Code | `~/.claude/settings.json` (all projects) or `.claude/settings.json` in your lessons folder | `{ "permissions": { "allow": ["mcp__rupok__*"] } }` |
| Codex | `~/.codex/config.toml` | `[mcp_servers.rupok]` then `default_tools_approval_mode = "approve"` |
| OpenCode | `opencode.json` | `{ "permission": { "rupok_*": "allow" } }` |

Each board change stays one undo step, so pre-approving is safe in class; in
Rupok's own chat, class mode still asks before destructive actions. (`rupok
agent` names the server `rupok`, so the same rules cover the in-app chat
where the agent reads them.)

## Setup

### 1. Install the `rupok` command

From a checkout of the Rupok repository (release binaries will follow):

```sh
cargo install --path crates/rupok-cli
rupok --help
```

### 2. Add Rupok to your agent

`rupok mcp` is an MCP server on stdio. `--live` opens the live board channel
(it is on by default; `--no-live` turns it off). `--workspace` is the folder
where the agent may also read and write `.rpk` lesson files; it is optional
for live use.

**Claude Code**

```sh
claude mcp add rupok -- rupok mcp --live
# available in every project, with a lessons folder:
claude mcp add --scope user rupok -- rupok mcp --live --workspace ~/Lessons
```

**Claude Desktop**: edit `claude_desktop_config.json` (Settings → Developer →
Edit Config; on macOS `~/Library/Application Support/Claude/`, on Windows
`%APPDATA%\Claude\`) and restart Claude:

```json
{
  "mcpServers": {
    "rupok": {
      "command": "/Users/me/.cargo/bin/rupok",
      "args": ["mcp", "--live", "--workspace", "/Users/me/Lessons"]
    }
  }
}
```

Use the full path to `rupok`: Claude Desktop does not read your shell's `PATH`.

**Codex**: add to `~/.codex/config.toml`:

```toml
[mcp_servers.rupok]
command = "rupok"
args = ["mcp", "--live", "--workspace", "/home/me/Lessons"]
```

**OpenCode**: add to `opencode.json` (project) or `~/.config/opencode/opencode.json`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "rupok": {
      "type": "local",
      "command": ["rupok", "mcp", "--live", "--workspace", "/home/me/Lessons"],
      "enabled": true
    }
  }
}
```

**Any other MCP client**: command `rupok`, arguments `mcp --live`, transport stdio.

Options of `rupok mcp`:

| Option | Meaning |
|---|---|
| `--workspace <dir>` | Folder for `.rpk` files (default: the current folder). |
| `--live` / `--no-live` | Open the live board channel (default) or not. |
| `--live-port <port>` | Listen on this port instead of the first free of 47373–47376. |
| `--live-origin <origin>` | Also accept a self-hosted board, e.g. `https://board.school.example`. Repeatable. |

### 3. Teach your agent the classroom rules (optional, recommended)

The Rupok agent skill tells the agent how to behave on a live board (new
content on a new page, confirm before deleting, keep objects themed). Claude
Code users can copy `skills/rupok-live-board/` and `skills/rupok-smart-objects/`
into `~/.claude/skills/`; other agents read `skills/AGENTS.md`.

## In class

1. Open Rupok (rupok.app, the desktop app, or a local copy) with your lesson.
2. Click the **AI icon** (Connect AI) at the top right of the board, next to
   Settings. Rupok finds the AI assistants running on this computer and shows
   a **6-digit code** for each.
3. Tell your AI: *"Connect to my Rupok board, code 482 913."* It calls
   `connect_board`, and the icon gets a mint dot (hover it for the AI's name).
4. Ask. While the AI works, the dot pulses and the tooltip says **"Claude Code
   is editing…"**; after a change an **Undo** button sits next to the icon for
   a few seconds (Ctrl+Z works any time).

Clicking the icon opens the AI panel, which has:

- **Allow AI to edit live.** Off: the AI can still look at the board (read
  pages, take snapshots) but cannot change anything, change the page or
  point.
- **Mark AI edits.** Tags the items the AI made on this page. Attribution is
  stored in the lesson history, so it survives saving and reopening.
- **Activity**: what the AI did, and what was refused.
- **Disconnect.** The AI can pair again only with a new code.

A page reload keeps the connection (the tab remembers its session); closing
the tab ends it. In Student view the AI cannot edit.

**Browser notes.** In Chrome and Edge, a page on `https://rupok.app` that
talks to `127.0.0.1` may trigger a **Local Network Access** prompt ("rupok.app
wants to connect to apps on this device"). Choose **Allow**. If you clicked
Block, open the site settings from the icon left of the address bar and set
*Local network access* to *Allow*. The Connect AI panel explains this when it
detects the permission. Firefox and Chrome allow `ws://127.0.0.1` from an
https page; Safari may not, so use the desktop app there.

**Several agents.** Each `rupok mcp` takes the next free port (47373, 47374,
47375, 47376), and the board shows one code per agent with its name ("Claude
Code", "Codex"…). The code you give an agent pairs the board with that agent.
One board pairs with one agent at a time.

## Prompt library

Prompts teachers can say mid-class. The agent reads the board first
(`get_current_page`, `get_page_snapshot`), puts new material on a new page, and
shows it when ready.

### Science

- "Read the circuit I drew and make a working simulation of it on the next page: a switch, a bulb and an ammeter the students can tap."
- "A student asked why the Moon has phases. Build a Sun–Earth–Moon model with 8 build steps, one phase per step."
- "Turn my sketch of the water cycle into a clean labelled diagram, in Bangla."
- "Make a projectile simulation with sliders for angle and speed, and show the range formula with the changing term highlighted."
- "Balance the equation I wrote and show it step by step on a new page."
- "Make 5 multiple-choice questions on photosynthesis from this slide, one per step, answer revealed on Next."
- "Point at the part of my diagram where the mistake is."

### Maths

- "Make 3 harder practice problems like this one on the next page, answers hidden until I press Next."
- "Convert my handwritten working to clean text so the back row can read it."
- "Graph y = x² − 4x + 3 on a grid page, mark the roots and the vertex."
- "A student asked what happens to the area if we double the side. Make an interactive square with a slider."
- "Check the steps I wrote on this page and point at the first wrong line."
- "Translate this word problem to Bangla and keep the numbers."
- "Make a fraction-wall activity: students drag pieces to make one whole."

### ICT

- "Make a binary counter with 8 bits the students can click, showing the decimal value."
- "Turn the flowchart I sketched into a clean diagram on a new page."
- "Build a quiz on logic gates: show a gate, students pick the output; 6 questions."
- "Animate a bubble sort on the numbers I wrote, one swap per Next."
- "Show how a packet travels from this computer to a web server, step by step."
- "Make an HTML tag matching game for 10 tags in Bangla and English."
- "Switch to chalkboard mode and make the code I wrote into a syntax-highlighted listing."

### Classroom moves

- "Next step." / "Show the answer." (`step_object`)
- "Go back to the slide with the graph." (`go_to_page`)
- "Where should they look?" (`point_at`)
- "Dark mode, please." / "Chalkboard." (`set_board_theme`)

### Page design

- "Make a chalkboard background with our school logo and a physics theme for
  Class 9." (`set_page_design`, `set_branding`)
- "Put the lesson title and page number in the footer of every slide."
  (`set_branding` with `class: "{lesson}", teacher: "{page} / {pages}"`)
- In Rupok's chat: **/design** followed by the wish, or **Design with AI…** in
  Page design (the current design goes with it).

## Tools

When a board is paired, these tools act on the lesson open on it. The lesson
tools (`get_lesson_outline`, `add_page`, `add_text`, `add_smart_object`,
`update_smart_object`, `delete_item`, `set_page_background`,
`set_page_design`, `set_branding`) do the same when called with
**`path: "live"`**; with a real `.rpk` path they still edit files.

| Tool | What it does |
|---|---|
| `connect_board(code)` | Pair with the board showing that 6-digit code. |
| `get_board_status()` | Paired or not, what the board shows, and a `hint` saying what to do next. |
| `get_current_page(page?)` | Items on the current (or given) page: ids, kinds, geometry, text, objects and steps, `age_s` ("what I just wrote"), the selection, the page `scheme`. |
| `get_page_snapshot(page?, item_ids?, region?, width?)` | A PNG of the page as the class sees it (MCP image content), optionally zoomed on items or a region, to read handwriting. |
| `get_selection()` | What the teacher has lassoed. |
| `go_to_page(page)` | Show a page. |
| `add_page_after_current(title?, pattern?, color?, show?)` | A new page after the current one, same look. |
| `step_object(item_id?, step?, delta?)` | A Smart Object's Next/Back. |
| `replace_items_with_text(item_ids, text, …)` | Ink to text as one undo step. |
| `add_shape(shape, x, y, w, h, color?, fill?, width?, dashed?, text?, page?)` | A clean shape from the Shapes picker (rect, circle, star, arrows, Venn, flowchart, number line, axes, speech bubble, braces…) with optional fill, dashes and centred text; one undo step. Run by the board's engine, which owns shape geometry. |
| `add_table(x, y, rows, cols, w?, h?, cells?, color?, header_fill?, text_size?, page?)` | A table (1–10 × 1–10) with cell text and an optional filled header row, grouped; one undo step. |
| `point_at(x, y, w?, h?, label?, seconds?)` | A temporary laser ring or highlight box (not saved). |
| `show_progress(id?, title?, step?, plan?, percent?, state?, kind?, x?, y?, w?, h?, page?, preview_html?, error?)` | A working card where your result will land, live for the class (not saved). See [AI in action](#show_progress). |
| `write_handwriting(lines, anchor?, style?, speed?, color?, size?, step_mode?)` | Write lines (text with inline LaTeX) in handwriting, animated stroke by stroke, below the anchor (or below earlier writing: `anchor.group`); one undo step. See [Solve this](#solve-this). |
| `set_board_theme(page_color?, pattern?, ui?)` | Page colour (white, paper, chalkboard, blackboard, #hex) and pattern; app light/dark. |
| `start_voice_text(x?, y?, w?, lang?, mode?, style?)` | Live only. Turn on [Voice on the board](#voice-on-the-board): the teacher speaks and the words appear in a box at `x, y`, `w` wide (default: under the last voice text, else mid-page), wrapped inside the page. `lang` `en-US` or `bn-BD`; `mode` `text` (default) or `ask` (the words become a chat prompt); `style` `typed` (default) or `handwriting`. Refused when the board has no speech recognition or voice input is off. |
| `stop_voice_text(cancel?)` | Live only. Finish: the words become one text item (one undo step) and the result has `text` and `item_id`; `cancel: true` throws them away. |
| `set_page_design(path, page?, all_pages?, color?, pattern?, background_svg?, background_image?, background_fit?, background_opacity?, background_dim?, remove_background?)` | Page design: colour, pattern and a background image (SVG, or base64 PNG/JPEG), for a page or every page. Only `path`: read the design. Live or on a file. |
| `set_branding(path, scope?, page?, logo_svg?, logo_image?, logo_corner?, logo_x?, logo_y?, logo_width?, logo_opacity?, remove_logo?, school?, teacher?, class?, date?, text_position?, text_align?, hidden?, remove?)` | Logo and header/footer line on every page (`scope: "lesson"`) or one page; text placeholders `{lesson}`, `{page}`, `{pages}`, `{date}`, `{teacher}`, `{school}`, `{subject}`, `{course}`, `{chapter}`. Only `path`: read it. |

`add_text` with a `w` wraps on the board: the board breaks the lines to fit
`w`, measured with its own font, and stores them as `\n` (text items never
reflow by themselves, so every reader shows the same lines). On a lesson file
the text is stored as given.

Every change is one undo step on the board, tagged with the agent's name in
the lesson history (the Loro commit message
`rupok-agent/1 {"agent":"Claude Code","op":"add_text"}`).
`update_smart_object` stores the new HTML as a new bundle and re-points the
placement, so it can be undone too.


### Everything else on the board

Rupok is agent-first: anything the teacher can do, the agent can do for
them ([agent-capabilities.md](agent-capabilities.md) is the full list, with
what stays the teacher's alone and why). Document tools take an optional
`path`: omit it (or pass `"live"`) for the board, or give a `.rpk` path to
edit a lesson file with the same code. Board tools need a paired board.

| Area | Tools | File | Live |
|---|---|---|---|
| Pages | `list_pages`, `move_pages(pages, to)`, `delete_pages(pages)`, `duplicate_pages(pages)`, `set_page_title(page?, title)` | ✅ | ✅ |
| Select and arrange | `select_items(item_ids)` (live), `move_items(item_ids, dx/dy or x/y)`, `align_items(item_ids, align?, distribute?, relative_to?)`, `group_items`, `ungroup_items`, `bring_to_front`, `send_to_back`, `delete_items` | ✅ | ✅ |
| Ink and style | `draw_stroke(points or strokes, color?, width?, brush?, opacity?, tool?)`, `restyle_items(item_ids, color?, width?, brush?, fill?, opacity?, dashed?, text_size?)`, `scale_items(item_ids, factor)`, `update_text(item_id, text?, size?, color?, x?, y?, w?)` | ✅ | ✅ |
| Stamps | `list_stamps`, `add_stamp(stamp, x?, y?, size?, color?, text?)`, `create_stamp`, `update_stamp`, `delete_stamp` (see [Custom stamps](#custom-stamps)) | ✅ | ✅ |
| Modes and view | `set_mode(prepare \| teach \| student)`, `zoom_to(zoom?, x?, y?)`, `fit_page`, `start_replay(page?, speed?)`, `stop_replay`, `celebrate(kind?)`, `set_tool(tool, color?, width?, brush?)`, `undo_last_change` | – | ✅ |
| Export | `export_pdf(pages?, layout?, ink?, print_friendly?, save_as?, download?)`: the board makes the PDF, the server saves it in the workspace | – | ✅ |
| Library | `library_list(folder?, query?)`, `library_open(id)`, `library_create_lesson(title, folder?, template?, open?)`, `library_move(id, folder?)`, `library_rename(id, name)`, `library_trash(id)` | – | ✅ |
| Settings | `get_settings`, `set_setting(key, value)`: an allowlist (theme, accent, density, pen style and defaults, gestures, PDF defaults, voice language, ink sharpness and the wet-ink layer…). Never AI keys, the cloud account, payments, storage or permissions. | – | ✅ |
| Smart Dock | `list_dock_items`, `get_dock_layout(mode?)`, `set_dock_layout(mode?, layout)`, `save_dock_preset(name, modes?)`, `apply_dock_preset(name)` (see [The Smart Dock](#the-smart-dock)) | – | ✅ |

Examples, one per area:

```text
move_pages(pages: [5], to: 1)                                  # page 5 to the front
align_items(item_ids: ["a", "b", "c"], align: "top", distribute: "horizontal")
restyle_items(item_ids: ["s1", "s2"], color: "#e5484d", brush: "chalk", width: 6)
draw_stroke(points: [[400, 620], [900, 640]], color: "#e5484d", width: 8)   # underline
add_stamp(stamp: "excellent", x: 1500, y: 80)
set_mode(mode: "teach") · zoom_to(zoom: 2, x: 600, y: 400) · fit_page()
export_pdf(pages: "2-5", layout: "handout-2", save_as: "handouts/forces.pdf")
library_create_lesson(title: "Week 3: Forces", template: "slides", open: true)
set_setting(key: "appearance.theme", value: "dark")
set_setting(key: "appearance.layout", value: "classic")      # three big islands, no top bar
set_dock_layout(mode: "teach", layout: {groups: [{items: ["pen", "highlighter", "eraser"]}], split: {position: "right", groups: [{items: ["undo"]}, {items: ["pager"]}]}, size: "l"})
```

Board-tool writes are refused in Student view and when *Allow AI to edit
live* is off, like edits. In Rupok's chat the agent asks permission for
settings, dock and mode changes exactly as for edits. A settings or dock
change is undone from the AI chip's **Undo**, not Ctrl+Z (it is not part of
the lesson). `undo_last_change` undoes only the agent's own newest change
on the board; the teacher's history is never touched.

## Custom stamps

Teachers ask: "make me a stamp for a great experiment" or "a dice my class
can roll". `create_stamp` makes it; it joins the teacher's **My stamps**
(kept on their device) and the lesson (so a shared lesson shows it), and
`add_stamp` places it. In Rupok's chat, **/stamp** or the palette's **Make a
stamp with AI…** button starts this.

- **Artwork**: `svg` (sanitised exactly like page design SVG: no scripts,
  event handlers, external references or SMIL; drawn only as an image),
  `emoji`, or `text` (a short pill label).
- **Behaviour, no code**: `animation` (`pop`, `sparkle`, `bounce`, `spin`,
  `fade-in`), `burst` (`stars`, `confetti`, `hearts`, `sparkles`) with
  `burst_colors`, `counter` (a 1, 2, 3… badge per placement on the page),
  `sound` (`chime`, `pop`, `ding`, `tada`, `clap`, `whoosh`; the teacher's
  *Stamp sounds* switch wins), `fade_after` seconds (the stamp is removed,
  as its own undo step).
- **Smart stamps, with your own logic**: `art: "smart"` and `html`: a tiny
  Smart Object, placed and sized like a stamp. It runs in the Smart Object
  sandbox (opaque origin, no network, the `window.rupok` SDK), never
  anywhere else. Good ones: a dice that rolls on tap, a 1-minute timer, a
  "correct answer" stamp that plays a little animation and counts taps
  (`rupok.state` keeps the count).

```text
create_stamp(name: "Super star", art: "svg", color: "#f5b301",
  svg: '<svg viewBox="0 0 100 100"><path d="M50 6 61 38 95 38 67 58 78 92 50 72 22 92 33 58 5 38 39 38Z" fill="currentColor"/></svg>',
  behavior: {animation: "sparkle", burst: "stars", counter: true, sound: "chime"})
create_stamp(id: "dice", name: "Dice", art: "smart", width: 160, height: 160,
  html: "<!doctype html><title>Dice</title><body style='margin:0;display:grid;place-items:center;height:100vh;font:120px system-ui'><b id=d>⚀</b><script>document.body.onclick=()=>{d.textContent='⚀⚁⚂⚃⚄⚅'[Math.floor(Math.random()*6)]}</script>")
add_stamp(stamp: "dice", x: 1600, y: 80)
```

**Designing a stamp** (projector rules):

- Bold and simple: one shape, a thick outline or solid fill, no fine
  detail or text inside the drawing. It must read at 96 px from the back
  row. Use `fill="currentColor"` so the teacher can recolour it.
- Celebratory but brief: one animation, at most one burst and one short
  sound. `fade_after` suits reactions ("Shh!", "Thinking…"), not rewards.
- Labels: 1–3 words, under 24 characters ("Well done!", "Try again").
- Smart stamps: the whole body is the button, 44 px targets or bigger,
  large type, works offline, respects the theme (`--rupok-color-*`).

`update_stamp(id, …)` changes only the fields given (placed stamps follow).
`delete_stamp(id)` removes it from My stamps; a lesson that still shows the
stamp keeps its definition unless `remove_placements: true`.

## The Smart Dock

Teachers say "put pen, highlighter and eraser on the left, undo and next
page on the right, bigger buttons". The dock is customisable per mode
(Prepare, Teach): its position, ordered **groups** of buttons (separated on
the dock), an optional **split** second dock on another edge, a button
`size` (`s`, `m`, `l`), per-button `sizes`, and `large` icons for low
vision. Named layouts: *Classic*, *Left-handed*, *Minimal teaching*, *Maths class*,
*Default*, and the teacher's own (`save_dock_preset`).

**The Classic layout** is a dock style: `style: "islands"` turns each group
into a big island along the bottom edge (the first at the left, the last at
the right, the others in the middle) and hides the top bar; the ☰ `menu`
holds files, import, export, print, settings, help and modes. Switch with
`set_setting(key: "appearance.layout", value: "classic" | "modern")` (Modern
brings back the teacher's own dock) or `apply_dock_preset(name: "Classic")`;
rearrange it with `set_dock_layout` like any dock. Its buttons: `menu`,
`mode`, `toolbox`, `share`, `zoom`, `addpage`, `recognize`. Pen, eraser and
undo always stay on an island.

```text
list_dock_items()                          # ids: pen, highlighter, eraser, lasso, laser, text, shapes, stamp,
                                           # undo, redo, pager, exit, celebrate, pages, solve, settings…
set_dock_layout(mode: "teach", layout: {
  position: "left", size: "l",
  groups: [{label: "Ink", items: ["pen", "highlighter", "eraser"]}, {items: ["laser", "lasso"]}],
  split: {position: "right", groups: [{items: ["undo"]}, {items: ["pager", "exit"]}]}})
save_dock_preset(name: "Big board")
```

Pen, highlighter, eraser, laser and undo always stay on a dock (a layout
that leaves one out gets it back in its first group). The result lists
`warnings`, e.g. when no button leads back to Settings. In Rupok's chat,
**/dock** and the dock's ⋯ → **Customize dock with AI…** start this; the
teacher's own editor is ⋯ → **Customize dock…** (or hold the dock's grip).

## Designing pages

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

## Solve this

The teacher (or a student, when the teacher allows it) lassos a problem on
the board and taps **Solve this** in the selection toolbar (in Teach, the
chip **Solve what I select** next to the AI chip, then lasso; in the chat,
`/solve`). The AI writes the solution on the board **in handwriting**,
stroke by stroke, below the problem.

**What the board sends** in the chat: a picture of the selection, the
selection as JSON (item ids, bounds), the offline maths recogniser's reading
(n-best LaTeX with confidences, `crates/rupok-ink/src/math`) and this prompt
(`apps/board/src/solve/index.ts`, `solvePrompt`):

```text
Solve this problem from my board, step by step, for my class.

The picture shows what I selected on page 1 (item ids: …).
Rupok's offline handwriting recogniser read it as (best guess first; check against the picture):
1. $2x+3=7$ (confidence 0.75)
2. $2x+5=7$ (confidence 0.09)

Please:
1. Work out the solution, then check every step and all the arithmetic before writing anything.
2. Call write_handwriting once, with anchor {"item_ids": [the ids above]} so it goes right below the
   problem. One short step per line (under about 30 characters), maths in $…$ (e.g. "$2x = 4$",
   "$x = \frac{4}{2}$"). Don't copy the problem out again; finish with the answer on its own line.
3. Keep to the level of the problem: the method a school student would use, no shortcuts they haven't met.
4. While you explain, you may use point_at to circle the part of the board you are talking about.
5. Then explain the steps in this chat in 2 to 4 short sentences.
The teacher checks the solution before the class sees it: when write_handwriting answers status
"preview", it is waiting for their approval. Do not call it again, and never write the solution any other way.
```

**`write_handwriting`** (op `write_handwriting`, `rupok_live::handwriting`):
`lines` (≤ 40, ≤ 400 characters each; text with inline LaTeX: `\frac`, `^`,
`_`, `\sqrt`, Greek, operators, `\text`), `anchor` (`item_ids`, `region`,
or `x`/`y`; `side`: `below` or `right`; `group`: continue below an earlier
call's writing and join its group; default: the selection), `style`
(`teacher` | `neat` | `joined`: neat letters joined within words), `speed` (0.25–4), `color`, `size` (capital height, px),
`step_mode` (`all` | `one-line-per-click`). The board synthesises the strokes
(`rupok_ink::write`: Hershey single-stroke glyphs or the teacher's own,
smoothed with a Catmull-Rom spline and written in a hand's pen order,
spaced by their ink, on a slowly wandering baseline, with pressure that
swells on downstrokes, maths layout and brisk pen timing), matches the problem's ink
colour, pen width and letter size, and places them below (else right of) the
anchor without covering anything, else on a new page. All strokes are one
Loro commit (one undo), grouped, attributed to the agent; each stroke's
`created` is when it is drawn, so Replay writes it the same way. The result
has `page`, `group`, `strokes`, `strokes_by_line`, `bounds`, `duration_ms`,
`style` and a `note`.

**Preview** (Settings → AI & agents → Solve this → *Check before showing the
class*, on by default): the board does not write; it answers `{"status":
"preview"}` and shows the teacher a card with the lines typeset and the
handwriting played back. *Write it on the board* writes it for the class;
*Ask for a change* (typed or dictated) sends the teacher's note back to the
agent. **Teach**: with *One line per click*, the first line is written and
each Next (clicker, arrow key, swipe or the *Next line* chip) writes the next.
Other settings: AI handwriting (my handwriting / neat), writing speed, let
students use Solve this, and **Teach Rupok my handwriting**: a calibration
screen where the teacher writes digits, letters and maths symbols once; the
samples (writing units, per device, `rupok.handwriting.v1`) drive both the
writer and the recogniser's user templates.

## Voice on the board

Teachers talk during class and don't want to write whole sentences. The
dock's **Voice text** tool (a mic with a T, in Prepare and Teach) puts what
they say on the board while they say it:

- **Place:** pick the tool, then tap the page (or drag across it to set the
  width). A voice box opens there with a pulsing mint mic and starts
  listening. Drag the box to move it, drag its right edge to change the
  width.
- **Live words:** words still being recognised show lighter; final words
  turn into the ink colour. Text wraps inside the box, and the box stays on
  the page (it moves up, then shrinks its text, rather than run off the
  slide).
- **Punctuation and lines:** spoken punctuation works as in every field
  ("comma", "full stop", "question mark", "new line"; কমা, দাঁড়ি,
  প্রশ্নবোধক চিহ্ন, নতুন লাইন). A pause of about 2 s starts a new line.
- **Finish:** tap the box, press Esc or ✓. The words become a normal text
  item (editable, resizable, recolourable), with the line breaks the teacher
  saw. **One undo step per box**: the box is not in the lesson until it is
  finished, so Ctrl+Z after finishing removes the whole voice text. ✕
  discards it.
- **Teach:** hold the dock button to talk (push-to-talk): the box opens under
  the last voice text and its words go on the board on release, the pen stays
  the tool, one undo step per hold. With the tool on, Space or a clicker's
  "blank" key (B or .) toggles listening.
- **On the box:** EN / বাং; **Text ↔ Ask AI**; typed ↔ **hand-written**
  (the Solve this synthesiser, English; Bangla stays typed); where the audio
  goes (offline Whisper on this computer, or the browser's cloud service).
- **Ask AI:** the words become a prompt to the in-app chat with the page (or
  the selection) attached. A small bubble confirms it; in Teach the chat
  stays closed. Not connected yet: the chat opens with the words in its box.

Settings → General → Voice on the board: what a new box does (text or Ask
AI), tap or hold to talk, hand-written by default, the pause before a new
line (0 turns it off), and whether students may ask the AI by voice in
Student view (the tool can then be added to Student view in Teaching modes;
students never write on the board).

Agents use `start_voice_text` / `stop_voice_text` (live only) when the teacher
asks ("take my dictation", "let me speak the notes"). The microphone is the
teacher's: the board shows it is listening, and the ops need *Allow AI to
edit live*.

## Pairing and security

- `rupok mcp` listens on **127.0.0.1 only**, never on the network.
- The WebSocket handshake is refused (HTTP 403) unless the `Host` header names
  the loopback server (this defeats DNS rebinding) and the `Origin` is a Rupok
  board: `https://rupok.app`, `http://127.0.0.1[:port]`,
  `http://localhost[:port]`, `http://[::1][:port]`, the Tauri shell
  (`tauri://localhost`, `http(s)://tauri.localhost`), or a `--live-origin`.
  Browsers always send `Origin`, and web pages cannot forge it, so other
  websites cannot reach the agent.
- A board that connects is only **waiting**: it gets a random 6-digit code
  (from the OS random generator, unique among waiting boards, valid 10
  minutes) and can do nothing else. The teacher reads the code on *their*
  board and gives it to *their* agent. Only `connect_board` with that code
  pairs the board. Five wrong codes in a row replace every waiting code.
- On pairing the board gets a 128-bit session token, kept in the tab's
  `sessionStorage`, so a reload reconnects without a code. It is valid until
  the agent process exits, the teacher disconnects, or another board pairs.
- One board per agent at a time; pairing a second board drops the first.
- The teacher's **Allow AI to edit live** switch and Student view are
  enforced by the board, not the agent.
- Smart Objects the agent adds are validated exactly like file lessons and run
  in the same sandbox (no network unless the object declares hosts).

## Live protocol v1

For developers of boards and agent bridges. Reference implementation:
`crates/rupok-live` (`protocol.rs`, `op.rs`, `apply.rs`), the server in
`crates/rupok-cli/src/live/`, the board in `apps/board/src/live.ts`.

### Transport

One WebSocket per board, opened by the board to `ws://127.0.0.1:<port>`, where
`<port>` is the first that answers of 47373, 47374, 47375, 47376 (the board
probes all four to find every agent). Frames are UTF-8 JSON text, at most
64 MiB. Binary payloads are base64 strings.

### Frames and channels

Every frame is a JSON object with a channel `ch` and a type `t`:

```json
{ "ch": "tools", "t": "op", "id": 7, "agent": "Claude Code", "op": { "op": "add_text", "text": "Hi", "x": 100, "y": 80 } }
```

| `ch` | Purpose |
|---|---|
| `session` | Hello, pairing, presence, board state, errors. |
| `doc` | Loro sync of the open lesson. |
| `tools` | Agent operations run on the board, and their results. |
| `chat` | The in-app chat with the teacher's agent (`rupok agent`, the desktop app). |

**Versioning.** `session/hello` carries `v` (this document: `1`) and
`channels`, the channels the board speaks; `welcome` and `paired` answer with
the channels the server will use. A server refuses a different `v` with
`session/error {code: "bad_hello"}`. Within a version, receivers **must ignore**
frames on unknown channels and unknown `t` values on known channels, and
ignore unknown fields; that is how new channels and messages are added
without breaking older peers. A breaking change bumps `v`.

### `session`

| Direction | `t` | Fields |
|---|---|---|
| board → server | `hello` | `v`, `client` (e.g. `rupok-board/0.1 (web)`), `channels`, `token?`, `doc?: {key, title}` |
| server → board | `welcome` | `v`, `agent`, `code` (6 digits), `expires_in` (s), `channels`, `pair` (`"agent"`: give the code to the agent; `"board"`: the teacher types the relay's code), `chat?` |
| server → board | `paired` | `v`, `token` (32 hex), `agent`, `channels` |
| server → board | `agent` | `agent`: the agent's display name changed |
| server → board | `error` | `error` (human text), `code`: `bad_hello`, `busy`, `replaced`, `unpaired`, `code_expired`; the server then closes |
| board → server | `state` | `page` (1-based), `page_count`, `allow_edit`, `read_only`, `title`; after pairing, on change, and before answering any non-read op |
| board → server | `pair` | `code`: the code the relay shows (only when `welcome.pair` is `"board"`) |
| board → server | `bye` | The teacher disconnected; the token is revoked. |

The first frame must be `hello` within 10 s. With a valid `token` the server
answers `paired` at once; otherwise `welcome`, and `paired` follows when the
agent calls `connect_board` with the code. Before `paired`, a connection's
other frames are ignored.

### `doc`

The board holds the authoritative Loro document; the server keeps a mirror.
`doc.key` identifies a lesson across saves and replicas:
`<meta.created as integer ms>-<peer id that wrote meta.format, hex>`.

| Direction | `t` | Fields |
|---|---|---|
| server → board | `sync` | `key?`, `vv?`: the mirror's version vector (Loro `VersionVector::encode`, base64) for lesson `key`, or both `null`: "send a snapshot" |
| board → server | `snapshot` | `data`: the lesson as `.rpk` bytes (base64), `doc: {key, title}` |
| both | `update` | `data`: a Loro update (`export(ExportMode::updates(from))`, base64) |
| board → server | `sync` | `key`, `vv`: the board's version vector, "send what I lack" |

After `paired`, the server sends `doc/sync`. If the key matches the board's
lesson, the board sends `doc/update` with what the mirror lacks, then its own
`doc/sync`, and the server answers with a `doc/update` of what the board lacks
(e.g. after a reload from an older autosave). Otherwise the board sends a
`doc/snapshot`. Afterwards the board sends a `doc/update` after every change
(the teacher's or the agent's), and a new snapshot when another lesson is
opened. Loro merges updates from both sides; importing twice is harmless.

### `tools`

| Direction | `t` | Fields |
|---|---|---|
| server → board | `op` | `id` (u64), `agent`, `op`: an operation object |
| board → server | `result` | `id`, `ok`; `result` when ok; else `error` (text) and `data?` (e.g. `{"error", "saved": false, "diagnostics": [...]}`) |

An operation is `{"op": "<name>", …fields}`; the names and fields are the
`Op` enum in `crates/rupok-live/src/op.rs` (`get_page`, `get_outline`,
`get_selection`, `snapshot`, `go_to_page`, `point_at`, `set_board_theme`,
`add_page`, `add_page_after_current`, `add_text`, `add_smart_object`,
`update_smart_object`, `add_media`, `add_model`, `delete_item`,
`set_page_background`, `set_page_design`, `set_branding`,
`replace_items_with_text`, `step_object`, `write_handwriting`, `add_shape`,
`add_table`, `show_progress`, `list_pages`, `move_pages`, `delete_pages`,
`duplicate_pages`, `set_page_title`, `select_items`, `move_items`,
`align_items`, `restyle_items`, `scale_items`, `group_items`,
`ungroup_items`, `reorder_items`, `delete_items`, `draw_stroke`,
`update_text`, `list_stamps`, `create_stamp`, `update_stamp`,
`delete_stamp`, `add_stamp`, and the board ops `set_mode`, `zoom_to`,
`fit_page`, `start_replay`, `stop_replay`, `celebrate`, `set_tool`, `undo`,
`export_pdf`, `library_list`, `library_open`, `library_create_lesson`,
`library_move`, `library_rename`, `library_trash`, `get_settings`,
`set_setting`, `get_dock_layout`, `set_dock_layout`, `list_dock_items`,
`save_dock_preset`, `apply_dock_preset`, `start_voice_text`,
`stop_voice_text`). `add_shape` and `add_table` run in the board's engine
(`crates/rupok-wasm` `engine/insert.rs`) and are drawn on stroke by stroke;
`show_progress` runs in the web app; the board ops run in
`apps/board/src/agentControl.ts`; the voice ops in the board app, which
owns the microphone (`apps/board/src/voice/board.ts`); the rest in
`rupok_live::apply` (also on lesson files). The board adds `def`
(`add_stamp`), `base` (`update_stamp`) and `in_library` (`delete_stamp`)
from the teacher's device library before running the stamp ops.
`export_pdf` answers `pdf_base64`. `add_media`
carries a `url` (YouTube or a trusted embed) or a base64 `file_base64` (video
or audio); `add_model` a base64 `.glb` as `file_base64`. The MCP tools
`add_youtube`, `add_media` and `add_3d_model` read files from the workspace and
send these ops when called with `path: "live"`. Pages are 1-based and `page`
omitted means the page on screen; coordinates are slide pixels in the
1920 × 1080 frame.

Each op has an access level: **read** (`get_*`, `snapshot`, `list_*`,
`library_list`), **control** (`go_to_page`, `point_at`, `show_progress`,
`set_board_theme` with only `ui`, `select_items`, `set_mode`, `zoom_to`,
`fit_page`, `start_replay`, `stop_replay`, `celebrate`, `set_tool`,
`export_pdf`, `library_open`; leaving Student view with `set_mode` is
refused) or **write** (everything else, including settings and dock
changes). With *Allow AI to edit live* off the board refuses control
and write ops; in Student view it refuses write ops. A write runs as **one**
Loro commit with the message `rupok-agent/1 {"agent": …, "op": …}` and the
board sends its `doc/update` *before* the `result`, so the server's mirror is
current when the agent gets the answer. The server waits 30 s for a result (180 s for `export_pdf`).

`snapshot` answers `{png, width, height, page, region, note}`; `png` is a
base64 PNG drawn by the board (Smart Objects as labelled cards).

### `chat`

Offered by `rupok agent` and the desktop app (`welcome.chat: true`; the
channel is listed in `paired.channels`). The relay speaks
[ACP](https://agentclientprotocol.com) v1 to the agent over its stdio and
gives it the Rupok MCP tools in `session/new` (an `http` MCP server on
`127.0.0.1`, random port, bearer token). Implementation:
`crates/rupok-cli/src/chat/`.

Pairing is reversed: `welcome.pair` is `"board"`, the relay shows a 6-digit
code (terminal, or handed to its own window by the desktop app) and the board
sends `session/pair {code}`. A wrong code gets `session/error {code:
"bad_code"}`; five in a row replace the code.

| Direction | `t` | Fields |
|---|---|---|
| server → board | `hello` | `agents: [{id, label, available, install[], login, logged_in?}]`, `current`, `default`; after pairing and on request |
| board → server | `hello` / `agents` | re-detect installed agents, answer `hello` |
| board → server | `start` | `agent?`: start (or switch to) an agent |
| board → server | `prompt` | `text`, `agent?`, `context?`: a list of `{kind: "page" \| "selection" \| "object" \| "json", uri, text, mime?}`, `{kind: "image", data, mime?}`, `{kind: "lesson", name, data}` (`.rpk` bytes; the relay sends the agent its outline) and `{kind: "file", name, mime, data}`. Starts the agent if needed. The relay sends what the agent accepts: without `embeddedContext`, resources become fenced text; without `image`, a note |
| server → board | `status` | `state`: `idle`, `starting`, `ready`, `busy`, `offline`, `error`; `agent`, `label`; on `ready`/`busy`: `session`, `caps` (`images`, `embedded`, `audio`, `load`, `list`, `http_mcp`), `modes` (ACP `SessionModeState`), `config` (ACP `SessionConfigOption`s), `models` (the unstable `SessionModelState`, if sent), `commands` (the latest `availableCommands`); on `error`: `code` (`not_installed`, `login`, `crashed`), `error`, `install?`, `login?` |
| server → board | `update` | `session`, `update`: the ACP `SessionUpdate` as JSON (`sessionUpdate`: `agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, `plan`, `current_mode_update`…) |
| server → board | `permission` | `id`, `tool_call`, `options` (ACP `PermissionOption`s with `optionId`, `name`, `kind`) |
| board → server | `permission_reply` | `id`, `option` (an `optionId`, or `null` to refuse) |
| server → board | `done` | `session`, `stop_reason` (ACP `StopReason`, or `"error"` with `error`) |
| board → server | `cancel` | stop the current turn (pending permissions are refused) |
| board → server | `new` | a new session; the server answers `cleared` |
| board → server | `set_mode` | `mode`: an id from `status.modes.availableModes` |
| board → server | `set_model` | `model`: through the `model` config option (`session/set_config_option`), else `session/set_model` |
| board → server | `set_config` | `id`, `value`: any other session config option |
| board → server | `list_sessions` | the server answers `sessions {agent, sessions: [{sessionId, title?, updatedAt?}], supported}` (`session/list`) |
| board → server | `load_session` | `session`, `agent?`: resume a chat (`session/load`); the server sends `loading`, the replayed `update`s, then `loaded` |
| server → board | `notice` | `error`: something the teacher should see (a model switch failed…) |
| board → server | `stop` | stop the agent process |

The relay forwards ACP JSON as-is, so new session update kinds reach the
board without a protocol change; a board ignores kinds it does not know.
