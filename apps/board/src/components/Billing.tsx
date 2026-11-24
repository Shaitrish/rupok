// Rupok Cloud plans in the app: the plan and usage panel (Settings → General
// → Rupok Cloud), the receipts list, and the Upgrade dialog with
// region-aware checkout (SSLCommerz redirect in Bangladesh, Paddle overlay
// elsewhere). Gates are soft: the board never stops working, and every
// limit offers a way forward ("try Pro free for 14 days").

import { For, Show, createMemo, createResource, createSignal, onMount } from "solid-js";
import "./Billing.css";
import { LibDialog } from "./LibraryDialogs";
import { CloudSignIn } from "./CloudAccount";
import { bytes } from "./LibraryIcon";
import { showToast } from "../state";
import { isDesktop } from "../platform";
import { cloudUser, refreshUsage } from "../cloud/account";
import { explain } from "../cloud/api";
import {
  type Plans,
  type Price,
  type Receipt,
  type Region,
  billing,
  billingApi,
  checkout,
  day,
  desktopUpgradeUrl,
  manageSubscription,
  money,
  openUpgrade,
  refreshBilling,
  setUpgradeRequest,
  upgradeRequest,
} from "../cloud/billing";

const PRO_POINTS = ["20 GB of cloud storage", "Unlimited student links", "Full version history", "150 Rupok AI credits a month", "Priority support"];
const CENTRE_POINTS = ["Pro for 5 teachers", "30 GB per teacher", "An admin view and a shared library (soon)", "Live classes for 300 students (soon)"];
const SOURCE: Record<string, string> = { trial: "free trial", sslcommerz: "prepaid", paddle: "subscription", org: "through your school", invoice: "invoice", manual: "" };

/** A labelled meter: used of limit. `limit` 0 hides the bar. */
function Meter(props: { label: string; used: number; limit: number; text: string; warn?: boolean }) {
  const pct = () => (props.limit > 0 ? Math.min(100, (props.used / props.limit) * 100) : 0);
  return (
    <div class="bl-meter" classList={{ warn: props.warn ?? pct() >= 90 }}>
      <div class="bl-meter-head">
        <span>{props.label}</span>
        <b>{props.text}</b>
      </div>
      <Show when={props.limit > 0}>
        <div class="lib-meter" role="meter" aria-label={props.label} aria-valuemin={0} aria-valuemax={props.limit} aria-valuenow={props.used}>
          <i style={{ width: `${Math.max(1, pct())}%` }} />
        </div>
      </Show>
    </div>
  );
}

/** Settings → Rupok Cloud → Plan: what you have, what you've used, and what to do next. */
export function BillingPanel() {
  onMount(() => void refreshBilling());
  const b = () => billing();
  const [busy, setBusy] = createSignal(false);
  const trial = async () => {
    setBusy(true);
    try {
      await billingApi.startTrial();
      await refreshBilling();
      void refreshUsage();
      showToast("Pro is on for 14 days. No card, nothing to cancel.", 4000);
    } catch (e) {
      showToast(explain(e), 5000);
    } finally {
      setBusy(false);
    }
  };
  const manage = async () => {
    try {
      await manageSubscription();
    } catch (e) {
      showToast(explain(e), 5000);
    }
  };
  return (
    <Show when={b()} fallback={<p class="bl-muted">Plans appear here once Rupok Cloud is reachable.</p>}>
      {(s) => (
        <div class="bl-panel">
          <div class="bl-plan">
            <span class="bl-plan-name">{s().plan.name}</span>
            <span class="bl-plan-sub">
              <Show when={s().plan.until} fallback={s().plan.id === "free" ? "Free forever. The whole app is free; this is your cloud plan." : ""}>
                {SOURCE[s().plan.source ?? ""] ? `${SOURCE[s().plan.source ?? ""]}, ` : ""}
                {s().subscription && !s().subscription!.cancel_at ? "renews" : "until"} {day(s().subscription?.period_end ?? s().plan.until!)}
              </Show>
            </span>
          </div>
          <Show when={s().prepaid?.renew_soon}>
            <div class="bl-notice" role="status">
              Your {s().plan.name} ends on {day(s().prepaid!.until)}. Renew to keep your space and links; your lessons stay either way.
              <button class="btn solid" onClick={() => openUpgrade(s().plan.id)}>Renew</button>
            </div>
          </Show>
          <Show when={s().subscription?.status === "past_due"}>
            <div class="bl-notice warn" role="status">
              Your last payment didn't go through. Update your card to keep {s().plan.name}.
              <button class="btn" onClick={() => void manage()}>Update card</button>
            </div>
          </Show>
          <div class="bl-meters">
            <Meter label="Cloud storage" used={s().usage.storage_bytes} limit={s().limits.storage_bytes} text={`${bytes(s().usage.storage_bytes)} of ${bytes(s().limits.storage_bytes)}`} />
            <Meter
              label="Rupok AI credits"
              used={Math.max(0, s().credits.monthly_allowance - s().credits.monthly_remaining)}
              limit={s().credits.monthly_allowance}
              warn={s().credits.available === 0}
              text={s().credits.monthly_allowance ? `${s().credits.available} left · ${s().credits.monthly_allowance} a month` : `${s().credits.available} left`}
            />
            <Meter
              label="Active student links"
              used={s().usage.active_links}
              limit={s().limits.active_links >= 1000 ? 0 : s().limits.active_links}
              text={s().limits.active_links >= 1000 ? `${s().usage.active_links} · unlimited` : `${s().usage.active_links} of ${s().limits.active_links}`}
            />
          </div>
          <p class="bl-muted">Your own Claude, Codex or OpenCode subscription always works for free; credits are for Rupok's hosted AI.</p>
          <div class="bl-actions">
            <Show when={s().plan.id === "free" || s().plan.source === "trial"}>
              <button class="btn solid" onClick={() => openUpgrade("pro")}>Upgrade</button>
            </Show>
            <Show when={s().trial.available}>
              <button class="btn" disabled={busy()} onClick={() => void trial()}>Try Pro free for {s().trial.days} days</button>
            </Show>
            <Show when={s().plan.id !== "free" && s().plan.source !== "trial"}>
              <button class="btn" onClick={() => openUpgrade(s().plan.id)}>{s().plan.source === "sslcommerz" ? "Add more time" : "Plans"}</button>
            </Show>
            <Show when={s().can_manage_subscription}>
              <button class="btn" onClick={() => void manage()}>Manage subscription</button>
            </Show>
            <button class="btn" onClick={() => openUpgrade(undefined, "credits")}>Buy AI credits</button>
          </div>
          <For each={s().orgs}>
            {(o) => (
              <p class="bl-muted">
                {o.role === "admin" ? "You manage" : "You're a teacher at"} <b>{o.name}</b> ({o.members} of {o.seats} seats used).
              </p>
            )}
          </For>
        </div>
      )}
    </Show>
  );
}

/** Settings → Rupok Cloud → Receipts. */
export function ReceiptsList() {
  const [list] = createResource(() => cloudUser()?.id, () => billingApi.receipts().catch(() => [] as Receipt[]));
  return (
    <Show when={(list() ?? []).length} fallback={<p class="bl-muted">No payments yet. Receipts for every payment appear here.</p>}>
      <ul class="bl-receipts">
        <For each={list()}>
          {(r) => (
            <li>
              <span class="bl-r-main">
                <b>{r.description}</b>
                <span>
                  {day(r.paid_at ?? r.created_at)} · {r.receipt_number ?? r.id}
                  {r.method ? ` · ${r.method.replace(/-.*/, "")}` : ""}
                  {r.status === "review" ? " · being checked" : ""}
                </span>
              </span>
              <span class="bl-r-amt">{money(r.amount, r.currency)}</span>
              <Show when={r.status === "paid"}>
                <a class="btn" href={billingApi.receiptUrl(r)} target="_blank" rel="noopener">{r.invoice_available ? "Invoice" : "Receipt"}</a>
              </Show>
            </li>
          )}
        </For>
      </ul>
    </Show>
  );
}

// ------------------------------------------------------------- Upgrade
export function UpgradeHost() {
  return <Show when={upgradeRequest()}>{(r) => <UpgradeDialog plan={r().plan} reason={r().reason} onClose={() => setUpgradeRequest(null)} />}</Show>;
}

function UpgradeDialog(props: { plan?: string; reason?: string; onClose: () => void }) {
  const [region, setRegion] = createSignal<Region | undefined>(billing()?.saved_region ?? undefined);
  const [plans] = createResource(region, (r) => billingApi.plans(r), { initialValue: undefined });
  // First load without a region: the server picks it (IP country, saved choice).
  const [first] = createResource(() => (region() === undefined ? "auto" : null), () => billingApi.plans());
  const data = (): Plans | undefined => (region() ? plans() : first());
  const cur = () => data()?.region ?? "INTL";
  const [period, setPeriod] = createSignal<"month" | "year">("year");
  const [months, setMonths] = createSignal(12);
  // Not remembered anywhere: it goes to SSLCommerz with this one payment.
  const [phone, setPhone] = createSignal("");
  const [busy, setBusy] = createSignal<string | null>(null);
  const credits = props.reason === "credits";

  const plan = (id: string) => data()?.plans.find((p) => p.id === id);
  const proPrice = createMemo<Price | undefined>(() => {
    const p = plan("pro")?.prices ?? [];
    return cur() === "BD" ? p.find((x) => x.months === months()) : p.find((x) => x.interval === period());
  });
  const perMonth = () => {
    const p = proPrice();
    if (!p?.per_month || (p.interval !== "year" && (p.months ?? 1) <= 1)) return "";
    return `${money(p.currency === "BDT" ? Math.round(Number(p.per_month)) : p.per_month, p.currency)} a month`;
  };
  const centrePrice = createMemo<Price | undefined>(() => (plan("centre")?.prices ?? []).find((x) => x.months === (months() === 6 ? 12 : months())));

  const switchRegion = (r: Region) => {
    setRegion(r);
    if (cloudUser()) void billingApi.setRegion(r).catch(() => {});
  };

  const buy = async (price: Price | undefined) => {
    if (!price) return;
    if (price.provider === "sslcommerz" && !/^\+?[0-9][0-9 -]{6,18}$/.test(phone().trim())) {
      showToast("Enter your mobile number first: SSLCommerz sends the payment receipt to it.", 4000);
      document.querySelector<HTMLInputElement>(".bl-phone input")?.focus();
      return;
    }
    setBusy(price.key);
    try {
      const r = await checkout(price, { phone: phone().trim() });
      if (r === "completed") {
        showToast("Payment received. Thank you! Your plan is active.", 4000);
        props.onClose();
      } else if (r === "pending") {
        showToast("Payment received. Your plan switches on in a minute or two.", 5000);
        props.onClose();
      }
    } catch (e) {
      showToast(explain(e), 6000);
    } finally {
      setBusy(null);
    }
  };

  const copyDesktopLink = async () => {
    const url = desktopUpgradeUrl(props.plan ?? "pro");
    try {
      await navigator.clipboard.writeText(url);
      showToast("Link copied. Open it in your browser to upgrade.", 4000);
    } catch {
      showToast(url, 8000);
    }
  };

  const payLabel = (p: Price | undefined) => {
    if (!p) return "Choose";
    const m = money(p.amount, p.currency);
    return p.provider === "sslcommerz" ? `Pay ${m}` : `Continue: ${m}`;
  };

  return (
    <LibDialog icon="cloud" title={credits ? "Rupok AI credits" : "Upgrade Rupok Cloud"} sub={props.reason && !credits ? props.reason : "The board stays free. Pick what you need in the cloud."} width={880} onClose={props.onClose}>
      <div class="bl-up">
        <Show when={!cloudUser()}>
          <div class="bl-signin">
            <p>Sign in first, so your plan goes to your account.</p>
            <CloudSignIn compact />
          </div>
        </Show>
        <div class="bl-controls">
          <div class="bl-seg" role="radiogroup" aria-label="Currency and way to pay">
            <button role="radio" aria-checked={cur() === "BD"} onClick={() => switchRegion("BD")}>৳ Taka <small>bKash · Nagad · cards</small></button>
            <button role="radio" aria-checked={cur() === "INTL"} onClick={() => switchRegion("INTL")}>$ US dollars <small>cards · PayPal</small></button>
          </div>
          <Show when={!credits}>
            <Show
              when={cur() === "BD"}
              fallback={
                <div class="bl-seg" role="radiogroup" aria-label="Billing period">
                  <button role="radio" aria-checked={period() === "month"} onClick={() => setPeriod("month")}>Monthly</button>
                  <button role="radio" aria-checked={period() === "year"} onClick={() => setPeriod("year")}>Yearly <span class="bl-save">save 29%</span></button>
                </div>
              }
            >
              <div class="bl-seg" role="radiogroup" aria-label="How long">
                <For each={[1, 6, 12]}>{(m) => <button role="radio" aria-checked={months() === m} onClick={() => setMonths(m)}>{m === 1 ? "1 month" : `${m} months`}{m === 12 ? <span class="bl-save">2 free</span> : null}</button>}</For>
              </div>
            </Show>
          </Show>
        </div>

        <Show when={data() && !data()!.billing_enabled}>
          <div class="bl-notice" role="status">Payments open soon. Nothing can be bought yet, and your free plan keeps working.</div>
        </Show>
        <Show when={isDesktop}>
          <div class="bl-notice" role="status">
            Upgrading happens on rupok.app in your browser; your plan then works here too.
            <button class="btn" onClick={() => void copyDesktopLink()}>Copy the link</button>
          </div>
        </Show>
        <Show when={plans.error || first.error}>
          <div class="bl-notice warn" role="status">Rupok Cloud can't be reached right now. Try again in a minute.</div>
        </Show>

        <Show when={!credits} fallback={<Topups data={data()} busy={busy()} buy={buy} />}>
          <div class="bl-cards">
            <article class="bl-card feat" classList={{ hl: !props.plan || props.plan === "pro" }}>
              <h3>Pro</h3>
              <p class="bl-amt">
                <Show when={proPrice()} fallback="…">{(p) => <>{money(p().amount, p().currency)}<small>{p().months ? (p().months === 1 ? " / month" : ` / ${p().months} months`) : p().interval === "year" ? " / year" : " / month"}</small></>}</Show>
              </p>
              <p class="bl-per">{perMonth()}</p>
              <ul><For each={PRO_POINTS}>{(x) => <li>{x}</li>}</For></ul>
              <button class="btn solid" disabled={!proPrice()?.available || !cloudUser() || isDesktop || busy() !== null} onClick={() => void buy(proPrice())}>
                {busy() === proPrice()?.key ? "Opening…" : payLabel(proPrice())}
              </button>
              <Show when={billing()?.trial.available}>
                <button class="btn ghost" onClick={async () => {
                  try {
                    await billingApi.startTrial();
                    await refreshBilling();
                    void refreshUsage();
                    showToast("Pro is on for 14 days. No card, nothing to cancel.", 4000);
                    props.onClose();
                  } catch (e) {
                    showToast(explain(e), 5000);
                  }
                }}>Or try it free for 14 days</button>
              </Show>
            </article>
            <Show when={cur() === "BD"}>
              <article class="bl-card" classList={{ hl: props.plan === "centre" }}>
                <h3>Coaching centre</h3>
                <p class="bl-amt"><Show when={centrePrice()} fallback="…">{(p) => <>{money(p().amount, p().currency)}<small>{p().months === 1 ? " / month" : " / year"}</small></>}</Show></p>
                <p class="bl-per">5 teachers{months() === 6 ? " · sold monthly or yearly" : ""}</p>
                <ul><For each={CENTRE_POINTS}>{(x) => <li>{x}</li>}</For></ul>
                <button class="btn solid" disabled={!centrePrice()?.available || !cloudUser() || busy() !== null} onClick={() => void buy(centrePrice())}>
                  {busy() === centrePrice()?.key ? "Opening…" : payLabel(centrePrice())}
                </button>
              </article>
            </Show>
            <article class="bl-card">
              <h3>School</h3>
              <p class="bl-amt">{cur() === "BD" ? "৳1,999" : "$36"}<small> / teacher / year</small></p>
              <p class="bl-per">Invoice and bank transfer; classroom board licences too.</p>
              <ul><li>Pro for every teacher</li><li>50 GB per teacher</li><li>Admin dashboard (soon)</li><li>A shared course library (soon)</li></ul>
              <a class="btn" href="https://rupok.app/pricing/" target="_blank" rel="noopener">Talk to us</a>
            </article>
          </div>
          <Show when={cur() === "INTL"}>
            <p class="bl-muted">Paid through Paddle, our reseller: cards, PayPal, Apple Pay or Google Pay. Tax is added where it applies. Cancel any time.</p>
          </Show>
        </Show>
        <Show when={cur() === "BD"}>
          <label class="bl-phone">
            <span>Mobile number for the payment receipt</span>
            <input class="input" type="tel" inputmode="tel" autocomplete="tel" placeholder="01XXXXXXXXX" value={phone()} onInput={(e) => setPhone(e.currentTarget.value)} />
            <small>Sent to SSLCommerz for this payment only; Rupok doesn't keep it. Plans are prepaid: nothing renews by itself.</small>
          </label>
        </Show>
      </div>
    </LibDialog>
  );
}

function Topups(props: { data: Plans | undefined; busy: string | null; buy: (p: Price) => void }) {
  return (
    <div class="bl-topups">
      <p class="bl-muted">Credits run Solve this, Design with AI and the chat without your own AI subscription. A chat reply uses about 1 credit, a full solution about 4, a new Smart Object about 10. Bought credits never expire.</p>
      <div class="bl-topup-row">
        <For each={props.data?.topups ?? []}>
          {(p) => (
            <button class="bl-topup" disabled={!p.available || !cloudUser() || isDesktop || props.busy !== null} onClick={() => props.buy(p)}>
              <b>{p.credits?.toLocaleString()} credits</b>
              <span>{money(p.amount, p.currency)}</span>
            </button>
          )}
        </For>
      </div>
    </div>
  );
}
