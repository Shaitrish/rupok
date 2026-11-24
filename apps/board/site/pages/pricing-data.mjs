// Site configuration for plans and prices: the single source the pricing
// pages (/pricing/, /bn/pricing/), the home page's plan cards and the
// Markdown twins are built from.
//
// The live price list the app charges lives in Rupok Cloud's catalog
// (private repo, src/billing/catalog.ts); keep the numbers here in step.
//
// Launch switches. Flip them, run `node site/build.mjs`, deploy:
//   cloudLive   - Rupok Cloud (sync, student links, storage) is open to everyone.
//   billingLive - paid plans can be bought. Until then every paid plan says
//                 "Coming soon" and no page claims you can pay.
export const PRICING_CONFIG = {
  cloudLive: false,
  billingLive: false,
  trialDays: 14,
  /** Features still being built: marked "soon" wherever they are listed. */
  soon: { hostedAi: true, liveClasses: true, analytics: true, branding: true, admin: true },
};

/** Prices: BDT through SSLCommerz (prepaid, VAT included), USD through Paddle (tax added at checkout). */
export const PRICES = {
  pro: { BD: { month: 299, sixMonths: 1599, year: 2999 }, INTL: { month: 7, year: 60 } },
  centre: { BD: { month: 1199, year: 11999, teachers: 5 } },
  school: { BD: { seatYear: 1999, minSeats: 10 }, INTL: { seatYear: 36, minSeats: 5 } },
  board: { BD: { year: 2500 }, INTL: { year: 29 } },
  credits: { BD: [[250, 599], [1000, 2199]], INTL: [[250, 5], [1000, 18]] },
};

/** Plan limits (Rupok Cloud migrations/0003_billing.sql). */
export const LIMITS = {
  free: { storage: "500 MB", file: "50 MB", links: 10, history: 30, credits: 25 },
  pro: { storage: "20 GB", file: "200 MB", credits: 150, liveMin: 3000, liveStudents: 100 },
  centre: { storage: "30 GB", credits: 150, liveMin: 6000, liveStudents: 300 },
  school: { storage: "50 GB", credits: 150, liveMin: 6000, liveStudents: 300 },
};

/** Format an amount for a language: Bangla digits on Bangla pages. */
export function money(amount, currency, lang = "en") {
  const n = Number(amount).toLocaleString(lang === "bn" ? "bn-BD" : "en-US", { maximumFractionDigits: 2 });
  return currency === "BDT" ? `৳${n}` : `$${n}`;
}
export const num = (n, lang = "en") => Number(n).toLocaleString(lang === "bn" ? "bn-BD" : "en-US");
