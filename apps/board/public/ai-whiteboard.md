# The AI whiteboard for teachers

> Ask Claude, ChatGPT, Codex or OpenCode for a simulation or quiz and teach it on your smartboard a minute later. Rupok is the free, open-source AI whiteboard for teachers, with an MCP server.

Canonical: https://rupok.app/ai-whiteboard/ · Language: en · Updated: 2026-11-23

Ask your AI for a simulation, a 3D model or a quiz. It lands on your slide as a live Smart Object you can write on with the pen and step through with your clicker. No new panel to buy: Rupok brings AI to the board you already have.

## From prompt to lesson in a minute

1. **Ask.** “Make a projectile-motion simulation for Class 9 with angle and speed sliders, Earth vs Moon vs Mars.”
2. **It lands on your slide.** Your AI writes a Smart Object: one self-contained HTML file that runs live on the board.
3. **Teach it.** Use it with touch, write on top of it with the pen, and press Next on your clicker to reveal each step.
4. **It’s in the replay.** Students who open the lesson file get the same live object, still working.

## What teachers make with it

These all ship with Rupok as examples. Try them on the [home page](https://rupok.app/#lessons).

- **Physics:** projectile motion, Ohm’s law circuits with moving electrons.
- **Chemistry:** water, methane and benzene in 3D; methane burning at particle level.
- **Maths:** y = a·sin(bx + c) with sliders.

- **ICT:** a binary counter that reveals place values one by one.
- **History:** a timeline of computing, one era per click.
- **Quizzes:** four questions with instant feedback and a score.

## Bring your own AI

- **Any chatbot**: Ask ChatGPT, Claude or Gemini for “a single self-contained HTML file”, then paste it onto the board or drop the .html file in.
- **Agents over MCP**: Claude Code, Codex and OpenCode build whole lessons through Rupok’s MCP server, and check every object before saving it.
- **Claude connector**: Manage lessons from Claude itself, with no install: coming soon with Rupok Cloud.

## Connect Claude, Codex or OpenCode (MCP)

The `rupok` command-line tool includes an MCP server. Your agent can create lessons, add pages and add validated Smart Objects, but only inside the folder you choose.

```sh
git clone https://github.com/Shaitrish/rupok && cd rupok
cargo install --path crates/rupok-cli      # installs the `rupok` command
claude mcp add rupok -- rupok mcp --workspace ~/Lessons   # Claude Code
```

- **Codex CLI:** in `~/.codex/config.toml`, add `[mcp_servers.rupok]` with `command = "rupok"` and `args = ["mcp", "--workspace", "/home/me/Lessons"]`.
- **OpenCode:** in `opencode.json`, add `"mcp": { "rupok": { "type": "local", "command": ["rupok", "mcp", "--workspace", "/home/me/Lessons"] } }`.
- **Any MCP client:** command `rupok`, arguments `mcp --workspace <folder>`, transport stdio.

Then ask: *“Make me a 6-slide Rupok lesson on binary numbers for Grade 7, with an interactive counter.”* The [agent guide](https://github.com/Shaitrish/rupok/blob/main/skills/AGENTS.md) lists all 11 tools. You need [Rust](https://rustup.rs) to build the CLI today; release builds are coming.

## Safe in a classroom

- **Sandboxed.** Every Smart Object runs in a locked-down frame. It can’t read your files, your other tabs or the rest of the lesson.
- **Offline by default.** Objects can’t reach the internet unless the lesson explicitly allows a specific site.
- **Checked before it lands.** The MCP server validates every object and rejects broken or unsafe ones, so the agent fixes them first.
- **Open spec.** The [Smart Object spec](https://github.com/Shaitrish/rupok/blob/main/spec/smart-object.md) is public (Apache-2.0), so anyone can audit it or build objects.

## Questions people ask

### What is an AI whiteboard?

A whiteboard where AI helps make the lesson. In Rupok, an AI writes interactive Smart Objects (simulations, 3D models, quizzes) that run live on your slide while you teach with the pen.

### Do I need to buy an AI smartboard?

No. Rupok runs on the board you already have, in Chrome or Edge. The AI is the assistant you already use.

### Which AI works with Rupok?

Any AI that can write an HTML file: ChatGPT, Claude or Gemini (paste the file onto the board). Claude Code, Codex and OpenCode can build whole lessons through Rupok’s MCP server.

### Does it cost anything?

Rupok is free. You use your own AI subscription, or a free AI plan. Optional Rupok AI credits are planned for the paid Pro plan.

### Can a Smart Object steal data or go online?

No. Objects run in a sandboxed frame with no access to your files or the rest of the page, and network access is blocked unless the lesson allows a specific site.
