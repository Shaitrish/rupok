// Download buttons that match the visitor's platform, on every marketing page.
//
// Slots: <div data-download="hero|final|foot|page"> renders a widget for this
// device. On /download/, links marked data-asset="exe|msi|deb|appimage|dmg"
// get the latest release's direct URL, and data-meta="<kind>" its version and
// size. Detection is synchronous (no layout jump); the release comes from the
// GitHub API, cached for the session, falling back to the releases page.

import { ASSET_SUFFIX, DL_ICONS, RELEASES_API, RELEASES_URL } from "../site/icons.mjs";

type Kind = keyof typeof ASSET_SUFFIX;
type Platform = "windows" | "mac" | "linux" | "mobile";
interface Asset { url: string; size: number }
interface Release { version: string; notes: string; assets: Partial<Record<Kind, Asset>> }

const CACHE = "rupok.release";

/** Phones, tablets and ChromeOS get the web app (no installer). */
export function detectPlatform(): Platform {
  const uad = (navigator as Navigator & { userAgentData?: { platform?: string; mobile?: boolean } }).userAgentData;
  const ua = navigator.userAgent;
  const p = (uad?.platform ?? "").toLowerCase();
  if (uad?.mobile || /android|iphone|ipad|ipod|cros|chrome os/i.test(p + " " + ua)) return "mobile";
  // iPadOS reports a Mac; a touch screen gives it away.
  if (p === "macos" || /macintosh|mac os x/i.test(ua)) return navigator.maxTouchPoints > 1 ? "mobile" : "mac";
  if (p === "windows" || /windows/i.test(ua)) return "windows";
  if (p === "linux" || /linux|x11/i.test(ua)) return "linux";
  return "mobile";
}

let pending: Promise<Release | null> | null = null;
export function latestRelease(): Promise<Release | null> {
  if (pending) return pending;
  pending = (async () => {
    try {
      const hit = sessionStorage.getItem(CACHE);
      if (hit) return JSON.parse(hit) as Release;
    } catch { /* storage unavailable */ }
    try {
      const r = await fetch(RELEASES_API, { headers: { Accept: "application/vnd.github+json" } });
      if (!r.ok) return null; // rate limit or no release: the fallback links stay
      const j = await r.json();
      const assets: Release["assets"] = {};
      for (const [kind, suffix] of Object.entries(ASSET_SUFFIX) as [Kind, string][]) {
        const a = (j.assets ?? []).find((x: { name: string }) => x.name.endsWith(suffix));
        if (a) assets[kind] = { url: a.browser_download_url, size: a.size };
      }
      const rel: Release = { version: String(j.tag_name ?? "").replace(/^v/, ""), notes: j.html_url ?? RELEASES_URL, assets };
      try { sessionStorage.setItem(CACHE, JSON.stringify(rel)); } catch { /* fine */ }
      return rel;
    } catch {
      return null;
    }
  })();
  return pending;
}

const T = {
  en: {
    windows: "Download for Windows", linux: "Download for Linux", mac: "Download for Mac",
    msi: "or .msi", appimage: "or AppImage", other: "Other platforms",
    mobile: "On a phone, tablet or Chromebook, Rupok runs in the browser.",
    open: "Open the board", install: "Install as an app", app: "Download the desktop app",
    page: "/download/", board: "/board/",
  },
  bn: {
    windows: "Windows-এর জন্য ডাউনলোড", linux: "Linux-এর জন্য ডাউনলোড", mac: "Mac-এর জন্য ডাউনলোড",
    msi: "অথবা .msi", appimage: "অথবা AppImage", other: "অন্য প্ল্যাটফর্ম",
    mobile: "ফোন, ট্যাবলেট বা Chromebook-এ Rupok ব্রাউজারেই চলে।",
    open: "বোর্ড খুলুন", install: "অ্যাপ হিসেবে ইনস্টল করুন", app: "ডেস্কটপ অ্যাপ ডাউনলোড করুন",
    page: "/bn/download/", board: "/board/",
  },
};
type Strings = (typeof T)["en"];

const mb = (n: number) => `${(n / 1e6).toFixed(n < 1e7 ? 1 : 0)} MB`;
const meta = (rel: Release | null, kind: Kind) => {
  const a = rel?.assets[kind];
  return rel && a ? `v${rel.version} · ${mb(a.size)}` : "";
};

// PWA install: the browser's prompt when it offers one, else the how-to.
let installEvent: (Event & { prompt?: () => Promise<void> }) | null = null;
addEventListener("beforeinstallprompt", (e) => {
  e.preventDefault();
  installEvent = e;
});
document.addEventListener("click", (e) => {
  const b = (e.target as HTMLElement).closest<HTMLAnchorElement>("[data-install]");
  if (!b || !installEvent?.prompt) return; // no prompt: follow the link to the how-to
  e.preventDefault();
  void installEvent.prompt();
  installEvent = null;
});

function widget(slot: string, os: Platform, rel: Release | null, t: Strings): string {
  const url = (k: Kind) => rel?.assets[k]?.url ?? RELEASES_URL;
  const dl = (k: Kind) => (rel?.assets[k] ? " download" : "");
  const other = `<a class="dl-link" href="${t.page}">${t.other}</a>`;
  const big = slot === "foot" ? "" : " lg";
  const main = (label: string, k: Kind, icon: string) =>
    `<a class="pill mint${big} dl-main" href="${url(k)}"${dl(k)}>${icon}<span>${label}</span></a>`;
  const metaLine = (parts: string[]) => `<p class="dl-meta">${parts.filter(Boolean).join(" · ")}</p>`;
  if (slot === "foot") {
    // One quiet line in the footer.
    if (os === "windows") return `<a href="${url("exe")}"${dl("exe")}>${t.windows}</a> · ${other}`;
    if (os === "linux") return `<a href="${url("deb")}"${dl("deb")}>${t.linux} (.deb)</a> · ${other}`;
    if (os === "mac") return `<a href="${url("dmg")}"${dl("dmg")}>${t.mac}</a> · ${other}`;
    return `<a href="${t.page}">${t.app}</a>`;
  }
  if (os === "windows")
    return main(t.windows, "exe", DL_ICONS.windows) + metaLine([meta(rel, "exe"), `<a class="dl-link" href="${url("msi")}"${dl("msi")}>${t.msi}</a>`, other]);
  if (os === "linux")
    return main(t.linux, "deb", DL_ICONS.linux) + metaLine([meta(rel, "deb") && `.deb · ${meta(rel, "deb")}`, `<a class="dl-link" href="${url("appimage")}"${dl("appimage")}>${t.appimage}</a>`, other]);
  if (os === "mac") return main(t.mac, "dmg", DL_ICONS.mac) + metaLine([meta(rel, "dmg"), other]);
  // Phones, tablets, Chromebooks: the web app.
  const note = t.mobile;
  // The hero and the final call to action already have "Open the board".
  const open = slot === "hero" || slot === "final" ? "" : `<a class="pill dark${big}" href="${t.board}">${t.open}</a>`;
  const install = os === "mobile" ? `<a class="pill ghost${big}" href="${t.page}#install" data-install>${t.install}</a>` : "";
  return `<p class="dl-note">${note}</p>${open || install ? `<div class="dl-row">${open}${install}</div>` : ""}${metaLine([other])}`;
}

function render(rel: Release | null) {
  const os = detectPlatform();
  document.querySelectorAll<HTMLElement>("[data-download]").forEach((el) => {
    const lang = el.closest("[lang]")?.getAttribute("lang") === "bn" ? "bn" : "en";
    el.innerHTML = widget(el.dataset.download || "hero", os, rel, T[lang]);
    el.dataset.os = os;
  });
  // /download/: direct links and sizes on every platform card.
  document.querySelectorAll<HTMLAnchorElement>("a[data-asset]").forEach((a) => {
    const asset = rel?.assets[a.dataset.asset as Kind];
    if (asset) {
      a.href = asset.url;
      a.setAttribute("download", "");
    }
  });
  document.querySelectorAll<HTMLElement>("[data-meta]").forEach((m) => (m.textContent = meta(rel, m.dataset.meta as Kind)));
  document.querySelectorAll<HTMLAnchorElement>("a[data-notes]").forEach((a) => (a.href = rel?.notes ?? RELEASES_URL));
  document.querySelectorAll<HTMLElement>("[data-os-card]").forEach((c) => c.classList.toggle("you", c.dataset.osCard === os));
}

// /download/: once a download starts, thank the teacher and point to /donate/.
// The panel ([data-dl-thanks], from site/pages/download.mjs) never blocks the
// page and closes with × or Esc.
const thanks = document.querySelector<HTMLElement>("[data-dl-thanks]");
if (thanks) {
  const hide = () => (thanks.hidden = true);
  document.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-dl-thanks-close]")) return hide();
    const a = t.closest<HTMLAnchorElement>("a[data-asset], [data-download]:not([data-download=foot]) a.dl-main, [data-download] a.dl-link[download]");
    if (a) thanks.hidden = false;
  });
  document.addEventListener("keydown", (e) => e.key === "Escape" && hide());
}

if (document.querySelector("[data-download], [data-asset], [data-os-card]")) {
  render(null);
  void latestRelease().then((rel) => rel && render(rel));
}
