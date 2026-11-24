# Offline web app and update notices

How the web board starts without internet, how open tabs and the desktop app
learn about new versions, and the first-run tour. Code:
`apps/board/pwa/` (service worker and its Vite plugin),
`apps/board/src/updates/`, `apps/board/src/release.ts`,
`apps/board/src/tour/`.

## Service worker (web only)

`pwa/sw.js` is a hand-written service worker (no Workbox). At build time
`pwa/plugin.ts` fills in this build's id and file lists and emits it as
`/sw.js` with scope `/`. Dev (`vite`) has no service worker.

| Requests | Strategy |
|---|---|
| Board app shell: `/board/` HTML, every JS/CSS chunk the board can load (its whole module graph, lazy chunks included), the wasm engine, woff2 fonts, icons, the manifest; also `/open/` and `/bn/open/` | Precached when the worker installs (about 17 MB before compression, most of it the wasm the board loads anyway); cache-first. Any `/board/…` navigation (`?view`, `?open=…`, `?overlay`) gets the cached board HTML. |
| `/rupoklib/` (Smart Object libraries) | Cached as they are used (Smart Object frames are controlled by the worker). An installed app (display mode standalone or fullscreen) caches all of them in the background, about 12 MB. |
| Other `/assets/` (website chunks) | Cache-first, trimmed to 160 entries. |
| Website pages and other same-origin files | Stale-while-revalidate, trimmed to 60 entries. An unvisited page offline gets a short "You're offline" page that links the board. |
| `/api/` and `/s/` (Rupok Cloud), `/version.json`, `/sw.js`, other origins (YouTube, embeds, GitHub), range requests, non-GET | Never touched: straight to the network. |

The service worker is registered only by the board (`/board/`), 1.5 s after
the page loads, so visiting the website alone never downloads the app. It is
skipped with Data Saver on, and in automated browsers
(`navigator.webdriver`) unless the test sets `localStorage["rupok.sw"] = "1"`.

### Build id and updates

The build id is a hash of the precached (content-hashed) file names and the
shell HTML, so it changes exactly when the board changes. The plugin writes
it into every HTML page (`<meta name="rupok-build">`), into `/sw.js`, and
into `/version.json` (`{ build, version, date }`). Cloudflare serves `/sw.js`
with `no-cache` and `/version.json` with `no-store` (`public/_headers`).

1. A new deploy changes `/sw.js`. The browser installs the new worker beside
   the old one; it precaches the new shell and then **waits**.
2. The open board learns about it in two ways: the waiting worker
   (`updatefound`), or `/version.json` on window focus, at most every
   30 minutes.
3. The card says "A new version of Rupok is ready · Reload". Reload posts
   `SKIP_WAITING` to the waiting worker; on `controllerchange` the page
   reloads on the new build. The old shell cache is deleted on activation.

The first install takes over at once (`skipWaiting` + `clients.claim`), so the
first visit already works offline afterwards.

## Desktop update notice

On launch (6 s after the board is ready) and at most once a day, the desktop
app asks `https://api.github.com/repos/Shaitrish/rupok/releases/latest`
(allowed in the desktop CSP `connect-src`) and compares it with the app
version (`@tauri-apps/api/app` `getVersion()`, else the About screen's
`VERSION`). If it is newer, the card offers **What's new** (the release page)
and **Download**: the installer for this OS, picked by `release.ts` exactly
like the website's download buttons (Windows `.exe`, macOS `.dmg`, Linux
`.deb`). **Skip this version** is remembered in localStorage. The release is
cached in sessionStorage; offline, rate limits and errors stay silent.

Settings → About → **Check for updates** checks now, ignoring the daily limit
and a skipped version. On the web it checks `/version.json`.

The card is never modal and never shows in Teach mode (it waits until the
teacher leaves Teach).

Console and tests: `rupokUpdates.checkRelease({ force: true })`,
`rupokUpdates.checkWeb({ force: true })`.

Fully automatic background updates are a follow-up:
[roadmap/auto-update.md](roadmap/auto-update.md).

## First-run tour

Five small callouts the first time the board opens in Prepare mode: pen and
colours, pages and +, Insert, Teach mode (F5), Save and Library. No scrim; the
board stays usable. Each callout is anchored to a `data-tour` attribute
(`pen`, `color`, `pages-add`, `hdr-insert`, `hdr-more`, `library`) or the mode
switcher's `data-mode="teach"`, with fallbacks when a control has collapsed
into ⋯, and it follows the control every 400 ms, so moving the dock or the bar
never strands it. It pauses in Teach and Student view and while a dialog or
the Library is open.

Finished or skipped is remembered (`localStorage["rupok.tour"]`). Restart it
from ⋯ → Quick tour or Settings → About → Quick tour. Automated browsers only
get it with `/board/?tour`. Console: `rupokTour.start()`.
