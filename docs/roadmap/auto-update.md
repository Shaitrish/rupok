# Automatic desktop updates (Tauri updater)

Status: planned. Today the desktop app shows a notice when GitHub has a newer
release and links the installer ([offline.md](../offline.md#desktop-update-notice)).
This plan makes the update download and install itself, signed.

## Goal

A teacher on 0.3 opens Rupok, sees "Rupok 0.4 is ready: Restart to update",
and is on 0.4 after one click. Nothing to download by hand, no admin prompt
(the Windows installer is per-user already), never during a lesson.

## How it works

1. **Signing key pair.** `bunx tauri signer generate -w ~/.tauri/rupok.key`
   makes a minisign key pair. The **public** key goes into
   `apps/desktop/tauri.conf.json` (`plugins.updater.pubkey`); it is public and
   safe to commit. The **private** key and its password become GitHub
   Actions secrets, `TAURI_SIGNING_PRIVATE_KEY` and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The founder creates them; agents never
   do. Keep an offline backup: losing the key means existing installs can
   never auto-update again (they would need one manual install of a build
   with a new key).
2. **Build.** With the secrets set, `tauri build` with
   `bundle.createUpdaterArtifacts: true` signs each bundle and writes a `.sig`
   next to it (NSIS `.exe`, `.app.tar.gz`, AppImage; `.deb` and `.msi` can't
   self-update and keep the notice).
3. **Manifest.** `desktop.yml` assembles `latest.json`
   (`version`, `notes`, `pub_date`, and per target `url` + `signature`) and
   uploads it to the GitHub release. `tauri-action` can do this
   (`includeUpdaterJson: true`).
4. **App.** Add `tauri-plugin-updater` (Rust + `@tauri-apps/plugin-updater`)
   and the `updater:default` permission in `capabilities/main.json`.
   Endpoint: `https://github.com/Shaitrish/rupok/releases/latest/download/latest.json`
   (add `https://github.com` and `https://objects.githubusercontent.com` to
   the desktop CSP `connect-src`).
5. **UI.** `updates.ts` keeps the once-a-day check, but on an updatable build
   calls `check()` from the plugin instead of the GitHub API. If there is an
   update: download in the background (`downloadAndInstall` with progress),
   then the same card says "Rupok 0.4 is ready · Restart to update"
   (`relaunch()` from `plugin-process`). Never in Teach mode; if a lesson has
   unsaved changes, save first. "Skip this version" and the Settings → About
   button stay. Linux `.deb` and Windows `.msi` installs keep today's notice
   with the download link.
6. **Setting.** Settings → About → "Install updates automatically" (on by
   default; schools that manage their machines turn it off).

## Release checklist addition

- [ ] Secrets present in the repository (`TAURI_SIGNING_PRIVATE_KEY`,
      `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
- [ ] `latest.json` attached to the release and its signatures verify.
- [ ] Update path tested from the previous release on Windows, macOS and
      Linux AppImage.

## Why not yet

It needs Rust changes (plugin, permissions), a CI change, and secrets only
the maintainer can create. Builds up to 0.2.1 have neither the notice nor the
updater: teachers install the next release by hand once (announce it), and
from then on the notice, and later the updater, reach them.
