// The download page: the desktop app for Windows and Linux (Mac coming), and
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
        ${card("mac", "macOS", `<p data-mac-soon>${t.macSoon}</p><div class="dl-acts" data-mac-soon><a class="pill dark" href="/board/">${t.open}</a></div><div class="dl-acts" data-mac-ready hidden>${btn("dmg", t.dmg)}</div>`)}
        ${card("mobile", t.mobileTitle, `<p>${t.mobile}</p><div class="dl-acts"><a class="pill dark" href="/board/">${t.open}</a><a class="pill ghost" href="#install" data-install>${t.install}</a></div>`)}
      </div>`;
}

const ld = (lang, path) => [
  {
    "@type": "SoftwareApplication",
    "@id": `${SITE}${path}#download`,
    name: "Rupok",
    applicationCategory: "EducationalApplication",
    operatingSystem: "Windows 10, Windows 11, Linux (Debian, Ubuntu and AppImage); macOS coming soon; any OS with Chrome or Edge",
    downloadUrl: RELEASES_URL,
    softwareVersion: "0.1.0",
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
  macSoon: "Mac app coming soon. Use Rupok in Chrome, Safari or Edge today.", dmg: "Download .dmg", open: "Open the board",
  mobileTitle: "Phones, tablets, Chromebooks", mobile: "No installer needed: Rupok runs in the browser. Install it as an app to open it from your home screen.",
  install: "Install as an app",
};

const en = {
  path: "/download/",
  lang: "en",
  alt: { bn: "/bn/download/" },
  title: "Download Rupok: Free Whiteboard for Windows and Linux | Rupok",
  ogTitle: "Download Rupok for Windows and Linux",
  description: "Download Rupok, the free, open-source whiteboard for teachers, for Windows (.exe, .msi) and Linux (.deb, AppImage). Mac is coming; phones and tablets use it in the browser.",
  bodyClass: "download-page",
  crumbs: [["Download"]],
  h1: "Download Rupok",
  lede: "The free whiteboard for teachers, as a desktop app for Windows and Linux. Or open it in Chrome or Edge on any device: same board, no install.",
  aside: `<div class="dl dl-hero" data-download="page"><a class="pill mint lg" href="${RELEASES_URL}">Download the latest release</a></div>`,
  ld: ld("en", "/download/"),
  sections: [
    { id: "all", h2: "Every platform", blocks: [{ html: grid(tEn) }, { html: `<p class="fine"><a data-notes href="${RELEASES_URL}" rel="noopener">Release notes and every file</a> on GitHub.</p>` }] },
    {
      id: "install",
      h2: "Install as an app",
      lede: "On a phone, tablet or Chromebook, the web app works like an installed one: its own window, offline, on the home screen.",
      blocks: [{ cols: [
        [{ h3: "Android, Chromebook, Windows" }, "Open [rupok.app/board](/board/) in Chrome or Edge, open the browser menu and choose **Install app** (or **Add to Home screen**)."],
        [{ h3: "iPhone and iPad" }, "Open [rupok.app/board](/board/) in Safari, tap **Share**, then **Add to Home Screen**."],
      ] }],
    },
    {
      id: "unsigned",
      h2: "A note on the warnings",
      blocks: [
        "The installers aren’t code-signed yet, so your computer may ask once before it opens Rupok. That’s expected; signing is on the roadmap.",
        { ul: [
          "**Windows:** if SmartScreen says “Windows protected your PC”, choose **More info**, then **Run anyway**.",
          "**Mac** (when the app ships): right-click the app, choose **Open**, then **Open** again.",
        ] },
      ],
    },
    {
      id: "requirements",
      h2: "System requirements",
      blocks: [{ dl: [
        ["Windows", "Windows 10 or 11, 64-bit"],
        ["Linux", "64-bit; .deb for Debian and Ubuntu, AppImage for other distributions"],
        ["Browser", "A recent Chrome or Edge (any OS), with WebGL2 or WebGPU"],
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
  macSoon: "Mac অ্যাপ শিগগিরই আসছে। আজই Chrome, Safari বা Edge-এ Rupok ব্যবহার করুন।", dmg: ".dmg ডাউনলোড", open: "বোর্ড খুলুন",
  mobileTitle: "ফোন, ট্যাবলেট, Chromebook", mobile: "ইনস্টলার লাগবে না: Rupok ব্রাউজারেই চলে। হোম স্ক্রিন থেকে খুলতে অ্যাপ হিসেবে ইনস্টল করুন।",
  install: "অ্যাপ হিসেবে ইনস্টল করুন",
};

const bn = {
  path: "/bn/download/",
  lang: "bn",
  alt: { en: "/download/" },
  title: "Rupok ডাউনলোড: Windows ও Linux-এর জন্য ফ্রি হোয়াইটবোর্ড | Rupok",
  ogTitle: "Windows ও Linux-এর জন্য Rupok ডাউনলোড করুন",
  description: "শিক্ষকদের ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড Rupok ডাউনলোড করুন: Windows (.exe, .msi) ও Linux (.deb, AppImage)। Mac আসছে; ফোন ও ট্যাবলেটে ব্রাউজারেই চলে।",
  bodyClass: "download-page",
  crumbs: [["ডাউনলোড"]],
  h1: "Rupok ডাউনলোড করুন",
  lede: "শিক্ষকদের ফ্রি হোয়াইটবোর্ড, Windows ও Linux-এর ডেস্কটপ অ্যাপ হিসেবে। অথবা যেকোনো ডিভাইসে Chrome বা Edge-এ খুলুন: একই বোর্ড, ইনস্টল ছাড়াই।",
  aside: `<div class="dl dl-hero" data-download="page"><a class="pill mint lg" href="${RELEASES_URL}">সর্বশেষ রিলিজ ডাউনলোড করুন</a></div>`,
  ld: ld("bn", "/bn/download/"),
  sections: [
    { id: "all", h2: "সব প্ল্যাটফর্ম", blocks: [{ html: grid(tBn) }, { html: `<p class="fine">GitHub-এ <a data-notes href="${RELEASES_URL}" rel="noopener">রিলিজ নোট ও সব ফাইল</a>।</p>` }] },
    {
      id: "install",
      h2: "অ্যাপ হিসেবে ইনস্টল করুন",
      lede: "ফোন, ট্যাবলেট বা Chromebook-এ ওয়েব অ্যাপ ইনস্টল করা অ্যাপের মতোই চলে: নিজের উইন্ডো, অফলাইন, হোম স্ক্রিনে।",
      blocks: [{ cols: [
        [{ h3: "Android, Chromebook, Windows" }, "Chrome বা Edge-এ [rupok.app/board](/board/) খুলুন, ব্রাউজারের মেনু থেকে **Install app** (বা **Add to Home screen**) বেছে নিন।"],
        [{ h3: "iPhone ও iPad" }, "Safari-তে [rupok.app/board](/board/) খুলুন, **Share** চাপুন, তারপর **Add to Home Screen**।"],
      ] }],
    },
    {
      id: "unsigned",
      h2: "সতর্কবার্তা নিয়ে একটি কথা",
      blocks: [
        "ইনস্টলারগুলো এখনো কোড-সাইন করা নয়, তাই প্রথমবার কম্পিউটার একবার জিজ্ঞেস করতে পারে। এটা স্বাভাবিক; সাইনিং রোডম্যাপে আছে।",
        { ul: [
          "**Windows:** SmartScreen “Windows protected your PC” দেখালে **More info**, তারপর **Run anyway** বেছে নিন।",
          "**Mac** (অ্যাপ এলে): অ্যাপে রাইট-ক্লিক করে **Open**, তারপর আবার **Open**।",
        ] },
      ],
    },
    {
      id: "requirements",
      h2: "সিস্টেম রিকোয়ারমেন্ট",
      blocks: [{ dl: [
        ["Windows", "Windows 10 বা 11, 64-বিট"],
        ["Linux", "64-বিট; Debian ও Ubuntu-র জন্য .deb, অন্য ডিস্ট্রোর জন্য AppImage"],
        ["ব্রাউজার", "সাম্প্রতিক Chrome বা Edge (যেকোনো OS), WebGL2 বা WebGPU সহ"],
        ["হার্ডওয়্যার", "যেকোনো স্মার্ট বোর্ড, ইন্টারঅ্যাক্টিভ প্যানেল, ল্যাপটপ বা ট্যাবলেট; পেন ঐচ্ছিক"],
        ["অ্যাকাউন্ট", "লাগবে না"],
      ] }],
    },
  ],
  final: { h2: "কিছু ইনস্টল করতে চান না?", p: "Rupok ব্রাউজারেও চলে, সব ফিচারসহ।", cta: [{ href: "/board/", label: "বোর্ড খুলুন" }] },
};

export const DOWNLOAD = [en, bn];
