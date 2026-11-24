// The pricing page's switches (site/pages/pricing.mjs): taka or dollars,
// monthly or yearly. The region is the visitor's saved choice, else the
// Rupok Cloud API's guess from Cloudflare's IP country (once billing is
// live), else Bangladesh's time zone. Static HTML already shows a sensible
// default, so this is progressive enhancement only.

const KEY_REGION = "rupok.pricing-region";
const KEY_PERIOD = "rupok.pricing-period";

type Region = "BD" | "INTL";
type Period = "month" | "year";

const read = (k: string): string | null => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const write = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private mode: the choice just isn't remembered */
  }
};

export function initPricing(root: HTMLElement): void {
  const set = (attr: "region" | "period", v: string) => {
    root.dataset[attr] = v;
    root.querySelectorAll<HTMLButtonElement>(`[data-set-${attr}]`).forEach((b) => b.setAttribute("aria-checked", String(b.dataset[attr === "region" ? "setRegion" : "setPeriod"] === v)));
  };
  root.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-set-region], button[data-set-period]");
    if (!b) return;
    if (b.dataset.setRegion) {
      set("region", b.dataset.setRegion);
      write(KEY_REGION, b.dataset.setRegion);
    } else if (b.dataset.setPeriod) {
      set("period", b.dataset.setPeriod);
      write(KEY_PERIOD, b.dataset.setPeriod);
    }
  });
  // Arrow keys move within a radio group.
  root.addEventListener("keydown", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>(".seg button");
    if (!b || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
    const all = [...b.parentElement!.querySelectorAll<HTMLButtonElement>("button")];
    const next = all[(all.indexOf(b) + (e.key === "ArrowRight" ? 1 : all.length - 1)) % all.length];
    next.focus();
    next.click();
    e.preventDefault();
  });

  const period = read(KEY_PERIOD);
  if (period === "month" || period === "year") set("period", period as Period);
  const saved = read(KEY_REGION);
  if (saved === "BD" || saved === "INTL") return set("region", saved as Region);
  let tz = "";
  try {
    tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    /* ignore */
  }
  if (tz === "Asia/Dhaka") set("region", "BD");
  if (root.dataset.live === "true") {
    const ctl = new AbortController();
    setTimeout(() => ctl.abort(), 2500);
    fetch("/api/v1/billing/plans", { signal: ctl.signal, credentials: "same-origin" })
      .then((r) => (r.ok ? r.json() : null))
      .then((j: { region?: string } | null) => {
        if (j && (j.region === "BD" || j.region === "INTL") && !read(KEY_REGION)) set("region", j.region);
      })
      .catch(() => {});
  }
}

const root = document.querySelector<HTMLElement>("[data-pricing]");
if (root) initPricing(root);
