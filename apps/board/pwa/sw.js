// Rupok's service worker: the web board starts without internet.
//
// Template: pwa/plugin.ts fills in BUILD and PRECACHE at build time and emits
// it as /sw.js (scope "/"). Hand-written, no Workbox. docs/offline.md.
//
//   board app shell   precached on install (HTML, JS/CSS chunks, wasm, fonts,
//                     icons); cache-first. A new build installs beside the old
//                     one and waits until the page's "Reload" activates it.
//   /rupoklib/        offline Smart Object libraries: cached as they are used,
//                     and all of them (about 12 MB) only once the app is
//                     installed (the page sends "CACHE_LIBRARY").
//   /assets/ (other)  hashed, immutable: cache-first, trimmed.
//   other pages       the website: stale-while-revalidate, trimmed.
//   never cached      /api/ and /s/ (Rupok Cloud), /version.json, other
//                     origins (YouTube, embeds, GitHub), ranges, non-GET.

/* eslint-disable no-restricted-globals */
const BUILD = "__RUPOK_BUILD__";
/** @type {string[]} */
const PRECACHE = __RUPOK_PRECACHE__;
/** Every /rupoklib/ file, for an installed app ("CACHE_LIBRARY"). @type {string[]} */
const LIBRARY = __RUPOK_LIBRARY__;

const SHELL = `rupok-shell-${BUILD}`;
const LIB = "rupok-lib-v1";
const ASSETS = "rupok-assets-v1";
const PAGES = "rupok-pages-v1";
const LIMITS = { [ASSETS]: 160, [PAGES]: 60 };

/** Navigations that are the board app (any query: ?view, ?overlay, ?open=…). */
const isBoard = (path) => path === "/board" || path.startsWith("/board/");
/** Requests that must always reach the network. */
const bypass = (url) =>
  url.pathname.startsWith("/api/") || url.pathname.startsWith("/s/") || url.pathname === "/version.json" || url.pathname === "/sw.js";

self.addEventListener("install", (event) => {
  event.waitUntil(
    (async () => {
      const cache = await caches.open(SHELL);
      // A few at a time, so a slow classroom connection isn't flooded. HTML
      // revalidates; hashed assets may come from the HTTP cache.
      const queue = PRECACHE.slice();
      const worker = async () => {
        for (let url = queue.shift(); url; url = queue.shift()) {
          const html = url.endsWith("/");
          const res = await fetch(url, { cache: html ? "no-cache" : "default", credentials: "same-origin" });
          if (!res.ok) throw new Error(`precache ${url}: ${res.status}`);
          await cache.put(url, res);
        }
      };
      await Promise.all([worker(), worker(), worker(), worker()]);
      // The first install takes over at once; an update waits for "Reload".
      if (!self.registration.active) await self.skipWaiting();
    })(),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    (async () => {
      for (const key of await caches.keys()) {
        if (key.startsWith("rupok-shell-") && key !== SHELL) await caches.delete(key);
      }
      await self.clients.claim();
    })(),
  );
});

/** Fetch the library files not cached yet, two at a time. */
async function cacheLibrary() {
  const cache = await caches.open(LIB);
  const have = new Set((await cache.keys()).map((r) => new URL(r.url).pathname));
  const queue = LIBRARY.filter((u) => !have.has(u));
  const worker = async () => {
    for (let url = queue.shift(); url; url = queue.shift()) {
      try {
        const res = await fetch(url);
        if (storable(res)) await cache.put(url, res);
      } catch { /* offline again: the rest come as they are used */ }
    }
  };
  await Promise.all([worker(), worker()]);
}

self.addEventListener("message", (event) => {
  if (event.data?.type === "SKIP_WAITING") void self.skipWaiting();
  else if (event.data?.type === "CACHE_LIBRARY") event.waitUntil(cacheLibrary());
  else if (event.data?.type === "BUILD") event.source?.postMessage({ type: "BUILD", build: BUILD });
});

async function trim(name) {
  const max = LIMITS[name];
  if (!max) return;
  const cache = await caches.open(name);
  const keys = await cache.keys();
  for (let i = 0; i < keys.length - max; i++) await cache.delete(keys[i]);
}

/** Store a copy if it is a complete, same-origin, cacheable response. */
function storable(res) {
  if (!res || res.status !== 200 || res.type === "opaque") return false;
  const cc = res.headers.get("Cache-Control") || "";
  return !/no-store/i.test(cc);
}

async function cacheFirst(req, name) {
  const cache = await caches.open(name);
  const hit = await cache.match(req);
  if (hit) return hit;
  const res = await fetch(req);
  if (storable(res)) {
    await cache.put(req, res.clone());
    void trim(name);
  }
  return res;
}

async function staleWhileRevalidate(event, req, name) {
  const cache = await caches.open(name);
  const hit = await cache.match(req, { ignoreVary: true });
  const fresh = fetch(req)
    .then(async (res) => {
      if (storable(res)) {
        await cache.put(req, res.clone());
        void trim(name);
      }
      return res;
    })
    .catch(() => null);
  if (hit) {
    event.waitUntil(fresh);
    return hit;
  }
  const res = await fresh;
  return res || offlinePage();
}

function offlinePage() {
  return new Response(
    '<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Offline · Rupok</title>' +
      '<body style="font:16px/1.5 system-ui,sans-serif;max-width:32em;margin:15vh auto;padding:0 16px">' +
      "<h1>You’re offline</h1><p>This page isn’t saved on this device yet. The board works without internet: " +
      '<a href="/board/">open the board</a>.</p>',
    { status: 503, headers: { "Content-Type": "text/html; charset=utf-8" } },
  );
}

/** The precached shell's copy of a URL (query ignored), if any. */
async function fromShell(key) {
  const cache = await caches.open(SHELL);
  return cache.match(key, { ignoreSearch: true });
}

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET" || req.headers.has("range")) return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin || bypass(url)) return;

  if (req.mode === "navigate") {
    if (isBoard(url.pathname)) {
      event.respondWith(fromShell("/board/").then((hit) => hit || fetch(req)));
      return;
    }
    const page = url.pathname.endsWith("/") ? url.pathname : `${url.pathname}/`;
    if (PRECACHE.includes(page)) {
      event.respondWith(fromShell(page).then((hit) => hit || fetch(req)));
      return;
    }
    event.respondWith(staleWhileRevalidate(event, req, PAGES));
    return;
  }

  if (PRECACHE.includes(url.pathname)) {
    event.respondWith(fromShell(url.pathname).then((hit) => hit || fetch(req)));
  } else if (url.pathname.startsWith("/rupoklib/")) {
    event.respondWith(cacheFirst(req, LIB));
  } else if (url.pathname.startsWith("/assets/")) {
    event.respondWith(cacheFirst(req, ASSETS));
  } else {
    event.respondWith(staleWhileRevalidate(event, req, PAGES));
  }
});
