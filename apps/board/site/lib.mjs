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
  if (b.code) return "```sh\n" + b.code + "\n```";
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
    homeLabel: "রূপক হোম",
    label: "প্রধান",
    links: [
      ["/bn/", "স্মার্ট বোর্ড সফটওয়্যার"],
      ["/bn/open/", ".rpk ফাইল খুলুন"],
      ["/bn/compare/note3/", "Note3 বনাম রূপক"],
    ],
    board: "বোর্ড খুলুন",
  },
};

export function nav(page) {
  const n = NAV[page.lang];
  const other = page.lang === "en" ? page.alt?.bn : page.alt?.en;
  const links = n.links
    .map(([href, label]) => `<a href="${href}"${href === page.path ? ' aria-current="page"' : ""}>${label}</a>`)
    .join("");
  const swap = other
    ? page.lang === "en"
      ? `<a href="${other}" hreflang="bn" lang="bn">বাংলা</a>`
      : `<a href="${other}" hreflang="en" lang="en">English</a>`
    : "";
  return `<header class="nav">
      <div class="wrap nav-in">
        <a class="brand" href="${n.home}" aria-label="${n.homeLabel}"><span class="mark">${MARK_SVG("#20C997")}</span><span class="word">rupok</span></a>
        <nav class="links" aria-label="${n.label}">${links}${swap}</nav>
        <div class="nav-cta">
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
    ? [a("/board/", "বোর্ড খুলুন"), a("/bn/", "স্মার্ট বোর্ড সফটওয়্যার"), a("/bn/open/", ".rpk ফাইল খুলুন"), a("/bn/compare/note3/", "Note3 বনাম রূপক"), a("/#plans", "মূল্য", ' hreflang="en"')]
    : [a("/board/", "Open the board"), a("/smartboard-software/", "Smartboard software"), a("/ai-whiteboard/", "AI whiteboard"), a("/online-class-whiteboard/", "Online class whiteboard"), a("/for-teachers/", "For teachers"), a("/open/", "Open a .rpk file"), a("/#plans", "Pricing")];
  const compare = [a("/compare/", bn ? "সব তুলনা" : "All comparisons", bn ? ' hreflang="en"' : ""), ...COMPARE_LINKS.map(([h, l]) => a(h, l, bn ? ' hreflang="en"' : ""))];
  const oss = [a(GITHUB, "GitHub", ' rel="noopener"'), a(`${GITHUB}/blob/main/ROADMAP.md`, bn ? "রোডম্যাপ" : "Roadmap", ' rel="noopener"'), a(`${GITHUB}/blob/main/CONTRIBUTING.md`, bn ? "অবদান রাখুন" : "Contribute", ' rel="noopener"'), a("/llms.txt", "llms.txt"), `<span class="lic">AGPL-3.0</span>`];
  const lang = bn
    ? [a("/", "Rupok in English", ' hreflang="en" lang="en"'), a("/open/", "Open a .rpk file", ' hreflang="en" lang="en"'), a("/compare/", "Compare", ' hreflang="en" lang="en"')]
    : [a("/bn/", "রূপক বাংলায়", ' hreflang="bn"'), a("/bn/open/", ".rpk ফাইল খুলুন", ' hreflang="bn"'), a("/bn/compare/note3/", "Note3 বনাম রূপক", ' hreflang="bn"')];
  const blurb = bn
    ? `শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। বাংলাদেশে বানিয়েছে <a href="https://shaitrish.com" rel="noopener">Shaitrish</a>, সব ক্লাসরুমের জন্য।`
    : `The free, open-source whiteboard for teachers. Made by <a href="https://shaitrish.com" rel="noopener">Shaitrish</a> in Bangladesh, for classrooms everywhere.`;
  return `<footer class="foot">
      <div class="wrap foot-in">
        <div class="foot-brand"><span class="word">${MARK_SVG("#3EE6B0")}rupok</span><p>${blurb}</p></div>
        <nav aria-label="${bn ? "রূপক" : "Product"}"><b>${bn ? "রূপক" : "Product"}</b>${product.join("")}</nav>
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
  alternateName: "রূপক",
  description:
    "Free, open-source smartboard and whiteboard software for teachers: GPU ink on slides, live AI-made Smart Objects, stroke-by-stroke class replay and a no-account student viewer for .rpk lesson files.",
  url: `${SITE}/`,
  applicationCategory: "EducationalApplication",
  applicationSubCategory: "Interactive whiteboard software",
  operatingSystem: "Any OS with Chrome or Edge (Windows, macOS, Linux, ChromeOS)",
  browserRequirements: "Requires a recent Chrome or Edge browser with WebGPU or WebGL2.",
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
      alternateName: "রূপক",
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
    const home = page.lang === "bn" ? ["রূপক", "/bn/"] : ["Rupok", "/"];
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
  const home = page.lang === "bn" ? ["রূপক", "/bn/"] : ["Rupok", "/"];
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
    ${hero}
    ${(page.sections ?? []).map(sectionHtml).join("\n    ")}
    ${faq}
    ${sources}
    ${fin}
    </main>
    ${footer(page)}
    <script type="module" src="/src/pages.ts"></script>
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
  for (const s of page.sections ?? []) {
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
