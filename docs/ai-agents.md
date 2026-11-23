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
- [Setup](#setup): install `rupok`, add it to your agent.
- [In class](#in-class): connect the board, then ask.
- [Prompt library](#prompt-library) for science, maths and ICT teachers.
- [Tools](#tools) the agent gets.
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
  and Rupok's: `/solve` (Solve this), `/quiz 5`, `/explain`, `/translate bn`,
  `/clear` (clears the view) and `/new`.
- **History**: the clock button lists the chats on this lesson (kept in this
  browser) and the assistant's own saved chats. Reopening one resumes it when
  the assistant can (ACP `session/load`); otherwise the transcript is shown
  and your next message starts a new chat.
- **Usage**: tokens used of the context window, and cost, when the assistant
  reports them.
- **Prepare**: the panel docks on the right (or left, in Settings) and the
  slide re-fits beside it, resizable from its edge. **Teach**: it floats over
  the slide and tucks into a small chip as soon as you draw; tap the chip to
  bring it back. **Student view**: chat is off.

When something is missing the panel says what to do: no relay (start
`rupok agent`), no assistant installed (the install commands), not logged in
("run `claude` once to log in"), the connection lost (it reconnects), or the
assistant stopped (*Restart*).

Every change is still one undo step, tagged with the assistant's name.

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

## Tools

When a board is paired, these tools act on the lesson open on it. The lesson
tools (`get_lesson_outline`, `add_page`, `add_text`, `add_smart_object`,
`update_smart_object`, `delete_item`, `set_page_background`) do the same when
called with **`path: "live"`**; with a real `.rpk` path they still edit files.

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
| `point_at(x, y, w?, h?, label?, seconds?)` | A temporary laser ring or highlight box (not saved). |
| `write_handwriting(lines, anchor?, style?, speed?, color?, size?, step_mode?)` | Write lines (text with inline LaTeX) in handwriting, animated stroke by stroke, below the anchor; one undo step. See [Solve this](#solve-this). |
| `set_board_theme(page_color?, pattern?, ui?)` | Page colour (white, paper, chalkboard, blackboard, #hex) and pattern; app light/dark. |

Every change is one undo step on the board, tagged with the agent's name in
the lesson history (the Loro commit message
`rupok-agent/1 {"agent":"Claude Code","op":"add_text"}`).
`update_smart_object` stores the new HTML as a new bundle and re-points the
placement, so it can be undone too.

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
or `x`/`y`; `side`: `below` or `right`; default: the selection), `style`
(`teacher` | `neat`), `speed` (0.25–4), `color`, `size` (capital height, px),
`step_mode` (`all` | `one-line-per-click`). The board synthesises the strokes
(`rupok_ink::write`: Hershey single-stroke glyphs or the teacher's own,
maths layout, natural variation, pen timing), matches the problem's ink
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
`update_smart_object`, `add_media`, `add_model`, `delete_item`, `set_page_background`,
`replace_items_with_text`, `step_object`, `write_handwriting`). `add_media`
carries a `url` (YouTube or a trusted embed) or a base64 `file_base64` (video
or audio); `add_model` a base64 `.glb` as `file_base64`. The MCP tools
`add_youtube`, `add_media` and `add_3d_model` read files from the workspace and
send these ops when called with `path: "live"`. Pages are 1-based and `page`
omitted means the page on screen; coordinates are slide pixels in the
1920 × 1080 frame.

Each op has an access level: **read** (`get_*`, `snapshot`), **control**
(`go_to_page`, `point_at`, `set_board_theme` with only `ui`) or **write**
(everything else). With *Allow AI to edit live* off the board refuses control
and write ops; in Student view it refuses write ops. A write runs as **one**
Loro commit with the message `rupok-agent/1 {"agent": …, "op": …}` and the
board sends its `doc/update` *before* the `result`, so the server's mirror is
current when the agent gets the answer. The server waits 30 s for a result.

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
