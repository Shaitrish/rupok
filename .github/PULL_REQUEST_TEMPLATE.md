## What and why

<!-- What does this change, and why? Link the issue or plan task, e.g. "Closes #12" or "docs/roadmap/tauri-desktop.md, task 3". -->

## How it was tested

<!-- Commands you ran, what you tried by hand (device, browser, pen/touch). -->

## Checklist

- [ ] I have signed the [Contributor License Agreement](../CLA.md) (the CLA bot asks on your first PR).
- [ ] Tests added or updated, and they pass: `cargo test --workspace` (and `node --test "packages/object-sdk/test/*.test.mjs"` if the SDK changed).
- [ ] `cargo fmt --all --check` and `cargo clippy --workspace -- -D warnings` are clean; the wasm build still works.
- [ ] **Perf-sensitive change?** (ink, core, render, the engine's pointer path, package I/O) I ran `cargo run -p rupok-bench --release` before and after and pasted both budget tables below. Otherwise: not perf-sensitive.
- [ ] **Format, protocol or SDK change?** `spec/` (and `packages/object-sdk/index.d.ts`) is updated in this PR, with a link to the accepted RFC. Otherwise: no spec impact.
- [ ] **Agent-controllable:** live op / MCP tool added (and a row in `docs/agent-capabilities.md`), or marked *teacher only* with a reason. Otherwise: no user-facing change.
- [ ] Docs updated (README, ROADMAP status, plan doc) if behaviour or status changed.

<details>
<summary>Bench results (if perf-sensitive)</summary>

```text
before:

after:
```

</details>
