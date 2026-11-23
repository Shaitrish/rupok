// The student viewer for Rupok Cloud share links:
//   rupok.app/s/<token> → /board/?view&src=/api/v1/s/<token>/content&share=<token>
// The lesson is fetched from `src` (same origin, /api/v1/ only), opened in the
// Student view, and the bar shows the title, the teacher and "Make a copy".

import { createSignal } from "solid-js";
import type { Controller } from "../controller";
import { CloudError, type ShareInfo, explain } from "./api";
import { cloud, cloudUser, devLogin, initCloud } from "./account";

export interface SharedLesson {
  token: string;
  info: ShareInfo | null;
  /** The link cannot be opened (revoked, expired, not found…). */
  error?: { title: string; message: string };
  loading: boolean;
}
export const [shared, setShared] = createSignal<SharedLesson | null>(null);
/** "Make a copy" wants a dev sign-in first (local development). */
export const [askDevLogin, setAskDevLogin] = createSignal(false);

/**
 * `src` is accepted only as a same-origin path under /api/v1/ (no other
 * origins, no scheme-relative `//host`, no backslashes, no `..`).
 */
export function safeSrc(src: string | null): string | null {
  if (!src || !src.startsWith("/api/v1/") || src.includes("\\") || src.includes("//") || /(^|\/)\.\.?(\/|$)/.test(src)) return null;
  try {
    const u = new URL(src, location.origin);
    if (u.origin !== location.origin || !u.pathname.startsWith("/api/v1/")) return null;
    return u.pathname + u.search;
  } catch {
    return null;
  }
}

const TOKEN_RE = /^[A-Za-z0-9_-]{32}$/;

/** Open the lesson a share link points at. Returns true if the URL asked for one. */
export function openSharedFromUrl(ctl: Controller): boolean {
  const q = new URLSearchParams(location.search);
  const rawSrc = q.get("src");
  if (rawSrc === null) return false;
  const tokenParam = q.get("share");
  const token = tokenParam && TOKEN_RE.test(tokenParam) ? tokenParam : "";
  const src = safeSrc(rawSrc);
  if (!src) {
    setShared({ token, info: null, loading: false, error: { title: "This link can't be opened", message: "Lesson links must come from Rupok itself. Ask your teacher to send the link again." } });
    return true;
  }
  setShared({ token, info: null, loading: true });
  void (async () => {
    try {
      const headers: Record<string, string> = token ? { "Rupok-Share-Token": token } : {};
      const res = await fetch(src, { credentials: "same-origin", headers });
      if (!res.ok) {
        const j = await res.json().catch(() => null);
        throw new CloudError(res.status, j?.error?.code ?? `http_${res.status}`, j?.error?.message ?? res.statusText);
      }
      const bytes = new Uint8Array(await res.arrayBuffer());
      const info = token ? await cloud.shareInfo(token).catch(() => null) : null;
      const name = (info?.node.name ?? "Lesson").replace(/[\\/:*?"<>|]+/g, "-") + ".rpk";
      setShared({ token, info, loading: false });
      await ctl.importFiles([new File([bytes as BlobPart], name)]);
      // Back from "Sign in to make a copy": finish what the student asked for.
      if (q.has("copy") && info) {
        await initCloud();
        if (cloudUser()) await makeCopy();
      }
    } catch (e) {
      const gone = e instanceof CloudError && (e.status === 410 || e.status === 404);
      setShared({
        token, info: null, loading: false,
        error: gone
          ? { title: e instanceof CloudError && e.code === "share_expired" ? "This link has expired" : "This link was turned off", message: explain(e) }
          : { title: "Couldn't open the lesson", message: explain(e) },
      });
    }
  })();
  return true;
}

/** Copying needs a role of `copier` or `editor` on the link, and an account. */
export const canMakeCopy = () => {
  const s = shared();
  return !!s?.info && (s.info.share.role === "copier" || s.info.share.role === "editor");
};

/** "Make a copy" into the student's cloud library, then open the copy. */
export async function makeCopy(): Promise<void> {
  const s = shared();
  if (!s?.info) return;
  await initCloud();
  if (!cloudUser()) {
    if (devLogin()) {
      setAskDevLogin(true);
      return;
    }
    const back = new URL(location.href);
    back.searchParams.set("copy", "1");
    location.href = cloud.signInUrl(back.pathname + back.search.replace(/=(?=&|$)/g, ""), "student");
    return;
  }
  const node = await cloud.copyShared(s.token, s.info.node.id);
  location.href = `${location.pathname}?cloud=${encodeURIComponent(node.name + ".rpk")}`;
}
