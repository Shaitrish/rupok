<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/rupok-lockup-dark.svg">
    <img src="brand/rupok-lockup.svg" alt="Rupok — Mind Over Matter" height="96">
  </picture>
</p>

<p align="center"><b>The open-source whiteboard for teachers, with your own AI built in.</b><br>
Write on slides with a pen. Drop in live, interactive lessons your AI made. Replay any class, stroke by stroke.</p>

<p align="center"><a href="https://rupok.app"><b>rupok.app</b></a> · works in Chrome and Edge, installable as an app</p>

> **Status: early alpha (v0.1).** The board works in the browser today at
> [rupok.app](https://rupok.app): pen and highlighter, slides and PDFs,
> Smart Objects with presets, Teach mode, replay and `.rpk` lesson files.
> Windows and Linux installers are built by CI. Much more is in active
> development. See [ROADMAP.md](ROADMAP.md) for what is done, what is in
> progress, and what you can help build.

---

## For teachers

Rupok is a free replacement for paid board apps like Note3, made for
online and classroom teaching.

- **Write naturally.** A GPU ink engine keeps the pen fast and smooth, on an
  86-inch panel, a laptop or a pen tablet. Palm rejection, pressure,
  highlighter, shapes and an infinite board.
- **Live lessons from your AI.** Ask your own AI agent (Claude Code, Codex,
  OpenCode…) for "a binary counter for Grade 7" or "a quiz on networking".
  It builds a **Smart Object**: a live, interactive diagram, simulation,
  timeline, quiz or game that sits on your slide. You write on top of it, and
  your clicker steps through it like slide animations.
- **Every class is recorded.** Every stroke is timestamped, so you can replay
  a lesson step by step for students who missed it.
- **Works offline.** Lessons are single `.rpk` files. Smart Objects run without
  internet, and common libraries ship inside the app.
- **Your files, your Drive** *(planned)*. Lessons sync through your own Google
  Drive. No Rupok account, no lock-in.
- **Safe by design.** Smart Objects run in a locked-down sandbox: no network,
  no access to your files.

### What works today, and what's next

| | Feature |
|---|---|
| ✅ | Fast pen ink with pressure, smoothing and palm rejection (engine) |
| ✅ | Pages, slides and infinite boards; undo/redo; lasso; text and images (engine) |
| ✅ | `.rpk` lesson files that keep the full class history |
| ✅ | AI agents create lessons and Smart Objects through the Rupok MCP server |
| ✅ | Smart Object SDK, examples and an AI skill pack |
| 🚧 | Board in the browser: toolbar, PDF import, Smart Objects, save/open, replay |
| 📋 | Desktop app for Windows and Linux; draw over any app (overlay) |
| 📋 | Google Drive sync, course library, agent panel inside the app |
| 📋 | Live broadcast to students, live quizzes, subject tools, Bangla |

The full list, with a plan for every item, is in **[ROADMAP.md](ROADMAP.md)**.

### Try the AI workflow now

You need [Rust](https://rustup.rs) and an AI agent that supports MCP.

```sh
git clone https://github.com/shaitrish/rupok && cd rupok
cargo install --path crates/rupok-cli          # installs the `rupok` command
claude mcp add rupok -- rupok mcp --workspace ~/Lessons   # Claude Code
```

For Codex, OpenCode and others, see [`skills/AGENTS.md`](skills/AGENTS.md).
For Claude Code, copy [`skills/rupok-smart-objects`](skills/rupok-smart-objects/)
into `~/.claude/skills/`. Then ask your agent: *"Make me a 6-slide Rupok
lesson on binary numbers for Grade 7, with an interactive counter."*

Check any Smart Object by hand:

```sh
rupok validate examples/objects/binary-counter.html --steps 10
```

---

## For contributors

Rupok is a Rust core compiled natively and to WebAssembly, a SolidJS UI, and
(soon) a Tauri 2 desktop shell. Read [AGENTS.md](AGENTS.md) for conventions
(it is written for AI coding agents, but it is the shortest tour for humans
too) and [CONTRIBUTING.md](CONTRIBUTING.md) for the workflow.

### Architecture

```text
 Teacher's AI agent (Claude Code, Codex, OpenCode…)
        │ MCP over stdio
        ▼
 ┌───────────────────────┐          ┌───────────────────────────────────────┐
 │ rupok CLI + MCP      │          │ Board UI: SolidJS (apps/board)        │
 │ crates/rupok-cli        │          │   Smart Object host: sandboxed        │
 └──────────┬────────────┘          │   iframes + SDK (packages/object-sdk) │
            │                       │ Desktop shell: Tauri 2 (planned)      │
            │                       └───────────────────┬───────────────────┘
            │                                           │ wasm-bindgen
            │                       ┌───────────────────▼───────────────────┐
            │                       │ rupok-wasm: board engine                │
            │                       │ tools, gestures, selection, replay    │
            │                       └──────┬───────────────────────┬────────┘
            ▼                              ▼                       ▼
 ┌───────────────────────┐   ┌───────────────────────┐ ┌───────────────────────┐
 │ rupok-format            │──▶│ rupok-core              │ │ rupok-render            │
 │ .rpk zip, Smart Object│   │ Loro CRDT lesson doc, │ │ wgpu ink: WebGPU,     │
 │ bundles, validation   │   │ undo, merge, timeline │ │ WebGL2, Vulkan, Metal,│
 └───────────────────────┘   └──────────┬────────────┘ │ DX12                  │
                                        ▼              └───────────────────────┘
                             ┌───────────────────────┐
                             │ rupok-ink               │
                             │ filter, pressure,     │
                             │ simplify, point codec │
                             └───────────────────────┘
```

### Repository layout

| Path | What | License |
|---|---|---|
| `crates/rupok-ink` | Ink pipeline and the stroke point codec | AGPL-3.0-or-later |
| `crates/rupok-core` | Lesson document on Loro, spatial scene index | AGPL-3.0-or-later |
| `crates/rupok-format` | `.rpk` package, Smart Object bundles and validator | Apache-2.0 |
| `crates/rupok-render` | GPU ink renderer (wgpu) | AGPL-3.0-or-later |
| `crates/rupok-wasm` | Board engine and wasm-bindgen API | AGPL-3.0-or-later |
| `crates/rupok-cli` | The `rupok` CLI and MCP server | AGPL-3.0-or-later |
| `bench/` | Performance harness and budgets | AGPL-3.0-or-later |
| `apps/board` | Board UI (Vite + SolidJS) | AGPL-3.0-or-later |
| `packages/object-sdk` | Smart Object SDK (`@rupok/object-sdk`) | Apache-2.0 |
| `spec/` | `.rpk` format and Smart Object specs | Apache-2.0 |
| `skills/` | Skill pack and instructions for AI agents | Apache-2.0 |
| `examples/objects/` | Example Smart Objects (validated in CI) | AGPL-3.0-or-later |
| `docs/roadmap/` | A plan for every planned feature | AGPL-3.0-or-later |
| `DESIGN.md`, `brand/` | Design system and logo | AGPL-3.0-or-later |

### Build and test

The Rust toolchain (1.100, with `rustfmt`, `clippy` and the
`wasm32-unknown-unknown` target) is pinned in `rust-toolchain.toml`; rustup
installs it on first use.

```sh
cargo test --workspace                       # all Rust tests
cargo run -p rupok-bench --release             # performance budgets (add -- --check to fail on a miss)
cargo fmt --all --check && cargo clippy --workspace -- -D warnings
cargo build -p rupok-wasm --target wasm32-unknown-unknown --release
node --test "packages/object-sdk/test/*.test.mjs"   # SDK tests (Node 22+)
```

Run the board UI (needs [Bun](https://bun.sh) or npm, and
`cargo install wasm-bindgen-cli --version 0.2.130`):

```sh
cd apps/board
bun install
bun run wasm      # builds rupok-wasm and generates src/wasm/
bun run dev
```

### Good first issues

- Issues labelled
  [`good-first-issue`](https://github.com/shaitrish/rupok/labels/good-first-issue).
- [Known issues](ROADMAP.md#known-issues-good-first-fixes) in ROADMAP.md:
  small, well-specified fixes found while writing the specs.
- Tasks tagged `good-first-issue` inside the plans in
  [`docs/roadmap/`](docs/roadmap/).
- New example Smart Objects for your subject in `examples/objects/`.

### Specs

- [`spec/rupok-format.md`](spec/rupok-format.md): the lesson file format.
- [`spec/smart-object.md`](spec/smart-object.md): Smart Objects, the sandbox
  and the bridge protocol.

Changes to either go through an RFC ([CONTRIBUTING.md](CONTRIBUTING.md#rfcs)).

## License

The app is **AGPL-3.0-or-later** ([LICENSE](LICENSE)). The specs, the
`rupok-format` crate, the Smart Object SDK and the skill pack are
**Apache-2.0**, so anyone can build compatible tools, objects and agents.
[COPYING.md](COPYING.md) explains the split. Your lessons and the Smart
Objects you make are yours.

Rupok is made by [Shaitrish](https://github.com/shaitrish) and contributors.
Please read our [Code of Conduct](CODE_OF_CONDUCT.md) and
[Security policy](SECURITY.md).
