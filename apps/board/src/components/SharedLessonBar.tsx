// Student viewer opened from a Rupok Cloud link: the lesson title, the
// teacher's name and "Make a copy"; or a friendly page when the link was
// turned off or has expired.

import { Show, createSignal } from "solid-js";
import { Icon } from "./Icon";
import { LIcon } from "./LibraryIcon";
import { LibDialog } from "./LibraryDialogs";
import { CloudSignIn } from "./CloudAccount";
import { showToast } from "../state";
import { cloudUser } from "../cloud/account";
import { explain } from "../cloud/api";
import { askDevLogin, canMakeCopy, makeCopy, setAskDevLogin, shared } from "../cloud/viewer";

export function SharedLessonBar() {
  const [busy, setBusy] = createSignal(false);
  const copy = async () => {
    setBusy(true);
    try {
      await makeCopy();
    } catch (e) {
      showToast(explain(e), 6000);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Show when={shared()}>
      {(s) => (
        <>
          <Show when={s().error}>
            {(err) => (
              <div class="shared-gone">
                <div class="shared-gone-card" role="alert">
                  <div class="badge"><LIcon name="lock" size={28} /></div>
                  <h2>{err().title}</h2>
                  <p>{err().message}</p>
                  <a class="btn solid" href="/">Go to Rupok</a>
                </div>
              </div>
            )}
          </Show>
          <Show when={s().loading}>
            <div class="shared-gone"><div class="shared-gone-card"><p>Opening the lesson…</p></div></div>
          </Show>
          <Show when={s().info}>
            {(info) => (
              <div class="shared-bar surface" role="region" aria-label="Shared lesson">
                <span class="shared-title">
                  <b>{info().node.lesson?.title ?? info().node.name}</b>
                  <span>Shared by {info().owner.name || "your teacher"}</span>
                </span>
                <Show when={canMakeCopy()}>
                  <button class="btn solid" disabled={busy()} onClick={() => void copy()} title={cloudUser() ? "Copy it into your Rupok Cloud" : "Sign in to keep your own copy"}>
                    <Icon name="duplicate" /><span class="label">{busy() ? "Copying…" : "Make a copy"}</span>
                  </button>
                </Show>
              </div>
            )}
          </Show>
          <Show when={askDevLogin()}>
            <LibDialog icon="cloud" title="Sign in to make a copy" sub="Your copy goes to your own Rupok Cloud space." width={460} onClose={() => setAskDevLogin(false)}>
              <CloudSignIn />
              <div class="actions">
                <div class="grow" />
                <button class="btn solid" disabled={!cloudUser()} onClick={() => { setAskDevLogin(false); void copy(); }}>Make a copy</button>
              </div>
            </LibDialog>
          </Show>
        </>
      )}
    </Show>
  );
}
