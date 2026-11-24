// Rupok Cloud plans and payments, for the account screen and the Upgrade
// dialog. The API is /api/v1/billing (Rupok Cloud's openapi.yaml):
//
//   Bangladesh (BDT): SSLCommerz. We ask for a session and send the browser
//     to the gateway (bKash, Nagad, Rocket, cards, banking); it comes back to
//     /board/?billing=success|failed|cancelled|pending|review.
//   Elsewhere (USD): Paddle. We load Paddle.js and open its overlay checkout;
//     Paddle tells the server through a webhook, and we poll until the plan
//     changes.
//
// Nothing here decides what a teacher may do: the server enforces every
// limit. This file only shows plans and starts checkouts.

import { createSignal } from "solid-js";
import { isDesktop } from "../platform";
import { CLOUD_ORIGIN, cloud, cloudUser, refreshUsage } from "./account";
import { CloudError } from "./api";

export type Region = "BD" | "INTL";

export interface Price {
  key: string;
  provider: "paddle" | "sslcommerz" | "invoice";
  kind: "subscription" | "prepaid" | "topup" | "seat" | "contact";
  plan_id: string | null;
  currency: "USD" | "BDT";
  amount: string;
  interval: "month" | "year" | null;
  months: number | null;
  credits: number | null;
  seats: number | null;
  min_quantity: number | null;
  per_month: string | null;
  label: string;
  label_bn: string | null;
  available: boolean;
}
export interface PlanInfo {
  id: string;
  name: string;
  storage_bytes: number;
  max_file_bytes: number;
  active_links: number;
  ai_credits_monthly: number;
  live_minutes_monthly: number;
  live_max_students: number;
  history_days: number;
  features: string[];
  prices: Price[];
}
export interface Plans {
  region: Region;
  country: string | null;
  currency: "BDT" | "USD";
  provider: "sslcommerz" | "paddle";
  billing_enabled: boolean;
  trial_days: number;
  paddle: { environment: "sandbox" | "production"; client_token: string | null };
  plans: PlanInfo[];
  topups: Price[];
}
export interface Credits {
  month: string;
  monthly_allowance: number;
  monthly_remaining: number;
  balance: number;
  available: number;
}
export interface BillingSummary {
  plan: { id: string; name: string; until: number | null; source: string | null };
  limits: { storage_bytes: number; max_file_bytes: number; active_links: number; ai_credits_monthly: number; live_minutes_monthly: number; live_max_students: number; history_days: number };
  features: string[];
  usage: { storage_bytes: number; active_links: number; live_minutes_this_month: number };
  credits: Credits;
  region: Region;
  saved_region: Region | null;
  subscription: { id: string; plan_id: string; price_key: string; status: string; quantity: number; period_end: number | null; cancel_at: number | null } | null;
  can_manage_subscription: boolean;
  prepaid: { until: number; renew_soon: boolean; source: string } | null;
  trial: { days: number; available: boolean; used_at: number | null };
  orgs: { id: string; name: string; plan_id: string; seats: number; role: string; members: number }[];
  notices: { id: string; kind: string; created_at: number }[];
  billing_enabled: boolean;
}
export interface Receipt {
  id: string;
  provider: "paddle" | "sslcommerz" | "invoice";
  receipt_number: string | null;
  description: string;
  amount: string;
  currency: string;
  status: string;
  created_at: number;
  paid_at: number | null;
  method: string | null;
  merchant_of_record: string;
  invoice_available: boolean;
}
interface PaddleCheckout {
  environment: "sandbox" | "production";
  client_token: string;
  items: { priceId: string; quantity: number }[];
  customer: { email?: string; id?: string };
  customData: Record<string, string>;
  settings: Record<string, unknown>;
}

// ------------------------------------------------------------------ API
export const billingApi = {
  plans: (region?: Region) => cloud.json<Plans>("GET", `/billing/plans${region ? `?region=${region}` : ""}`),
  me: () => cloud.json<BillingSummary>("GET", "/billing/me"),
  setRegion: (region: Region | null) => cloud.json<{ region: Region | null }>("PUT", "/billing/region", { region }),
  startTrial: () => cloud.json<{ plan: string; until: number }>("POST", "/billing/trial", {}),
  receipts: async () => (await cloud.json<{ receipts: Receipt[] }>("GET", "/billing/receipts")).receipts,
  paddleCheckout: (price_key: string, quantity?: number) => cloud.json<PaddleCheckout>("POST", "/billing/paddle/checkout", { price_key, quantity }),
  portal: async () => (await cloud.json<{ url: string }>("POST", "/billing/paddle/portal", {})).url,
  sslInit: (price_key: string, phone: string) => cloud.json<{ tran_id: string; gateway_url: string }>("POST", "/billing/sslcommerz/init", { price_key, phone }),
  receiptUrl: (r: Receipt) => cloud.url(`/billing/receipts/${encodeURIComponent(r.id)}/${r.provider === "paddle" ? "invoice" : "print"}`),
  seenNotice: (id: string) => cloud.json("POST", `/billing/notices/${encodeURIComponent(id)}/seen`, {}),
};

// ---------------------------------------------------------------- state
export const [billing, setBilling] = createSignal<BillingSummary | null>(null);

/** Refresh the plan and usage. Quietly keeps the old numbers when offline or before billing exists on the server. */
export async function refreshBilling(): Promise<BillingSummary | null> {
  if (!cloudUser()) {
    setBilling(null);
    return null;
  }
  try {
    const b = await billingApi.me();
    setBilling(b);
    return b;
  } catch {
    return billing();
  }
}

/** The Upgrade dialog: null (closed) or the plan to highlight. */
export const [upgradeRequest, setUpgradeRequest] = createSignal<{ plan?: string; reason?: string } | null>(null);
export const openUpgrade = (plan?: string, reason?: string) => setUpgradeRequest({ plan, reason });

/** A friendly soft gate for a 402 from the server ("Pro feature, try free for 14 days"). Returns true if handled. */
export function softGate(e: unknown): boolean {
  if (!(e instanceof CloudError) || (e.code !== "plan_limit" && e.code !== "insufficient_credits")) return false;
  openUpgrade("pro", e.message);
  return true;
}

// ------------------------------------------------------------- checkout
const PADDLE_JS = "https://cdn.paddle.com/paddle/v2/paddle.js";
type PaddleGlobal = {
  Environment: { set(env: string): void };
  Initialize(o: { token: string; eventCallback?: (ev: { name?: string }) => void }): void;
  Checkout: { open(o: Record<string, unknown>): void; close(): void };
};
let paddleLoad: Promise<PaddleGlobal> | null = null;
let paddleEvents: ((ev: { name?: string }) => void) | null = null;

function loadPaddle(env: string, token: string): Promise<PaddleGlobal> {
  paddleLoad ??= new Promise<PaddleGlobal>((resolve, reject) => {
    const s = document.createElement("script");
    s.src = PADDLE_JS;
    s.async = true;
    s.onload = () => {
      const P = (window as unknown as { Paddle?: PaddleGlobal }).Paddle;
      if (!P) return reject(new Error("Paddle didn't load"));
      if (env === "sandbox") P.Environment.set("sandbox");
      P.Initialize({ token, eventCallback: (ev) => paddleEvents?.(ev) });
      resolve(P);
    };
    s.onerror = () => {
      paddleLoad = null;
      reject(new Error("Couldn't load the payment page. Check your connection and try again."));
    };
    document.head.appendChild(s);
  });
  return paddleLoad;
}

/** Wait for the server to see the new plan (webhooks can take a few seconds). */
async function waitForPlan(previous: string | undefined, credits: number | undefined, tries = 20): Promise<BillingSummary | null> {
  for (let i = 0; i < tries; i++) {
    await new Promise((r) => setTimeout(r, i < 5 ? 1500 : 3000));
    const b = await refreshBilling();
    if (b && (b.plan.id !== previous || (credits !== undefined && b.credits.available > credits))) return b;
  }
  return null;
}

export type CheckoutResult = "redirected" | "completed" | "pending" | "closed";

/** The desktop app has no payment pages of its own: the upgrade opens on rupok.app in the browser. */
export const desktopUpgradeUrl = (plan = "pro") => `${CLOUD_ORIGIN}/board/?upgrade=${encodeURIComponent(plan)}`;

/** Start a checkout for a catalog price. Bangladesh redirects away; Paddle resolves when the overlay finishes. */
export async function checkout(price: Price, opts: { phone?: string; quantity?: number } = {}): Promise<CheckoutResult> {
  if (price.provider === "sslcommerz") {
    const r = await billingApi.sslInit(price.key, opts.phone ?? "");
    location.href = r.gateway_url;
    return "redirected";
  }
  if (price.provider !== "paddle") throw new Error("This plan is sold by invoice: contact us.");
  if (isDesktop) throw new Error("Open rupok.app in your browser to upgrade.");
  const before = billing();
  const cfg = await billingApi.paddleCheckout(price.key, opts.quantity);
  const P = await loadPaddle(cfg.environment, cfg.client_token);
  return new Promise<CheckoutResult>((resolve) => {
    let done = false;
    paddleEvents = (ev) => {
      if (ev.name === "checkout.completed" && !done) {
        done = true;
        void waitForPlan(before?.plan.id, price.kind === "topup" ? before?.credits.available : undefined).then((b) => {
          void refreshUsage();
          resolve(b ? "completed" : "pending");
        });
      } else if (ev.name === "checkout.closed" && !done) {
        done = true;
        resolve("closed");
      }
    };
    P.Checkout.open({ items: cfg.items, customer: cfg.customer, customData: cfg.customData, settings: cfg.settings });
  });
}

/** Paddle's customer portal (change plan or card, cancel, invoices) in a new tab. */
export async function manageSubscription(): Promise<void> {
  const url = await billingApi.portal();
  window.open(url, "_blank", "noopener");
}

// ------------------------------------------------------------ formatting
export function money(amount: string | number, currency: string): string {
  const n = Number(amount);
  const s = n.toLocaleString("en-US", { minimumFractionDigits: Number.isInteger(n) ? 0 : 2, maximumFractionDigits: 2 });
  return currency === "BDT" ? `৳${s}` : `$${s}`;
}
export const day = (ms: number) => new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

/**
 * Read ?billing= (back from the gateway) and ?upgrade= (from the pricing
 * page) once at start-up. Returns a message to show, if any.
 */
export function takeBillingParams(): { message?: string; upgrade?: string } {
  const q = new URLSearchParams(location.search);
  const b = q.get("billing");
  const up = q.get("upgrade");
  if (!b && !up) return {};
  q.delete("billing");
  q.delete("payment");
  q.delete("upgrade");
  const rest = q.toString();
  history.replaceState(history.state, "", location.pathname + (rest ? `?${rest}` : "") + location.hash);
  const messages: Record<string, string> = {
    success: "Payment received. Thank you! Your plan is active.",
    pending: "Payment received. Your plan switches on in a minute or two.",
    review: "Your payment is being checked. Your plan switches on as soon as it clears.",
    failed: "The payment didn't go through, and you weren't charged. Try again or pick another way to pay.",
    cancelled: "Payment cancelled. Nothing was charged.",
  };
  return { message: b ? messages[b] : undefined, upgrade: up ?? undefined };
}
