// The latest desktop release on GitHub, and which installer fits this device.
// Shared by the website's download buttons (download.ts) and the desktop
// app's update notice (updates/updates.ts). No side effects on import.

import { ASSET_SUFFIX, RELEASES_API, RELEASES_URL } from "../site/icons.mjs";

export type AssetKind = keyof typeof ASSET_SUFFIX;
export type Platform = "windows" | "mac" | "linux" | "mobile";
export interface Asset { url: string; size: number }
export interface Release { version: string; notes: string; assets: Partial<Record<AssetKind, Asset>> }

export { RELEASES_URL };

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

/** The main installer for a platform: what the big download button links to. */
export const MAIN_ASSET: Record<Platform, AssetKind | null> = { windows: "exe", mac: "dmg", linux: "deb", mobile: null };

/** Turn the GitHub API's release JSON into a Release. */
export function parseRelease(j: any): Release {
  const assets: Release["assets"] = {};
  for (const [kind, suffix] of Object.entries(ASSET_SUFFIX) as [AssetKind, string][]) {
    const a = (j?.assets ?? []).find((x: { name?: string }) => typeof x?.name === "string" && x.name.endsWith(suffix));
    if (a) assets[kind] = { url: a.browser_download_url, size: a.size };
  }
  return { version: String(j?.tag_name ?? "").replace(/^v/, ""), notes: j?.html_url ?? RELEASES_URL, assets };
}

let pending: Promise<Release | null> | null = null;
/**
 * The latest release, cached for the session (sessionStorage). Null on a rate
 * limit, an error or no release: callers keep their fallback and stay quiet.
 * `fresh` skips the session cache (Settings → About → Check for updates).
 */
export function latestRelease(fresh = false): Promise<Release | null> {
  if (pending && !fresh) return pending;
  const run = (async () => {
    if (!fresh) {
      try {
        const hit = sessionStorage.getItem(CACHE);
        if (hit) return JSON.parse(hit) as Release;
      } catch { /* storage unavailable */ }
    }
    try {
      const r = await fetch(RELEASES_API, { headers: { Accept: "application/vnd.github+json" }, cache: "no-store" });
      if (!r.ok) return null; // rate limit or no release
      const rel = parseRelease(await r.json());
      if (!rel.version) return null;
      try { sessionStorage.setItem(CACHE, JSON.stringify(rel)); } catch { /* fine */ }
      return rel;
    } catch {
      return null;
    }
  })();
  pending = run;
  return run;
}

/**
 * Compare dotted versions ("0.2.10" > "0.2.9"). A pre-release ("0.3.0-rc.1")
 * sorts before its release. Returns <0, 0 or >0.
 */
export function compareVersions(a: string, b: string): number {
  const split = (v: string) => {
    const [core, pre = ""] = v.trim().replace(/^v/, "").split("-", 2);
    return { nums: core.split(".").map((n) => parseInt(n, 10) || 0), pre };
  };
  const x = split(a);
  const y = split(b);
  for (let i = 0; i < Math.max(x.nums.length, y.nums.length); i++) {
    const d = (x.nums[i] ?? 0) - (y.nums[i] ?? 0);
    if (d) return d;
  }
  if (x.pre === y.pre) return 0;
  if (!x.pre) return 1;
  if (!y.pre) return -1;
  return x.pre < y.pre ? -1 : 1;
}
