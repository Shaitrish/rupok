// Rupok Cloud API client (the public API: rupok.app/api/v1, see the cloud
// service's openapi.yaml). Written for the app; no server code here.
//
// Web: same origin, the HttpOnly session cookie, and the session's CSRF token
// in `X-CSRF-Token` on every state-changing request.
// Desktop (Tauri): no cookies from the app origin, so a bearer token from the
// native sign-in (desktop.ts) goes in `Authorization` instead.

export interface Plan {
  id: string;
  name: string;
  quota_bytes: number;
  max_file_bytes: number;
}
export interface CloudUser {
  id: string;
  email: string;
  name: string;
  avatar_url: string | null;
  roles?: { teacher?: boolean; student?: boolean };
  plan?: Plan;
}
export interface Me extends CloudUser {
  usage?: { used_bytes: number; quota_bytes: number; available_bytes: number; lessons?: number; folders?: number; trashed_items?: number };
}
export interface CloudNode {
  id: string;
  kind: "folder" | "lesson";
  name: string;
  parent_id: string | null;
  role?: "subject" | "course" | "chapter" | null;
  copied_from?: string | null;
  created_at: number;
  updated_at: number;
  trashed_at?: number | null;
  lesson?: { title: string; pages: number; size: number; content_version: number };
}
export type ShareRole = "viewer" | "copier" | "editor";
export interface Share {
  id: string;
  node_id: string;
  type: "link" | "user";
  role: ShareRole;
  email: string | null;
  created_at: number;
  expires_at: number | null;
  revoked_at: number | null;
  active: boolean;
  token?: string;
  url?: string;
}
export interface ShareInfo {
  share: { role: ShareRole; expires_at: number | null };
  your_role: string;
  signed_in: boolean;
  can_copy: boolean;
  can_edit: boolean;
  owner: { name: string };
  node: CloudNode;
}
export type VV = Record<string, number>;
export interface PushResult {
  seq: number;
  duplicate?: boolean;
  merged?: boolean;
  head?: VV;
}

export class CloudError extends Error {
  constructor(readonly status: number, readonly code: string, message: string, readonly details?: Record<string, unknown>) {
    super(message);
    this.name = "CloudError";
  }
  /** Out of space or file too big: the teacher has to act. */
  get isQuota() {
    return this.status === 507 || this.status === 413 || this.code === "quota_exceeded" || this.code === "file_too_large" || this.code === "daily_upload_limit";
  }
  get isAuth() {
    return this.status === 401;
  }
  get isOffline() {
    return this.status === 0;
  }
}

/** A friendly sentence for an error, with what to do about it. */
export function explain(e: unknown): string {
  if (!(e instanceof CloudError)) return String((e as Error)?.message ?? e);
  switch (e.code) {
    case "quota_exceeded":
      return "You're out of space in Rupok Cloud. Empty the cloud trash, delete lessons you no longer need, or keep this lesson on this device.";
    case "file_too_large":
      return "This lesson is too big for your Rupok Cloud plan. Remove large images or videos, or keep it on this device.";
    case "daily_upload_limit":
      return "You've uploaded a lot today. Try again tomorrow; your lesson is safe on this device.";
    case "share_revoked":
      return "The teacher has turned this link off. Ask them for a new one.";
    case "share_expired":
      return "This link has expired. Ask your teacher for a new one.";
    case "rate_limited":
      return "Too many requests. Wait a minute and try again.";
    case "unauthorized":
      return "Please sign in to Rupok Cloud again.";
    case "offline":
      return "You're offline. Your work is saved on this device and syncs when you're back online.";
    default:
      return e.message || `Rupok Cloud error (${e.status})`;
  }
}

const enc = encodeURIComponent;
const encPath = (p: string) => p.split("/").map(enc).join("/");

export class CloudApi {
  private csrf: string | null = null;
  constructor(
    readonly base = "/api/v1",
    /** Desktop: the bearer token; web: null (cookie). */
    public token: string | null = null,
  ) {}

  url(path: string): string {
    return this.base + path;
  }

  async raw(method: string, path: string, body?: BodyInit | null, headers: Record<string, string> = {}, retried = false): Promise<Response> {
    const h: Record<string, string> = { ...headers };
    if (this.token) h.Authorization = `Bearer ${this.token}`;
    else if (method !== "GET" && method !== "HEAD") {
      if (!this.csrf) await this.session().catch(() => null);
      if (this.csrf) h["X-CSRF-Token"] = this.csrf;
    }
    let res: Response;
    try {
      res = await fetch(this.url(path), { method, body: body ?? undefined, headers: h, credentials: this.token ? "omit" : "same-origin" });
    } catch {
      throw new CloudError(0, "offline", "You're offline.");
    }
    if (res.status === 403 && !retried && !this.token) {
      const err = await res.clone().json().catch(() => null);
      if (err?.error?.code === "csrf_failed") {
        this.csrf = null;
        await this.session().catch(() => null);
        return this.raw(method, path, body, headers, true);
      }
    }
    if (!res.ok) throw await toError(res);
    return res;
  }

  async json<T>(method: string, path: string, body?: unknown): Promise<T> {
    const res = await this.raw(method, path, body === undefined ? undefined : JSON.stringify(body), body === undefined ? {} : { "Content-Type": "application/json" });
    return res.json() as Promise<T>;
  }

  async bytes(method: string, path: string, body?: BodyInit | null, headers: Record<string, string> = {}): Promise<Uint8Array> {
    return new Uint8Array(await (await this.raw(method, path, body, headers)).arrayBuffer());
  }

  // ------------------------------------------------------------- auth

  async session(): Promise<{ user: CloudUser | null; csrf_token: string | null; dev_login?: boolean }> {
    const res = await fetch(this.url("/auth/session"), {
      credentials: this.token ? "omit" : "same-origin",
      headers: this.token ? { Authorization: `Bearer ${this.token}` } : {},
    }).catch(() => {
      throw new CloudError(0, "offline", "You're offline.");
    });
    if (!res.ok) throw await toError(res);
    const s = await res.json();
    this.csrf = s.csrf_token ?? null;
    return s;
  }
  /** Where "Sign in with Google" goes (web). */
  signInUrl(returnTo: string, role?: "teacher" | "student"): string {
    const q = new URLSearchParams({ return_to: returnTo });
    if (role) q.set("role", role);
    return this.url(`/auth/google/start?${q}`);
  }
  /** Local development only (ENVIRONMENT=dev on a local host). */
  async devLogin(email: string, roles: ("teacher" | "student")[] = ["teacher"], name?: string): Promise<CloudUser> {
    const r = await this.json<{ user: CloudUser; csrf_token: string }>("POST", "/auth/dev-login", { email, roles, name });
    this.csrf = r.csrf_token;
    return r.user;
  }
  async logout(): Promise<void> {
    await this.json("POST", "/auth/logout", {});
    this.csrf = null;
  }
  me(): Promise<Me> {
    return this.json("GET", "/me");
  }

  // ------------------------------------------------------------ files

  async list(parentId: string | null): Promise<CloudNode[]> {
    const r = await this.json<{ items: CloudNode[] }>("GET", `/files${parentId ? `?parent_id=${enc(parentId)}` : ""}`);
    return r.items;
  }
  node(id: string): Promise<CloudNode> {
    return this.json("GET", `/files/${enc(id)}`);
  }
  createFolder(name: string, parentId: string | null): Promise<CloudNode> {
    return this.json("POST", "/folders", { name, parent_id: parentId });
  }
  upload(rpk: Uint8Array, name: string, parentId: string | null): Promise<CloudNode> {
    const q = new URLSearchParams({ name });
    if (parentId) q.set("parent_id", parentId);
    return this.raw("POST", `/files?${q}`, rpk as BlobPart as BodyInit, { "Content-Type": "application/vnd.rupok.lesson+zip" }).then((r) => r.json());
  }
  /** Whole-file save: the server merges it with what it has (no conflicted copies). */
  replace(id: string, rpk: Uint8Array): Promise<CloudNode> {
    return this.raw("PUT", `/files/${enc(id)}/content`, rpk as BlobPart as BodyInit, { "Content-Type": "application/vnd.rupok.lesson+zip" }).then((r) => r.json());
  }
  download(id: string): Promise<Uint8Array> {
    return this.bytes("GET", `/files/${enc(id)}/content`);
  }
  update(id: string, patch: { name?: string; parent_id?: string | null }): Promise<CloudNode> {
    return this.json("PATCH", `/files/${enc(id)}`, patch);
  }
  trash(id: string): Promise<unknown> {
    return this.json("DELETE", `/files/${enc(id)}`);
  }
  copy(id: string, parentId: string | null): Promise<CloudNode> {
    return this.json("POST", `/files/${enc(id)}/copy`, { parent_id: parentId });
  }
  async sharedWithMe(): Promise<(CloudNode & { role?: string; owner?: { name: string } })[]> {
    return (await this.json<{ items: (CloudNode & { role?: string; owner?: { name: string } })[] }>("GET", "/shared-with-me")).items;
  }

  // ------------------------------------------------------------- sync

  push(id: string, update: Uint8Array): Promise<PushResult> {
    return this.raw("POST", `/lessons/${enc(id)}/push`, update as BlobPart as BodyInit, { "Content-Type": "application/octet-stream" }).then((r) => r.json());
  }
  /** Updates a document at `vv` lacks: `RPU1` then (u32 BE length, blob)*. */
  async pull(id: string, vv: VV): Promise<{ updates: Uint8Array[]; more: boolean }> {
    const res = await this.raw("POST", `/lessons/${enc(id)}/pull`, JSON.stringify({ vv }), { "Content-Type": "application/json" });
    const b = new Uint8Array(await res.arrayBuffer());
    const updates: Uint8Array[] = [];
    if (b.length >= 4 && String.fromCharCode(...b.subarray(0, 4)) === "RPU1") {
      const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
      for (let p = 4; p + 4 <= b.length; ) {
        const n = v.getUint32(p);
        p += 4;
        if (p + n > b.length) break;
        updates.push(b.slice(p, p + n));
        p += n;
      }
    }
    return { updates, more: res.headers.get("Rupok-More") === "1" };
  }
  async entries(id: string): Promise<{ path: string; hash: string; size: number }[]> {
    return (await this.json<{ items: { path: string; hash: string; size: number }[] }>("GET", `/lessons/${enc(id)}/entries`)).items;
  }
  putEntry(id: string, path: string, bytes: Uint8Array): Promise<unknown> {
    return this.raw("PUT", `/lessons/${enc(id)}/entries/${encPath(path)}`, bytes as BlobPart as BodyInit, { "Content-Type": "application/octet-stream" }).then((r) => r.json());
  }

  // ----------------------------------------------------------- shares

  async shares(id: string): Promise<Share[]> {
    return (await this.json<{ items: Share[] }>("GET", `/files/${enc(id)}/shares`)).items;
  }
  shareLink(id: string, role: ShareRole): Promise<Share> {
    return this.json("POST", `/files/${enc(id)}/shares`, { type: "link", role });
  }
  revoke(shareId: string): Promise<unknown> {
    return this.json("DELETE", `/shares/${enc(shareId)}`);
  }
  shareInfo(token: string): Promise<ShareInfo> {
    return this.json("GET", `/s/${enc(token)}`);
  }
  copyShared(token: string, nodeId?: string, parentId: string | null = null): Promise<CloudNode> {
    return this.json("POST", `/s/${enc(token)}/copy`, { node_id: nodeId, parent_id: parentId });
  }

  // ------------------------------------------------------------ trash

  async listTrash(): Promise<CloudNode[]> {
    return (await this.json<{ items: CloudNode[] }>("GET", "/trash")).items;
  }
  restore(id: string): Promise<unknown> {
    return this.json("POST", `/trash/${enc(id)}/restore`, {});
  }
  deleteForever(id: string): Promise<unknown> {
    return this.json("DELETE", `/trash/${enc(id)}`);
  }
}

async function toError(res: Response): Promise<CloudError> {
  let code = `http_${res.status}`;
  let message = res.statusText || `HTTP ${res.status}`;
  let details: Record<string, unknown> | undefined;
  try {
    const j = await res.json();
    if (j?.error) ({ code = code, message = message, details } = j.error);
  } catch { /* not JSON */ }
  return new CloudError(res.status, code, message, details);
}
