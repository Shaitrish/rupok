# Security policy

Rupok runs on classroom computers, opens lesson files from anywhere, and
executes HTML written by AI agents. We take security seriously, and we are
grateful for responsible reports.

## Reporting a vulnerability

Email **security@shaitrish.com** *(placeholder address until the project's
contact is set up)*. Please do **not** open a public issue for a
vulnerability. Where available, you may instead use GitHub's private
vulnerability reporting ("Report a vulnerability" on the Security tab).

Include:

- what an attacker can do, and against whom (teacher, student, school);
- the affected component and version or commit;
- steps to reproduce, ideally with a minimal `.rpk` file or Smart Object HTML;
- any suggested fix.

We aim to acknowledge reports within 3 working days and to agree on a fix
and disclosure timeline with you. We will credit you in the release notes
unless you prefer otherwise.

## Supported versions

Rupok is pre-alpha (Phase 0). Only the `main` branch receives security
fixes. A supported-versions table will appear here with the first release.

## Threat model

### What we protect

- The teacher's computer, files and accounts (for example Google Drive
  tokens, once sync exists).
- Students' privacy: names, answers and recordings (planned features).
- The integrity of lessons: a shared file must not silently damage other
  lessons or the app.

### Untrusted inputs

| Input | Why untrusted | Defences |
|---|---|---|
| **Smart Object HTML** written by AI agents or shared by others | It is arbitrary code. It may be buggy, prompt-injected or malicious. | Runs only in `<iframe sandbox="allow-scripts">` with an opaque origin: no storage, cookies, top navigation, popups or modals. A run-time Content-Security-Policy blocks the network unless the manifest lists hosts. It talks to the host only through the validated `postMessage` bridge (`spec/smart-object.md`). The static validator catches common mistakes but is **not** the security boundary. |
| **`.rpk` files** from links, e-mail, USB sticks or students | They are crafted ZIP archives. | Path rules (no absolute paths, `..`, backslashes or control characters), caps on entry count, entry size and total size enforced while inflating (zip bombs), point-count caps in the stroke codec, version gates, and no code execution on read (`spec/rupok-format.md#reader-limits-and-safety`). Smart Objects inside a file get the same sandbox as above. |
| **MCP tool calls** from AI agents | Agents can be misled by prompt injection in web pages or documents. | The MCP server only reads and writes inside its `--workspace` directory, validates every Smart Object before saving, and has no tool that runs code or reaches the network. |
| **Network peers** (planned: LAN send, live broadcast, Drive sync) | Anyone on the network or server. | Plans require pairing, end-to-end encryption and least-privilege OAuth (`drive.file`); see `docs/roadmap/`. |

### In scope

- Escaping the Smart Object sandbox: reaching the host app, the file system,
  other objects, or the network without permission.
- Any `.rpk` file that writes outside its target, executes code, or causes
  unbounded memory or CPU use.
- MCP or CLI operations that escape the workspace.
- Crashes (panics) on untrusted input. These are denial of service in a live
  class.

### Out of scope

- A Smart Object that is wrong, ugly or misleading but stays inside the
  sandbox (that is a content problem; the teacher reviews what they present).
- Attacks that need an already-compromised computer or browser.
- Social engineering of maintainers.

## Known issues

- `ObjectBundle::validate()` can panic on Smart Object HTML containing `//`
  shortly after non-ASCII text (byte-index slicing in the URL scanner). It is
  a crash (denial of service) in tools that validate objects. Tracked in
  [ROADMAP.md](ROADMAP.md#known-issues-good-first-fixes).
- The desktop host (Tauri) and its run-time network policy are not built yet.
  The browser board injects a CSP through `srcdoc`.
