# Rupok desktop (Tauri 2)

The installable Rupok app for **Windows, macOS and Linux**. It wraps the board from
[`apps/board`](../board) (SolidJS + the `rupok-wasm` engine, unchanged) in a
Tauri 2 shell and adds what a browser cannot do:

- native Open/Save dialogs and **crash-safe saving** (validate → temp file → fsync → rename);
- **`.rpk` file association**, open from the command line, and a **single instance**
  (double-clicking a second lesson opens it in the running window); on macOS,
  Finder's double-click, "Open With" and drops on the Dock icon;
- remembered window size and position;
- **draw over any app**: a transparent, always-on-top ink layer over the whole
  screen with a floating toolbar, click-through, and "Save to lesson" (beta on
  macOS);
- on macOS, a native menu bar (About, Hide, Quit ⌘Q, Edit, Window), and the
  board's Ctrl shortcuts also work with ⌘.

**Mac users:** see [macOS](#macos) for install and first launch.

Plans: [tauri-desktop](../../docs/roadmap/tauri-desktop.md) ·
[desktop-overlay](../../docs/roadmap/desktop-overlay.md).

## Layout

| Path | What |
|---|---|
| `tauri.conf.json` | Windows, CSP, bundle targets, file association (`.rpk`, UTI `app.rupok.lesson` on macOS), macOS bundle settings. `frontendDist` is `../board/dist`; `devUrl` is `http://127.0.0.1:3737/board/` |
| `capabilities/` | Least-privilege permissions: `main.json` (board window), `overlay.json` (overlay windows) |
| `src/app.rs` | Builder, plugins, `save_lesson`, files opened by the OS (argv, and `RunEvent::Opened` on macOS), the macOS menu bar |
| `src/overlay.rs` | Overlay windows, click-through, global shortcut, capture, snapshot hand-off; macOS window level and Spaces |
| `src/files.rs` | Atomic save and argv parsing (plain Rust, unit-tested) |
| `src/capture.rs` | Screen capture: `xcap` on Windows and macOS (with the Screen Recording permission check), pure-Rust X11 on Linux |
| `src/geometry.rs` | Toolbar placement, points/pixels, and the screenshot wire format (unit-tested) |
| `linux/` | `.desktop` template (`Exec=… %F`) and the shared-mime-info entry for `.rpk` |
| `macos/` | `Entitlements.plist` for the hardened runtime (no exceptions; see the comment in it) |
| `icons/` | Generated app icons (see below) |

On the board side: [`src/platform.ts`](../board/src/platform.ts) is the file
bridge (Tauri dialogs and fs in the app; File System Access API or
download/`<input type=file>` in a browser) and [`src/overlay/`](../board/src/overlay)
is the overlay UI.

## Prerequisites

- Rust (pinned by `rust-toolchain.toml`), bun, and the wasm-bindgen CLI
  (`cargo install wasm-bindgen-cli --version 0.2.130 --locked`), as for the board.
- **Linux**: the WebKitGTK development packages:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev librsvg2-dev
  # for bundling an AppImage as well:
  sudo apt install patchelf libfuse2
  ```

- **Windows**: WebView2 (preinstalled on Windows 10/11) and the MSVC build tools.
- **macOS**: the Xcode Command Line Tools (`xcode-select --install`). For a
  universal (Apple silicon + Intel) build, both Rust targets:
  `rustup target add aarch64-apple-darwin x86_64-apple-darwin`.

## Develop

```sh
cd apps/desktop
bun install
bun run dev        # = tauri dev: builds the wasm engine, starts the board dev
                   #   server on :3737 (scripts/dev.sh), opens the app window
```

The Tauri code is behind the crate's `app` feature, which `tauri.conf.json`
(`build.features`) turns on for `tauri dev`/`tauri build`. Plain
`cargo test --workspace` and `cargo clippy --workspace` therefore build this
crate **without** Tauri and need no WebKitGTK; they run the unit tests of
`files.rs` and `geometry.rs`. To lint the real app:

```sh
cargo clippy -p rupok-desktop --features app -- -D warnings      # needs WebKitGTK on Linux
cargo test   -p rupok-desktop --features capture                 # capture code, no WebKitGTK needed
```

## Build installers

```sh
cd apps/desktop
bun run build      # builds the wasm engine and the board, then `tauri build`
```

Output in `target/release/bundle/` at the repo root: `nsis/*.exe` and
`msi/*.msi` on Windows, `deb/*.deb` and `appimage/*.AppImage` on Linux,
`macos/Rupok.app` and `dmg/*.dmg` on macOS. If the board is already built,
`bun run bundle` runs only `tauri build`. On a Mac, the universal build is

```sh
bun run bundle -- --target universal-apple-darwin
# -> target/universal-apple-darwin/release/bundle/dmg/Rupok_<version>_universal.dmg
```

CI ([`.github/workflows/desktop.yml`](../../.github/workflows/desktop.yml))
builds all three platforms on every push to `main` and on pull requests
(installers as workflow artifacts: `rupok-windows`, `rupok-linux`,
`rupok-macos`) and attaches them to a **draft GitHub Release** for `v*` tags.
The macOS job runs on an Apple silicon runner (`macos-15`; GitHub has retired
`macos-14`), builds one universal binary and prints the app's architectures,
signature and `Info.plist` in the log ("Inspect the macOS app"). The same workflow runs the
board's key flows in **Playwright WebKit** on macOS
([`apps/board/e2e`](../board/e2e/playwright.config.ts)): load, drawing with
pointer events, ⌘Z, a Smart Object preset, Teach mode, save/open, PDF import
and the website pages.

## Icons

The icons are generated from the brand artwork with one command, so a new logo
is one re-run away:

```sh
cd apps/desktop && bun run icons
# = bunx tauri icon ../../brand/rupok-app-icon.svg && rm -rf icons/android icons/ios
```

Any square SVG or PNG works as input (for example `apps/board/public/icon-512.png`,
or `brand/rupok-mark.svg` for rounded corners on the taskbar).

## The overlay: draw over any app

- **Ctrl+Shift+D** (global; **⌃⇧D**, Control-Shift-D, on a Mac) shows or hides
  the overlay on the monitor under the mouse. `platform.openOverlay()` / `closeOverlay()` do the same from the board.
- The **ink window** (`/board/?overlay`) is transparent, undecorated, always on
  top, skipped from the taskbar and covers the monitor. It shows only ink: no
  page frame, no canvas colour. It is a separate small lesson and never touches
  the teacher's lesson or its autosave.
- The **toolbar window** (`/board/?overlay=toolbar`) is a small always-on-top
  window owned by the ink window: pen colours, highlighter, eraser, laser, undo,
  clear, click-through, Save to lesson, exit. Drag it by the grip.
- **Click-through** calls `set_ignore_cursor_events(true)` on the ink window:
  clicks go to the apps underneath and the ink stays visible. The toolbar is a
  different window, so it keeps working. Picking a pen turns drawing back on.
- **Esc** in the overlay (or the × button) hides it. The ink is kept until you
  clear it, so toggling the overlay off and on does not lose anything.
- **Save to lesson** hides the overlay for a moment, captures the monitor,
  composites the ink on top in the webview and adds the image as a new page in
  the main window's lesson.
- In a browser, `/board/?overlay` is a plain page with the toolbar inline (handy
  for development; Save to lesson needs the desktop app).

## macOS

One **universal** app for Apple silicon (M1 and later) and Intel Macs,
**macOS 11 Big Sur or later**. Rupok uses the system WebKit (WKWebView), so
keep macOS and Safari updated. The overlay is **beta on macOS**; everything
else is the same board as on Windows and Linux.

### Install

1. Download `Rupok_<version>_universal.dmg` from the
   [GitHub Releases](https://github.com/shaitrish/rupok/releases) page (or,
   for a test build, the `rupok-macos` artifact of a
   [Desktop workflow run](https://github.com/shaitrish/rupok/actions/workflows/desktop.yml)).
2. Open the `.dmg` and drag **Rupok** onto **Applications**.

### First launch while the app is unsigned

Until the founder's Apple Developer account exists (see
[Signing and notarising](#signing-and-notarising-for-macos)), the app is
ad-hoc signed, not notarised, so macOS asks once before the first launch:

- **macOS 14 Sonoma and earlier:** in Applications, **right-click** (or
  Control-click) Rupok → **Open**, then **Open** again in the dialog.
- **macOS 15 Sequoia and later** (right-click → Open no longer skips the
  check): double-click Rupok and click **Done** in the warning. Open
  **System Settings → Privacy & Security**, scroll down to *Security*, click
  **Open Anyway** next to "Rupok was blocked…", enter your password, then
  **Open Anyway** once more.

After that Rupok opens normally, until an update replaces the app. (For IT
staff: `xattr -dr com.apple.quarantine /Applications/Rupok.app` does the same
from Terminal.)

### What is different on a Mac

- **Menu bar:** Rupok (About, Hide ⌘H, Hide Others, Quit ⌘Q), Edit (Undo,
  Redo, Cut, Copy, Paste, Select All: these also make ⌘C/⌘V work in text
  fields, which WKWebView only gets through the menu), View (Enter Full
  Screen ⌃⌘F) and Window (Minimize ⌘M, Zoom, Close ⌘W). The board handles its
  own keys first, so ⌘Z on the canvas undoes ink and in a text field undoes
  typing.
- **⌘ shortcuts:** every Ctrl shortcut of the board works with ⌘ (⌘Z, ⇧⌘Z,
  ⌘C, ⌘X, ⌘V, ⌘D, ⌘G, ⇧⌘G, ⌘E, ⌘A, ⌘S, ⌘O, ⇧⌘E). Option+1/2/3 picks the
  mode. Teach is F5 (fn+F5 on most Mac keyboards). Trackpad pinch zooms the
  board, two-finger scroll pans.
- **Lessons:** `.rpk` files get their own type (UTI `app.rupok.lesson`,
  conforming to `public.zip-archive`) with Rupok as the owner, so a
  double-click in Finder, "Open With" or a drop on the Dock icon opens the
  lesson (`RunEvent::Opened` in `src/app.rs`), also while Rupok is running.
- **Privacy prompts:** voice input asks for the microphone and for speech
  recognition, with the reasons in `apps/desktop/Info.plist`
  (`NSMicrophoneUsageDescription`, `NSSpeechRecognitionUsageDescription`,
  merged into the bundle by Tauri), and `macos/Entitlements.plist` carries
  `com.apple.security.device.audio-input` for the hardened runtime. Rupok
  uses no camera, location or contacts. Screen Recording (overlay "Save to
  lesson") has no `Info.plist` key; macOS asks through its own prompt and
  System Settings.

### The overlay on macOS (beta)

It compiles and is linted on macOS in CI, but it has **not been tried on a
real Mac yet**, nor with real pen tablets there. What it does:

- The ink window is transparent (Tauri draws a transparent WKWebView with
  WebKit's private `drawsBackground` key, always on since Tauri 2.12.1, so
  `macOSPrivateApi` is not needed in `tauri.conf.json`). It sits at the
  status-bar window level, above the menu bar and the Dock, joins **every
  Space**, and is allowed over **full-screen apps** (Keynote or PowerPoint
  slideshows): collection behaviour `canJoinAllSpaces | stationary |
  ignoresCycle | fullScreenAuxiliary` (`overlay::macos` in `src/overlay.rs`).
- Click-through uses `setIgnoresMouseEvents`; the toolbar is a child window
  one level above the ink, so it stays clickable.
- **Save to lesson needs the Screen Recording permission.** Without it macOS
  would hand Rupok only the wallpaper, so Rupok checks first
  (`CGPreflightScreenCaptureAccess`). If it is off, the ink is saved alone and
  the overlay explains why, with an **Open System Settings** button: it hides
  the overlay, asks macOS for the permission (which adds Rupok to the list)
  and opens *Privacy & Security → Screen & System Audio Recording*. Turn on
  Rupok, then quit and reopen it (macOS applies the change after a restart).
  While the app is ad-hoc signed, each update counts as a new app, so the
  switch has to be turned on again after updating. macOS 15 and later may
  also ask again from time to time whether Rupok may keep recording.
- Untested and possibly rough: activating the overlay over a full-screen app
  may switch Spaces (macOS's "switch to a Space with open windows" setting),
  and displays with different scale factors.

**App Store trade-off.** The private `drawsBackground` key that makes the
overlay transparent is not allowed in the Mac App Store, and the App Store
also requires the App Sandbox, which the overlay and "open lessons
anywhere" do not fit. Rupok is distributed outside the App Store, signed with
a Developer ID and notarised, where both are fine. An App Store build would
need the overlay off and the sandbox entitlements added to
`macos/Entitlements.plist`.

### Signing and notarising for macOS

CI signs and notarises the macOS app **only when all six secrets below
exist**; with none of them it builds the ad-hoc signed app described above (a
partial set prints a warning and builds ad hoc too). The bundle already uses
the **hardened runtime** (required for notarisation) with
[`macos/Entitlements.plist`](macos/Entitlements.plist): a non-sandboxed
WKWebView app needs no exceptions, because JavaScript and WebAssembly run in
WebKit's own process.

What the founder has to do once (only the Apple account holder can):

1. **Enrol in the Apple Developer Program** at
   <https://developer.apple.com/programs/enroll/> (US$99 a year), with an
   Apple Account that has two-factor authentication on. Enrol as an
   individual (your name is shown as the developer) or as an organisation
   (needs a D-U-N-S number; shows "Shaitrish"). Approval takes from hours to
   a few days.
2. **Create a "Developer ID Application" certificate.**
   - *On a Mac:* Keychain Access → Certificate Assistant → *Request a
     Certificate From a Certificate Authority* → your email and name, *Saved
     to disk*. Then on <https://developer.apple.com/account/resources/certificates/list>
     click **+**, choose **Developer ID Application** (G2 Sub-CA), upload
     the `.certSigningRequest`, download the `.cer` and double-click it.
   - *Without a Mac* (any Linux or Windows machine with OpenSSL):

     ```sh
     openssl req -new -newkey rsa:2048 -nodes -keyout devid.key -out devid.csr \
       -subj "/emailAddress=you@example.com/CN=Your Name/C=BD"
     # upload devid.csr as above, download developerID_application.cer, then:
     openssl x509 -inform der -in developerID_application.cer -out devid.pem
     ```

3. **Export it as a `.p12`** with a strong password.
   - *Mac:* Keychain Access → *login* → *My Certificates* → right-click
     "Developer ID Application: …" → **Export** → `rupok-devid.p12`.
   - *OpenSSL 3:* `openssl pkcs12 -export -legacy -inkey devid.key -in devid.pem -out rupok-devid.p12`
     (`-legacy` keeps it readable by macOS's `security import`; leave it out
     on OpenSSL 1.1).

   Keep the `.p12`, its password and `devid.key` somewhere safe and offline;
   anyone with them can sign apps as you. Revoke the certificate on the
   developer site if they leak.
4. **Note the identity and team ID.** The identity is the certificate's full
   name, e.g. `Developer ID Application: Your Name (AB12CD34EF)`
   (`security find-identity -v -p codesigning` on a Mac, or
   `openssl x509 -in devid.pem -noout -subject` and take the `CN`). The team
   ID is the 10 characters in brackets, also under *Membership details* on
   <https://developer.apple.com/account>.
5. **Create an app-specific password** for notarisation: <https://account.apple.com>
   → *Sign-In and Security* → **App-Specific Passwords** → **+**, name it
   "Rupok notarisation", copy the `xxxx-xxxx-xxxx-xxxx` password.
6. **Add six repository secrets** on GitHub: *Settings → Secrets and
   variables → Actions → New repository secret* (or with `gh secret set`):

   | Secret | Value |
   |---|---|
   | `APPLE_CERTIFICATE` | the `.p12` as one line of base64: `base64 -i rupok-devid.p12` (Mac) or `base64 -w0 rupok-devid.p12` (Linux) |
   | `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
   | `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Your Name (AB12CD34EF)` |
   | `APPLE_ID` | the Apple Account email |
   | `APPLE_PASSWORD` | the app-specific password (not the account password) |
   | `APPLE_TEAM_ID` | `AB12CD34EF` |

   ```sh
   base64 -w0 rupok-devid.p12 | gh secret set APPLE_CERTIFICATE --repo shaitrish/rupok
   gh secret set APPLE_CERTIFICATE_PASSWORD --repo shaitrish/rupok   # prompts for the value
   # …and the same for the other four
   ```

7. **Check it:** run the Desktop workflow (*Actions → Desktop → Run
   workflow*, or push to `main`). The macOS job logs "Signing with a Developer
   ID and notarising"; in "Inspect the macOS app", `Authority=Developer ID
   Application: …` and `spctl` reports `accepted, source=Notarized Developer
   ID`. Notarisation usually takes a few minutes. From then on the `.dmg`
   opens with a normal double-click, and the first-launch steps above are no
   longer needed.

## Known platform limits

| Platform | Limit |
|---|---|
| Linux, Wayland | Apps cannot place windows, keep them on top or grab global shortcuts. The app therefore runs under **XWayland** on Wayland sessions (it sets `GDK_BACKEND=x11` unless you set it, or `RUPOK_WAYLAND=1`). Even so, Ctrl+Shift+D only fires while a Rupok or other X11 window has focus, and **screen capture is not possible**, so Save to lesson saves **the ink only** (it says so). |
| Linux, X11 | Transparency needs a compositor (all mainstream desktops have one); without it the overlay is black. Some window managers keep undecorated windows out of panel areas. |
| Linux, AppImage | The file association is not registered system-wide (that needs AppImageLauncher or the `.deb`). |
| Windows | Full-screen *exclusive* apps (some games and video players) can cover the overlay; PowerPoint slideshows and full-screen browsers work. Installers are unsigned for now, so SmartScreen warns on first run. |
| macOS | The overlay is **beta**: it builds in CI but has not been tried on a real Mac yet (see [The overlay on macOS](#the-overlay-on-macos-beta)). Until the app is signed, Gatekeeper asks once on first launch (see [First launch](#first-launch-while-the-app-is-unsigned)). |
| Global shortcut | Ctrl+Shift+D is skipped (with a log line) if another app already owns it. A settings editor for shortcuts is still to do. |
| Multi-monitor | One overlay at a time, on the monitor under the mouse when it opens. |
| Smart Objects | Objects run as in the browser (`srcdoc` iframes). Objects that `import` ES modules from `/rupoklib/` are untested in the app: the opaque-origin iframe may need CORS headers that only the planned `rupokobj://` protocol will add. |

## Security notes

- The CSP allows `'wasm-unsafe-eval'` for the engine. It also allows
  `'unsafe-inline'`/`'unsafe-eval'` scripts because Smart Objects are `srcdoc`
  iframes, and a `srcdoc` document **inherits** the board's CSP: without them no
  object could run. The objects stay isolated by `sandbox="allow-scripts"`
  (opaque origin, no Tauri IPC) and their own stricter CSP. For the same reason
  `dangerousDisableAssetCspModification` keeps Tauri from adding nonces to
  `script-src`/`style-src`, which would switch `'unsafe-inline'` off.
- `save_lesson` only writes `.rpk` files the teacher chose in a dialog or
  opened from the OS (the fs scope), and refuses bytes that do not parse as a
  lesson package, so a broken page can never overwrite a good lesson.
- Each window gets only the commands it needs (`build.rs` generates
  `allow-<command>` permissions; see `capabilities/`).

## Not done yet

From the plan: autosave to the app data folder with restore, recent files, the
`rupokobj://` Smart Object protocol with vendored `rupoklib://` libraries, the
MCP sidecar, the updater and Windows code signing (macOS signing is wired but
waits for the Apple secrets); for the overlay: freeze-frame, per-monitor
overlays, a shortcut editor, and a first test on a real Mac. The board's mode switcher opens the
overlay through `platform.openOverlay()`.
