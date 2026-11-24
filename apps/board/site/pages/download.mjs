// The download page: the desktop app for Windows, macOS and Linux, and
// the web app for phones, tablets and Chromebooks. Static links point at the
// latest GitHub release page; src/download.ts swaps in direct file links,
// versions and sizes from the GitHub API and marks the visitor's platform.

import { DL_ICONS, RELEASES_URL } from "../icons.mjs";
import { SITE } from "../lib.mjs";

const card = (os, title, body) =>
  `<article class="dl-card" data-os-card="${os}"><span class="dl-ico">${DL_ICONS[os]}</span><h3>${title}</h3>${body}</article>`;
const btn = (asset, label, kind = "dark") => `<a class="pill ${kind}" data-asset="${asset}" href="${RELEASES_URL}">${label}</a>`;
const alt = (asset, label) => `<a class="dl-link" data-asset="${asset}" href="${RELEASES_URL}">${label}</a>`;
const metaOf = (asset) => `<span class="dl-size" data-meta="${asset}"></span>`;

function grid(t) {
  return `<div class="dl-grid">
        ${card("windows", "Windows", `<p>${t.win}</p><div class="dl-acts">${btn("exe", t.exe)}${alt("msi", t.msi)}</div>${metaOf("exe")}`)}
        ${card("linux", "Linux", `<p>${t.linux}</p><div class="dl-acts">${btn("deb", t.deb)}${alt("appimage", t.appimage)}</div>${metaOf("deb")}`)}
        ${card("mac", "macOS", `<p>${t.mac}</p><div class="dl-acts">${btn("dmg", t.dmg)}</div>${metaOf("dmg")}`)}
        ${card("mobile", t.mobileTitle, `<p>${t.mobile}</p><div class="dl-acts"><a class="pill dark" href="/board/">${t.open}</a><a class="pill ghost" href="#install" data-install>${t.install}</a></div>`)}
      </div>`;
}

/** Shown by src/download.ts once a download button is clicked. */
const thanks = (t) => `<div class="dl-thanks" role="status" data-dl-thanks hidden>
          <button type="button" class="dl-thanks-x" data-dl-thanks-close aria-label="${t.close}">×</button>
          <b>${t.title}</b>
          <p>${t.p}</p>
          <div class="dl-thanks-acts"><a class="pill mint" href="${t.href}">${t.donate}</a><a class="dl-link" href="#unsigned">${t.help}</a></div>
        </div>`;

const ld = (lang, path) => [
  {
    "@type": "SoftwareApplication",
    "@id": `${SITE}${path}#download`,
    name: "Rupok",
    applicationCategory: "EducationalApplication",
    operatingSystem: "Windows 10, Windows 11, macOS 11 or later (Apple silicon and Intel), Linux (Debian, Ubuntu and AppImage); any OS with Chrome, Edge or Safari",
    downloadUrl: RELEASES_URL,
    softwareVersion: "0.2.2",
    releaseNotes: RELEASES_URL,
    fileSize: "Varies by platform",
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    isAccessibleForFree: true,
    license: "https://www.gnu.org/licenses/agpl-3.0.html",
    inLanguage: lang,
    isPartOf: { "@id": `${SITE}/#app` },
  },
];

// ----------------------------------------------------------------- English
const tEn = {
  win: "Windows 10 and 11, 64-bit. The installer adds Rupok to the Start menu.",
  exe: "Download .exe", msi: "or .msi",
  linux: "Ubuntu, Debian, Mint and others, 64-bit. Or the AppImage for any distribution.",
  deb: "Download .deb", appimage: "or AppImage",
  mac: "macOS 11 Big Sur or later, on Apple silicon and Intel Macs (one universal app).", dmg: "Download .dmg", open: "Open the board",
  mobileTitle: "Phones, tablets, Chromebooks", mobile: "No installer needed: Rupok runs in the browser. Install it as an app to open it from your home screen.",
  install: "Install as an app",
};

const en = {
  path: "/download/",
  lang: "en",
  alt: { bn: "/bn/download/" },
  title: "Download Rupok: Free Whiteboard for Windows, Mac and Linux | Rupok",
  ogTitle: "Download Rupok for Windows, Mac and Linux",
  description: "Download Rupok, the free, open-source whiteboard for teachers, for Windows (.exe, .msi), macOS (.dmg) and Linux (.deb, AppImage). Phones and tablets use it in the browser.",
  bodyClass: "download-page",
  crumbs: [["Download"]],
  h1: "Download Rupok",
  lede: "The free whiteboard for teachers, as a desktop app for Windows, Mac and Linux. Or open it in Chrome, Edge or Safari on any device: same board, no install.",
  aside: `<div class="dl dl-hero" data-download="page"><a class="pill mint lg" href="${RELEASES_URL}">Download the latest release</a></div>
        ${thanks({ title: "Thanks for downloading Rupok!", p: "Rupok is free. If it helps your class, consider supporting it.", donate: "Support Rupok ❤️", href: "/donate/", help: "Install help", close: "Close" })}`,
  fine: "Rupok is free. If it helps your class, consider [supporting it](/donate/).",
  ld: ld("en", "/download/"),
  sections: [
    { id: "all", h2: "Every platform", blocks: [{ html: grid(tEn) }, { html: `<p class="fine"><a data-notes href="${RELEASES_URL}" rel="noopener">Release notes and every file</a> on GitHub.</p>` }] },
    {
      id: "install",
      h2: "Install as an app",
      lede: "On a phone, tablet or Chromebook, the web app works like an installed one: its own window, on the home screen. After the first visit it also starts without internet: the board, your lessons and the student viewer work offline.",
      blocks: [{ cols: [
        [{ h3: "Android, Chromebook, Windows" }, "Open [rupok.app/board](/board/) in Chrome, Edge or Safari, open the browser menu and choose **Install app** (or **Add to Home screen**)."],
        [{ h3: "iPhone and iPad" }, "Open [rupok.app/board](/board/) in Safari, tap **Share**, then **Add to Home Screen**."],
      ] }],
    },
    {
      id: "unsigned",
      h2: "A note on the warnings",
      blocks: [
        "The installers aren’t code-signed yet, so your computer may ask once before it opens Rupok. That’s expected; signed installers are coming soon.",
        { ul: [
          "**Windows:** if SmartScreen says “Windows protected your PC”, choose **More info**, then **Run anyway**.",
          "**Mac:** right-click Rupok in Applications, choose **Open**, then **Open** again. Or open **System Settings → Privacy & Security** and choose **Open Anyway**.",
        ] },
      ],
    },
    {
      id: "requirements",
      h2: "System requirements",
      blocks: [{ dl: [
        ["Windows", "Windows 10 or 11, 64-bit"],
        ["macOS", "macOS 11 Big Sur or later, Apple silicon or Intel"],
        ["Linux", "64-bit; .deb for Debian and Ubuntu, AppImage for other distributions"],
        ["Browser", "A recent Chrome, Edge or Safari (any OS), with WebGL2 or WebGPU"],
        ["Hardware", "Any smartboard, interactive panel, laptop or tablet; a pen is optional"],
        ["Account", "None"],
      ] }],
    },
  ],
  final: { h2: "Rather not install anything?", p: "Rupok runs in your browser too, with every feature.", cta: [{ href: "/board/", label: "Open the board" }] },
};

// ------------------------------------------------------------------- Bangla
const tBn = {
  win: "Windows 10 ও 11, 64-বিট। ইনস্টলার Rupok-কে Start মেনুতে যোগ করে।",
  exe: ".exe ডাউনলোড", msi: "অথবা .msi",
  linux: "Ubuntu, Debian, Mint সহ অন্যান্য, 64-বিট। যেকোনো ডিস্ট্রোর জন্য AppImage।",
  deb: ".deb ডাউনলোড", appimage: "অথবা AppImage",
  mac: "macOS 11 Big Sur বা নতুন, Apple silicon ও Intel Mac-এ (একটাই ইউনিভার্সাল অ্যাপ)।", dmg: ".dmg ডাউনলোড", open: "বোর্ড খুলুন",
  mobileTitle: "ফোন, ট্যাবলেট, Chromebook", mobile: "ইনস্টলার লাগবে না: Rupok ব্রাউজারেই চলে। হোম স্ক্রিন থেকে খুলতে অ্যাপ হিসেবে ইনস্টল করুন।",
  install: "অ্যাপ হিসেবে ইনস্টল করুন",
};

const bn = {
  path: "/bn/download/",
  lang: "bn",
  alt: { en: "/download/" },
  title: "Rupok ডাউনলোড: Windows, Mac ও Linux-এর জন্য ফ্রি হোয়াইটবোর্ড | Rupok",
  ogTitle: "Windows, Mac ও Linux-এর জন্য Rupok ডাউনলোড করুন",
  description: "শিক্ষকদের ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড Rupok ডাউনলোড করুন: Windows (.exe, .msi), macOS (.dmg) ও Linux (.deb, AppImage)। ফোন ও ট্যাবলেটে ব্রাউজারেই চলে।",
  bodyClass: "download-page",
  crumbs: [["ডাউনলোড"]],
  h1: "Rupok ডাউনলোড করুন",
  lede: "শিক্ষকদের ফ্রি হোয়াইটবোর্ড, Windows, Mac ও Linux-এর ডেস্কটপ অ্যাপ হিসেবে। অথবা যেকোনো ডিভাইসে Chrome, Edge বা Safari-তে খুলুন: একই বোর্ড, ইনস্টল ছাড়াই।",
  aside: `<div class="dl dl-hero" data-download="page"><a class="pill mint lg" href="${RELEASES_URL}">সর্বশেষ রিলিজ ডাউনলোড করুন</a></div>
        ${thanks({ title: "Rupok ডাউনলোড করার জন্য ধন্যবাদ!", p: "Rupok ফ্রি। আপনার ক্লাসে কাজে লাগলে Rupok-এর পাশে থাকার কথা ভাবুন।", donate: "Rupok-কে সহায়তা করুন ❤️", href: "/bn/donate/", help: "ইনস্টলে সাহায্য", close: "বন্ধ করুন" })}`,
  fine: "Rupok ফ্রি। আপনার ক্লাসে কাজে লাগলে [Rupok-এর পাশে থাকুন](/bn/donate/)।",
  ld: ld("bn", "/bn/download/"),
  sections: [
    { id: "all", h2: "সব প্ল্যাটফর্ম", blocks: [{ html: grid(tBn) }, { html: `<p class="fine">GitHub-এ <a data-notes href="${RELEASES_URL}" rel="noopener">রিলিজ নোট ও সব ফাইল</a>।</p>` }] },
    {
      id: "install",
      h2: "অ্যাপ হিসেবে ইনস্টল করুন",
      lede: "ফোন, ট্যাবলেট বা Chromebook-এ ওয়েব অ্যাপ ইনস্টল করা অ্যাপের মতোই চলে: নিজের উইন্ডো, হোম স্ক্রিনে। প্রথমবার খোলার পর ইন্টারনেট ছাড়াও চালু হয়: বোর্ড, আপনার লেসন আর স্টুডেন্ট ভিউয়ার অফলাইনে চলে।",
      blocks: [{ cols: [
        [{ h3: "Android, Chromebook, Windows" }, "Chrome, Edge বা Safari-তে [rupok.app/board](/board/) খুলুন, ব্রাউজারের মেনু থেকে **Install app** (বা **Add to Home screen**) বেছে নিন।"],
        [{ h3: "iPhone ও iPad" }, "Safari-তে [rupok.app/board](/board/) খুলুন, **Share** চাপুন, তারপর **Add to Home Screen**।"],
      ] }],
    },
    {
      id: "unsigned",
      h2: "সতর্কবার্তা নিয়ে একটি কথা",
      blocks: [
        "ইনস্টলারগুলো এখনো কোড-সাইন করা নয়, তাই প্রথমবার কম্পিউটার একবার জিজ্ঞেস করতে পারে। এটা স্বাভাবিক; সাইন করা ইনস্টলার শিগগিরই আসছে।",
        { ul: [
          "**Windows:** SmartScreen “Windows protected your PC” দেখালে **More info**, তারপর **Run anyway** বেছে নিন।",
          "**Mac:** Applications-এ Rupok-এ রাইট-ক্লিক করে **Open**, তারপর আবার **Open**। অথবা **System Settings → Privacy & Security** খুলে **Open Anyway** বেছে নিন।",
        ] },
      ],
    },
    {
      id: "requirements",
      h2: "সিস্টেম রিকোয়ারমেন্ট",
      blocks: [{ dl: [
        ["Windows", "Windows 10 বা 11, 64-বিট"],
        ["macOS", "macOS 11 Big Sur বা নতুন, Apple silicon বা Intel"],
        ["Linux", "64-বিট; Debian ও Ubuntu-র জন্য .deb, অন্য ডিস্ট্রোর জন্য AppImage"],
        ["ব্রাউজার", "সাম্প্রতিক Chrome, Edge বা Safari (যেকোনো OS), WebGL2 বা WebGPU সহ"],
        ["হার্ডওয়্যার", "যেকোনো স্মার্ট বোর্ড, ইন্টারঅ্যাক্টিভ প্যানেল, ল্যাপটপ বা ট্যাবলেট; পেন ঐচ্ছিক"],
        ["অ্যাকাউন্ট", "লাগবে না"],
      ] }],
    },
  ],
  final: { h2: "কিছু ইনস্টল করতে চান না?", p: "Rupok ব্রাউজারেও চলে, সব ফিচারসহ।", cta: [{ href: "/board/", label: "বোর্ড খুলুন" }] },
};

export const DOWNLOAD = [en, bn];
