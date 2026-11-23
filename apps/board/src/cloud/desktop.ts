// Desktop (Tauri) sign-in to Rupok Cloud. Cookies don't work from the app's
// origin, so the desktop app gets a bearer token instead:
//
// 1. The app makes a PKCE verifier and a random `state`, and asks the Rust
//    side (`cloud_auth_begin`) to listen on a loopback port and open the
//    system browser at rupok.app/api/v1/auth/native/start (Rust builds that
//    URL itself: web content can never make it open anything else).
// 2. The teacher signs in with Google in the browser. The server redirects to
//    http://127.0.0.1:<port>/callback?code=…&state=… with a one-time code.
// 3. `cloud_auth_wait` returns the code once `state` matches; the app trades
//    code + verifier for a session token (POST /auth/native/token). The code
//    is useless without the verifier, works once, and expires in a minute.

import { desktopInvoke } from "../platform";
import { cloud } from "./account";

const b64url = (b: Uint8Array) => btoa(String.fromCharCode(...b)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const random = (n: number) => b64url(crypto.getRandomValues(new Uint8Array(n)));

/** Sign in through the system browser. Resolves to the token, or null if cancelled. */
export async function desktopSignIn(): Promise<string | null> {
  const verifier = random(32);
  const challenge = b64url(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier))));
  const state = random(16);
  await desktopInvoke("cloud_auth_begin", { challenge, state });
  const r = await desktopInvoke<{ code: string } | null>("cloud_auth_wait", { state });
  if (!r?.code) return null;
  const res = await fetch(cloud.url("/auth/native/token"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code: r.code, verifier }),
    credentials: "omit",
  });
  if (!res.ok) throw new Error(`Sign-in failed (${res.status})`);
  const j = await res.json();
  return typeof j.token === "string" ? j.token : null;
}
