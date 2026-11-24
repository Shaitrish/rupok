// Rendering for the static marketing pages: one content source per page
// (site/pages/*.mjs) becomes an HTML page, a Markdown twin for AI agents, a
// sitemap entry and JSON-LD. No dependencies; run `node site/build.mjs`.

export const SITE = "https://rupok.app";
export const GITHUB = "https://github.com/Shaitrish/rupok";
/** Default "last updated" date for pages and facts. */
export const UPDATED = "2026-11-23";

export const esc = (s) =>
  String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

const abs = (u) => (u.startsWith("/") ? SITE + u : u);
const external = (u) => /^https?:/.test(u) && !u.startsWith(SITE);

// ------------------------------------------------------------ inline text
// A tiny Markdown subset shared by HTML and Markdown output:
// **bold**, `code`, [text](url), ^3 (footnote to source 3).

export function inline(s) {
  let out = esc(s);
  out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
  out = out.replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>");
  out = out.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href="${u}"${external(u) ? ' rel="noopener"' : ""}>${t}</a>`);
  out = out.replace(/(?:\^\d+)+/g, (m) => {
    const ids = m.split("^").filter(Boolean);
    return `<sup>${ids.map((id) => `<a href="#src-${id}" aria-label="Source ${id}">${id}</a>`).join(",")}</sup>`;
  });
  return out;
}

export function md(s) {
  return String(s)
    .replace(/\[([^\]]+)\]\((\/[^)\s]*)\)/g, (_, t, u) => `[${t}](${abs(u)})`)
    .replace(/\^(\d+)/g, "[^$1]");
}

export function plain(s) {
  return String(s)
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\[([^\]]+)\]\([^)\s]+\)/g, "$1")
    .replace(/\s?\^\d+/g, "");
}

// Table cells may start with a verdict: "yes:", "no:", "part:", "soon:".
const MARKS = { yes: "✓", no: "✗", part: "◐", soon: "◷" };
function cell(s, tag, attrs = "") {
  const m = /^(yes|no|part|soon):\s*/.exec(s);
  if (!m) return `<${tag}${attrs}>${inline(s)}</${tag}>`;
  return `<${tag}${attrs} class="v-${m[1]}"><span class="mk" aria-hidden="true">${MARKS[m[1]]}</span> ${inline(s.slice(m[0].length))}</${tag}>`;
}
const mdCell = (s) => md(s.replace(/^(yes|no|part|soon):\s*/, (_, k) => MARKS[k] + " ")).replace(/\|/g, "\\|");

// ------------------------------------------------------------------ blocks

export function blockHtml(b) {
  if (typeof b === "string") return `<p>${inline(b)}</p>`;
  if (b.h3) return `<h3>${inline(b.h3)}</h3>`;
  if (b.ul) return `<ul class="list">${b.ul.map((x) => `<li>${inline(x)}</li>`).join("")}</ul>`;
  if (b.ticks) return `<ul class="list ticks">${b.ticks.map((x) => `<li>${inline(x)}</li>`).join("")}</ul>`;
  if (b.steps)
    return `<ol class="steps">${b.steps.map((s) => `<li><b>${inline(s.b)}</b><span>${inline(s.t)}</span></li>`).join("")}</ol>`;
  if (b.cards)
    return `<ul class="cards row">${b.cards.map((c) => `<li><b>${inline(c.b)}</b><span>${inline(c.t)}</span></li>`).join("")}</ul>`;
  if (b.links)
    return `<ul class="linkcards">${b.links
      .map((l) => `<li><a href="${l.href}"><b>${inline(l.b)}</b><span>${inline(l.t)}</span><i aria-hidden="true">→</i></a></li>`)
      .join("")}</ul>`;
  if (b.dl)
    return `<dl class="kv">${b.dl.map(([k, v]) => `<div><dt>${inline(k)}</dt><dd>${inline(v)}</dd></div>`).join("")}</dl>`;
  if (b.table) {
    const t = b.table;
    const head = `<thead><tr>${t.head.map((h) => `<th scope="col">${inline(h)}</th>`).join("")}</tr></thead>`;
    const rows = t.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 ? cell(c, "th", ' scope="row"') : cell(c, "td"))).join("")}</tr>`).join("");
    return `<div class="tbl${t.wide ? " wide" : ""}${t.hl ? ` hl-${t.hl}` : ""}" role="region" aria-label="${esc(plain(t.caption))}" tabindex="0"><table><caption>${inline(t.caption)}</caption>${head}<tbody>${rows}</tbody></table></div>`;
  }
  if (b.code) return `<pre class="code"><code>${esc(b.code)}</code></pre>`;
  if (b.cta) return `<div class="ctas">${b.cta.map(pill).join("")}</div>`;
  if (b.note) return `<p class="fine">${inline(b.note)}</p>`;
  if (b.cols) return `<div class="cols">${b.cols.map((c) => `<div>${c.map(blockHtml).join("")}</div>`).join("")}</div>`;
  if (b.html) return b.html;
  throw new Error(`unknown block ${JSON.stringify(b).slice(0, 80)}`);
}

export function blockMd(b) {
  if (typeof b === "string") return md(b);
  if (b.h3) return `### ${md(b.h3)}`;
  if (b.ul || b.ticks) return (b.ul || b.ticks).map((x) => `- ${md(x)}`).join("\n");
  if (b.steps) return b.steps.map((s, i) => `${i + 1}. **${md(s.b)}** ${md(s.t)}`).join("\n");
  if (b.cards) return b.cards.map((c) => `- **${md(c.b)}**: ${md(c.t)}`).join("\n");
  if (b.links) return b.links.map((l) => `- [${md(l.b)}](${abs(l.href)}): ${md(l.t)}`).join("\n");
  if (b.dl) return b.dl.map(([k, v]) => `- **${md(k)}**: ${md(v)}`).join("\n");
  if (b.table) {
    const t = b.table;
    return [`*${md(t.caption)}*`, "", `| ${t.head.map(mdCell).join(" | ")} |`, `|${t.head.map(() => "---").join("|")}|`, ...t.rows.map((r) => `| ${r.map(mdCell).join(" | ")} |`)].join("\n");
  }
  if (b.code) return "```" + (b.lang ?? "sh") + "\n" + b.code + "\n```";
  if (b.cta) return b.cta.map((c) => `[${c.label}](${abs(c.href)})`).join(" · ");
  if (b.note) return `*${md(b.note)}*`;
  if (b.cols) return b.cols.map((c) => c.map(blockMd).join("\n\n")).join("\n\n");
  if (b.html) return b.md ?? "";
  throw new Error("unknown block");
}

export function pill(c) {
  const ext = external(c.href) ? ' rel="noopener"' : "";
  return `<a class="pill ${c.kind ?? "dark"} lg" href="${c.href}"${ext}>${inline(c.label)}</a>`;
}

// ------------------------------------------------------------ chrome

export const MARK_SVG = (mint) =>
  `<svg viewBox="0 0 64 64" aria-hidden="true"><g stroke-width="4" stroke-linejoin="round"><path d="M12 18A6 6 0 0 1 18 12H44L12 44Z" fill="currentColor" stroke="currentColor"/><path d="M52 20V46A6 6 0 0 1 46 52H20Z" fill="${mint}" stroke="${mint}"/></g></svg>`;

const NAV = {
  en: {
    home: "/",
    homeLabel: "Rupok home",
    label: "Main",
    links: [
      ["/smartboard-software/", "Smartboard software"],
      ["/ai-whiteboard/", "AI whiteboard"],
      ["/compare/", "Compare"],
      ["/open/", "Open .rpk"],
    ],
    board: "Open the board",
  },
  bn: {
    home: "/bn/",
    homeLabel: "Rupok হোম",
    label: "প্রধান",
    links: [
      ["/bn/", "স্মার্ট বোর্ড সফটওয়্যার"],
      ["/bn/open/", ".rpk ফাইল খুলুন"],
      ["/bn/compare/note3/", "Note3 বনাম Rupok"],
    ],
    board: "বোর্ড খুলুন",
  },
};

// Light / dark theme: the inline script applies a saved choice before first
// paint; src/theme.ts wires the toggle. Keep both in step with index.html.
export const THEME_HEAD_SCRIPT = `<script>try{var t=localStorage.getItem("rupok.site-theme");if(t==="light"||t==="dark"){document.documentElement.dataset.theme=t;document.documentElement.style.colorScheme=t}}catch(e){}</script>`;
const THEME_LABELS = {
  en: { group: "Colour theme", system: "Use the system theme", light: "Light theme", dark: "Dark theme" },
  bn: { group: "রঙের থিম", system: "সিস্টেমের থিম", light: "লাইট থিম", dark: "ডার্ক থিম" },
};
const THEME_ICONS = {
  system: `<rect x="3" y="4" width="18" height="12" rx="2.5"/><path d="M8.5 20h7M12 16v4"/>`,
  light: `<circle cx="12" cy="12" r="4"/><path d="M12 2.5v2M12 19.5v2M4.6 4.6 6 6M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4 6 18M18 6l1.4-1.4"/>`,
  dark: `<path d="M20 14.6A8.2 8.2 0 0 1 9.4 4 8.2 8.2 0 1 0 20 14.6Z"/>`,
};
export function themeToggle(lang) {
  const l = THEME_LABELS[lang] ?? THEME_LABELS.en;
  const btn = (k) =>
    `<button type="button" role="radio" aria-checked="${k === "system"}" data-theme-set="${k}" aria-label="${l[k]}" title="${l[k]}"><svg viewBox="0 0 24 24" aria-hidden="true">${THEME_ICONS[k]}</svg></button>`;
  return `<div class="theme-toggle" role="radiogroup" aria-label="${l.group}">${["system", "light", "dark"].map(btn).join("")}</div>`;
}

export function nav(page) {
  const n = NAV[page.lang];
  const other = page.lang === "en" ? page.alt?.bn : page.alt?.en;
  const links = n.links
    .map(([href, label]) => `<a href="${href}"${href === page.path ? ' aria-current="page"' : ""}>${label}</a>`)
    .join("");
  // The language switch sits with the buttons, so it stays visible on phones
  // (the text links collapse below 960px). Phones show the short label.
  const swap = other
    ? page.lang === "en"
      ? `<a class="lang-swap" href="${other}" hreflang="bn" lang="bn" aria-label="বাংলা"><span class="ls-long">বাংলা</span><span class="ls-short" aria-hidden="true">বাং</span></a>`
      : `<a class="lang-swap" href="${other}" hreflang="en" lang="en" aria-label="English"><span class="ls-long">English</span><span class="ls-short" aria-hidden="true">EN</span></a>`
    : "";
  return `<header class="nav">
      <div class="wrap nav-in">
        <a class="brand" href="${n.home}" aria-label="${n.homeLabel}"><span class="mark">${MARK_SVG("#20C997")}</span><span class="word">rupok</span></a>
        <nav class="links" aria-label="${n.label}">${links}</nav>
        <div class="nav-cta">
          ${swap}
          ${themeToggle(page.lang)}
          <a class="pill ghost" href="${GITHUB}" rel="noopener">GitHub</a>
          <a class="pill dark" href="/board/">${n.board}</a>
        </div>
      </div>
    </header>`;
}

export const COMPARE_LINKS = [
  ["/compare/note3/", "Rupok vs Note3"],
  ["/compare/smart-notebook/", "Rupok vs SMART Notebook"],
  ["/compare/openboard/", "Rupok vs OpenBoard"],
  ["/compare/jamboard/", "Jamboard alternative"],
  ["/compare/microsoft-whiteboard/", "Rupok vs Microsoft Whiteboard"],
  ["/compare/activinspire/", "Rupok vs ActivInspire"],
  ["/compare/explain-everything/", "Rupok vs Explain Everything"],
  ["/compare/onenote/", "Rupok vs OneNote"],
];

export function footer(page) {
  const bn = page.lang === "bn";
  const a = (href, label, extra = "") => `<a href="${href}"${extra}>${label}</a>`;
  const product = bn
    ? [a("/board/", "বোর্ড খুলুন"), a("/bn/download/", "ডাউনলোড"), a("/bn/", "স্মার্ট বোর্ড সফটওয়্যার"), a("/bn/open/", ".rpk ফাইল খুলুন"), a("/bn/rpk/", "আরপিকে ফাইল ফরম্যাট"), a("/bn/compare/note3/", "Note3 বনাম Rupok"), a("/bn/pricing/", "দাম")]
    : [a("/board/", "Open the board"), a("/download/", "Download"), a("/smartboard-software/", "Smartboard software"), a("/ai-whiteboard/", "AI whiteboard"), a("/online-class-whiteboard/", "Online class whiteboard"), a("/for-teachers/", "For teachers"), a("/open/", "Open a .rpk file"), a("/rpk/", "RPK file format"), a("/pricing/", "Pricing")];
  const compare = [a("/compare/", bn ? "সব তুলনা" : "All comparisons", bn ? ' hreflang="en"' : ""), ...COMPARE_LINKS.map(([h, l]) => a(h, l, bn ? ' hreflang="en"' : ""))];
  const oss = [a(bn ? "/bn/donate/" : "/donate/", bn ? "অনুদান দিন" : "Donate"), a(GITHUB, "GitHub", ' rel="noopener"'), a(`${GITHUB}/blob/main/ROADMAP.md`, bn ? "রোডম্যাপ" : "Roadmap", ' rel="noopener"'), a(`${GITHUB}/blob/main/CONTRIBUTING.md`, bn ? "অবদান রাখুন" : "Contribute", ' rel="noopener"'), a("/llms.txt", "llms.txt"), `<span class="lic">AGPL-3.0</span>`];
  const lang = bn
    ? [a("/", "Rupok in English", ' hreflang="en" lang="en"'), a("/open/", "Open a .rpk file", ' hreflang="en" lang="en"'), a("/compare/", "Compare", ' hreflang="en" lang="en"')]
    : [a("/bn/", "Rupok বাংলায়", ' hreflang="bn"'), a("/bn/open/", ".rpk ফাইল খুলুন", ' hreflang="bn"'), a("/bn/compare/note3/", "Note3 বনাম Rupok", ' hreflang="bn"')];
  const blurb = bn
    ? `শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। বাংলাদেশে বানিয়েছে <a href="https://shaitrish.com" rel="noopener">Shaitrish</a>, সব ক্লাসরুমের জন্য।`
    : `The free, open-source whiteboard for teachers. Made by <a href="https://shaitrish.com" rel="noopener">Shaitrish</a> in Bangladesh, for classrooms everywhere.`;
  return `<footer class="foot">
      <div class="wrap foot-in">
        <div class="foot-brand"><span class="word">${MARK_SVG("#3EE6B0")}rupok</span><p>${blurb}</p><p class="foot-dl" data-download="foot"><a href="${bn ? "/bn/download/" : "/download/"}">${bn ? "ডেস্কটপ অ্যাপ ডাউনলোড করুন" : "Download the desktop app"}</a></p></div>
        <nav aria-label="${bn ? "Rupok" : "Product"}"><b>${bn ? "Rupok" : "Product"}</b>${product.join("")}</nav>
        <nav aria-label="${bn ? "তুলনা" : "Compare"}"><b>${bn ? "তুলনা" : "Compare"}</b>${compare.join("")}</nav>
        <nav aria-label="Open source"><b>${bn ? "ওপেন সোর্স" : "Open source"}</b>${oss.join("")}</nav>
        <nav aria-label="${bn ? "English" : "বাংলা"}"${bn ? ' lang="en"' : ' lang="bn"'}><b>${bn ? "English" : "বাংলা"}</b>${lang.join("")}</nav>
      </div>
    </footer>`;
}

// --------------------------------------------------------------- JSON-LD

export const ORG = {
  "@type": "Organization",
  "@id": `${SITE}/#org`,
  name: "Shaitrish",
  url: "https://shaitrish.com",
  logo: `${SITE}/icon-512.png`,
  sameAs: ["https://github.com/Shaitrish"],
};

export const APP = {
  "@type": "SoftwareApplication",
  "@id": `${SITE}/#app`,
  name: "Rupok",
  description:
    "Free, open-source smartboard and whiteboard software for teachers: GPU ink on slides, live AI-made Smart Objects, stroke-by-stroke class replay and a no-account student viewer for .rpk lesson files.",
  url: `${SITE}/`,
  applicationCategory: "EducationalApplication",
  applicationSubCategory: "Interactive whiteboard software",
  operatingSystem: "Windows, macOS and Linux (desktop app); any OS with Chrome, Edge or Safari (Windows, macOS, Linux, ChromeOS)",
  browserRequirements: "Requires a recent Chrome, Edge or Safari browser with WebGPU or WebGL2.",
  offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  isAccessibleForFree: true,
  license: "https://www.gnu.org/licenses/agpl-3.0.html",
  image: `${SITE}/og.png`,
  screenshot: `${SITE}/hero.webp`,
  featureList: [
    "GPU-accelerated pen ink with pressure and palm rejection",
    "Write on PDF slides",
    "AI-made interactive Smart Objects (simulations, 3D models, quizzes)",
    "Stroke-by-stroke class replay",
    "Smart shapes and handwritten maths to LaTeX, offline",
    "AI chat and live board editing with your own Claude, Codex or OpenCode",
    "PDF export with handouts",
    "Lesson library with folders",
    "Read-only student viewer for .rpk lesson files, no account needed",
    "MCP server for Claude, Codex and OpenCode",
    "Bangla text in lessons",
  ],
  author: { "@id": `${SITE}/#org` },
  publisher: { "@id": `${SITE}/#org` },
  sameAs: [GITHUB],
};

export function jsonLd(page, { home = false } = {}) {
  const url = SITE + page.path;
  const graph = [ORG, APP];
  if (home) {
    graph.push({
      "@type": "WebSite",
      "@id": `${SITE}/#website`,
      url: `${SITE}/`,
      name: "Rupok",
      inLanguage: ["en", "bn"],
      publisher: { "@id": `${SITE}/#org` },
    });
  }
  const crumbs = page.crumbs ?? [];
  graph.push({
    "@type": "WebPage",
    "@id": `${url}#page`,
    url,
    name: page.title,
    description: page.description,
    inLanguage: page.lang,
    isPartOf: { "@id": `${SITE}/#website` },
    about: { "@id": `${SITE}/#app` },
    dateModified: page.updated ?? UPDATED,
    ...(crumbs.length ? { breadcrumb: { "@id": `${url}#crumbs` } } : {}),
  });
  if (crumbs.length) {
    const home = page.lang === "bn" ? ["Rupok", "/bn/"] : ["Rupok", "/"];
    const items = [home, ...crumbs];
    graph.push({
      "@type": "BreadcrumbList",
      "@id": `${url}#crumbs`,
      itemListElement: items.map(([name, href], i) => ({
        "@type": "ListItem",
        position: i + 1,
        name,
        item: SITE + (href ?? page.path),
      })),
    });
  }
  if (page.faq?.length) {
    graph.push({
      "@type": "FAQPage",
      "@id": `${url}#faq`,
      mainEntity: page.faq.map((f) => ({
        "@type": "Question",
        name: plain(f.q),
        acceptedAnswer: { "@type": "Answer", text: [].concat(f.a).map(plain).join(" ") },
      })),
    });
  }
  if (page.ld) graph.push(...page.ld); // extra nodes from the page (e.g. /download/)
  const json = JSON.stringify({ "@context": "https://schema.org", "@graph": graph }, null, 0);
  return `<script type="application/ld+json">${json.replace(/</g, "\\u003c")}</script>`;
}

// ------------------------------------------------------------------ head

export function head(page) {
  const url = SITE + page.path;
  const alts = page.alt
    ? Object.entries({ [page.lang]: page.path, ...page.alt })
        .map(([l, p]) => `<link rel="alternate" hreflang="${l}" href="${SITE + p}" />`)
        .concat(`<link rel="alternate" hreflang="x-default" href="${SITE + (page.lang === "en" ? page.path : page.alt.en)}" />`)
        .join("\n    ")
    : "";
  const locale = page.lang === "bn" ? "bn_BD" : "en_US";
  return `<meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
    <meta name="color-scheme" content="light dark" />
    ${THEME_HEAD_SCRIPT}
    <title>${esc(page.title)}</title>
    <meta name="description" content="${esc(page.description)}" />
    <meta name="theme-color" content="#ffffff" />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
    <link rel="manifest" href="/manifest.webmanifest" />
    <link rel="canonical" href="${url}" />
    ${alts}
    <link rel="alternate" type="text/markdown" href="${mdPath(page)}" title="Markdown version" />
    <meta property="og:type" content="website" />
    <meta property="og:site_name" content="Rupok" />
    <meta property="og:title" content="${esc(page.ogTitle ?? page.title)}" />
    <meta property="og:description" content="${esc(page.description)}" />
    <meta property="og:url" content="${url}" />
    <meta property="og:image" content="${SITE}/og.png" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta property="og:locale" content="${locale}" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:title" content="${esc(page.ogTitle ?? page.title)}" />
    <meta name="twitter:description" content="${esc(page.description)}" />
    <meta name="twitter:image" content="${SITE}/og.png" />
    ${jsonLd(page)}`;
}

/** "/compare/note3/" → "/compare/note3.md", "/" → "/index.md". */
export function mdPath(page) {
  return page.path === "/" ? "/index.md" : page.path.replace(/\/$/, "") + ".md";
}

// ------------------------------------------------------------------ page

function crumbsHtml(page) {
  if (!page.crumbs?.length) return "";
  const home = page.lang === "bn" ? ["Rupok", "/bn/"] : ["Rupok", "/"];
  const items = [home, ...page.crumbs];
  return `<nav class="crumbs" aria-label="${page.lang === "bn" ? "ব্রেডক্রাম্ব" : "Breadcrumb"}"><ol>${items
    .map(([name, href], i) =>
      i === items.length - 1 ? `<li><span aria-current="page">${esc(name)}</span></li>` : `<li><a href="${href}">${esc(name)}</a></li>`,
    )
    .join("")}</ol></nav>`;
}

function sectionHtml(s) {
  const cls = ["sec", s.band === "mint" ? "band mint" : s.band === "sun" ? "band sun" : s.band === "dark" ? "dark-band" : ""].filter(Boolean).join(" ");
  const tagCls = s.band === "dark" ? "tag light" : "tag";
  const headInner = `${s.tag ? `<span class="${tagCls}">${esc(s.tag)}</span>` : ""}<h2>${inline(s.h2)}</h2>${s.lede ? `<p class="sec-lede">${inline(s.lede)}</p>` : ""}`;
  const body = (s.blocks ?? []).map(blockHtml).join("\n");
  if (s.split) {
    return `<section${s.id ? ` id="${s.id}"` : ""} class="${cls}">
      <div class="wrap split${s.split === "rev" ? " rev" : ""}">
        <div class="copy">${headInner}${(s.copy ?? []).map(blockHtml).join("")}</div>
        <div class="side">${body}</div>
      </div>
    </section>`;
  }
  return `<section${s.id ? ` id="${s.id}"` : ""} class="${cls}">
      <div class="wrap${s.narrow ? " narrow" : ""}">
        <div class="head">${headInner}</div>
        <div class="body">${body}</div>
      </div>
    </section>`;
}

const T = {
  en: { faq: "Questions people ask", sources: "Sources", checked: (d) => `Facts about other products were checked on ${d}. Prices and features change; if you spot something out of date, please [open an issue](${GITHUB}/issues).` },
  bn: { faq: "যা সবাই জানতে চান", sources: "তথ্যসূত্র", checked: (d) => `অন্যান্য সফটওয়্যারের তথ্য ${d} তারিখে যাচাই করা। দাম আর ফিচার বদলায়; ভুল চোখে পড়লে [GitHub-এ জানান](${GITHUB}/issues)।` },
};

export function faqHtml(page) {
  if (!page.faq?.length) return "";
  return `<section id="faq" class="sec">
      <div class="wrap narrow">
        <div class="head"><span class="tag">FAQ</span><h2>${inline(page.faqTitle ?? T[page.lang].faq)}</h2></div>
        <div class="faq">${page.faq
          .map((f, i) => `<details${i === 0 ? " open" : ""}><summary>${inline(f.q)}</summary><div class="a">${[].concat(f.a).map((p) => `<p>${inline(p)}</p>`).join("")}</div></details>`)
          .join("\n")}</div>
      </div>
    </section>`;
}

export function pageHtml(page, source) {
  const t = T[page.lang];
  const hero = `<section class="phero${page.aside ? " has-aside" : ""}">
      <div class="wrap phero-in">
        <div class="phero-copy">
          ${crumbsHtml(page)}
          ${page.tag ? `<span class="tag">${esc(page.tag)}</span>` : ""}
          <h1>${inline(page.h1)}</h1>
          <p class="lede">${inline(page.lede)}</p>
          ${page.cta ? `<div class="ctas">${page.cta.map(pill).join("")}</div>` : ""}
          ${page.fine ? `<p class="fine">${inline(page.fine)}</p>` : ""}
        </div>
        ${page.aside ? `<div class="phero-aside">${page.aside}</div>` : ""}
      </div>
    </section>`;
  const faq = faqHtml(page);
  const sources = page.sources?.length
    ? `<section id="sources" class="sec sources">
      <div class="wrap narrow">
        <h2 class="h-small">${t.sources}</h2>
        <p class="fine">${inline(t.checked(page.checked ?? "23 November 2026"))}</p>
        <ol>${page.sources.map((s, i) => `<li id="src-${i + 1}">${inline(s)}</li>`).join("")}</ol>
        ${page.disclaimer ? `<p class="fine">${inline(page.disclaimer)}</p>` : ""}
      </div>
    </section>`
    : "";
  const fin = page.final
    ? `<section class="final small">
      <div class="wrap">
        <h2>${inline(page.final.h2)}</h2>
        <p>${inline(page.final.p)}</p>
        <div class="ctas center">${page.final.cta.map(pill).join("")}</div>
      </div>
    </section>`
    : "";
  return `<!doctype html>
<!-- Generated by apps/board/site/build.mjs from site/pages/${source}. Edit that file, then run: node site/build.mjs -->
<html lang="${page.lang}">
  <head>
    ${head(page)}
  </head>
  <body class="page${page.bodyClass ? " " + page.bodyClass : ""}">
    ${nav(page)}
    <main>
    ${page.layout === "home" ? homeMain(page) : hero}
    ${page.layout === "home" ? "" : (page.sections ?? []).map(sectionHtml).join("\n    ")}
    ${faq}
    ${sources}
    ${page.layout === "home" ? homeFinal(page) : fin}
    </main>
    ${footer(page)}
    <script type="module" src="${page.layout === "home" ? "/src/site.ts" : "/src/pages.ts"}"></script>
  </body>
</html>
`;
}

export function pageMd(page) {
  const out = [];
  out.push(`# ${plain(page.h1)}`, "", `> ${page.description}`, "");
  out.push(`Canonical: ${SITE + page.path} · Language: ${page.lang} · Updated: ${page.updated ?? UPDATED}`, "");
  out.push(md(page.lede), "");
  if (page.mdIntro) out.push(page.mdIntro, "");
  for (const s of page.layout === "home" ? page.parts.map(partSection) : page.sections ?? []) {
    out.push(`## ${plain(s.h2)}`, "");
    if (s.lede) out.push(md(s.lede), "");
    for (const b of [...(s.copy ?? []), ...(s.blocks ?? [])]) {
      const m = blockMd(b);
      if (m) out.push(m, "");
    }
  }
  if (page.faq?.length) {
    out.push(`## ${plain(page.faqTitle ?? T[page.lang].faq)}`, "");
    for (const f of page.faq) out.push(`### ${plain(f.q)}`, "", ...[].concat(f.a).map(md).flatMap((p) => [p, ""]));
  }
  if (page.sources?.length) {
    out.push(`## ${T[page.lang].sources}`, "", md(T[page.lang].checked(page.checked ?? "23 November 2026")), "");
    page.sources.forEach((s, i) => out.push(`[^${i + 1}]: ${md(s)}`));
    out.push("");
    if (page.disclaimer) out.push(`*${md(page.disclaimer)}*`, "");
  }
  return out.join("\n").replace(/\n{3,}/g, "\n\n");
}

// ------------------------------------------------------------- home layout
// `layout: "home"` renders the interactive home page (hero video, draw pad,
// live Smart Object tabs, replay, students, plans) from data, with the same
// markup and classes as the hand-written index.html, and loads src/site.ts.
// Used by /bn/. `page.parts` is a list of { kind, ... }; kind "section" (or
// none) is an ordinary content section.

const more = (m) => (m ? ` <a class="more" href="${m.href}">${inline(m.label)}</a>` : "");
const tagHtml = (t, light = false) => (t ? `<span class="tag${light ? " light" : ""}">${esc(t)}</span>` : "");
const para = (p) => [].concat(p ?? []).map((x) => `<p>${inline(x)}</p>`);

function homeHero(page) {
  const h = page.hero;
  const v = h.video;
  return `<section class="hero">
        <div class="wrap hero-in">
          <div class="hero-copy">
            <h1><span class="h1-kicker">${inline(h.kicker)}</span>${inline(h.title)}</h1>
            <p class="lede">${inline(h.lede)}</p>
            <div class="ctas">${h.cta.map(pill).join("")}</div>
            <div class="dl" data-download="hero"><a class="dl-link" href="${h.dlHref}">${inline(h.dlLabel)}</a></div>
            ${h.fine ? `<p class="fine">${inline(h.fine)}</p>` : ""}
          </div>
          ${h.collage ?? ""}
        </div>
        <div class="wrap">
          <figure class="shot">
            <video id="hero-video" class="demo" autoplay muted loop playsinline preload="metadata" width="1920" height="1080" poster="/demo/rupok-demo-light.webp" aria-label="${esc(v.label)}">
              <source src="/demo/rupok-demo-light.webm" type="video/webm; codecs=vp9" />
              <source src="/demo/rupok-demo-light.mp4" type="video/mp4" />
            </video>
            <script>(function(){var v=document.getElementById("hero-video"),d=document.documentElement;if(!v)return;var t=d.dataset.theme||(matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light");if(t==="dark"){v.poster=v.poster.replace("-light.","-dark.");v.querySelectorAll("source").forEach(function(s){s.src=s.src.replace("-light.","-dark.")})}if(matchMedia("(prefers-reduced-motion: reduce)").matches){v.autoplay=false;v.removeAttribute("autoplay")}})()</script>
            <button class="vplay" type="button" hidden aria-label="${esc(v.play)}"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l10.5-6.5z"/></svg></button>
            <button class="vtoggle" type="button" aria-label="${esc(v.pause)}" title="${esc(v.pause)}"><svg class="i-pause" viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5h3v14H8zM13 5h3v14h-3z"/></svg><svg class="i-play" viewBox="0 0 24 24" aria-hidden="true"><path d="M8 5.5v13l10.5-6.5z"/></svg></button>
          </figure>
        </div>
      </section>`;
}

const PARTS = {
  draw: (p) => `<section id="${p.id}" class="band mint">
        <div class="wrap split">
          <div class="copy">
            ${tagHtml(p.tag)}
            <h2>${inline(p.h2)}</h2>
            <p>${inline(p.p)}${more(p.more)}</p>
            <ul class="facts">${p.facts.map(([b, t]) => `<li><b>${inline(b)}</b><span>${inline(t)}</span></li>`).join("")}</ul>
          </div>
          <div class="pad">
            <div class="pad-top"><span>${inline(p.pad.label)}</span><div class="inks" role="group" aria-label="${esc(p.pad.group)}">${["#111418", "#1c7ed6", "#e03131", "#12b886"]
              .map((c, i) => `<button type="button" data-ink="${c}"${i === 0 ? ' class="on"' : ""} aria-label="${esc(p.pad.colours[i])}"></button>`)
              .join("")}</div><button class="clear" type="button">${inline(p.pad.clear)}</button></div>
            <canvas id="pad" aria-label="${esc(p.pad.canvas)}"></canvas>
          </div>
        </div>
      </section>`,
  lessons: (p) => `<section id="${p.id}" class="plain">
        <div class="wrap">
          <div class="head">
            ${tagHtml(p.tag)}
            <h2>${inline(p.h2)}</h2>
            <p>${inline(p.p)}${more(p.more)}</p>
          </div>
          <div class="tabs" role="tablist" id="tabs" aria-label="${esc(p.tabsLabel)}"></div>
          <div class="stage">
            <iframe id="demo" title="${esc(p.frame)}" sandbox="allow-scripts"></iframe>
          </div>
          <div class="prompt"><span class="who">${inline(p.who)}</span><p id="demo-prompt"></p></div>
          ${p.note ? `<p class="fine">${inline(p.note)}</p>` : ""}
        </div>
      </section>`,
  replay: (p) => `<section id="${p.id}" class="band sun">
        <div class="wrap split rev">
          <div class="copy">
            ${tagHtml(p.tag)}
            <h2>${inline(p.h2)}</h2>
            <p>${inline(p.p)}${more(p.more)}</p>
          </div>
          <div class="replay">
            <canvas id="rp" aria-label="${esc(p.canvas)}"></canvas>
            <div class="rp-bar"><button id="rp-play" class="rp-btn" type="button" aria-label="${esc(p.play)}">▶</button><input id="rp-range" type="range" min="0" max="1000" value="0" aria-label="${esc(p.position)}" /><span id="rp-time">0:00</span></div>
          </div>
        </div>
      </section>`,
  students: (p) => `<section id="${p.id}" class="dark-band">
        <div class="wrap split">
          <div class="copy">
            ${tagHtml(p.tag, true)}
            <h2>${inline(p.h2)}</h2>
            ${para(p.p).join("")}
            ${pill({ ...p.cta, kind: "mint" })}
          </div>
          <ul class="cards">${p.cards.map((c) => `<li><b>${inline(c.b)}</b><span>${inline(c.t)}</span></li>`).join("")}</ul>
        </div>
      </section>`,
  plans: (p) => `<section id="${p.id}" class="plain">
        <div class="wrap">
          <div class="head center">
            ${tagHtml(p.tag)}
            <h2>${inline(p.h2)}</h2>
            <p>${inline(p.p)}</p>
          </div>
          <div class="plans">${p.plans
            .map(
              (x) => `<article class="plan${x.feat ? " feat" : ""}">${x.badge ? `<span class="badge">${esc(x.badge)}</span>` : ""}<h3>${inline(x.name)}</h3><p class="price">${inline(x.price)}</p><ul>${x.items
                .map(([t, soon]) => `<li>${inline(t)}${soon ? ` <em>${esc(soon)}</em>` : ""}</li>`)
                .join("")}</ul>${pill({ ...x.cta, kind: x.cta.kind ?? "ghost" }).replace(" lg", "")}</article>`,
            )
            .join("")}</div>
        </div>
      </section>`,
};

/** The home page's plan cards (index.html region "plans"; /bn/ uses the same part). */
export const plansHtml = (p) => PARTS.plans(p);

export function homeMain(page) {
  return [homeHero(page), ...page.parts.map((p) => (PARTS[p.kind] ?? sectionHtml)(p))].join("\n    ");
}

export function homeFinal(page) {
  const f = page.final;
  return `<section class="final">
        <div class="wrap">
          <h2>${inline(f.h2)}</h2>
          <p>${inline(f.p)}</p>
          <div class="ctas center">${f.cta.map(pill).join("")}</div>
          <div class="dl center" data-download="final"><a class="dl-link" href="${page.hero.dlHref}">${inline(page.hero.dlLabel)}</a></div>
        </div>
      </section>`;
}

/** A home part as an ordinary section, for the Markdown twin. */
function partSection(p) {
  if (!p.kind || p.kind === "section") return p;
  const blocks = [].concat(p.p ?? []);
  if (p.more) blocks.push(`[${p.more.label.replace(/\s*→$/, "")}](${p.more.href})`);
  if (p.facts) blocks.push({ ul: p.facts.map(([b, t]) => `**${b}** ${t}`) });
  if (p.cards) blocks.push({ cards: p.cards });
  if (p.cta) blocks.push({ cta: [p.cta] });
  if (p.plans) blocks.push({ ul: p.plans.map((x) => `**${x.name} (${x.price}):** ${x.items.map(([t, s]) => (s ? `${t} (${s})` : t)).join("; ")}`) });
  return { h2: p.h2, blocks };
}
