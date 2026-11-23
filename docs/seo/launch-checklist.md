# SEO launch checklist (for the founder)

`Status: to do · written 23 Nov 2026`

The site side is done in the repo: titles, descriptions, canonical URLs,
`hreflang`, JSON-LD, `sitemap.xml`, `robots.txt`, `llms.txt`, Markdown twins of
each page, and the keyword pages (see [keywords.md](keywords.md)). These steps
need your accounts, so an agent can't do them. Tick them off in order.

## 1. Before you deploy

- [ ] Read the comparison pages once (`/compare/…`). Every claim about another
      product is dated and sourced, but you're the publisher.
- [ ] Decide `ai-train` in `apps/board/public/robots.txt`. It's unset on
      purpose (no preference either way). `ai-train=yes` helps future models
      know Rupok exists; `ai-train=no` opts out. Search and AI answers
      (`search=yes, ai-input=yes`) are allowed either way.
- [ ] Optional: change `not_found_handling` in `apps/board/wrangler.jsonc`
      from `single-page-application` to `404-page` and add a `404.html`. Today
      any unknown URL returns the home page with status 200 (a "soft 404"),
      which search engines dislike. The board only uses `/board/` and query
      strings, so it doesn't need the SPA fallback.
- [ ] Deploy (`cd apps/board && bunx vite build && bunx wrangler deploy`, or
      push to `main`).
- [ ] Check these URLs on the live site return 200: `/robots.txt`,
      `/sitemap.xml`, `/llms.txt`, `/llms-full.txt`, `/open/`, `/bn/`,
      `/compare/note3/`, `/compare/note3.md`.

## 2. Cloudflare dashboard (rupok.app zone)

- [ ] **AI Crawl Control / bots:** make sure nothing blocks AI crawlers. As
      of November 2026 new zones default to allowing Search, Agents and
      Training, but the onboarding presets (since 15 Sep 2026) can disallow
      training or block agents
      ([docs](https://developers.cloudflare.com/bots/additional-configurations/block-ai-bots/)).
      Allow GPTBot, OAI-SearchBot, ChatGPT-User, ClaudeBot, Claude-SearchBot,
      Claude-User, PerplexityBot, Google-Extended and Applebot.
- [ ] **Managed robots.txt:** keep it **off**. When it's on, Cloudflare
      prepends `Disallow: /` for ClaudeBot, GPTBot, Google-Extended and others
      to our robots.txt
      ([docs](https://developers.cloudflare.com/bots/additional-configurations/managed-robots-txt/)).
      Afterwards, open `https://rupok.app/robots.txt` and check it's exactly
      our file.
- [ ] **Bot Fight Mode:** if on, check that it doesn't challenge Googlebot or
      Bingbot (verified bots should pass; test with Search Console's URL
      Inspection).
- [ ] **Crawler Hints:** turn on (Caching → Configuration). It sends IndexNow
      pings to Bing, Yandex and others when pages change.

## 3. Search engines

- [ ] **Google Search Console:** add `rupok.app` as a Domain property (DNS TXT
      record in Cloudflare). Submit `https://rupok.app/sitemap.xml`. Use URL
      Inspection on `/`, `/open/`, `/bn/` and `/compare/note3/` and request
      indexing.
- [ ] **Bing Webmaster Tools:** import from Search Console (one click) or
      verify with DNS. Submit the sitemap. Bing's index feeds ChatGPT search,
      Copilot and DuckDuckGo, so this matters as much as Google for AI
      answers.
- [ ] After two to four weeks: in Search Console, check the Performance report
      for queries, and the Enhancements for FAQ and breadcrumb structured data.
      Update [keywords.md](keywords.md) with what people actually search.
- [ ] Check `hreflang` pairs show no errors (`/` ↔ `/bn/`, `/open/` ↔
      `/bn/open/`, `/compare/note3/` ↔ `/bn/compare/note3/`).

## 4. Get listed

Directories and lists rank for the alternative searches themselves, and AI
assistants read them.

- [ ] **AlternativeTo:** create a Rupok entry (free, open source, web,
      Windows/Linux coming), then add it as an alternative on: SMART Notebook,
      OpenBoard, Jamboard, Microsoft Whiteboard, ActivInspire, Explain
      Everything, IPEVO Whiteboard, LiveBoard. Create Note3 and EasiNote entries
      too (none exists).
- [ ] **Product Hunt:** launch when the landing page and `/open/` are live.
      Its "alternatives" pages rank.
- [ ] **G2, Capterra, GetApp, Software Advice** (free vendor listings; the last
      three are one Gartner listing). Category: interactive whiteboard /
      education software.
- [ ] **SourceForge:** project listing in the whiteboard directory (it ranks
      for "open source whiteboard software"). Also Slashdot software,
      edtechimpact.com, TrustRadius.
- [ ] **GitHub:** add topics to the repo: `whiteboard`, `smartboard`,
      `interactive-whiteboard`, `education`, `teachers`, `edtech`, `mcp`,
      `mcp-server`, `bangla`, `webgpu`, `open-source`. Add the website URL to
      the repo's About box.
- [ ] **Awesome lists:** open PRs to `awesome-open-source-education`,
      `awesome-edtech-tools` and the main `awesome-mcp-servers` lists
      (education category).
- [ ] **MCP directories:** mcp.so, mcpservers.org, mcpmarket.com. When the
      remote connector ships (see
      [rupok-cloud](../roadmap/rupok-cloud.md)), apply for the Claude
      connectors directory.
- [ ] **File-extension sites:** submit the `.rpk` "Rupok lesson" format to
      fileinfo.com, filext.com and file.org, linking to `https://rupok.app/open/`.
- [ ] **Linux Foundation LFX** "interactive whiteboard applications"
      collection: request inclusion.

## 5. Teachers and Bangladesh

- [ ] **শিক্ষক বাতায়ন (teachers.gov.bd):** post a Bangla how-to (teachers
      can publish blogs and content there; it already ranks for "অনলাইন ক্লাস
      হোয়াইটবোর্ড"). Link to `/bn/`.
- [ ] Bangladeshi teacher Facebook groups and YouTube: a short Bangla video
      "Note3-এর ফ্রি বিকল্প" showing PDF → Rupok → replay → WhatsApp. Link to
      `/bn/compare/note3/`.
- [ ] Tech news sections: Jago News, The Daily Campus, Prothom Alo tech.
- [ ] Interactive-panel retailers (Star Tech, Ryans, Techland, Digimark, GCTL)
      as bundling partners: "free software that runs on every panel you sell".
- [ ] Teacher bloggers who write "Jamboard replacements" and "free whiteboard"
      roundups: Ditch That Textbook, Alice Keeler, TCEA, Class Tech Tips,
      Edutopia, Zapier's and Goodnotes' whiteboard roundups.
- [ ] itsfoss.com (covered OpenBoard) for an open-source feature.

## 6. Keep it true

- [ ] Every six months, re-check the facts on `/compare/` pages (prices,
      editions, platforms) and update the "checked on" date in
      `apps/board/site/pages/compare.mjs` and `site/lib.mjs`. Then run
      `node site/build.mjs`.
- [ ] When a "coming soon" feature ships (desktop app, Bangla interface,
      offline install, Rupok Cloud), update `site/pages/facts.mjs` and the
      pages that mention it, then rebuild.
