// Update notices (docs/offline.md):
//
// - Desktop: on launch, at most once a day, ask GitHub for the latest release
//   (release.ts). If it is newer than this app, a small card offers the
//   installer for this OS and the release notes. "Skip this version" is
//   remembered. Errors and rate limits stay silent.
// - Web: the service worker (pwa/sw.js) keeps the board working offline. When
//   a new build is deployed while a tab is open (/version.json on focus, at
//   most every 30 minutes, or a new service worker waiting), the card offers
//   "Reload", which activates the new worker and reloads.
//
// The card never shows in Teach mode (UpdateNotice.tsx).

import { createSignal } from "solid-js";
import { isDesktop } from "../platform";
import { MAIN_ASSET, RELEASES_URL, compareVersions, detectPlatform, latestRelease, type Release } from "../release";
import { VERSION } from "../components/settings/About";

export type Notice =
  | { kind: "desktop"; version: string; notes: string; download: string }
  | { kind: "web" };

const [notice, setNotice] = createSignal<Notice | null>(null);
export { notice };

const DAY = 24 * 60 * 60 * 1000;
const WEB_EVERY = 30 * 60 * 1000;
const K = { checked: "rupok.update.checked", release: "rupok.update.release", skip: "rupok.update.skip", web: "rupok.update.web" };

const get = (store: () => Storage, key: string): string | null => {
  try { return store().getItem(key); } catch { return null; }
};
const put = (store: () => Storage, key: string, value: string) => {
  try { store().setItem(key, value); } catch { /* private window, blocked storage */ }
};
const local = () => localStorage;
const session = () => sessionStorage;

/** This app's version: the Tauri app version on desktop, else the About screen's. */
export async function appVersion(): Promise<string> {
  if (isDesktop) {
    try {
      const { getVersion } = await import("@tauri-apps/api/app");
      return await getVersion();
    } catch { /* fall through */ }
  }
  return VERSION;
}

// ------------------------------------------------------------------ desktop

export type ReleaseCheck = { status: "newer" | "latest" | "error"; latest?: string; current: string };

/**
 * Compare the latest GitHub release with this app. `force` (Settings → About)
 * ignores the once-a-day limit, the session cache and "Skip this version".
 * `current` overrides the app version (tests).
 */
export async function checkRelease(opts: { force?: boolean; current?: string } = {}): Promise<ReleaseCheck> {
  const current = opts.current ?? (await appVersion());
  let rel: Release | null = null;
  const last = Number(get(local, K.checked) ?? 0);
  if (!opts.force && Date.now() - last < DAY) {
    // Checked today: reuse that answer, no request.
    try { rel = JSON.parse(get(local, K.release) ?? "null"); } catch { rel = null; }
    if (!rel) return { status: "latest", current };
  } else {
    rel = await latestRelease(!!opts.force);
    if (!rel) return { status: "error", current }; // offline, rate limit: silent
    put(local, K.checked, String(Date.now()));
    put(local, K.release, JSON.stringify(rel));
  }
  if (compareVersions(rel.version, current) <= 0) {
    if (notice()?.kind === "desktop") setNotice(null);
    return { status: "latest", latest: rel.version, current };
  }
  if (opts.force || get(local, K.skip) !== rel.version) {
    const kind = MAIN_ASSET[detectPlatform()];
    const asset = kind ? rel.assets[kind] : undefined;
    setNotice({ kind: "desktop", version: rel.version, notes: rel.notes || RELEASES_URL, download: asset?.url ?? rel.notes ?? RELEASES_URL });
  }
  return { status: "newer", latest: rel.version, current };
}

/** "Skip this version": no card for it again (a newer one still shows). */
export function skipVersion(version: string) {
  put(local, K.skip, version);
  setNotice(null);
}

export function dismissNotice() {
  setNotice(null);
}

// ---------------------------------------------------------------------- web

/** This page's build (pwa/plugin.ts puts it in the HTML; absent in dev). */
const BUILD = typeof document !== "undefined" ? document.querySelector<HTMLMetaElement>('meta[name="rupok-build"]')?.content ?? "" : "";
let registration: ServiceWorkerRegistration | null = null;
let reloading = false;

const sw = () => (typeof navigator !== "undefined" && "serviceWorker" in navigator ? navigator.serviceWorker : null);

/** Running as the installed app (home screen, Start menu, dock)? */
const installed = () => matchMedia("(display-mode: standalone), (display-mode: fullscreen), (display-mode: minimal-ui)").matches;

function watchRegistration(reg: ServiceWorkerRegistration) {
  registration = reg;
  // The teacher installed Rupok: keep every Smart Object library offline too.
  if (installed()) void navigator.serviceWorker.ready.then((r) => r.active?.postMessage({ type: "CACHE_LIBRARY" }));
  // A worker for a newer build installed while (or before) this page loaded.
  if (reg.waiting && navigator.serviceWorker.controller) setNotice({ kind: "web" });
  reg.addEventListener("updatefound", () => {
    const w = reg.installing;
    w?.addEventListener("statechange", () => {
      if (w.state === "installed" && navigator.serviceWorker.controller) setNotice({ kind: "web" });
    });
  });
}

/**
 * Is a newer build deployed? Fetches /version.json (never cached), at most
 * every 30 minutes unless `force`. Returns "newer", "latest" or "error".
 */
export async function checkWeb(opts: { force?: boolean } = {}): Promise<"newer" | "latest" | "error"> {
  if (!BUILD) return "latest";
  const last = Number(get(session, K.web) ?? 0);
  if (!opts.force && Date.now() - last < WEB_EVERY) return notice()?.kind === "web" ? "newer" : "latest";
  put(session, K.web, String(Date.now()));
  try {
    const r = await fetch(`/version.json?t=${Date.now()}`, { cache: "no-store" });
    if (!r.ok) return "error";
    const j = (await r.json()) as { build?: string };
    if (!j.build || j.build === BUILD) return "latest";
    setNotice({ kind: "web" });
    void registration?.update().catch(() => {}); // start fetching the new worker now
    return "newer";
  } catch {
    return "error";
  }
}

/** "Reload": activate the waiting service worker (if any), then reload. */
export async function applyWebUpdate(): Promise<void> {
  const container = sw();
  const reg = registration;
  if (!container || !reg) {
    location.reload();
    return;
  }
  reloading = true;
  const fallback = setTimeout(() => location.reload(), 10000);
  try {
    if (!reg.waiting && !reg.installing) await reg.update();
  } catch { /* offline: reload anyway */ }
  const worker = reg.waiting ?? reg.installing;
  if (!worker) {
    clearTimeout(fallback);
    location.reload();
    return;
  }
  const activate = () => worker.postMessage({ type: "SKIP_WAITING" });
  if (worker.state === "installed") activate();
  else worker.addEventListener("statechange", () => worker.state === "installed" && activate());
  // controllerchange (below) reloads.
}

/** Register the service worker and watch for new builds (browser, production builds only). */
function startWeb() {
  const container = sw();
  if (!BUILD || !container) return;
  // Data saver: no 15 MB offline copy behind the teacher's back.
  const conn = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection;
  if (conn?.saveData) return;
  // Automated browsers (the pen-latency CI, e2e suites) measure the page, not
  // a background download, unless a test opts in: localStorage rupok.sw = "1".
  if (navigator.webdriver && get(local, "rupok.sw") !== "1") return;
  container.addEventListener("controllerchange", () => {
    if (reloading) location.reload();
  });
  const register = () =>
    container
      .register("/sw.js", { scope: "/", updateViaCache: "none" })
      .then(watchRegistration)
      .catch(() => { /* unsupported (private mode, file://): the board still works online */ });
  // After the board is up, so precaching never competes with the first paint.
  if (document.readyState === "complete") setTimeout(register, 1500);
  else addEventListener("load", () => setTimeout(register, 1500), { once: true });
  const onFocus = () => document.visibilityState === "visible" && void checkWeb();
  addEventListener("focus", onFocus);
  document.addEventListener("visibilitychange", onFocus);
}

// -------------------------------------------------------------------- start

let started = false;
/** Call once when the board is ready. */
export function startUpdateChecks() {
  if (started) return;
  started = true;
  if (isDesktop) setTimeout(() => void checkRelease(), 6000); // after launch settles
  else startWeb();
}

// For the console and e2e tests: `rupokUpdates.checkRelease({ force: true })`.
if (typeof window !== "undefined") {
  (window as any).rupokUpdates = { checkRelease, checkWeb, applyWebUpdate, notice, dismiss: dismissNotice, build: BUILD };
}
