// Rupok Cloud account state: who is signed in, usage against the quota.
// The app works fully without it; everything cloud is hidden until sign-in.

import { createSignal } from "solid-js";
import { isDesktop } from "../platform";
import { CloudApi, type CloudUser, type Me } from "./api";
import { CloudDriver, CloudLibrary } from "./library";

const TOKEN_KEY = "rupok.cloud.token";

function savedToken(): string | null {
  if (!isDesktop) return null;
  try {
    return localStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** Desktop talks to rupok.app directly; the web app is on the same origin. */
export const CLOUD_ORIGIN = "https://rupok.app";
export const cloud = new CloudApi(isDesktop ? `${CLOUD_ORIGIN}/api/v1` : "/api/v1", savedToken());

export const [cloudUser, setCloudUser] = createSignal<CloudUser | null>(null);
export const [cloudMe, setCloudMe] = createSignal<Me | null>(null);
/** The server offers dev-login (local development only). */
export const [devLogin, setDevLogin] = createSignal(false);
/** Rupok Cloud could not be reached (offline, or not deployed here). */
export const [cloudOffline, setCloudOffline] = createSignal(false);

let started: Promise<void> | null = null;

let lib: CloudLibrary | null = null;
/** The signed-in user's cloud library (a Library provider), or null. */
export function cloudLibrary(): CloudLibrary | null {
  const u = cloudUser();
  if (!u) return null;
  if (lib?.id !== `cloud:${u.id}`) lib = new CloudLibrary(new CloudDriver(cloud, `cloud:${u.id}`));
  return lib;
}
/** After start-up's session check. */
export async function cloudLibraryReady(): Promise<CloudLibrary | null> {
  await initCloud();
  return cloudLibrary();
}

/** Ask who is signed in. Never throws; offline just means signed out. */
export function initCloud(): Promise<void> {
  started ??= refreshCloud();
  return started;
}

export async function refreshCloud(): Promise<void> {
  try {
    const s = await cloud.session();
    setCloudOffline(false);
    setDevLogin(!!s.dev_login);
    setCloudUser(s.user);
    if (s.user) await refreshUsage();
    else setCloudMe(null);
  } catch {
    setCloudOffline(true);
    setCloudUser(null);
  }
}

export async function refreshUsage(): Promise<void> {
  try {
    setCloudMe(await cloud.me());
  } catch { /* keep the last numbers */ }
}

/** "Sign in with Google". Web: a full-page redirect back to `returnTo`. */
export async function signIn(returnTo = location.pathname + location.search): Promise<void> {
  if (isDesktop) {
    const { desktopSignIn } = await import("./desktop");
    const token = await desktopSignIn();
    if (!token) return;
    setToken(token);
    await refreshCloud();
    return;
  }
  location.href = cloud.signInUrl(returnTo);
}

/** Local development: sign in as any email (the server allows it only on localhost in dev). */
export async function signInDev(email: string, roles: ("teacher" | "student")[] = ["teacher"]): Promise<void> {
  const user = await cloud.devLogin(email, roles);
  setCloudUser(user);
  await refreshUsage();
}

export async function signOut(): Promise<void> {
  try {
    await cloud.logout();
  } catch { /* signed out locally anyway */ }
  setToken(null);
  setCloudUser(null);
  setCloudMe(null);
}

function setToken(token: string | null) {
  cloud.token = token;
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch { /* storage unavailable */ }
}
