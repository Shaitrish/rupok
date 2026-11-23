# AGENTS.md

Guidance for AI coding agents (Claude Code, Codex, OpenCode, Cursor…)
working **on the Rupok codebase**. To *use* Rupok to make lessons, see
[`skills/AGENTS.md`](skills/AGENTS.md) instead.

Rupok is a GPU-accelerated whiteboard and presentation app for teachers: a
Rust core (Loro CRDT, wgpu) compiled natively and to wasm32, a SolidJS UI, and
a Tauri 2 shell (planned). Status: Phase 0, pre-alpha. See
[ROADMAP.md](ROADMAP.md).

## Crate and package map

| Path | Role | Key items |
|---|---|---|
| `crates/rupok-ink` | Pen input pipeline, wasm-safe | `StrokeBuilder` (wet ink), `filter::OneEuro2`, `pressure::PressureCurve`, `simplify`, `codec::{encode_points, decode_points}` (point codec v1), `MIN_WIDTH_RATIO`, `point_radius`, `math` (handwritten maths → LaTeX), `write` (text + LaTeX → timed handwriting strokes) |
| `crates/rupok-core` | Lesson document on Loro | `Lesson` (`doc.rs`: all mutations), model types (`model.rs`: `ItemData`, `StrokeData`, `ObjectData`, `PageSpec`…), `Scene` (`scene.rs`: R*-tree hit-test, eraser, lasso), `DOC_FORMAT` |
| `crates/rupok-format` | `.rpk` package and Smart Object bundles (Apache-2.0) | `Package::{read, write, from_lesson, add_asset, content_markdown}`, `readable` (derived `README.md`/`content.md`/`content.json`), `ObjectBundle::{from_single_html, from_files, from_dir, validate}`, `LIBS`, `safe_path`, `FORMAT_VERSION`, `MIMETYPE` |
| `crates/rupok-live` | Live agent bridge shared by the board and `rupok mcp --live` | `protocol.rs` (WebSocket protocol v1: channels, ports, origin/host checks), `op.rs` (`Op`), `apply.rs` (`apply`: one attributed commit per agent op), `attribution.rs`, `sync` |
| `crates/rupok-render` | wgpu ink renderer | `InkRenderer` (SDF capsules, chunk culling, cached dry layer, wet layer), `Camera`, `Gpu::headless`, `ink.wgsl` |
| `crates/rupok-wasm` | Board engine and wasm-bindgen API | `engine.rs` (`Engine`: tools, gestures, selection), `lib.rs` (`Board` JS API), `replay.rs`, `shapes.rs` |
| `crates/rupok-cli` | `rupok` binary: CLI and MCP server | `cli.rs` (clap), `ops.rs` (shared operations), `mcp.rs` + `tools.rs` (12 MCP tools), `workspace.rs` (path sandbox), `spec.rs`, `live/` (`LiveHub`: loopback WebSocket, pairing, Loro mirror) + `mcp_live.rs` (live tools; docs/ai-agents.md), `chat/` (`rupok agent`: ACP client relaying the in-app chat) + `mcp_http.rs` (MCP over loopback HTTP for it) |
| `bench/` | Performance harness and budgets | `cargo run -p rupok-bench --release [-- --check]` |
| `apps/board` | SolidJS board UI (Vite) | `controller.ts`, `objects.ts` (Smart Object host), `pdf.ts`, `storage.ts`, `live.ts` + `snapshot.ts` + `components/LivePanel.tsx` (Connect AI), `chat.ts` + `components/ChatPanel.tsx` + `markdown.ts` (AI chat), `components/` |
| `packages/object-sdk` | In-object SDK, `window.rupok` (Apache-2.0) | `sdk.js` (ES2019 IIFE), `index.d.ts`, `test/` |
| `spec/` | Normative specs (Apache-2.0) | `rupok-format.md`, `smart-object.md` |
| `examples/objects/` | Validated example Smart Objects | checked by `crates/rupok-format/tests/examples.rs` |
| `skills/` | Agent skill pack (Apache-2.0) | `rupok-smart-objects/SKILL.md`, `rupok-live-board/SKILL.md`, `read-rpk/` (SKILL.md + `read_rpk.py`, tests: `python3 -m unittest discover -s skills/read-rpk/tests`), `AGENTS.md` |
| `docs/roadmap/` | One plan per planned feature | |

Dependency direction: `rupok-ink` ← `rupok-core` ← `rupok-format` ← `rupok-live` ← (`rupok-wasm`,
`rupok-cli`); `bench` uses `rupok-format`; `rupok-render` depends on `rupok-ink` and `rupok-core`. Never
add a dependency that points the other way.

## Commands

```sh
cargo test --workspace                                   # all Rust tests
cargo test -p rupok-format --test examples                 # example objects validate
cargo fmt --all --check
cargo clippy --workspace -- -D warnings
cargo build -p rupok-wasm --target wasm32-unknown-unknown --release   # wasm must keep building
cargo run -p rupok-bench --release                         # performance budgets
node bench/latency/pen-latency.mjs --swiftshader --check   # pen-to-pixel budgets (board built; docs/performance.md)
node --test "packages/object-sdk/test/*.test.mjs"        # SDK (Node 22+)
cargo run -p rupok-cli -- validate examples/objects/binary-counter.html --steps 10
```

The toolchain (1.100, edition 2024) is pinned in `rust-toolchain.toml`.
`.cargo/config.toml` sets the `getrandom` wasm backend flag.

## Conventions

- **No clock in the core.** `rupok-ink`, `rupok-core`, `rupok-format` and `rupok-wasm`
  run in `wasm32-unknown-unknown`, where `SystemTime::now()` panics. Never call
  `SystemTime`/`Instant` there. Wall-clock time is a parameter, `now_ms: f64`
  (ms since the Unix epoch), supplied by the caller. This also keeps tests
  deterministic. Native-only helpers go behind
  `#[cfg(not(target_arch = "wasm32"))]`.
- **Every mutation commits.** Each `Lesson` mutating method ends with exactly
  one `doc.commit()`: one user action is one Loro change, one undo step, one
  replay step and one sync delta. Don't batch unrelated actions; don't leave
  uncommitted changes.
- **Stroke points are immutable.** Moving a stroke changes `ox`/`oy`, never
  `pts`.
- **Untrusted input never panics.** `.rpk` files, Smart Object HTML and MCP
  arguments come from strangers and from AI. Return errors; bound allocations
  (see the limits in `package.rs` and `codec.rs`); validate paths with
  `safe_path`; slice `str` only on char boundaries.
- **Errors:** `thiserror` enums in the library crates; `anyhow` only in the CLI.
- **Formatting:** `cargo fmt` with the default configuration; clippy must be
  clean with `-D warnings`.
- **Tests** live next to the code (`#[cfg(test)] mod tests`), plus
  integration tests in `tests/`. Bug fixes come with a regression test.

## Rules

1. **Keep the specs in sync with the code.** If you change what is stored in
   `doc.loro`, the package layout, the point codec, validation codes,
   `LIBS`, the bridge protocol or the SDK API, update `spec/rupok-format.md` or
   `spec/smart-object.md` (and `packages/object-sdk/index.d.ts`) in the same
   change. Non-editorial spec changes need an RFC (CONTRIBUTING.md); if you
   are an agent, stop and ask the human first. If you find code and spec
   disagreeing, report it rather than silently "fixing" the spec.
2. **Perf-sensitive changes must run the bench.** Anything touching
   `rupok-ink`, `rupok-core`, `rupok-render`, the pointer path in `rupok-wasm`, or
   package I/O: run `cargo run -p rupok-bench --release` before and after, and
   report both budget tables. Budgets: stroke commit p99 ≤ 5 ms, open lesson
   ≤ 1000 ms, pan frame p99 ≤ 8.3 ms, wet frame p99 ≤ 4 ms, pen move → wet
   update p99 ≤ 0.5 ms, wet vs dry pixels ≤ 0.5 %. Changes to the pen path
   (`controller.ts` pointer handlers, `wetInk.ts`, the stroke builder, the
   ink shader) also run `bench/latency` before and after. Wet and dry ink
   must look identical: see "The wet/dry contract" in docs/performance.md.
3. **Keep wasm building.** Run the wasm build after touching any crate that
   `rupok-wasm` depends on.
4. **Respect the license split.** `spec/`, `crates/rupok-format`,
   `packages/object-sdk` and `skills/` are Apache-2.0; don't move AGPL code
   into them (see COPYING.md).
5. **Don't weaken the sandbox.** Never add `allow-same-origin` or other
   sandbox flags, relax the object CSP, or let objects reach the network
   without `permissions.network`.
6. **Commits:** contributors sign the CLA (CLA.md) once; no sign-off needed. Commit only when the human
   asks.
7. **Stay in scope.** Other contributors (human or agent) may be editing
   other parts of the tree; change only what your task needs.
