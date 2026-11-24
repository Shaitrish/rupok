// Rupok Cloud account: "Sign in with Google", or who you are, your plan and
// how much of your space is used. Used in the Library sidebar and Settings.

import { Show, createSignal } from "solid-js";
import { bytes } from "./LibraryIcon";
import { Icon } from "./Icon";
import { showToast } from "../state";
import { cloudMe, cloudOffline, cloudUser, devLogin, signIn, signInDev, signOut } from "../cloud/account";
import { explain } from "../cloud/api";
import { openUpgrade } from "../cloud/billing";
import "./Billing.css";

/** Usage against the quota, as a bar and a sentence. */
export function CloudUsage() {
  const u = () => cloudMe()?.usage;
  const pct = () => (u() ? Math.min(100, (u()!.used_bytes / Math.max(1, u()!.quota_bytes)) * 100) : 0);
  return (
    <Show when={u()}>
      <div class="cloud-usage" classList={{ full: pct() >= 90 }}>
        <div class="lib-meter" role="meter" aria-label="Rupok Cloud space used" aria-valuemin={0} aria-valuemax={u()!.quota_bytes} aria-valuenow={u()!.used_bytes}>
          <i style={{ width: `${Math.max(1, pct())}%` }} />
        </div>
        <span class="lib-foot-note">{bytes(u()!.used_bytes)} of {bytes(u()!.quota_bytes)} used</span>
        <Show when={pct() >= 80 && (cloudMe()?.plan?.id ?? "free") === "free"}>
          <span class="bl-cloud-hint">Running out of space? <button onClick={() => openUpgrade("pro", "More room for your lessons: 20 GB with Pro.")}>Try Pro free for 14 days</button></span>
        </Show>
      </div>
    </Show>
  );
}

function Avatar(props: { size?: number }) {
  const u = () => cloudUser();
  const initial = () => (u()?.name || u()?.email || "?").trim()[0]?.toUpperCase() ?? "?";
  return (
    <span class="cloud-avatar" style={{ width: `${props.size ?? 32}px`, height: `${props.size ?? 32}px` }}>
      <Show when={u()?.avatar_url} fallback={initial()}>
        <img src={u()!.avatar_url!} alt="" referrerpolicy="no-referrer" />
      </Show>
    </span>
  );
}

/** The sign-in button (and, on a local dev server, a dev sign-in). */
export function CloudSignIn(props: { compact?: boolean }) {
  const [email, setEmail] = createSignal("teacher@example.org");
  const dev = async () => {
    try {
      await signInDev(email().trim());
      showToast("Signed in to Rupok Cloud");
    } catch (e) {
      showToast(explain(e), 5000);
    }
  };
  return (
    <div class="cloud-signin">
      <button class="btn cloud-google" onClick={() => void signIn()} disabled={cloudOffline() && !devLogin()} title="Sync lessons between devices and share links with students">
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path fill="#4285F4" d="M23 12.3c0-.8-.1-1.6-.2-2.3H12v4.4h6.2a5.3 5.3 0 0 1-2.3 3.5v2.9h3.7c2.2-2 3.4-5 3.4-8.5z" /><path fill="#34A853" d="M12 23.5c3.1 0 5.7-1 7.6-2.8l-3.7-2.9c-1 .7-2.4 1.1-3.9 1.1-3 0-5.5-2-6.4-4.7H1.8v3A11.5 11.5 0 0 0 12 23.5z" /><path fill="#FBBC05" d="M5.6 14.2a6.9 6.9 0 0 1 0-4.4v-3H1.8a11.5 11.5 0 0 0 0 10.4l3.8-3z" /><path fill="#EA4335" d="M12 5.1c1.7 0 3.2.6 4.4 1.7l3.3-3.3A11.5 11.5 0 0 0 1.8 6.8l3.8 3C6.5 7.1 9 5.1 12 5.1z" /></svg>
        <span class="label">Sign in with Google</span>
      </button>
      <Show when={!props.compact}>
        <span class="hint">{cloudOffline() ? "Rupok Cloud is coming soon (or not reachable right now)." : "Optional: sync between devices and share links. 500 MB free."}</span>
      </Show>
      <Show when={devLogin()}>
        <form class="cloud-dev" onSubmit={(e) => { e.preventDefault(); void dev(); }}>
          <input class="input" type="email" aria-label="Dev sign-in email" value={email()} onInput={(e) => setEmail(e.currentTarget.value)} />
          <button class="btn" type="submit">Dev sign-in</button>
        </form>
      </Show>
    </div>
  );
}

/** Signed in: avatar, name, plan, usage and sign out. Signed out: the button. */
export function CloudAccount(props: { compact?: boolean }) {
  return (
    <Show when={cloudUser()} fallback={<CloudSignIn compact={props.compact} />}>
      {(u) => (
        <div class="cloud-account">
          <div class="cloud-who">
            <Avatar />
            <span class="cloud-name">
              <b>{u().name || u().email}</b>
              <span>{cloudMe()?.plan?.name ?? u().plan?.name ?? "Free"} plan · Rupok Cloud</span>
            </span>
            <button class="btn icon" title="Sign out of Rupok Cloud" aria-label="Sign out" onClick={() => void signOut().then(() => showToast("Signed out of Rupok Cloud"))}>
              <Icon name="exit" size={18} />
            </button>
          </div>
          <CloudUsage />
        </div>
      )}
    </Show>
  );
}
