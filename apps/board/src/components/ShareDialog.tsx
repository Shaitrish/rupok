// Share a lesson through Rupok Cloud: a link with a role (view, view and
// copy, edit), copy it, send it on WhatsApp, show a QR code for the class,
// and see or turn off existing links. Signed out or offline, "Share lesson"
// keeps its old behaviour (send the .rpk file).

import { For, Show, createResource, createSignal } from "solid-js";
import { Icon } from "./Icon";
import { LIcon } from "./LibraryIcon";
import { LibDialog } from "./LibraryDialogs";
import { showToast } from "../state";
import { cloud, cloudLibrary, cloudOffline, cloudUser, refreshUsage } from "../cloud/account";
import { type Share, type ShareRole, explain } from "../cloud/api";
import { softGate } from "../cloud/billing";
import type { CloudLibrary } from "../cloud/library";
import { qrSvg } from "../cloud/qr";
import { adoptBoard, controllerRef, current, flush } from "../library/session";
import { basename, isSpecial, stem } from "../library/paths";

interface Target {
  /** The cloud node id, once the lesson is in Rupok Cloud. */
  nodeId: string | null;
  title: string;
  /** Called by "Send the file instead". */
  fallback: () => void;
}
const [target, setTarget] = createSignal<Target | null>(null);

const ROLES: { id: ShareRole; label: string; hint: string }[] = [
  { id: "viewer", label: "Can view", hint: "Students open it in the browser. No account needed." },
  { id: "copier", label: "Can view and copy", hint: "Signed-in students can also make their own copy." },
  { id: "editor", label: "Can edit", hint: "Signed-in colleagues can change the lesson with you." },
];
const roleLabel = (r: ShareRole) => ROLES.find((x) => x.id === r)?.label ?? r;

/**
 * "Share lesson…": opens the dialog when signed in to Rupok Cloud and online.
 * Returns false otherwise, so the caller shares the file the old way.
 */
export async function openShareDialog(fallback: () => void): Promise<boolean> {
  if (!cloudUser() || cloudOffline()) return false;
  const cur = current();
  const lib = cur?.lib as CloudLibrary | undefined;
  let nodeId: string | null = null;
  if (cur && lib && lib === cloudLibrary()) {
    await flush();
    nodeId = (await lib.node(cur.path))?.id ?? null;
  }
  setTarget({ nodeId, title: controllerRef().board.title() || "Lesson", fallback });
  return true;
}

export function ShareHost() {
  return <Show when={target()}>{(t) => <ShareDialog t={t()} onClose={() => setTarget(null)} />}</Show>;
}

function ShareDialog(props: { t: Target; onClose: () => void }) {
  const [nodeId, setNodeId] = createSignal(props.t.nodeId);
  const [role, setRole] = createSignal<ShareRole>("viewer");
  const [busy, setBusy] = createSignal(false);
  const [fresh, setFresh] = createSignal<Share | null>(null);
  const [links, { refetch }] = createResource(nodeId, async (id) => (await cloud.shares(id)).filter((s) => s.type === "link" && s.active));

  // A plan limit (10 active links on Free) opens the friendly Upgrade dialog instead of a dead end.
  const fail = (e: unknown) => {
    if (!softGate(e)) showToast(explain(e), 6000);
  };

  /** A local lesson goes to Rupok Cloud first (same folders), and is edited there from now on. */
  const upload = async () => {
    const lib = cloudLibrary();
    if (!lib) return;
    setBusy(true);
    try {
      const cur = current();
      const rel = cur && !isSpecial(cur.path) ? cur.path : `${stem(basename(cur?.path ?? "")) || props.t.title}.rpk`;
      const dir = rel.split("/").slice(0, -1).join("/");
      const path = await lib.freeLessonPath(dir, stem(basename(rel)) || props.t.title);
      await adoptBoard(path, { lib, id: cur?.id, author: cur?.author, copiedFrom: cur?.copiedFrom });
      const node = await lib.node(path);
      if (!node) throw new Error("The upload did not arrive");
      setNodeId(node.id);
      void refreshUsage();
      showToast("The lesson is in Rupok Cloud now: edits sync, and links always show the latest version");
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const create = async () => {
    const id = nodeId();
    if (!id) return;
    setBusy(true);
    try {
      setFresh(await cloud.shareLink(id, role()));
      void refetch();
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const copy = async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
      showToast("Link copied");
    } catch {
      showToast(url, 8000);
    }
  };
  const whatsapp = (url: string) => {
    const text = `${props.t.title}: ${url}`;
    window.open(`https://wa.me/?text=${encodeURIComponent(text)}`, "_blank", "noopener");
  };
  const revoke = async (s: Share) => {
    try {
      await cloud.revoke(s.id);
      if (fresh()?.id === s.id) setFresh(null);
      void refetch();
      showToast("Link turned off. Anyone who opens it now sees that it was turned off.");
    } catch (e) {
      fail(e);
    }
  };

  return (
    <LibDialog icon="cloud" title={`Share “${props.t.title}”`} sub="Send a link: students open the lesson in the browser, with replay, no account needed." width={620} onClose={props.onClose}>
      <Show
        when={nodeId()}
        fallback={
          <div class="share-upload">
            <p>To share a link, this lesson goes to your Rupok Cloud space first. After that, your edits sync there and the link always shows the latest version.</p>
            <div class="actions">
              <button class="btn" onClick={() => { props.onClose(); props.t.fallback(); }}>Send the file instead</button>
              <div class="grow" />
              <button class="btn solid" disabled={busy()} onClick={() => void upload()}><LIcon name="cloud" /><span class="label">{busy() ? "Uploading…" : "Upload and share"}</span></button>
            </div>
          </div>
        }
      >
        <div class="field">
          <label>Anyone with the link</label>
          <div class="seg share-roles" role="radiogroup" aria-label="Link role">
            <For each={ROLES}>
              {(r) => (
                <button role="radio" aria-checked={role() === r.id} classList={{ on: role() === r.id }} data-role={r.id} onClick={() => setRole(r.id)}>{r.label}</button>
              )}
            </For>
          </div>
          <span class="hint">{ROLES.find((r) => r.id === role())?.hint}</span>
        </div>
        <Show when={!fresh()}>
          <div class="actions" style={{ "margin-bottom": "16px" }}>
            <button class="btn solid" disabled={busy()} onClick={() => void create()}><Icon name="plus" /><span class="label">Create link</span></button>
          </div>
        </Show>
        <Show when={fresh()}>
          {(s) => (
            <div class="share-link">
              <div class="share-qr" innerHTML={qrSvg(s().url!)} role="img" aria-label="QR code for the link" />
              <div class="share-side">
                <span class="share-role">{roleLabel(s().role)}</span>
                <input class="input share-url" readOnly value={s().url} aria-label="Share link" onFocus={(e) => e.currentTarget.select()} />
                <div class="share-acts">
                  <button class="btn solid" onClick={() => void copy(s().url!)}><Icon name="duplicate" /><span class="label">Copy link</span></button>
                  <button class="btn" onClick={() => whatsapp(s().url!)}><LIcon name="shared" /><span class="label">WhatsApp</span></button>
                  <button class="btn" onClick={() => setFresh(null)}>New link</button>
                </div>
                <span class="hint">Show the QR code on the board: students scan it with their phone.</span>
              </div>
            </div>
          )}
        </Show>
        <Show when={(links() ?? []).length}>
          <div class="section-title">Links</div>
          <ul class="share-list">
            <For each={links()}>
              {(s) => (
                <li>
                  <span class="share-role">{roleLabel(s.role)}</span>
                  <span class="share-when">Made {new Date(s.created_at).toLocaleDateString(undefined, { day: "numeric", month: "short" })}</span>
                  <div class="grow" />
                  <button class="btn" onClick={() => void copy(s.url!)}>Copy</button>
                  <button class="btn danger" onClick={() => void revoke(s)}>Turn off</button>
                </li>
              )}
            </For>
          </ul>
        </Show>
        <div class="actions" style={{ "margin-top": "16px" }}>
          <button class="btn" onClick={() => { props.onClose(); props.t.fallback(); }}><Icon name="download" /><span class="label">Send the file instead</span></button>
          <div class="grow" />
          <button class="btn" onClick={props.onClose}>Done</button>
        </div>
      </Show>
    </LibDialog>
  );
}
