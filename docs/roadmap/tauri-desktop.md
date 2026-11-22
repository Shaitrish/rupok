# Desktop app (Tauri 2) for Windows and Linux

`Status: 📋 planned · Phase 1 · Difficulty: intermediate`

Wrap the browser board in a Tauri 2 shell so teachers get an installable, offline app that opens `.rpk` files, saves to disk safely and hosts Smart Objects securely.

## Goal

Ship `apps/desktop`: a Tauri 2 application that loads the `apps/board` UI
(SolidJS + `rupok-wasm`), adds native file handling, autosave, the Smart Object
host (sandbox, `rupoklib://`, SDK injection) and installers for Windows and Linux,
built by CI on every tag.

## User story

- *As a teacher*, I double-click `binary-numbers.rpk` on the classroom PC and
  Rupok opens it full-screen, even without internet.
- *As a teacher*, the power goes out mid-class; when I restart, my lesson is
  there up to the last few seconds.
- *As a teacher*, I install one `.exe` on Windows or an AppImage on Ubuntu; no
  admin rights, no account.
- *As a teacher*, my AI agent can find the `rupok` command to run its MCP
  server because the installer put it on my PATH (opt-in).

## Scope

- Tauri 2 project in `apps/desktop` using the built `apps/board` as `frontendDist`.
- Open/Save/Save As/Recent files; `.rpk` file association; open from argv;
  single instance (a second launch focuses the window and opens the file).
- Crash-safe saving and autosave.
- Smart Object host duties from `spec/smart-object.md#host-duties`.
- Installers: Windows (NSIS `.exe`, MSI) and Linux (AppImage, `.deb`).
- Bundled `rupok` CLI as a sidecar for the MCP server.
- Release workflow with code signing hooks and the Tauri updater (opt-in).

## Non-goals

- macOS builds (welcome later; nothing here should block them).
- Desktop overlay annotation: see [desktop-overlay](./desktop-overlay.md).
- Sync: see [google-drive-sync](./google-drive-sync.md).
- Moving the engine out of wasm into the Rust side. The engine stays in the
  webview (`rupok-wasm::Board`) so desktop and browser behave identically.

## Design

### Layout

```text
apps/desktop/                       (new)
  package.json                      scripts: dev, build (runs apps/board build first)
  src-tauri/
    Cargo.toml                      tauri 2, tauri-plugin-{dialog,fs,single-instance,updater,shell}
    tauri.conf.json                 fileAssociations: .rpk; CSP; bundle targets
    capabilities/default.json       least-privilege permissions
    src/main.rs                     builder, plugins, commands
    src/files.rs                    open/save/autosave commands
    src/objects.rs                  Smart Object protocol handler (rupokobj)
    src/libs.rs                     vendored library handler
```

`apps/desktop/src-tauri` must be added to the workspace `members` (a
`Cargo.toml` change for the maintainers) or kept as its own workspace.

### Files and saving

The engine produces bytes with `Board::save()` (`.rpk`, via
`Engine::save` → `Package::write`) and consumes them with `Board::load()`.
The Rust side only moves bytes:

```rust
#[tauri::command] async fn open_lesson(path: PathBuf) -> Result<Vec<u8>, String>;
#[tauri::command] async fn save_lesson(path: PathBuf, bytes: Vec<u8>) -> Result<(), String>;
#[tauri::command] async fn autosave(doc_key: String, bytes: Vec<u8>) -> Result<(), String>;
```

- **Atomic save**: write `<name>.rpk.tmp` in the same directory, `fsync`,
  rename over the target. Never truncate the original first.
- **Validate before replacing**: run `rupok_format::Package::read` on the new
  bytes before the rename; refuse to overwrite with a corrupt file.
- **Autosave**: the UI calls `autosave` when `Board::revision()` changed and the
  pen has been up for 2 s, at most every 10 s. Autosaves go to the app data
  directory (`autosave/<doc_key>.rpk`), not over the user's file. On start,
  offer to restore an autosave newer than the file.
- Large transfers: pass bytes as `tauri::ipc::Response` / raw body to avoid
  JSON number arrays.

### Smart Object host

Objects are rendered by the UI as `<iframe sandbox="allow-scripts">`. The
desktop serves bundle files through a custom protocol:

```text
rupokobj://<doc_key>/<bundle>/<path>     bundle files (from the open Package)
```

The scheme name starts with a letter on purpose (see the `rupoklib://` note below).

The webview registers the bundle bytes for the open document with a command
(`register_bundles(doc_key, bundles)`), and `objects.rs` serves them with:

- `Content-Security-Policy` from `spec/smart-object.md#network` (network
  hosts appended only if `permissions.network` lists them);
- the SDK (`packages/object-sdk/sdk.js`) injected as the first `<script>` in
  `<head>` of the entry HTML;
- `Access-Control-Allow-Origin: *` and correct MIME types.

**`rupoklib://` cannot be a registered URL scheme.** URL schemes must start with
a letter (WHATWG URL "scheme start state"), so browsers parse
`rupoklib://three/three.module.js` as a *relative* URL. The host therefore:

1. rewrites `rupoklib://` references in the entry HTML and in `.js`/`.mjs`/`.css`
   bundle files to `rupokobj://lib/<name>/<file>` while serving them, and
2. injects an import map `{"imports": {"rupoklib://": "rupokobj://lib/"}}` so module
   specifiers like `import * as THREE from "rupoklib://three/three.module.js"`
   resolve, and
3. as a fallback, serves any request whose path contains a `rupoklib:` segment
   from the vendored libraries.

The vendored libraries live in `apps/desktop/src-tauri/libs/<name>/…`,
produced by `scripts/vendor-libs.mjs` (named in
`crates/rupok-format/src/object.rs`; not written yet). On Windows, WebView2 maps
custom schemes to `http://<scheme>.localhost`; use Tauri's
`convertFileSrc`-style helper on the UI side instead of hard-coding URLs.

### Rendering backends

`Board::create(canvas, force_webgl)` prefers WebGPU and falls back to WebGL2.
WebView2 (Windows) has WebGPU; WebKitGTK (Linux) generally does not, so Linux
uses WebGL2. The `forceWebgl` preference already exists in
`apps/board/src/state.ts`. The bench budgets (`bench/`) apply to both.

### MCP sidecar

Bundle the `rupok` binary (`crates/rupok-cli`) as a Tauri sidecar and add a
Settings page that shows the exact MCP config snippet for Claude Code, Codex
and OpenCode (`rupok mcp --workspace <library dir>`), with a copy button.

### Release

`.github/workflows/release-desktop.yml` (new): on tag `v*`, build
`apps/board` (including `npm run wasm`), then `tauri build` on
`windows-latest` and `ubuntu-22.04`; upload installers to a draft GitHub
release. Signing secrets are optional so forks can build unsigned.

## Implementation tasks

1. **Scaffold `apps/desktop`** with Tauri 2 pointing at `apps/board/dist`;
   `bun run dev` opens the board in a window. *(good-first-issue)*
2. **Capabilities file** with least privilege (dialog, fs scoped to picked
   paths, no shell except the sidecar). *(intermediate)*
3. **Open/Save/Save As** commands with atomic write + `Package::read` check;
   UI menu and Ctrl+O/Ctrl+S. *(intermediate)*
4. **Recent files** list (app data JSON, max 10, prune missing). *(good-first-issue)*
5. **File association + single instance + open from argv**. *(intermediate)*
6. **Autosave and restore** prompt. *(intermediate)*
7. **`scripts/vendor-libs.mjs`**: download pinned versions of every library in
   `LIBS`, verify SHA-256, write `libs/<name>/<entry files>` and a
   `libs/manifest.json`. *(intermediate)*
8. **`rupokobj` protocol handler**: bundle files, CSP, SDK injection,
   `rupoklib://` rewriting + import map. Unit tests for the rewriter. *(advanced)*
9. **Object iframe integration** in the UI (bridge host: `init`, `step`,
   `state`, `goto`, `steps`) calling `Board::set_object_step` /
   `Board::set_object_state`. *(advanced, shared with the board UI work)*
10. **MCP sidecar** + Settings page with config snippets. *(intermediate)*
11. **Release workflow** for Windows and Linux installers. *(intermediate)*
12. **Updater** (opt-in setting, signed update manifests). *(intermediate)*

## Acceptance criteria and tests

- Double-clicking a `.rpk` file opens it; a second double-click on another file
  opens it in the running instance.
- Killing the process during a save never leaves a truncated `.rpk` (test:
  Rust unit test that simulates a failed write leaves the original intact).
- Autosave restores strokes made up to 10 s before a crash (manual test script
  in `apps/desktop/TESTING.md`).
- The three examples in `examples/objects/` run in the desktop host with
  steps, state and theme working; a test object that calls
  `fetch("https://example.com")` is blocked by CSP.
- An object importing `rupoklib://three/three.module.js` renders offline.
- CI produces installers for both platforms from a tag on a fork with no secrets.

## Performance budget

| Metric | Budget |
|---|---|
| Cold start to interactive board | ≤ 2 s on a 4-core laptop with SSD |
| Save of a 100k-stroke lesson | ≤ 1 s, UI not blocked (save runs off the pen path) |
| Installer size | ≤ 25 MB (Windows), ≤ 40 MB AppImage, excluding PDFium |
| Pen latency | same as the browser: wet frame p99 ≤ 4 ms (bench) |

## Open questions

- Should the engine move to the Rust side later for very large lessons, with
  the webview as a thin view? (Keep the `Board` API stable either way.)
- Linux: AppImage vs Flatpak as the main channel; WebKitGTK version floor for
  `color-mix()` and import maps used by objects.
- Code-signing certificate for Windows (SmartScreen warnings) — who holds it?
- Per-document `doc_key` for `rupokobj`: random per open, or a hash of the path?

## References

- `spec/smart-object.md` — `#sandbox`, `#host-duties`, `#offline-libraries-rupoklib`, `#bridge-protocol`
- `spec/rupok-format.md` — `#reader-limits-and-safety`
- `crates/rupok-wasm/src/lib.rs` — `Board::create`, `Board::save`, `Board::load`, `Board::revision`
- Related: [desktop-overlay](./desktop-overlay.md), [pdf-import-native](./pdf-import-native.md),
  [acp-agent-panel](./acp-agent-panel.md), [web-viewer](./web-viewer.md)
