# Agent panel (ACP) and live agent edits

`Status: 📋 planned · Phase 2 · Difficulty: advanced`

A chat panel inside Rupok where the teacher talks to their own AI agent (Claude Code, Codex, OpenCode, Gemini CLI…) over the Agent Client Protocol, and the agent edits the open lesson live, even during class.

## Goal

Today an agent edits `.rpk` files on disk through the MCP server
(`crates/rupok-cli`). That works for preparing lessons, but teachers also want
to ask mid-lesson: "add a slide with a worked example of 1011 + 0110". This plan
makes Rupok an **ACP client** that runs the teacher's agent, shows its
messages and tool calls, asks the teacher for permissions, and routes the
agent's MCP edits into the **live document**, merged by Loro with whatever the
teacher is drawing.

## User story

- *As a teacher*, I open the Agent panel, pick "Claude Code" (already installed
  on my laptop with my own subscription), and type "make a 5-slide intro to
  networks with a quiz at the end"; slides appear while I watch.
- *As a teacher*, in the middle of class I say (type) "add a diagram of a
  router connecting two networks next to my drawing"; it appears on the current
  page without interrupting my pen.
- *As a teacher*, the agent asks "Write 3 files?" and I approve or deny.
- *As a teacher*, I don't like what the agent added; one tap reverts the
  agent's last change and keeps my own ink.

## Scope

- ACP client in the desktop app: spawn agent subprocess, JSON-RPC over stdio.
- Agent registry: detect installed agents and adapters; custom command entry.
- Panel UI: conversation, streamed text, tool-call cards, plan, diffs,
  permission prompts, cancel.
- The Rupok MCP server passed to every session (`session/new` `mcpServers`).
- **Live edits**: MCP server attached to the running app, edits merged into the
  open lesson; per-batch revert.
- Present-mode safety rules.

## Non-goals

- Shipping or paying for a model. The teacher brings their own agent and plan.
- A built-in LLM client (no API keys handled by Rupok).
- Browser-only support (spawning local processes needs the desktop app).

## Design

### Components

```text
crates/rupok-acp/                  (new, AGPL, native only) ACP client wrapper
  src/lib.rs                     AgentProcess, Session, events
  src/registry.rs                known agents: command, args, detection
apps/desktop/src-tauri/src/agent.rs   (new) Tauri commands + event forwarding
apps/desktop/src-tauri/src/live.rs    (new) live-edit IPC endpoint
apps/board/src/agent/                 (new) panel UI
crates/rupok-cli                         + `mcp --attach <endpoint>` mode
```

Use the `agent-client-protocol` Rust crate for message types. Main flows:

```text
rupok (client)                      agent subprocess
  initialize ───────────────────────▶
  session/new {cwd, mcpServers:[rupok mcp --attach …]} ─▶
  session/prompt {text} ────────────▶
  ◀──────── session/update (message chunks, tool_call, plan)
  ◀──────── session/request_permission ──▶ UI prompt ──▶ reply
  ◀──────── fs/read_text_file / fs/write_text_file (client capability)
  session/cancel ───────────────────▶
```

- `fs/*` capabilities are scoped to the library folder (same rules as
  `crates/rupok-cli/src/workspace.rs`); writes always need a permission prompt
  outside `.rpk` files.
- Terminal capability: not offered in v1.

### Live edits

The MCP server started for the session runs in **attached mode**:

```text
agent ──MCP──▶ rupok mcp --attach <endpoint> --token <t>
                     │  (IPC: Unix socket / Windows named pipe, token-authenticated)
                     ▼
            desktop app live.rs ──▶ webview Board (rupok-wasm Engine)
```

Protocol (JSON lines, versioned `live: 1`):

| Request | Reply |
|---|---|
| `snapshot {}` | `.rpk` bytes of the open lesson (base64) + version vector |
| `apply { updates, bundles?, assets?, label }` | `{ ok, page_ids, item_ids }` |
| `focus {}` | current page id, camera, selection (for "next to my drawing") |

- The attached MCP server keeps a local `Lesson` replica: it loads the
  snapshot, runs the existing operations in `crates/rupok-cli/src/ops.rs` against
  it, and sends `Lesson::export_updates(&vv_before)` plus any new Smart Object
  bundles / assets (they live in `Package.objects` / `Package.assets`, not in
  the Loro doc).
- The webview applies them with a new engine method:

  ```rust
  // crates/rupok-wasm/src/engine.rs (new)
  pub fn apply_remote(&mut self, updates: &[u8], bundles: Vec<(String, ObjectBundle)>, assets: Vec<(String, Vec<u8>)>, label: &str) -> Result<AgentBatch, String>;
  ```

  It inserts bundles/assets into `self.pkg`, calls `Lesson::import` (CRDT
  merge — teacher strokes made meanwhile are kept), then `Engine::resync`.
- Every Smart Object from the agent is validated
  (`ObjectBundle::validate`) before `apply_remote` accepts it.
- Loro's undo manager only undoes local operations, so Ctrl+Z will not undo
  agent edits. Instead each `apply` is an **agent batch** shown in the panel
  with a **Revert** button (see open questions for the mechanism).

### Present-mode rules

- In `present` mode, agent edits never change the current page's existing
  items; they may add items, add pages after the current one, or update the
  agent's own items. The engine enforces this in `apply_remote` by diffing
  touched item ids against the page's pre-batch item set.
- Agent edits never move the camera or change page while the teacher is
  writing (`gestures` non-empty); a toast says "Agent added 2 items — show".
- A "Pause agent edits" switch queues batches until resumed.

### Agent registry

`registry.rs` lists known agents with detection commands and ACP launch
arguments (some agents speak ACP natively, others through an adapter
package). The list is data (`agents.json`) so contributors can add agents
without code. Custom entries: command + args + env.

## Implementation tasks

1. **`rupok-acp` crate**: spawn process, initialize, new session, prompt,
   stream updates; integration test against a mock agent binary in
   `crates/rupok-acp/tests/`. *(advanced)*
2. **Registry + detection** with `agents.json`. *(good-first-issue)*
3. **Tauri commands/events** (`agent_start`, `agent_prompt`, `agent_cancel`,
   `agent_permission_reply`). *(intermediate)*
4. **Panel UI**: messages, streaming, tool-call cards, permission dialog,
   cancel. *(intermediate)*
5. **`fs/*` capability** scoped to the library with workspace rules.
   *(intermediate)*
6. **Live IPC endpoint** in `live.rs` with token auth. *(advanced)*
7. **`rupok mcp --attach`** mode in `crates/rupok-cli` (replica + update
   export). *(advanced)*
8. **`Engine::apply_remote`** with validation, merge, resync, and unit tests
   using two `Lesson` replicas. *(advanced)*
9. **Agent batches + Revert**. *(advanced)*
10. **Present-mode rules** and "Pause agent edits". *(intermediate)*
11. **`focus` request** so agents can place items near the teacher's current
    view and selection. *(intermediate)*

## Acceptance criteria and tests

- Mock-agent integration test: a prompt produces streamed chunks, one tool
  call, one permission request; denying it is reported back to the agent.
- Live merge test (engine unit test): teacher adds 3 strokes while an agent
  batch adds 1 page and 1 object from a replica forked earlier; after
  `apply_remote` both the strokes and the agent content exist, item order is
  identical on both replicas.
- A batch containing an invalid Smart Object is rejected with the validator
  messages and changes nothing.
- In present mode, an agent update that deletes an existing item on the
  current page is refused.
- Revert removes the batch's items and keeps strokes made after it.
- The IPC endpoint rejects connections without the token.

## Performance budget

| Metric | Budget |
|---|---|
| `apply_remote` for a batch adding 1 page with 1 object | ≤ 30 ms (no pen hitch) |
| Pen latency while an agent batch is applied | wet frame p99 ≤ 4 ms (unchanged) |
| Panel idle CPU | ≈ 0 when the agent is idle |

## Open questions

- Revert mechanism: Loro history (diff the batch's changes and apply the
  inverse) vs recording inverse operations ourselves in `apply_remote`. Needs
  a spike with the Loro version in `Cargo.toml`.
- Should agent-created items carry an author marker (e.g. an optional
  `author` item key)? That is a format change and would **require an RFC**
  (`spec/rupok-format.md#items`).
- Voice prompts during class (speech-to-text) — later, and only on-device?
- How much lesson context to give the agent per prompt (outline only vs
  current page contents)?

## References

- Agent Client Protocol: <https://agentclientprotocol.com>
- `crates/rupok-cli/src/{mcp.rs,ops.rs,workspace.rs}`
- `crates/rupok-core/src/doc.rs` — `Lesson::import`, `Lesson::export_updates`, `Lesson::version`
- `spec/smart-object.md` — `#validation`
- Related: [knowledge-base](./knowledge-base.md), [tauri-desktop](./tauri-desktop.md),
  [live-broadcast](./live-broadcast.md)
