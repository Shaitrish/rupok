// The pricing pages: /pricing/ and /bn/pricing/. Plan cards with a
// monthly/yearly toggle and a taka/dollar switch (src/pricing.ts picks the
// visitor's region and remembers their choice), a feature comparison, ways
// to pay, an FAQ and JSON-LD Offers. Numbers and launch switches come from
// pricing-data.mjs; keep claims honest: anything not live says "soon".

import { SITE } from "../lib.mjs";
import { LIMITS, PRICES, PRICING_CONFIG as CFG, money, num } from "./pricing-data.mjs";

const live = CFG.billingLive;

const T = {
  en: {
    taka: "৳ Taka",
    takaSub: "bKash · Nagad · cards",
    usd: "$ US dollars",
    usdSub: "cards · PayPal",
    regionLabel: "Currency and way to pay",
    periodLabel: "Billing period",
    monthly: "Monthly",
    yearly: "Yearly",
    save: "save up to 29%",
    noteBD: "Taka prices include VAT. Pay with bKash, Nagad, Rocket, a card or internet banking through SSLCommerz. Plans are prepaid: nothing renews by itself, and we remind you before your time runs out.",
    noteINTL: "Prices in US dollars. Pay by card, PayPal, Apple Pay or Google Pay through Paddle, our reseller; tax is added at checkout where it applies. Cancel any time.",
    perMonth: "/month",
    perYear: "/year",
    perSeat: "/teacher/year",
    perBoard: "/board/year",
    soon: "soon",
    cloudSoon: "Rupok Cloud, opening soon:",
    freeBadge: "Free forever",
    centreBadge: "Bangladesh",
    schoolBadge: "Invoice",
    comingSoon: "Coming soon",
    popular: "Most popular",
    free: { name: "Free", desc: "For every teacher and student, forever.", cta: "Start free" },
    pro: { name: "Pro", desc: "For a teacher who shares lessons every week.", cta: "Get Pro" },
    centre: { name: "Coaching centre", desc: "For a coaching or online-class centre with up to 5 teachers.", cta: "Get Coaching centre" },
    school: { name: "School", desc: "For schools and colleges, with invoices and classroom boards.", cta: "Talk to us" },
    startFree: "Start free now",
    opensSoon: "Opens soon. Start free today: your lessons carry over.",
    trial: (d) => `Try it free for ${d} days, no card needed.`,
    proSubBDm: (six) => `or ${six} for 6 months`,
    proSubBDy: (m) => `${m} a month: 2 months free`,
    proSubINTLm: "billed monthly",
    proSubINTLy: (m) => `${m} a month, billed yearly`,
    centreSubM: "for 5 teachers",
    centreSubY: (m) => `${m} a month for 5 teachers`,
    schoolSub: (n, invoice) => `from ${n} teachers, billed yearly${invoice ? " by invoice" : ""}`,
    items: {
      free: [
        ["The whole app, every feature, offline"],
        ["Your own AI: Claude Code, Codex, OpenCode"],
        [`${LIMITS.free.storage} Rupok Cloud storage`, "cloud"],
        [`${LIMITS.free.links} active student links`, "cloud"],
        [`${LIMITS.free.history} days of version history`, "cloud"],
        [`${LIMITS.free.credits} Rupok AI credits to try`, "hostedAi"],
      ],
      pro: [
        ["Everything in Free"],
        [`${LIMITS.pro.storage} cloud storage`, "cloud"],
        ["Unlimited student links", "cloud"],
        ["Full version history", "cloud"],
        [`${LIMITS.pro.credits} Rupok AI credits a month`, "hostedAi"],
        [`Live classes for ${LIMITS.pro.liveStudents} students`, "liveClasses"],
        ["Student view analytics", "analytics"],
        ["Priority support"],
      ],
      centre: [
        ["Pro for 5 teachers"],
        [`${LIMITS.centre.storage} per teacher`, "cloud"],
        ["Admin view and a shared library", "admin"],
        [`Live classes for ${LIMITS.centre.liveStudents} students`, "liveClasses"],
        ["Attendance and quizzes", "admin"],
      ],
      school: [
        ["Pro for every teacher"],
        [`${LIMITS.school.storage} per teacher`, "cloud"],
        ["Admin dashboard", "admin"],
        ["A shared course library", "admin"],
        ["Invoices and bank transfer"],
        ["Classroom board licence", null, "board"],
      ],
    },
    boardItem: (bd, intl) => `Classroom board licence: <span data-r="BD">${bd}</span><span data-r="INTL">${intl}</span> per board a year`,
    creditsH: "Need more AI?",
    creditsP: "Rupok AI credits run Solve this, Design with AI and the chat for teachers without their own AI subscription. Top-ups never expire.",
    creditsItem: (n, p) => `${num(n)} credits for ${p}`,
  },
  bn: {
    taka: "৳ টাকা",
    takaSub: "বিকাশ · নগদ · কার্ড",
    usd: "$ ডলার",
    usdSub: "কার্ড · PayPal",
    regionLabel: "মুদ্রা ও পেমেন্টের উপায়",
    periodLabel: "কত দিনের জন্য",
    monthly: "মাসিক",
    yearly: "বাৎসরিক",
    save: "২৯% পর্যন্ত সাশ্রয়",
    noteBD: "টাকার দামে ভ্যাট ধরা আছে। SSLCommerz-এর মাধ্যমে বিকাশ, নগদ, রকেট, কার্ড বা ইন্টারনেট ব্যাংকিংয়ে দিন। প্ল্যানগুলো প্রিপেইড: নিজে থেকে টাকা কাটবে না, মেয়াদ শেষ হওয়ার আগে আমরা মনে করিয়ে দেব।",
    noteINTL: "দাম মার্কিন ডলারে। আমাদের রিসেলার Paddle-এর মাধ্যমে কার্ড, PayPal, Apple Pay বা Google Pay দিয়ে দিন; প্রযোজ্য হলে চেকআউটে কর যোগ হবে। যখন খুশি বাতিল করা যায়।",
    perMonth: "/মাস",
    perYear: "/বছর",
    perSeat: "/শিক্ষক/বছর",
    perBoard: "/বোর্ড/বছর",
    soon: "শীঘ্রই",
    cloudSoon: "Rupok Cloud, শীঘ্রই চালু হচ্ছে:",
    freeBadge: "চিরকাল ফ্রি",
    centreBadge: "বাংলাদেশ",
    schoolBadge: "ইনভয়েস",
    comingSoon: "শীঘ্রই আসছে",
    popular: "সবচেয়ে জনপ্রিয়",
    free: { name: "ফ্রি", desc: "সব শিক্ষক আর শিক্ষার্থীর জন্য, চিরকাল।", cta: "ফ্রি শুরু করুন" },
    pro: { name: "Pro", desc: "যে শিক্ষক প্রতি সপ্তাহে লেসন শেয়ার করেন।", cta: "Pro নিন" },
    centre: { name: "কোচিং সেন্টার", desc: "৫ জন পর্যন্ত শিক্ষকের কোচিং বা অনলাইন ক্লাস সেন্টারের জন্য।", cta: "কোচিং সেন্টার প্ল্যান নিন" },
    school: { name: "স্কুল", desc: "স্কুল-কলেজের জন্য, ইনভয়েস আর ক্লাসরুম বোর্ডসহ।", cta: "যোগাযোগ করুন" },
    startFree: "এখনই ফ্রি শুরু করুন",
    opensSoon: "শীঘ্রই চালু হবে। আজই ফ্রি শুরু করুন; আপনার লেসনগুলো থেকে যাবে।",
    trial: (d) => `${num(d, "bn")} দিন ফ্রি চালিয়ে দেখুন, কার্ড লাগবে না।`,
    proSubBDm: (six) => `অথবা ৬ মাস ${six}`,
    proSubBDy: (m) => `মাসে ${m}: ২ মাস ফ্রি`,
    proSubINTLm: "প্রতি মাসে বিল",
    proSubINTLy: (m) => `মাসে ${m}, বছরে একবার বিল`,
    centreSubM: "৫ জন শিক্ষকের জন্য",
    centreSubY: (m) => `৫ জন শিক্ষকের জন্য মাসে ${m}`,
    schoolSub: (n, invoice) => `অন্তত ${num(n, "bn")} জন শিক্ষক, বছরে একবার${invoice ? " ইনভয়েসে" : " বিল"}`,
    items: {
      free: [
        ["পুরো অ্যাপ, সব ফিচার, ইন্টারনেট ছাড়াও"],
        ["নিজের AI: Claude Code, Codex, OpenCode"],
        ["৫০০ MB Rupok Cloud স্টোরেজ", "cloud"],
        [`${num(LIMITS.free.links, "bn")}টি সক্রিয় শিক্ষার্থী-লিংক`, "cloud"],
        [`${num(LIMITS.free.history, "bn")} দিনের ভার্সন হিস্ট্রি`, "cloud"],
        [`চালিয়ে দেখার জন্য ${num(LIMITS.free.credits, "bn")} Rupok AI ক্রেডিট`, "hostedAi"],
      ],
      pro: [
        ["ফ্রি প্ল্যানের সবকিছু"],
        ["২০ GB ক্লাউড স্টোরেজ", "cloud"],
        ["যত খুশি শিক্ষার্থী-লিংক", "cloud"],
        ["পুরো ভার্সন হিস্ট্রি", "cloud"],
        [`মাসে ${num(LIMITS.pro.credits, "bn")} Rupok AI ক্রেডিট`, "hostedAi"],
        [`${num(LIMITS.pro.liveStudents, "bn")} জন শিক্ষার্থী নিয়ে লাইভ ক্লাস`, "liveClasses"],
        ["শিক্ষার্থীরা কে কতটা দেখল তার হিসাব", "analytics"],
        ["আগে সাপোর্ট"],
      ],
      centre: [
        ["৫ জন শিক্ষকের জন্য Pro"],
        ["প্রতি শিক্ষক ৩০ GB", "cloud"],
        ["অ্যাডমিন আর সবার জন্য একটি লাইব্রেরি", "admin"],
        [`${num(LIMITS.centre.liveStudents, "bn")} জন শিক্ষার্থী নিয়ে লাইভ ক্লাস`, "liveClasses"],
        ["উপস্থিতি আর কুইজ", "admin"],
      ],
      school: [
        ["প্রত্যেক শিক্ষকের জন্য Pro"],
        ["প্রতি শিক্ষক ৫০ GB", "cloud"],
        ["অ্যাডমিন ড্যাশবোর্ড", "admin"],
        ["স্কুলের নিজস্ব কোর্স লাইব্রেরি", "admin"],
        ["ইনভয়েস আর ব্যাংক ট্রান্সফার"],
        ["ক্লাসরুম বোর্ড লাইসেন্স", null, "board"],
      ],
    },
    boardItem: (bd, intl) => `ক্লাসরুম বোর্ড লাইসেন্স: প্রতি বোর্ড বছরে <span data-r="BD">${bd}</span><span data-r="INTL">${intl}</span>`,
    creditsH: "আরও AI দরকার?",
    creditsP: "যাদের নিজের AI সাবস্ক্রিপশন নেই, Rupok AI ক্রেডিট দিয়ে তারা Solve this, Design with AI আর চ্যাট চালাতে পারেন। কেনা ক্রেডিটের মেয়াদ শেষ হয় না।",
    creditsItem: (n, p) => `${num(n, "bn")} ক্রেডিট ${p}`,
  },
};

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

// "Soon" chips mark features still being built. Cloud basics (storage,
// links, history) open with Rupok Cloud itself: paid cards already say
// "Coming soon", and the free card gets one "opens soon" sub-heading.
function itemsHtml(t, list, lang, { free = false } = {}) {
  let headed = false;
  return list
    .map(([text, flag, special]) => {
      if (special === "board") return `<li>${t.boardItem(money(PRICES.board.BD.year, "BDT", lang), money(PRICES.board.INTL.year, "USD", lang))}</li>`;
      let head = "";
      if (free && flag && !CFG.cloudLive && !headed) {
        headed = true;
        head = `<li class="psub">${esc(t.cloudSoon)}</li>`;
      }
      const soon = flag && flag !== "cloud" && !!CFG.soon[flag] && !(free && !CFG.cloudLive);
      return `${head}<li>${esc(text)}${soon ? ` <em>${esc(t.soon)}</em>` : ""}</li>`;
    })
    .join("");
}

const amt = (r, p, value, per) => `<span data-r="${r}"${p ? ` data-p="${p}"` : ""}>${value}<small>${per}</small></span>`;
const sub = (r, p, text) => `<span data-r="${r}"${p ? ` data-p="${p}"` : ""}>${esc(text)}</span>`;

function cta(t, plan, href) {
  if (plan === "free") return `<a class="pill dark" href="/board/">${esc(t.free.cta)}</a>`;
  if (plan === "school") return `<a class="pill ghost" href="${href}" rel="noopener">${esc(t.school.cta)}</a>`;
  if (live) return `<a class="pill ${plan === "pro" ? "mint" : "dark"}" href="/board/?upgrade=${plan}">${esc(t[plan].cta)}</a><p class="pfine">${esc(t.trial(CFG.trialDays))}</p>`;
  return `<a class="pill ghost" href="/board/">${esc(t.startFree)}</a><p class="pfine">${esc(t.opensSoon)}</p>`;
}

export function pricingHtml(lang, region) {
  const t = T[lang];
  const m = (a, c) => money(a, c, lang);
  const P = PRICES;
  // Every card has a badge row, so names and prices line up across cards.
  const badge = (paidLabel, liveLabel) => `<span class="pbadge${live ? "" : " soon"}">${esc(live ? liveLabel : paidLabel)}</span>`;
  const seg = (attr, opts, cur, label) =>
    `<div class="seg" role="radiogroup" aria-label="${esc(label)}">${opts
      .map(([v, l, s]) => `<button type="button" role="radio" aria-checked="${v === cur}" data-set-${attr}="${v}">${l}${s ? ` <small>${s}</small>` : ""}</button>`)
      .join("")}</div>`;
  return `<div class="pricing" data-pricing data-region="${region}" data-period="year" data-live="${live}">
        <div class="pr-controls">
          ${seg("region", [["BD", esc(t.taka), esc(t.takaSub)], ["INTL", esc(t.usd), esc(t.usdSub)]], region, t.regionLabel)}
          ${seg("period", [["month", esc(t.monthly)], ["year", esc(t.yearly), `<span class="save">${esc(t.save)}</span>`]], "year", t.periodLabel)}
        </div>
        <p class="pr-note" data-r="BD">${esc(t.noteBD)}</p>
        <p class="pr-note" data-r="INTL">${esc(t.noteINTL)}</p>
        <div class="pcards">
          <article class="pcard"><span class="pbadge free">${esc(t.freeBadge)}</span>
            <h3>${esc(t.free.name)}</h3>
            <p class="amt">${amt("BD", null, m(0, "BDT"), "")}${amt("INTL", null, m(0, "USD"), "")}</p>
            <p class="amt-sub">&nbsp;</p>
            <p class="pdesc">${esc(t.free.desc)}</p>
            <ul>${itemsHtml(t, t.items.free, lang, { free: true })}</ul>
            ${cta(t, "free")}
          </article>
          <article class="pcard feat">${badge(t.comingSoon, t.popular)}
            <h3>${esc(t.pro.name)}</h3>
            <p class="amt">${amt("BD", "month", m(P.pro.BD.month, "BDT"), t.perMonth)}${amt("BD", "year", m(P.pro.BD.year, "BDT"), t.perYear)}${amt("INTL", "month", m(P.pro.INTL.month, "USD"), t.perMonth)}${amt("INTL", "year", m(P.pro.INTL.year, "USD"), t.perYear)}</p>
            <p class="amt-sub">${sub("BD", "month", t.proSubBDm(m(P.pro.BD.sixMonths, "BDT")))}${sub("BD", "year", t.proSubBDy(m(Math.round(P.pro.BD.year / 12), "BDT")))}${sub("INTL", "month", t.proSubINTLm)}${sub("INTL", "year", t.proSubINTLy(m(P.pro.INTL.year / 12, "USD")))}</p>
            <p class="pdesc">${esc(t.pro.desc)}</p>
            <ul>${itemsHtml(t, t.items.pro, lang)}</ul>
            ${cta(t, "pro")}
          </article>
          <article class="pcard" data-r="BD">${badge(t.comingSoon, t.centreBadge)}
            <h3>${esc(t.centre.name)}</h3>
            <p class="amt">${amt("BD", "month", m(P.centre.BD.month, "BDT"), t.perMonth)}${amt("BD", "year", m(P.centre.BD.year, "BDT"), t.perYear)}</p>
            <p class="amt-sub">${sub("BD", "month", t.centreSubM)}${sub("BD", "year", t.centreSubY(m(Math.round(P.centre.BD.year / 12), "BDT")))}</p>
            <p class="pdesc">${esc(t.centre.desc)}</p>
            <ul>${itemsHtml(t, t.items.centre, lang)}</ul>
            ${cta(t, "centre")}
          </article>
          <article class="pcard">${badge(t.comingSoon, t.schoolBadge)}
            <h3>${esc(t.school.name)}</h3>
            <p class="amt">${amt("BD", null, m(P.school.BD.seatYear, "BDT"), t.perSeat)}${amt("INTL", null, m(P.school.INTL.seatYear, "USD"), t.perSeat)}</p>
            <p class="amt-sub">${sub("BD", null, t.schoolSub(P.school.BD.minSeats, true))}${sub("INTL", null, t.schoolSub(P.school.INTL.minSeats, false))}</p>
            <p class="pdesc">${esc(t.school.desc)}</p>
            <ul>${itemsHtml(t, t.items.school, lang)}</ul>
            ${cta(t, "school", "https://shaitrish.com")}
          </article>
        </div>
        <div class="pr-credits">
          <div><h3>${esc(t.creditsH)}${CFG.soon.hostedAi ? ` <em>${esc(t.soon)}</em>` : ""}</h3><p>${esc(t.creditsP)}</p></div>
          <ul>${P.credits.BD.map(([n, p]) => `<li data-r="BD">${esc(t.creditsItem(n, m(p, "BDT")))}</li>`).join("")}${P.credits.INTL.map(([n, p]) => `<li data-r="INTL">${esc(t.creditsItem(n, m(p, "USD")))}</li>`).join("")}</ul>
        </div>
      </div>`;
}

/** The same plans as plain Markdown (for the .md twin and llms-full.txt). */
function pricingMd(lang) {
  const P = PRICES;
  const m = (a, c) => money(a, c, lang);
  if (lang === "bn")
    return [
      `**ফ্রি (৳০):** পুরো অ্যাপ আর নিজের AI, এখনই; ${CFG.cloudLive ? "" : "Rupok Cloud চালু হলে "}৫০০ MB ক্লাউড, ১০টি সক্রিয় শিক্ষার্থী-লিংক, ৩০ দিনের ভার্সন হিস্ট্রি।`,
      `**Pro:** ${m(P.pro.BD.month, "BDT")}/মাস, ${m(P.pro.BD.sixMonths, "BDT")}/৬ মাস, ${m(P.pro.BD.year, "BDT")}/বছর (বিকাশ, নগদ, কার্ড; ভ্যাটসহ)। অন্য দেশে ${m(P.pro.INTL.month, "USD")}/মাস বা ${m(P.pro.INTL.year, "USD")}/বছর। ২০ GB, যত খুশি লিংক, পুরো হিস্ট্রি, মাসে ১৫০ AI ক্রেডিট (শীঘ্রই)।`,
      `**কোচিং সেন্টার (৫ জন শিক্ষক):** ${m(P.centre.BD.month, "BDT")}/মাস বা ${m(P.centre.BD.year, "BDT")}/বছর।`,
      `**স্কুল:** প্রতি শিক্ষক বছরে ${m(P.school.BD.seatYear, "BDT")} (অন্তত ১০ জন, ইনভয়েস), বা ${m(P.school.INTL.seatYear, "USD")} (অন্তত ৫ জন)। ক্লাসরুম বোর্ড লাইসেন্স প্রতি বোর্ড বছরে ${m(P.board.BD.year, "BDT")} / ${m(P.board.INTL.year, "USD")}।`,
      `**AI ক্রেডিট:** ২৫০টি ${m(599, "BDT")} / $৫, ১,০০০টি ${m(2199, "BDT")} / $১৮।`,
      live ? "" : `*পেইড প্ল্যানগুলো শীঘ্রই চালু হবে; এখন ফ্রি প্ল্যান চালু আছে।*`,
    ]
      .filter(Boolean)
      .join("\n\n");
  return [
    `**Free (৳0 / $0):** the whole app and your own AI, now; ${CFG.cloudLive ? "" : "when Rupok Cloud opens, "}500 MB cloud, 10 active student links, 30 days of version history.`,
    `**Pro:** ${m(P.pro.BD.month, "BDT")}/month, ${m(P.pro.BD.sixMonths, "BDT")}/6 months or ${m(P.pro.BD.year, "BDT")}/year in Bangladesh (bKash, Nagad, cards; VAT included); ${m(P.pro.INTL.month, "USD")}/month or ${m(P.pro.INTL.year, "USD")}/year elsewhere. 20 GB, unlimited links, full history, 150 AI credits a month (soon).`,
    `**Coaching centre (5 teachers, Bangladesh):** ${m(P.centre.BD.month, "BDT")}/month or ${m(P.centre.BD.year, "BDT")}/year.`,
    `**School:** ${m(P.school.BD.seatYear, "BDT")} per teacher per year (10 or more, invoice) or ${m(P.school.INTL.seatYear, "USD")} (5 or more). Classroom board licence ${m(P.board.BD.year, "BDT")} / ${m(P.board.INTL.year, "USD")} per board per year.`,
    `**AI credit top-ups:** 250 for ৳599 / $5, 1,000 for ৳2,199 / $18. They never expire.`,
    live ? "" : `*Paid plans open soon; the free plan is available now.*`,
  ]
    .filter(Boolean)
    .join("\n\n");
}

// ------------------------------------------------------------ comparison
function compareTable(lang) {
  const s = (flag) => (flag === "cloud" ? !CFG.cloudLive : !!CFG.soon[flag]);
  const v = (text, flag) => (flag && s(flag) ? `soon: ${text}` : `yes: ${text}`);
  if (lang === "bn") {
    return {
      caption: "কোন প্ল্যানে কী আছে",
      head: ["", "ফ্রি", "Pro", "কোচিং সেন্টার", "স্কুল"],
      wide: true,
      hl: "pro",
      rows: [
        ["পুরো অ্যাপ: বোর্ড, কালি, PDF, টিচ মোড, রিপ্লে, স্মার্ট অবজেক্ট, ডেস্কটপ অ্যাপ", "yes: ", "yes: ", "yes: ", "yes: "],
        ["নিজের AI (Claude Code, Codex, OpenCode)", "yes: যত খুশি", "yes: যত খুশি", "yes: যত খুশি", "yes: যত খুশি"],
        ["ক্লাউড স্টোরেজ", v("৫০০ MB", "cloud"), v("২০ GB", "cloud"), v("প্রতি শিক্ষক ৩০ GB", "cloud"), v("প্রতি শিক্ষক ৫০ GB", "cloud")],
        ["শিক্ষার্থী-লিংক (শিক্ষার্থীর অ্যাকাউন্ট লাগে না)", v("১০টি সক্রিয়", "cloud"), v("যত খুশি", "cloud"), v("যত খুশি", "cloud"), v("যত খুশি", "cloud")],
        ["কপি করা, সহকর্মীর সাথে শেয়ার, সব ডিভাইসে সিঙ্ক", v("", "cloud"), v("", "cloud"), v("", "cloud"), v("", "cloud")],
        ["ভার্সন হিস্ট্রি", v("৩০ দিন", "cloud"), v("পুরোটা", "cloud"), v("পুরোটা", "cloud"), v("পুরোটা", "cloud")],
        ["Rupok AI ক্রেডিট (AI সাবস্ক্রিপশন লাগে না)", v("একবার ২৫", "hostedAi"), v("মাসে ১৫০", "hostedAi"), v("প্রতি শিক্ষক মাসে ১৫০", "hostedAi"), v("প্রতি শিক্ষক মাসে ১৫০", "hostedAi")],
        ["লাইভ ক্লাস", v("মাসে ৩০০ মিনিট, ৩০ জন", "liveClasses"), v("মাসে ৩,০০০ মিনিট, ১০০ জন", "liveClasses"), v("মাসে ৬,০০০ মিনিট, ৩০০ জন", "liveClasses"), v("মাসে ৬,০০০ মিনিট, ৩০০ জন", "liveClasses")],
        ["শিক্ষার্থীদের দেখার হিসাব, ব্র্যান্ডিং টেমপ্লেট", "no: ", v("", "analytics"), v("", "analytics"), v("", "analytics")],
        ["অ্যাডমিন, শেয়ার্ড লাইব্রেরি, উপস্থিতি, কুইজ", "no: ", "no: ", v("", "admin"), v("", "admin")],
        ["ইনভয়েস আর ব্যাংক ট্রান্সফার", "no: ", "no: ", "no: ", "yes: "],
        ["আগে সাপোর্ট", "no: ", "yes: ", "yes: ", "yes: "],
      ],
    };
  }
  return {
    caption: "What each plan includes",
    head: ["", "Free", "Pro", "Coaching centre", "School"],
    wide: true,
    hl: "pro",
    rows: [
      ["The whole app: board, ink, PDF, Teach mode, replay, Smart Objects, desktop apps", "yes: ", "yes: ", "yes: ", "yes: "],
      ["Your own AI (Claude Code, Codex, OpenCode)", "yes: unlimited", "yes: unlimited", "yes: unlimited", "yes: unlimited"],
      ["Cloud storage", v("500 MB", "cloud"), v("20 GB", "cloud"), v("30 GB per teacher", "cloud"), v("50 GB per teacher", "cloud")],
      ["Student links (no student account needed)", v("10 active", "cloud"), v("unlimited", "cloud"), v("unlimited", "cloud"), v("unlimited", "cloud")],
      ["Make a copy, share with colleagues, sync devices", v("", "cloud"), v("", "cloud"), v("", "cloud"), v("", "cloud")],
      ["Version history", v("30 days", "cloud"), v("full", "cloud"), v("full", "cloud"), v("full", "cloud")],
      ["Rupok AI credits (no AI subscription needed)", v("25 to try", "hostedAi"), v("150 a month", "hostedAi"), v("150 per teacher a month", "hostedAi"), v("150 per teacher a month", "hostedAi")],
      ["Live classes", v("300 min a month, 30 students", "liveClasses"), v("3,000 min a month, 100 students", "liveClasses"), v("6,000 min, 300 students", "liveClasses"), v("6,000 min, 300 students", "liveClasses")],
      ["Student view analytics, branding templates", "no: ", v("", "analytics"), v("", "analytics"), v("", "analytics")],
      ["Admin, shared library, attendance, quizzes", "no: ", "no: ", v("", "admin"), v("", "admin")],
      ["Invoices and bank transfer", "no: ", "no: ", "no: ", "yes: "],
      ["Priority support", "no: ", "yes: ", "yes: ", "yes: "],
    ],
  };
}

// --------------------------------------------------------------- JSON-LD
function offersLd(path) {
  const availability = live ? "https://schema.org/InStock" : "https://schema.org/PreOrder";
  const url = SITE + path;
  const offer = (name, price, currency, duration, extra = {}) => ({
    "@type": "Offer",
    name,
    price: String(price),
    priceCurrency: currency,
    availability,
    url,
    ...(duration
      ? { priceSpecification: { "@type": "UnitPriceSpecification", price: String(price), priceCurrency: currency, billingDuration: duration, ...(extra.unitText ? { unitText: extra.unitText } : {}) } }
      : {}),
    ...(currency === "BDT" ? { eligibleRegion: { "@type": "Country", name: "Bangladesh" } } : {}),
  });
  const P = PRICES;
  return [
    {
      "@type": "Product",
      "@id": `${url}#rupok-cloud`,
      name: "Rupok Cloud",
      description:
        "Hosted sync, student share links, version history, Rupok AI credits and live classes for Rupok, the free open-source whiteboard for teachers. The app itself is free.",
      brand: { "@type": "Brand", name: "Rupok" },
      isRelatedTo: { "@id": `${SITE}/#app` },
      offers: [
        { "@type": "Offer", name: "Free", price: "0", priceCurrency: "USD", availability: "https://schema.org/InStock", url },
        offer("Pro, monthly", P.pro.INTL.month, "USD", "P1M"),
        offer("Pro, yearly", P.pro.INTL.year, "USD", "P1Y"),
        offer("Pro, 1 month (Bangladesh)", P.pro.BD.month, "BDT", "P1M"),
        offer("Pro, 6 months (Bangladesh)", P.pro.BD.sixMonths, "BDT", "P6M"),
        offer("Pro, 12 months (Bangladesh)", P.pro.BD.year, "BDT", "P1Y"),
        offer("Coaching centre, 5 teachers, 1 month", P.centre.BD.month, "BDT", "P1M"),
        offer("Coaching centre, 5 teachers, 12 months", P.centre.BD.year, "BDT", "P1Y"),
        offer("School, per teacher per year", P.school.INTL.seatYear, "USD", "P1Y", { unitText: "teacher" }),
        offer("School, per teacher per year (Bangladesh)", P.school.BD.seatYear, "BDT", "P1Y", { unitText: "teacher" }),
        offer("Classroom board licence, per board per year", P.board.INTL.year, "USD", "P1Y", { unitText: "board" }),
        offer("Classroom board licence, per board per year (Bangladesh)", P.board.BD.year, "BDT", "P1Y", { unitText: "board" }),
      ],
    },
  ];
}

// ----------------------------------------------------------------- pages
const EN = {
  path: "/pricing/",
  lang: "en",
  alt: { bn: "/bn/pricing/" },
  updated: "2026-11-24",
  title: "Rupok Pricing — Free Whiteboard App, Cloud from ৳299 or $7",
  ogTitle: "Rupok pricing: the app is free, forever",
  description:
    "Rupok's whiteboard app is free and open source, forever. Rupok Cloud adds storage, student links, history and AI credits: free 500 MB plan, Pro from ৳299 or $7 a month, coaching centres and schools.",
  tag: "Pricing",
  h1: "The app is free. The classroom cloud is cheap.",
  lede: "Every board feature is free and open source, forever: ink, PDF, replay, Smart Objects, desktop apps and your own AI. Rupok Cloud adds storage, student links, history and hosted AI. Pay in taka with bKash or Nagad, or in dollars by card.",
  crumbs: [["Pricing"]],
  bodyClass: "pricing-page",
  sections: [
    { id: "plans", h2: "Plans", blocks: [{ html: pricingHtml("en", "INTL"), md: pricingMd("en") }] },
    {
      id: "compare",
      h2: "Compare plans",
      lede: live ? "" : "Rupok Cloud opens soon. Items marked “soon” are being built; nothing on this page can be bought yet.",
      blocks: [{ table: compareTable("en") }],
    },
    {
      id: "pay",
      h2: "Ways to pay",
      blocks: [
        {
          cols: [
            [{ h3: "In Bangladesh, in taka" }, { ul: ["**bKash, Nagad, Rocket**, Visa, Mastercard, AMEX and internet banking, through SSLCommerz.", "Prepaid for 1, 6 or 12 months. Nothing renews by itself; we remind you a week before.", "Prices include VAT. You get a receipt for every payment."] }],
            [{ h3: "Everywhere else, in US dollars" }, { ul: ["**Cards, PayPal, Apple Pay and Google Pay**, through Paddle, our reseller and merchant of record.", "Monthly or yearly. Change plan, card or cancel any time from Settings.", "Paddle adds sales tax or VAT where it applies and sends the invoice."] }],
          ],
        },
        { note: "You can switch between taka and dollars on this page and in the app. Taka prices are for teachers in Bangladesh." },
      ],
    },
  ],
  faq: [
    { q: "Is Rupok really free?", a: "Yes. The app is open source (AGPL-3.0) and every board feature is free forever, on every device. You only pay for Rupok Cloud extras: more storage, unlimited student links, full history, hosted AI and, soon, live classes." },
    { q: "What stays free forever?", a: "The whole app, offline, with plain `.rpk` files; every board feature, including future ones; using your own Claude, Codex or OpenCode; the 500 MB cloud plan; student links students open with no account; exporting and deleting your data." },
    { q: "What happens if I stop paying?", a: "Nothing is deleted. You keep every lesson, can open, download and share them, and can delete things to get back under the free limits. Only new uploads stop while you're over 500 MB." },
    { q: "How do I pay from Bangladesh?", a: "In taka, with bKash, Nagad, Rocket, a card or internet banking, through SSLCommerz. Buy 1, 6 or 12 months at a time; nothing renews automatically, and the app reminds you before your time runs out." },
    { q: "How do I pay from other countries?", a: "In US dollars by card, PayPal, Apple Pay or Google Pay, through Paddle. Paddle is our reseller: it handles tax and invoices. Change plan, update your card or cancel any time from Settings → Account → Manage subscription." },
    { q: "What are Rupok AI credits?", a: "Credits run Rupok's hosted AI (Solve this, Design with AI, the chat) for teachers who don't have their own AI subscription. A chat reply or a quick solution uses about 1 credit, a full step-by-step solution about 4, and designing a Smart Object about 10. Using your own Claude Code, Codex or OpenCode is always free." },
    { q: "Do you offer refunds?", a: "We plan a 14-day, no-questions refund on your first purchase. The exact policy will be in our terms before payments open." },
    { q: "Can my school pay by invoice?", a: "Yes. School plans and classroom board licences are invoiced once a year and paid by bank transfer, with a VAT receipt in Bangladesh. [Talk to us](https://shaitrish.com) for a quote or a free pilot." },
    { q: "Do students pay?", a: "Never. Students open lessons from a link with no account, and a student account is free." },
    { q: "Is there a free trial?", a: `Yes: ${CFG.trialDays} days of Pro, no card needed, once per account${live ? "" : ", when paid plans open"}.` },
  ],
  ld: offersLd("/pricing/"),
  final: { h2: "Start with the free plan today.", p: "Open the board in your browser, or download the desktop app. No account needed.", cta: [{ href: "/board/", label: "Open the board" }, { href: "/download/", label: "Download", kind: "ghost" }] },
};

const BN = {
  path: "/bn/pricing/",
  lang: "bn",
  alt: { en: "/pricing/" },
  updated: "2026-11-24",
  title: "Rupok-এর দাম — ফ্রি হোয়াইটবোর্ড অ্যাপ, ক্লাউড মাসে ৳২৯৯ থেকে",
  ogTitle: "Rupok-এর দাম: অ্যাপটি চিরকাল ফ্রি",
  description:
    "Rupok হোয়াইটবোর্ড অ্যাপ চিরকাল ফ্রি আর ওপেন সোর্স। Rupok Cloud-এ স্টোরেজ, শিক্ষার্থী-লিংক, হিস্ট্রি আর AI ক্রেডিট: ৫০০ MB ফ্রি, Pro মাসে ৳২৯৯ থেকে, বিকাশ বা নগদে। কোচিং সেন্টার আর স্কুলের প্ল্যানও আছে।",
  tag: "দাম",
  h1: "অ্যাপটি ফ্রি। ক্লাসরুমের ক্লাউড সস্তা।",
  lede: "বোর্ডের সব ফিচার ফ্রি আর ওপেন সোর্স, চিরকাল: কালি, PDF, রিপ্লে, স্মার্ট অবজেক্ট, ডেস্কটপ অ্যাপ আর নিজের AI। Rupok Cloud যোগ করে স্টোরেজ, শিক্ষার্থী-লিংক, হিস্ট্রি আর AI। টাকায় দিন বিকাশ বা নগদে, অথবা ডলারে কার্ডে।",
  crumbs: [["দাম"]],
  bodyClass: "pricing-page",
  sections: [
    { id: "plans", h2: "প্ল্যান", blocks: [{ html: pricingHtml("bn", "BD"), md: pricingMd("bn") }] },
    {
      id: "compare",
      h2: "প্ল্যানগুলো মিলিয়ে দেখুন",
      lede: live ? "" : "Rupok Cloud শীঘ্রই চালু হবে। “শীঘ্রই” লেখা জিনিসগুলো তৈরি হচ্ছে; এই পাতার কিছুই এখনো কেনা যায় না।",
      blocks: [{ table: compareTable("bn") }],
    },
    {
      id: "pay",
      h2: "কীভাবে টাকা দেবেন",
      blocks: [
        {
          cols: [
            [{ h3: "বাংলাদেশ থেকে, টাকায়" }, { ul: ["SSLCommerz-এর মাধ্যমে **বিকাশ, নগদ, রকেট**, ভিসা, মাস্টারকার্ড, অ্যামেক্স আর ইন্টারনেট ব্যাংকিং।", "১, ৬ বা ১২ মাসের জন্য আগে টাকা দিন। নিজে থেকে কিছু কাটবে না; মেয়াদ শেষের এক সপ্তাহ আগে আমরা মনে করিয়ে দেব।", "দামে ভ্যাট ধরা আছে। প্রতিটি পেমেন্টের রসিদ পাবেন।"] }],
            [{ h3: "অন্য দেশ থেকে, ডলারে" }, { ul: ["আমাদের রিসেলার Paddle-এর মাধ্যমে **কার্ড, PayPal, Apple Pay আর Google Pay**।", "মাসিক বা বাৎসরিক। সেটিংস থেকে যখন খুশি প্ল্যান বা কার্ড বদলান, বা বাতিল করুন।", "প্রযোজ্য হলে Paddle কর যোগ করে ইনভয়েস পাঠায়।"] }],
          ],
        },
        { note: "এই পাতায় আর অ্যাপে টাকা আর ডলারের মধ্যে বদলানো যায়। টাকার দাম বাংলাদেশের শিক্ষকদের জন্য।" },
      ],
    },
  ],
  faqTitle: "দাম নিয়ে যা জানতে চান",
  faq: [
    { q: "Rupok কি সত্যিই ফ্রি?", a: "হ্যাঁ। অ্যাপটি ওপেন সোর্স (AGPL-3.0), আর বোর্ডের সব ফিচার সব ডিভাইসে চিরকাল ফ্রি। টাকা লাগে শুধু Rupok Cloud-এর বাড়তি সুবিধার জন্য: বেশি স্টোরেজ, যত খুশি শিক্ষার্থী-লিংক, পুরো হিস্ট্রি, AI আর শীঘ্রই লাইভ ক্লাস।" },
    { q: "কোনগুলো চিরকাল ফ্রি থাকবে?", a: "পুরো অ্যাপ, ইন্টারনেট ছাড়াও, সাধারণ `.rpk` ফাইলসহ; বোর্ডের সব ফিচার, ভবিষ্যতেরগুলোও; নিজের Claude, Codex বা OpenCode ব্যবহার; ৫০০ MB-এর ক্লাউড প্ল্যান; শিক্ষার্থীরা অ্যাকাউন্ট ছাড়াই যে লিংক খোলে; নিজের ডেটা এক্সপোর্ট আর মুছে ফেলা।" },
    { q: "টাকা দেওয়া বন্ধ করলে কী হবে?", a: "কিছুই মুছে যাবে না। সব লেসন থাকবে; খুলতে, ডাউনলোড আর শেয়ার করতে পারবেন, আর কিছু মুছে ফ্রি সীমার মধ্যে ফিরতে পারবেন। ৫০০ MB-এর বেশি থাকলে শুধু নতুন আপলোড বন্ধ থাকবে।" },
    { q: "বাংলাদেশ থেকে কীভাবে টাকা দেব?", a: "টাকায়, SSLCommerz-এর মাধ্যমে বিকাশ, নগদ, রকেট, কার্ড বা ইন্টারনেট ব্যাংকিংয়ে। একবারে ১, ৬ বা ১২ মাস কিনুন; নিজে থেকে নবায়ন হয় না, মেয়াদ শেষের আগে অ্যাপ মনে করিয়ে দেবে।" },
    { q: "Rupok AI ক্রেডিট কী?", a: "যাদের নিজের AI সাবস্ক্রিপশন নেই, ক্রেডিট দিয়ে তারা Rupok-এর AI (Solve this, Design with AI, চ্যাট) চালান। একটি চ্যাট উত্তর বা ছোট সমাধানে প্রায় ১ ক্রেডিট, ধাপে ধাপে পুরো সমাধানে প্রায় ৪, আর একটি স্মার্ট অবজেক্ট বানাতে প্রায় ১০। নিজের Claude Code, Codex বা OpenCode সবসময় ফ্রি।" },
    { q: "টাকা ফেরত পাওয়া যায়?", a: "প্রথম কেনাকাটায় ১৪ দিনের মধ্যে কোনো প্রশ্ন ছাড়াই পুরো টাকা ফেরতের পরিকল্পনা আছে। পেমেন্ট চালুর আগে পুরো নীতিমালা আমাদের শর্তাবলিতে থাকবে।" },
    { q: "স্কুল কি ইনভয়েসে টাকা দিতে পারবে?", a: "হ্যাঁ। স্কুল প্ল্যান আর ক্লাসরুম বোর্ড লাইসেন্সের ইনভয়েস বছরে একবার হয়, টাকা দেওয়া যায় ব্যাংক ট্রান্সফারে, ভ্যাট রসিদসহ। কোটেশন বা ফ্রি পাইলটের জন্য [যোগাযোগ করুন](https://shaitrish.com)।" },
    { q: "শিক্ষার্থীদের কি টাকা দিতে হয়?", a: "কখনোই না। শিক্ষার্থীরা অ্যাকাউন্ট ছাড়াই লিংক থেকে লেসন খোলে, আর শিক্ষার্থীর অ্যাকাউন্টও ফ্রি।" },
    { q: "ফ্রি ট্রায়াল আছে?", a: `হ্যাঁ: ${num(CFG.trialDays, "bn")} দিনের Pro, কার্ড লাগবে না, প্রতি অ্যাকাউন্টে একবার${live ? "" : ", পেইড প্ল্যান চালু হলে"}।` },
  ],
  ld: offersLd("/bn/pricing/"),
  final: { h2: "আজই ফ্রি প্ল্যান দিয়ে শুরু করুন।", p: "ব্রাউজারে বোর্ড খুলুন, অথবা ডেস্কটপ অ্যাপ নামিয়ে নিন। অ্যাকাউন্ট লাগবে না।", cta: [{ href: "/board/", label: "বোর্ড খুলুন" }, { href: "/bn/download/", label: "ডাউনলোড", kind: "ghost" }] },
};

export const PRICING = [EN, BN];

// ------------------------------------------------------ home page plans
/** The home pages' three plan cards (index.html region "plans", and /bn/). */
export function homePlans(lang) {
  const bn = lang === "bn";
  const s = (flag) => (flag === "cloud" ? !CFG.cloudLive : !!CFG.soon[flag]) ? (bn ? "শীঘ্রই" : "soon") : undefined;
  const P = PRICES;
  const m = (a, c) => money(a, c, lang);
  const badge = (feat) => (live ? (feat ? (bn ? "সবচেয়ে জনপ্রিয়" : "Most popular") : undefined) : bn ? "শীঘ্রই আসছে" : "Coming soon");
  const pricing = bn ? "/bn/pricing/" : "/pricing/";
  const it = (text, flag) => (flag && s(flag) ? [text, s(flag)] : [text]);
  if (bn)
    return {
      kind: "plans",
      id: "plans",
      tag: "দাম",
      h2: "অ্যাপটি ফ্রি। চিরকাল।",
      p: `Rupok ওপেন সোর্স (AGPL-3.0), তাই কেউ ফ্রি সংস্করণ বন্ধ করে দিতে পারবে না। Rupok Cloud যোগ করে স্টোরেজ, শিক্ষার্থী-লিংক, হিস্ট্রি আর AI, প্রত্যেক শিক্ষকের জন্য ফ্রি প্ল্যানসহ। [সব প্ল্যান আর দাম দেখুন](${pricing})।`,
      plans: [
        { name: "ফ্রি", price: "৳০", items: [it("পুরো অ্যাপ, সব ফিচার"), it("নিজের AI, যত খুশি"), it("৫০০ MB ক্লাউড", "cloud"), it("১০টি শিক্ষার্থী-লিংক", "cloud")], cta: { href: "/board/", label: "এখনই শুরু করুন", kind: "dark" } },
        {
          name: "Pro",
          feat: true,
          badge: badge(true),
          price: `মাসে ${m(P.pro.BD.month, "BDT")}`,
          items: [it("ফ্রি প্ল্যানের সবকিছু"), it("২০ GB ক্লাউড স্টোরেজ", "cloud"), it("যত খুশি শিক্ষার্থী-লিংক", "cloud"), it("মাসে ১৫০ Rupok AI ক্রেডিট", "hostedAi"), it("লাইভ ক্লাস", "liveClasses")],
          cta: live ? { href: "/board/?upgrade=pro", label: "Pro নিন", kind: "mint" } : { href: pricing, label: "দাম দেখুন" },
        },
        {
          name: "কোচিং ও স্কুল",
          badge: badge(false),
          price: `মাসে ${m(P.centre.BD.month, "BDT")} থেকে`,
          items: [it("প্রত্যেক শিক্ষকের জন্য Pro"), it("অ্যাডমিন আর শেয়ার্ড লাইব্রেরি", "admin"), it("ইনভয়েস আর ব্যাংক ট্রান্সফার"), it("ক্লাসরুম বোর্ড লাইসেন্স")],
          cta: { href: pricing, label: "প্ল্যান দেখুন" },
        },
      ],
    };
  return {
    kind: "plans",
    id: "plans",
    tag: "Pricing",
    h2: "The app is free. Forever.",
    p: `Rupok is open source. Rupok Cloud adds storage, student links, history and hosted AI, with a free plan for every teacher. [See all plans and prices](${pricing}).`,
    plans: [
      { name: "Free", price: "৳0 · $0", items: [it("The full app, every feature"), it("Your own AI, unlimited"), it("500 MB cloud", "cloud"), it("10 student links", "cloud")], cta: { href: "/board/", label: "Start now", kind: "dark" } },
      {
        name: "Pro",
        feat: true,
        badge: badge(true),
        price: `${m(P.pro.BD.month, "BDT")} or ${m(P.pro.INTL.month, "USD")} a month`,
        items: [it("Everything in Free"), it("20 GB cloud storage", "cloud"), it("Unlimited student links", "cloud"), it("150 Rupok AI credits a month", "hostedAi"), it("Live classes", "liveClasses")],
        cta: live ? { href: "/board/?upgrade=pro", label: "Get Pro", kind: "mint" } : { href: pricing, label: "See pricing" },
      },
      {
        name: "Schools & centres",
        badge: badge(false),
        price: `From ${m(P.centre.BD.month, "BDT")} a month`,
        items: [it("Pro for every teacher"), it("Admin and shared library", "admin"), it("Invoices and bank transfer"), it("Classroom board licences")],
        cta: { href: pricing, label: "See plans" },
      },
    ],
  };
}
