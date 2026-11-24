// The donate page (/donate/ and /bn/donate/): why to support Rupok, what the
// money pays for, how to give, other ways to help, an honest impact counter
// (GitHub release downloads, fetched by src/donate.ts at runtime) and an
// opt-in thank-you list. No analytics, no payment scripts: every method is a
// plain transfer the donor makes in their own app.
//
// METHODS is the one place the payment details live. A method that is `null`
// is not shown; fill it in to make it appear on both pages. Anything marked
// PLACEHOLDER still needs the founder's real account.

import { GITHUB, SITE, esc, inline } from "../lib.mjs";

export const METHODS = {
  /** Where donors write to be thanked, ask for a receipt or another way to pay.
   * Reused from studio.shaitrish.com/donate. */
  contact: "jahangir@shaitrish.com",
  /** What donors write as the reference, so the money is matched to Rupok. */
  reference: "Rupok",
  /** bKash Send Money (reused from studio.shaitrish.com/donate). */
  bkash: { number: "01635191148" },
  /** PLACEHOLDER: Nagad Send Money number, e.g. { number: "01XXXXXXXXX" }. Not on the 37Studio page. */
  nagad: null,
  /** US bank transfer (ACH), reused from studio.shaitrish.com/donate. */
  usBank: {
    holder: "Md Jahangir Hossain",
    account: "30000005325754",
    routing: "028000024",
    type: "Checking",
    bank: "JPMorgan Chase Bank, N.A., 270 Park Avenue, New York, NY 10017",
    address: "Dhaka 1207, Bangladesh",
    minUsd: 2,
  },
  /** PLACEHOLDER: a card checkout link (SSLCommerz or Paddle, once billing ships), e.g. "https://rupok.app/donate/card". */
  card: null,
  /** PLACEHOLDER: a PayPal.me link, e.g. "https://paypal.me/…". Not on the 37Studio page. */
  paypal: null,
  /** PLACEHOLDER: GitHub Sponsors, e.g. "https://github.com/sponsors/Shaitrish". No listing exists yet. */
  githubSponsors: null,
};

/** Suggested one-time amounts. Any amount is welcome. */
export const AMOUNTS = { bdt: [100, 300, 500, 1000, 5000], usd: [2, 5, 10, 25, 99] };

/** Opt-in only: add someone here only when they ask to be thanked by name.
 * { name: "…", url?: "https://…", note?: "…" } */
export const SUPPORTERS = [];

// ----------------------------------------------------------------- helpers

const BN_DIGITS = "০১২৩৪৫৬৭৮৯";
const bnNum = (s) => String(s).replace(/\d/g, (d) => BN_DIGITS[d]);
const group = (n, lang) => (lang === "bn" ? bnNum(n.toLocaleString("en-IN")) : n.toLocaleString("en-US"));
const taka = (n, lang) => `৳${group(n, lang)}`;
const usd = (n) => `$${n}`;

const copyBtn = (value, t) =>
  `<button type="button" class="copy" data-copy="${esc(value)}" data-done="${esc(t.copied)}" hidden>${esc(t.copy)}</button>`;

const amounts = (kind, list, fmt, t) =>
  `<div class="amts" role="radiogroup" aria-label="${esc(t.amountsLabel[kind])}" data-amts="${kind}" hidden>${list
    .map((n) => `<button type="button" role="radio" aria-checked="false" data-amt="${esc(fmt(n))}">${esc(fmt(n))}</button>`)
    .join("")}</div>`;

/** A "Send Money" card for bKash or Nagad. */
function walletCard(id, name, w, t, lang) {
  return `<article class="method" id="${id}">
          <span class="chip">${t.inBd}</span>
          <h3>${name}</h3>
          <p class="method-sub">${t.walletSub(name)}</p>
          ${amounts("bdt", AMOUNTS.bdt, (n) => taka(n, lang), t)}
          <ol class="msteps">
            <li>${t.walletStep1(name)}</li>
            <li>${t.walletStep2("<span data-amt-out>" + t.anyAmount + "</span>")} <span class="num">${esc(w.number)}</span> ${copyBtn(w.number, t)}</li>
            <li>${t.refStep(METHODS.reference)}</li>
          </ol>
          <p class="method-fine">${inline(t.thanks(METHODS.contact))}</p>
        </article>`;
}

function bankCard(b, t) {
  const row = (k, v, copy = false) => `<div><dt>${k}</dt><dd><span>${esc(v)}</span>${copy ? copyBtn(v, t) : ""}</dd></div>`;
  return `<article class="method" id="us-bank">
          <span class="chip">${t.abroad}</span>
          <h3>${t.bankTitle}</h3>
          <p class="method-sub">${t.bankSub(b.minUsd)}</p>
          ${amounts("usd", AMOUNTS.usd, usd, t)}
          <p class="amt-line">${t.bankSend("<span data-amt-out>" + t.anyAmount + "</span>")}</p>
          <dl class="bank">
            ${row(t.holder, b.holder)}
            ${row(t.account, b.account, true)}
            ${row(t.routing, b.routing, true)}
            ${row(t.type, b.type)}
            ${row(t.bank, b.bank)}
            ${row(t.address, b.address)}
            ${row(t.reference, METHODS.reference)}
          </dl>
          <p class="method-fine">${inline(t.bankFine(METHODS.contact))}</p>
        </article>`;
}

/** Card, PayPal and GitHub Sponsors: links when configured, otherwise an honest "not yet". */
function otherCard(t) {
  const links = [
    METHODS.card && [METHODS.card, t.payCard],
    METHODS.paypal && [METHODS.paypal, "PayPal"],
    METHODS.githubSponsors && [METHODS.githubSponsors, "GitHub Sponsors"],
  ].filter(Boolean);
  const missing = [!METHODS.card && t.cardWord, !METHODS.paypal && "PayPal", !METHODS.githubSponsors && "GitHub Sponsors"].filter(Boolean);
  return `<article class="method other">
          <h3>${t.otherTitle}</h3>
          ${links.length ? `<div class="ctas">${links.map(([href, label]) => `<a class="pill dark" href="${esc(href)}" rel="noopener">${esc(label)}</a>`).join("")}</div>` : ""}
          ${missing.length ? `<p>${inline(t.notYet(missing, METHODS.contact))}</p>` : ""}
        </article>`;
}

function giveHtml(t, lang) {
  const cards = [];
  if (METHODS.bkash) cards.push(walletCard("bkash", "bKash", METHODS.bkash, t, lang));
  if (METHODS.nagad) cards.push(walletCard("nagad", t.nagad, METHODS.nagad, t, lang));
  if (METHODS.usBank) cards.push(bankCard(METHODS.usBank, t));
  cards.push(otherCard(t));
  return `<div class="methods" data-give>${cards.join("\n        ")}</div>`;
}

function giveMd(t, lang) {
  const out = [];
  if (METHODS.bkash) out.push(`### bKash (${t.inBd})`, "", `${t.mdWallet("bKash", METHODS.bkash.number, METHODS.reference)} ${t.mdAmounts}: ${AMOUNTS.bdt.map((n) => taka(n, lang)).join(", ")}.`);
  if (METHODS.nagad) out.push("", `### ${t.nagad}`, "", t.mdWallet(t.nagad, METHODS.nagad.number, METHODS.reference));
  if (METHODS.usBank) {
    const b = METHODS.usBank;
    out.push("", `### ${t.bankTitle} (${t.abroad})`, "", t.bankSub(b.minUsd).replace(/<[^>]+>/g, ""), "",
      `- **${t.holder}**: ${b.holder}`, `- **${t.account}**: ${b.account}`, `- **${t.routing}**: ${b.routing}`, `- **${t.type}**: ${b.type}`,
      `- **${t.bank}**: ${b.bank}`, `- **${t.address}**: ${b.address}`, `- **${t.reference}**: ${METHODS.reference}`, "",
      `${t.mdAmounts}: ${AMOUNTS.usd.map(usd).join(", ")}.`);
  }
  out.push("", t.thanks(METHODS.contact));
  return out.join("\n");
}

const IMPACT_ICONS = {
  dl: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12m-4.5-4.5L12 15l4.5-4.5M4 17v1.5A2.5 2.5 0 0 0 6.5 21h11a2.5 2.5 0 0 0 2.5-2.5V17"/></svg>`,
  star: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m12 3 2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 16.8l-5.4 2.9 1.1-6.1-4.5-4.2 6.1-.8z"/></svg>`,
  tag: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12V4a1 1 0 0 1 1-1h8l9 9-9 9z"/><circle cx="8" cy="8" r="1.6"/></svg>`,
};

function impactHtml(t, lang) {
  const stat = (key, icon, label) =>
    `<div class="impact-stat"><span class="impact-ico">${IMPACT_ICONS[icon]}</span><b data-impact-n="${key}">–</b><span>${label}</span></div>`;
  return `<aside class="impact" data-impact data-lang="${lang}" aria-labelledby="impact-h" aria-live="polite">
          <p class="impact-k" id="impact-h">${t.impactTitle}</p>
          ${stat("downloads", "dl", t.impactDownloads)}
          ${stat("stars", "star", t.impactStars)}
          ${stat("releases", "tag", t.impactReleases)}
          <p class="impact-src" data-impact-ok>${inline(t.impactSource)}</p>
          <p class="impact-src" data-impact-err hidden>${inline(t.impactError)}</p>
        </aside>`;
}

function supportersHtml(t) {
  if (!SUPPORTERS.length) return `<div class="thanks-empty"><p>${inline(t.noSupporters)}</p></div>`;
  return `<ul class="supporters">${SUPPORTERS.map((s) => {
    const name = s.url ? `<a href="${esc(s.url)}" rel="noopener nofollow">${esc(s.name)}</a>` : esc(s.name);
    return `<li><b>${name}</b>${s.note ? `<span>${esc(s.note)}</span>` : ""}</li>`;
  }).join("")}</ul><p class="fine">${inline(t.supportersNote)}</p>`;
}

const ld = (path, lang) => [
  {
    "@type": "DonateAction",
    "@id": `${SITE}${path}#donate`,
    name: lang === "bn" ? "Rupok-কে অনুদান দিন" : "Donate to Rupok",
    description: lang === "bn" ? "শিক্ষকদের ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড Rupok-কে সহায়তা করুন।" : "Support Rupok, the free, open-source whiteboard for teachers.",
    recipient: { "@id": `${SITE}/#org` },
    object: { "@id": `${SITE}/#app` },
    target: `${SITE}${path}`,
  },
];

// ----------------------------------------------------------------- English
const tEn = {
  inBd: "In Bangladesh",
  abroad: "From abroad",
  nagad: "Nagad",
  walletSub: (name) => `Taka, from the ${name} app. Any amount helps.`,
  walletStep1: (name) => `In the ${name} app, choose <b>Send Money</b>.`,
  walletStep2: (amt) => `Send ${amt} to`,
  refStep: (ref) => `Write <b>${ref}</b> as the reference.`,
  anyAmount: "any amount",
  amountsLabel: { bdt: "Suggested amounts in taka", usd: "Suggested amounts in US dollars" },
  copy: "Copy",
  copied: "Copied",
  thanks: (mail) => `Want a thank-you? Email [${mail}](mailto:${mail}?subject=Rupok%20donation) with your name. We never list anyone without asking.`,
  bankTitle: "US bank transfer (ACH)",
  bankSub: (min) => `US dollars, from a US bank account, Wise, or any app that pays US bank accounts. The minimum is $${min}.`,
  bankSend: (amt) => `Send ${amt} to:`,
  holder: "Account holder", account: "Account number", routing: "Routing number", type: "Account type", bank: "Bank",
  address: "Recipient address", reference: "Reference",
  bankFine: (mail) => `ACH takes 1 to 3 business days. Then email [${mail}](mailto:${mail}?subject=Rupok%20donation) if you'd like to be thanked or need a receipt.`,
  otherTitle: "Card, PayPal or GitHub Sponsors?",
  payCard: "Pay by card",
  cardWord: "card payments",
  notYet: (missing, mail) =>
    `Not yet: ${missing.join(", ").replace(/, ([^,]*)$/, " and $1")} aren't set up. Can't use bKash or a bank transfer? Email [${mail}](mailto:${mail}?subject=Rupok%20donation) and we'll find a way that works for you.`,
  mdWallet: (name, num, ref) => `In the ${name} app, choose **Send Money**, send any amount to **${num}** and write **${ref}** as the reference.`,
  mdAmounts: "Suggested one-time amounts",
  impactTitle: "Rupok so far",
  impactDownloads: "desktop app downloads",
  impactStars: "stars on GitHub",
  impactReleases: "releases",
  impactSource: `Live from GitHub's public numbers. Counted, never estimated. [See every release](${GITHUB}/releases)`,
  impactError: `Couldn't reach GitHub just now. [See the numbers on GitHub](${GITHUB}/releases)`,
  noSupporters: "This list is empty on purpose. We only thank people by name when they ask us to. If you'd like your name (or your school's) here, say so when you email us after donating.",
  supportersNote: "Listed only with their permission. Thank you.",
};

const en = {
  path: "/donate/",
  lang: "en",
  alt: { bn: "/bn/donate/" },
  title: "Support Rupok: Donate to the Free Whiteboard for Teachers | Rupok",
  ogTitle: "Support Rupok, the free whiteboard for teachers",
  description:
    "Rupok is free and open source, with no ads or tracking. Donate once by bKash or US bank transfer to pay for signing certificates, servers, test devices and development time, or help by starring, sharing and translating.",
  bodyClass: "donate-page",
  crumbs: [["Donate"]],
  tag: "Support Rupok",
  h1: "Help keep Rupok free for every teacher",
  lede: "Rupok is free and open source, and it stays free whether or not you donate. Donations pay for what a free app still costs: signing certificates, servers, test devices and the time to build it.",
  cta: [{ href: "#give", label: "Donate", kind: "mint" }, { href: "#help", label: "Other ways to help", kind: "ghost" }],
  aside: impactHtml(tEn, "en"),
  ld: ld("/donate/", "en"),
  sections: [
    {
      id: "why",
      h2: "Why support Rupok",
      blocks: [{
        html: `<ul class="why">
          <li><b>Free for every teacher</b><span>Every feature, on every device, with no account. No trial, no paid version of the app.</span></li>
          <li><b>Open source</b><span>The code is on GitHub under the AGPL. Anyone can read it, fix it, or keep it going.</span></li>
          <li><b>Made in Bangladesh</b><span>Built by Shaitrish in Dhaka, for classrooms here and everywhere, with Bangla in mind from day one.</span></li>
          <li><b>No ads, no tracking</b><span>No analytics, no tracking cookies, nothing sold. This page doesn't track you either.</span></li>
        </ul>`,
        md: "- **Free for every teacher:** every feature, on every device, with no account.\n- **Open source:** the code is on GitHub under the AGPL.\n- **Made in Bangladesh:** built by Shaitrish in Dhaka, with Bangla in mind from day one.\n- **No ads, no tracking:** no analytics, no tracking cookies, nothing sold.",
      }],
    },
    {
      id: "costs",
      h2: "What your donation pays for",
      lede: "Nothing goes to ads or marketing. This is where the money goes.",
      blocks: [{
        html: `<ul class="costs">
          <li><span class="cost-tag">$99 a year</span><b>Signing certificates</b><span>Apple's developer membership and a Windows code-signing certificate, so Mac and Windows stop warning teachers when they install Rupok (<a href="/download/#unsigned">why the warning appears</a>).</span></li>
          <li><span class="cost-tag">Every month</span><b>Servers</b><span>rupok.app, downloads, and the free Rupok Cloud plan every teacher will get.</span></li>
          <li><span class="cost-tag">Once in a while</span><b>Devices for testing</b><span>Interactive panels, pen tablets, a Mac and cheap Android tablets, so Rupok works on the boards schools really have.</span></li>
          <li><span class="cost-tag">Every day</span><b>Development time</b><span>Fixes, new features, the Bangla interface and answering teachers' questions.</span></li>
        </ul>`,
        md: "- **Signing certificates:** Apple's developer membership ($99 a year) and a Windows code-signing certificate, so installs stop showing warnings.\n- **Servers:** rupok.app, downloads, and the free Rupok Cloud plan.\n- **Devices for testing:** interactive panels, pen tablets, a Mac and Android tablets.\n- **Development time:** fixes, new features, the Bangla interface and support.",
      }],
    },
    {
      id: "give",
      band: "mint",
      h2: "Donate once",
      lede: "Pick a way that works for you. Every taka and every dollar goes into Rupok.",
      blocks: [{ html: giveHtml(tEn, "en"), md: giveMd(tEn, "en") }],
    },
    {
      id: "help",
      h2: "Other ways to help",
      lede: "Money isn't the only way. These help just as much.",
      blocks: [{ links: [
        { href: GITHUB, b: "Star Rupok on GitHub", t: "It helps other teachers and developers find it." },
        { href: "https://wa.me/?text=" + encodeURIComponent("Rupok: a free whiteboard for teachers. https://rupok.app"), b: "Share it with a teacher", t: "Send it to your school's WhatsApp group or show a colleague." },
        { href: `${GITHUB}/issues`, b: "Report a bug or ask for a feature", t: "Tell us what broke or what your class needs." },
        { href: `${GITHUB}/blob/main/CONTRIBUTING.md`, b: "Translate", t: "Help bring the Bangla interface, and more languages, to every classroom." },
        { href: `${GITHUB}/blob/main/CONTRIBUTING.md`, b: "Contribute code or lessons", t: "Send a pull request, or make a Smart Object other teachers can use." },
      ] }],
    },
    {
      id: "thanks",
      h2: "Thank you",
      blocks: [{ html: supportersHtml(tEn), md: SUPPORTERS.length ? SUPPORTERS.map((s) => `- ${s.name}`).join("\n") : tEn.noSupporters }],
    },
  ],
  faqTitle: "Questions about donating",
  faq: [
    { q: "Is Rupok still free if I don't donate?", a: "Yes, completely. Every feature works the same for everyone. Donations don't unlock anything; they help us keep building." },
    { q: "What's the difference between a donation and Rupok Cloud?", a: "A donation is a gift to the project. Rupok Cloud is an optional hosted service (sync, sharing, live classes) that will have a free plan for every teacher and paid plans for more storage. The app itself is free either way." },
    { q: "Can I pay by card, PayPal or GitHub Sponsors?", a: `Not yet. Today you can donate by bKash or by US bank transfer (ACH, including from Wise). If neither works for you, email [${METHODS.contact}](mailto:${METHODS.contact}) and we'll find a way.` },
    { q: "Can I donate every month?", a: "There's no subscription. If you'd like to give regularly, set up a repeat transfer in your bank or bKash app, with Rupok as the reference." },
    { q: "Will my name appear on this page?", a: "Only if you ask. We never list donors by default. Email us after donating if you'd like your name, or your school's, on the thank-you list." },
    { q: "Does donating share my data or track me?", a: "No. This page has no analytics or payment scripts. We only see what your bank or bKash shows us for the transfer (usually a name or a number), and we use it only to say thank you." },
    { q: "Where do the numbers at the top come from?", a: "They are read live from GitHub's public API: the total downloads of every desktop release, the repository's stars, and the number of releases. If GitHub can't be reached, the page links to the numbers instead of guessing." },
    { q: "Can my school or company sponsor Rupok?", a: `Yes, and thank you. Email [${METHODS.contact}](mailto:${METHODS.contact}) and we'll talk about what helps most, such as test devices or a year of signing certificates.` },
  ],
  final: {
    h2: "Thank you for helping teachers",
    p: "Every donation, star and share keeps Rupok free for the next classroom.",
    cta: [{ href: "#give", label: "Donate", kind: "mint" }, { href: "/board/", label: "Open the board" }],
  },
};

// ------------------------------------------------------------------- Bangla
const tBn = {
  inBd: "বাংলাদেশ থেকে",
  abroad: "বিদেশ থেকে",
  nagad: "নগদ",
  walletSub: (name) => `টাকায়, ${name} অ্যাপ থেকে। যেকোনো পরিমাণই কাজে লাগে।`,
  walletStep1: (name) => `${name} অ্যাপে <b>Send Money</b> বেছে নিন।`,
  walletStep2: (amt) => `${amt} পাঠান এই নম্বরে:`,
  refStep: (ref) => `রেফারেন্সে লিখুন <b>${ref}</b>।`,
  anyAmount: "যেকোনো পরিমাণ",
  amountsLabel: { bdt: "টাকায় প্রস্তাবিত পরিমাণ", usd: "মার্কিন ডলারে প্রস্তাবিত পরিমাণ" },
  copy: "কপি",
  copied: "কপি হয়েছে",
  thanks: (mail) => `ধন্যবাদ জানাতে চাইলে আপনার নামসহ [${mail}](mailto:${mail}?subject=Rupok%20donation)-এ ইমেইল করুন। অনুমতি ছাড়া আমরা কারও নাম প্রকাশ করি না।`,
  bankTitle: "মার্কিন ব্যাংক ট্রান্সফার (ACH)",
  bankSub: (min) => `মার্কিন ডলারে, যুক্তরাষ্ট্রের ব্যাংক অ্যাকাউন্ট, Wise বা মার্কিন অ্যাকাউন্টে টাকা পাঠানো যায় এমন যেকোনো অ্যাপ থেকে। সর্বনিম্ন $${min}।`,
  bankSend: (amt) => `${amt} পাঠান এই অ্যাকাউন্টে:`,
  holder: "অ্যাকাউন্টধারী", account: "অ্যাকাউন্ট নম্বর", routing: "রাউটিং নম্বর", type: "অ্যাকাউন্টের ধরন", bank: "ব্যাংক",
  address: "প্রাপকের ঠিকানা", reference: "রেফারেন্স",
  bankFine: (mail) => `ACH-এ ১ থেকে ৩ কার্যদিবস লাগে। ধন্যবাদ বা রসিদ চাইলে পরে [${mail}](mailto:${mail}?subject=Rupok%20donation)-এ ইমেইল করুন।`,
  otherTitle: "কার্ড, PayPal বা GitHub Sponsors?",
  payCard: "কার্ডে দিন",
  cardWord: "কার্ড পেমেন্ট",
  notYet: (missing, mail) =>
    `এখনো নয়: ${missing.join(", ").replace(/, ([^,]*)$/, " ও $1")} চালু হয়নি। বিকাশ বা ব্যাংক ট্রান্সফার সম্ভব না হলে [${mail}](mailto:${mail}?subject=Rupok%20donation)-এ ইমেইল করুন, আপনার সুবিধামতো একটা উপায় বের করব।`,
  mdWallet: (name, num, ref) => `${name} অ্যাপে **Send Money** বেছে নিয়ে **${num}** নম্বরে যেকোনো পরিমাণ পাঠান, রেফারেন্সে লিখুন **${ref}**।`,
  mdAmounts: "প্রস্তাবিত এককালীন পরিমাণ",
  impactTitle: "এখন পর্যন্ত Rupok",
  impactDownloads: "ডেস্কটপ অ্যাপ ডাউনলোড",
  impactStars: "GitHub স্টার",
  impactReleases: "রিলিজ",
  impactSource: `GitHub-এর প্রকাশ্য সংখ্যা থেকে সরাসরি। গোনা, আন্দাজ নয়। [সব রিলিজ দেখুন](${GITHUB}/releases)`,
  impactError: `এই মুহূর্তে GitHub-এ পৌঁছানো যাচ্ছে না। [GitHub-এ সংখ্যাগুলো দেখুন](${GITHUB}/releases)`,
  noSupporters: "তালিকাটি ইচ্ছে করেই ফাঁকা। কেউ চাইলে তবেই আমরা তাঁর নাম লিখে ধন্যবাদ জানাই। আপনার (বা আপনার স্কুলের) নাম এখানে চাইলে অনুদানের পর ইমেইলে জানিয়ে দিন।",
  supportersNote: "শুধু তাঁদের অনুমতি নিয়েই নাম দেওয়া হয়েছে। ধন্যবাদ।",
};

const bn = {
  path: "/bn/donate/",
  lang: "bn",
  alt: { en: "/donate/" },
  title: "Rupok-কে সহায়তা করুন: শিক্ষকদের ফ্রি হোয়াইটবোর্ডে অনুদান | Rupok",
  ogTitle: "শিক্ষকদের ফ্রি হোয়াইটবোর্ড Rupok-কে সহায়তা করুন",
  description:
    "Rupok ফ্রি ও ওপেন সোর্স, কোনো বিজ্ঞাপন বা ট্র্যাকিং নেই। বিকাশ বা মার্কিন ব্যাংক ট্রান্সফারে অনুদান দিয়ে সাইনিং সার্টিফিকেট, সার্ভার, টেস্ট ডিভাইস আর ডেভেলপমেন্টের খরচে সাহায্য করুন, অথবা স্টার, শেয়ার ও অনুবাদ করে পাশে থাকুন।",
  bodyClass: "donate-page",
  crumbs: [["অনুদান"]],
  tag: "Rupok-এর পাশে থাকুন",
  h1: "প্রত্যেক শিক্ষকের জন্য Rupok ফ্রি রাখতে সাহায্য করুন",
  lede: "Rupok ফ্রি আর ওপেন সোর্স; আপনি অনুদান দিন বা না দিন, ফ্রি-ই থাকবে। ফ্রি অ্যাপেরও খরচ আছে: সাইনিং সার্টিফিকেট, সার্ভার, পরীক্ষার ডিভাইস আর বানানোর সময়। আপনার অনুদান সেই খরচ মেটায়।",
  cta: [{ href: "#give", label: "অনুদান দিন", kind: "mint" }, { href: "#help", label: "অন্যভাবে সাহায্য", kind: "ghost" }],
  aside: impactHtml(tBn, "bn"),
  ld: ld("/bn/donate/", "bn"),
  sections: [
    {
      id: "why",
      h2: "কেন Rupok-\u2060এর পাশে থাকবেন", // word joiner: never break after the hyphen
      blocks: [{
        html: `<ul class="why">
          <li><b>সব শিক্ষকের জন্য ফ্রি</b><span>সব ফিচার, সব ডিভাইসে, অ্যাকাউন্ট ছাড়াই। কোনো ট্রায়াল নেই, অ্যাপের পেইড সংস্করণও নেই।</span></li>
          <li><b>ওপেন সোর্স</b><span>কোড GitHub-এ, AGPL লাইসেন্সে। যে কেউ পড়তে, ঠিক করতে বা এগিয়ে নিতে পারেন।</span></li>
          <li><b>বাংলাদেশে তৈরি</b><span>ঢাকায় Shaitrish বানাচ্ছে, এ দেশের ও সারা বিশ্বের ক্লাসরুমের জন্য, শুরু থেকেই বাংলার কথা মাথায় রেখে।</span></li>
          <li><b>বিজ্ঞাপন নেই, ট্র্যাকিং নেই</b><span>কোনো অ্যানালিটিক্স বা ট্র্যাকিং কুকি নেই, কিছুই বিক্রি হয় না। এই পেজও আপনাকে ট্র্যাক করে না।</span></li>
        </ul>`,
        md: "- **সব শিক্ষকের জন্য ফ্রি:** সব ফিচার, সব ডিভাইসে, অ্যাকাউন্ট ছাড়াই।\n- **ওপেন সোর্স:** কোড GitHub-এ, AGPL লাইসেন্সে।\n- **বাংলাদেশে তৈরি:** ঢাকায় Shaitrish বানাচ্ছে, শুরু থেকেই বাংলার কথা মাথায় রেখে।\n- **বিজ্ঞাপন নেই, ট্র্যাকিং নেই:** কোনো অ্যানালিটিক্স বা ট্র্যাকিং কুকি নেই।",
      }],
    },
    {
      id: "costs",
      h2: "আপনার অনুদান কোথায় যায়",
      lede: "বিজ্ঞাপন বা মার্কেটিংয়ে এক টাকাও যায় না। খরচ হয় এসবে:",
      blocks: [{
        html: `<ul class="costs">
          <li><span class="cost-tag">বছরে $৯৯</span><b>সাইনিং সার্টিফিকেট</b><span>Apple-এর ডেভেলপার মেম্বারশিপ আর Windows-এর কোড-সাইনিং সার্টিফিকেট, যাতে ইনস্টলের সময় Mac ও Windows আর সতর্কবার্তা না দেখায় (<a href="/bn/download/#unsigned">কেন সতর্কবার্তা আসে</a>)।</span></li>
          <li><span class="cost-tag">প্রতি মাসে</span><b>সার্ভার</b><span>rupok.app, ডাউনলোড, আর প্রত্যেক শিক্ষকের জন্য আসছে যে ফ্রি Rupok Cloud প্ল্যান।</span></li>
          <li><span class="cost-tag">মাঝে মাঝে</span><b>পরীক্ষার ডিভাইস</b><span>ইন্টারঅ্যাক্টিভ প্যানেল, পেন ট্যাবলেট, একটি Mac আর কম দামের Android ট্যাবলেট, যাতে স্কুলে আসলে যে বোর্ড আছে তাতেই Rupok চলে।</span></li>
          <li><span class="cost-tag">প্রতিদিন</span><b>ডেভেলপমেন্টের সময়</b><span>বাগ ঠিক করা, নতুন ফিচার, বাংলা ইন্টারফেস আর শিক্ষকদের প্রশ্নের উত্তর দেওয়া।</span></li>
        </ul>`,
        md: "- **সাইনিং সার্টিফিকেট:** Apple-এর ডেভেলপার মেম্বারশিপ (বছরে $৯৯) আর Windows-এর কোড-সাইনিং সার্টিফিকেট।\n- **সার্ভার:** rupok.app, ডাউনলোড আর ফ্রি Rupok Cloud প্ল্যান।\n- **পরীক্ষার ডিভাইস:** ইন্টারঅ্যাক্টিভ প্যানেল, পেন ট্যাবলেট, Mac ও Android ট্যাবলেট।\n- **ডেভেলপমেন্টের সময়:** বাগ ঠিক করা, নতুন ফিচার, বাংলা ইন্টারফেস।",
      }],
    },
    {
      id: "give",
      band: "mint",
      h2: "এককালীন অনুদান দিন",
      lede: "আপনার সুবিধামতো উপায় বেছে নিন। প্রতিটি টাকা, প্রতিটি ডলার Rupok-এর কাজেই লাগে।",
      blocks: [{ html: giveHtml(tBn, "bn"), md: giveMd(tBn, "bn") }],
    },
    {
      id: "help",
      h2: "টাকা ছাড়াও সাহায্য করা যায়",
      lede: "এগুলোও সমান কাজে লাগে।",
      blocks: [{ links: [
        { href: GITHUB, b: "GitHub-এ স্টার দিন", t: "এতে অন্য শিক্ষক ও ডেভেলপাররা Rupok খুঁজে পান।" },
        { href: "https://wa.me/?text=" + encodeURIComponent("Rupok: শিক্ষকদের জন্য ফ্রি হোয়াইটবোর্ড। https://rupok.app/bn/"), b: "শিক্ষক বন্ধুদের জানান", t: "স্কুলের WhatsApp গ্রুপে পাঠান বা সহকর্মীকে দেখান।" },
        { href: `${GITHUB}/issues`, b: "বাগ বা নতুন ফিচারের কথা জানান", t: "কী কাজ করছে না বা আপনার ক্লাসে কী দরকার, লিখে জানান।" },
        { href: `${GITHUB}/blob/main/CONTRIBUTING.md`, b: "অনুবাদে হাত লাগান", t: "বাংলা ইন্টারফেস আর অন্য ভাষাগুলো সব ক্লাসরুমে পৌঁছে দিতে সাহায্য করুন।" },
        { href: `${GITHUB}/blob/main/CONTRIBUTING.md`, b: "কোড বা লেসন দিয়ে অবদান রাখুন", t: "পুল রিকোয়েস্ট পাঠান, অথবা অন্য শিক্ষকদের কাজে লাগবে এমন Smart Object বানান।" },
      ] }],
    },
    {
      id: "thanks",
      h2: "ধন্যবাদ",
      blocks: [{ html: supportersHtml(tBn), md: SUPPORTERS.length ? SUPPORTERS.map((s) => `- ${s.name}`).join("\n") : tBn.noSupporters }],
    },
  ],
  faqTitle: "অনুদান নিয়ে প্রশ্ন",
  faq: [
    { q: "অনুদান না দিলেও কি Rupok ফ্রি থাকবে?", a: "হ্যাঁ, পুরোপুরি। সবার জন্য সব ফিচার একই রকম কাজ করে। অনুদানে নতুন কিছু খোলে না; এতে আমরা কাজ চালিয়ে যেতে পারি।" },
    { q: "অনুদান আর Rupok Cloud-এর পার্থক্য কী?", a: "অনুদান হলো প্রজেক্টের জন্য উপহার। Rupok Cloud একটি ঐচ্ছিক অনলাইন সেবা (সিঙ্ক, শেয়ার, লাইভ ক্লাস): প্রত্যেক শিক্ষকের জন্য ফ্রি প্ল্যান থাকবে, বেশি জায়গার জন্য পেইড প্ল্যান। অ্যাপটি দুই ক্ষেত্রেই ফ্রি।" },
    { q: "কার্ড, PayPal বা GitHub Sponsors দিয়ে দেওয়া যায়?", a: `এখনো নয়। আপাতত বিকাশ বা মার্কিন ব্যাংক ট্রান্সফারে (ACH, Wise থেকেও) দেওয়া যায়। এগুলো সম্ভব না হলে [${METHODS.contact}](mailto:${METHODS.contact})-এ ইমেইল করুন, উপায় বের করব।` },
    { q: "প্রতি মাসে দেওয়া যায়?", a: "সাবস্ক্রিপশন নেই। নিয়মিত দিতে চাইলে ব্যাংক বা বিকাশ অ্যাপে রিপিট ট্রান্সফার সেট করুন, রেফারেন্সে Rupok লিখে।" },
    { q: "আমার নাম কি এই পেজে দেখাবে?", a: "শুধু আপনি চাইলে। আমরা নিজে থেকে কারও নাম দিই না। ধন্যবাদ তালিকায় আপনার বা আপনার স্কুলের নাম চাইলে অনুদানের পর ইমেইল করুন।" },
    { q: "অনুদান দিলে কি আমার তথ্য যায় বা ট্র্যাক করা হয়?", a: "না। এই পেজে কোনো অ্যানালিটিক্স বা পেমেন্ট স্ক্রিপ্ট নেই। ট্রান্সফারে ব্যাংক বা বিকাশ যা দেখায় (সাধারণত নাম বা নম্বর) শুধু সেটুকুই আমরা দেখি, আর তা কেবল ধন্যবাদ জানাতেই ব্যবহার করি।" },
    { q: "ওপরের সংখ্যাগুলো কোথা থেকে আসে?", a: "GitHub-এর প্রকাশ্য API থেকে সরাসরি: সব ডেস্কটপ রিলিজের মোট ডাউনলোড, রিপোজিটরির স্টার আর রিলিজের সংখ্যা। GitHub-এ পৌঁছানো না গেলে পেজটি আন্দাজ না করে GitHub-এর লিংক দেখায়।" },
    { q: "আমার স্কুল বা প্রতিষ্ঠান কি স্পনসর করতে পারে?", a: `অবশ্যই, ধন্যবাদ। [${METHODS.contact}](mailto:${METHODS.contact})-এ ইমেইল করুন; পরীক্ষার ডিভাইস বা এক বছরের সাইনিং সার্টিফিকেটের মতো যা সবচেয়ে কাজে লাগে, তা নিয়ে কথা বলব।` },
  ],
  final: {
    h2: "শিক্ষকদের পাশে থাকার জন্য ধন্যবাদ",
    p: "প্রতিটি অনুদান, স্টার আর শেয়ার পরের ক্লাসরুমের জন্য Rupok ফ্রি রাখে।",
    cta: [{ href: "#give", label: "অনুদান দিন", kind: "mint" }, { href: "/board/", label: "বোর্ড খুলুন" }],
  },
};

export const DONATE = [en, bn];
