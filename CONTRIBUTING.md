# Contributing to Rupok

Thank you for helping teachers. Contributions of every size are welcome: a
bug report from a classroom, a translation, an example Smart Object for your
subject, a renderer optimisation.

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
Security problems go to [SECURITY.md](SECURITY.md), not public issues.

## Ways to help

- **Teachers:** report what breaks or slows you down in class, share lesson
  ideas, test pre-releases on your board, translate.
- **Developers:** pick an item from [ROADMAP.md](ROADMAP.md). Every 📋 item
  has a plan in [`docs/roadmap/`](docs/roadmap/) split into PR-sized tasks.
- **Designers:** see [DESIGN.md](DESIGN.md).
- **Agent builders:** improve [`skills/`](skills/) and the example objects.

## Good first issues

- Issues labelled `good-first-issue`.
- [Known issues](ROADMAP.md#known-issues-good-first-fixes): small,
  well-specified gaps between the specs and the code.
- Tasks tagged `good-first-issue` in the plan docs.

Comment on the issue to claim it, so two people don't do the same work. If
you are stuck for more than a day, ask in the issue. Small questions are
welcome.

## Setup

| Tool | Version | Notes |
|---|---|---|
| Rust | pinned in `rust-toolchain.toml` (1.100) | rustup installs it with `rustfmt`, `clippy` and `wasm32-unknown-unknown` |
| Node.js | 22 or newer | SDK tests |
| Bun or npm | recent | `apps/board` |
| wasm-bindgen-cli | 0.2.130 (matches `Cargo.lock`) | `cargo install wasm-bindgen-cli --version 0.2.130` |

```sh
cargo test --workspace
node --test "packages/object-sdk/test/*.test.mjs"
```

## Workflow

1. Fork, and create a branch from `main`.
2. Keep pull requests small and focused: one task from a plan is ideal.
3. Before pushing, run:

   ```sh
   cargo fmt --all
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   cargo build -p rupok-wasm --target wasm32-unknown-unknown --release
   node --test "packages/object-sdk/test/*.test.mjs"   # if you touched the SDK
   ```

4. Add or update tests for every behaviour change. Bug fixes come with a test
   that failed before the fix.
5. If the change can affect performance, run the bench (see below) and paste
   the budget table into the PR.
6. If the change touches the file format, the Smart Object protocol or the
   SDK API, update `spec/` in the same PR. A non-editorial spec change needs
   an accepted RFC.
7. Sign off every commit (DCO, below) and open the PR. CI must be green.

Commit messages: an imperative subject of 72 characters or fewer, with an area
prefix when it helps (`core: skip items with unknown kinds`), and a body that
says why.

## Developer Certificate of Origin (DCO)

Rupok does not use a CLA. Instead, every commit must be signed off to
certify the [Developer Certificate of Origin 1.1](https://developercertificate.org/):
that you wrote the change or have the right to submit it under the project's
licenses.

```sh
git commit -s -m "render: cull empty chunks"
```

This adds a `Signed-off-by: Your Name <you@example.com>` line, using your real
name. To sign off earlier commits: `git rebase --signoff main`. Pull requests
with unsigned commits cannot be merged.

Contributions are licensed under the license of the part they change (see
[COPYING.md](COPYING.md)).

## RFCs

The `.rpk` format and the Smart Object protocol are contracts with other
implementations and with every lesson teachers have already saved. They
change through an RFC.

**Needs an RFC:** any change to `spec/rupok-format.md` or
`spec/smart-object.md` beyond editorial fixes; new `.rpk` entries, document
keys, item kinds or version bumps; new bridge messages; changes to the
`rupok` SDK API or theme tokens; new `rupoklib` libraries or version changes
that break documented usage.

**Doesn't need one:** bug fixes that make code match the spec, editorial
spec fixes, internal refactors, UI changes.

Process:

1. Open an issue labelled `rfc` with the problem and a sketch, to check
   interest early.
2. Open a pull request adding `rfcs/NNNN-short-title.md` (use the next free
   number). Include:
   - **Summary** and **Motivation** (a teacher's story helps);
   - **Design**: exact keys, types, messages and normative text;
   - **Compatibility**: which version gate changes, what old readers do with
     new files and new readers with old files, and the migration;
   - **Test vectors**: example bytes, JSON or message sequences;
   - **Alternatives** and **Unresolved questions**.
3. The comment period is at least 7 days. Maintainers then accept, ask for
   changes, or decline, with reasons.
4. The implementation PR changes the spec, the reference code
   (`rupok-format`, `rupok-core`, `rupok-ink`, `packages/object-sdk`) and the tests
   together.

## Performance budgets

Rupok must stay fast on cheap classroom PCs and huge panels. `bench/`
simulates a long class (100,000 strokes by default) and enforces:

| Budget | Limit |
|---|---|
| Stroke commit, p99 | ≤ 5 ms |
| Open a lesson (load + read items) | ≤ 1000 ms |
| Pan frame at 100 % zoom, p99 | ≤ 8.3 ms (120 Hz) |
| Wet-ink frame while writing, p99 (pen latency) | ≤ 4 ms |

```sh
cargo run -p rupok-bench --release              # report
cargo run -p rupok-bench --release -- --check   # exit 1 if a budget is missed
```

**Perf-sensitive** means anything touching `rupok-ink`, `rupok-core`,
`rupok-render`, the engine's pointer path in `rupok-wasm`, or `.rpk`
reading/writing. Run the bench before and after, on the same machine, and
put both tables in the PR. A change that misses a budget needs a very good
reason, and the budget only changes through discussion in the PR.

## Code conventions

The short version (details in [AGENTS.md](AGENTS.md)):

- The core crates also build for `wasm32-unknown-unknown`. No
  `std::time::SystemTime`/`Instant`, threads, files or network in `rupok-ink`,
  `rupok-core`, `rupok-format` (except `cfg(not(target_arch = "wasm32"))`
  helpers) or `rupok-wasm`. Wall-clock time is passed in as `now_ms`.
- Every document mutation commits once, so one user action is one undo step,
  one replay step and one sync delta.
- Untrusted input (`.rpk` files, Smart Object HTML, MCP arguments) must never
  panic, allocate without bounds, or escape its sandbox or workspace.
- Keep `spec/` in sync with the code.

## Reviews

Maintainers aim to give a first response to pull requests within a week. A
review is about the code, not the person. Disagreement is fine; be kind, and
assume good intent.
