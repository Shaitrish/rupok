# SEO keyword map

`Status: live in the repo · researched 23 Nov 2026`

Which searches each page on rupok.app targets, what the searcher wants, and
who ranks today. The pages are generated from `apps/board/site/pages/*.mjs`
(`cd apps/board && node site/build.mjs`); the home page is
`apps/board/index.html`.

## How this was researched

Each keyword was searched with a web-search tool (US index) on 23 Nov 2026.
That tool returns result links and titles, not ranking positions, "People
also ask" boxes or search volumes. So the competitor lists below are roughly
ordered, the questions are inferred from result titles, and nothing here is a
volume estimate. Bangla coverage is a lower bound: the tool mostly returned
English pages for Bangla queries. Re-check in Google Search Console once the
site has impressions (see [launch-checklist.md](launch-checklist.md)).

## Summary: keyword → page

| Cluster | Keywords | Intent | Page |
|---|---|---|---|
| Core | best / free smartboard software; smartboard software for teachers; interactive whiteboard software for teachers; smart board software free download for pc | Commercial investigation | [`/smartboard-software/`](https://rupok.app/smartboard-software/) (pillar), `/` |
| Open source | open source whiteboard software; open source smartboard software | Informational | `/` (title and kicker), `/smartboard-software/#open-source`, GitHub README |
| AI | AI whiteboard for teachers; AI smartboard software; (AI whiteboard) | Commercial | [`/ai-whiteboard/`](https://rupok.app/ai-whiteboard/) |
| MCP | Claude whiteboard; whiteboard MCP server | Navigational / developer | `/ai-whiteboard/#mcp`, `/llms.txt` |
| Online teaching | online class whiteboard; whiteboard for online teaching; digital whiteboard for teaching | Informational + commercial | [`/online-class-whiteboard/`](https://rupok.app/online-class-whiteboard/) |
| Teacher workflow | note taking software for teachers (smartboard); whiteboard app for teachers; record / replay whiteboard lessons | Mixed | [`/for-teachers/`](https://rupok.app/for-teachers/) |
| Alternatives | free SMART Notebook alternative; Jamboard alternative / replacement; OpenBoard alternative; Note3 (EasiNote) alternative; Microsoft Whiteboard alternative; ActivInspire alternative; Explain Everything alternative; OneNote alternative for teachers | Commercial investigation | [`/compare/`](https://rupok.app/compare/) hub + `/compare/<product>/` |
| RPK file | rpk file; open rpk file; what is an rpk file; rpk viewer; rpk file opener; rpk to pdf; rupok file; .rpk | Informational / "how do I open it" | [`/open/`](https://rupok.app/open/) |
| Bangla | স্মার্ট বোর্ড সফটওয়্যার / স্মার্টবোর্ড সফটওয়্যার; ফ্রি স্মার্টবোর্ড সফটওয়্যার; শিক্ষকদের জন্য সফটওয়্যার; অনলাইন ক্লাসের হোয়াইটবোর্ড; মাল্টিমিডিয়া ক্লাসরুম; RPK ফাইল (secondary; `/bn/open/` owns it) | Mixed | [`/bn/`](https://rupok.app/bn/) |
| Bangla RPK | rpk ফাইল খুলুন; আরপিকে ফাইল; rpk ফাইল কীভাবে খুলবো; rpk থেকে pdf | Informational | [`/bn/open/`](https://rupok.app/bn/open/) |
| Bangla Note3 | Note3 বিকল্প; Note3 ফ্রি; EasiNote | Commercial | [`/bn/compare/note3/`](https://rupok.app/bn/compare/note3/) |

Decisions from the research:

- **One pillar for the core cluster.** "best smartboard software", "free
  smartboard software", "smartboard software for teachers" and "interactive
  whiteboard software for teachers" return almost the same results, so they
  share `/smartboard-software/`. The home page targets the brand plus "free,
  open-source smartboard and AI whiteboard software".
- **`/for-teachers/` gets its own intent** (the teacher's day: plan, teach,
  replay, share; note-taking), so it doesn't compete with the pillar.
- **No generic "AI whiteboard" fight.** Zoom, ClickUp and Miro own it. The AI
  page targets the teacher variants and "AI smartboard without a new panel".
- **The MCP section lives on `/ai-whiteboard/#mcp`** rather than its own page,
  plus `llms.txt` for agents. Revisit if "whiteboard MCP" searches grow.
- **Note3 got a Bangla page too.** It is the clearest gap: real South Asian
  demand (YouTube tutorials, "free download" sites) and almost no legitimate
  content.
- **OneNote** has a page because the founder asked for one, but it's low
  priority: the "OneNote alternative for teachers" results are loose.
- **`/open/` avoids the bare word "rpk"** in titles because of Redpanda's
  `rpk` CLI; it says ".rpk files" and "RPK viewer".

## Cluster detail

### Core: smartboard software → `/smartboard-software/`

- **best smartboard software** (commercial investigation). Results: BenQ's
  blog, templated "Top 10 of 2026" listicles (gitnux, wifitalents, zipdo,
  worldmetrics), a SMART vs Promethean post, vibe.us, ViewSonic,
  AlternativeTo. Products that keep appearing: OpenBoard, Explain Everything,
  Miro, Microsoft Whiteboard, FigJam, Jamboard, ActivInspire, myViewBoard,
  SMART Notebook. The listicles are thin; an honest page with a comparison
  table can compete.
- **free smartboard software** (commercial, download-leaning). Results: SMART's
  "free with your SMART Board" pages, SourceForge, Softpedia (Smoothboard),
  Wikipedia (OpenBoard), itsfoss.com/openboard, AlternativeTo. OpenBoard is
  the dominant free pick. Angle: works on any board, no licence tied to
  hardware.
- **smartboard software for teachers**: heavily SMART-branded results (Notebook,
  Lumio, district guides). Same page.
- **interactive whiteboard software for teachers**: BenQ EZWrite, ViewSonic
  myViewBoard, a Microsoft Store app, OpenBoard. Used as an H2 on the pillar.
- **smart board software free download for pc** (South Asian phrasing): covered
  by the FAQ "Do I need to download smartboard software for my PC?" (no:
  browser; a desktop app is also available).
- Questions inferred: Is there free smart board software? Can I use SMART
  software on a non-SMART board? What is the best free interactive whiteboard
  software?

### Open source → `/`, `/smartboard-software/#open-source`

- Results: SourceForge whiteboard directory, itsfoss.com, Wikipedia OpenBoard,
  openboard.ch, the Linux Foundation LFX "interactive whiteboard applications"
  collection, GitHub repos (OpenBoard, Spacedeck Open), GetApp. OpenBoard is
  effectively the only maintained competitor; the SERP is weak.
- Also needed off-site: a SourceForge project listing and GitHub topics
  (see the launch checklist).

### AI → `/ai-whiteboard/`

- **AI smartboard software**: hardware vendors dominate (ViewSonic, BenQ,
  SenseBoard, JOI, Studynlearn, TrueBoard, Teachmint), many of them Indian
  panel brands selling into the same market as Bangladesh. No software-only
  competitor on page one. Angle: AI on the board you already have.
- **AI whiteboard for teachers**: thin and fragmented (Google Workspace
  Marketplace "Whiteboard AI" listings in many locales, MockFlow, BenQ blog,
  dev.to posts, research papers). The best AI opportunity.
- **AI whiteboard** (generic): ClickUp, Zoom, Webex, MockFlow, Boardmix, tldraw.
  Not targeted directly.
- **Claude / MCP whiteboard**: claude.com's whiteboard use case, Excalidraw
  and tldraw MCP apps, a paid Whiteboard MCP. No product combines a classroom
  board, AI-generated simulations and an MCP server.

### Online teaching → `/online-class-whiteboard/`

- **online class whiteboard / whiteboard for online teaching**: university
  teaching centres (Stanford, Wake Forest, Emory, Duke, UChicago), vendors
  (Lucid, WizIQ, webwhiteboard, Excalidraw, Creately, Newline), pen-tablet
  listicles (XP-Pen), Kaltura's "7 best". Products: Zoom Whiteboard,
  Microsoft Whiteboard, Padlet, Excalidraw, OneNote.
- **digital whiteboard for teaching**: guide pages from MIT Sloan, Carleton,
  NCSU, Caltech, Georgetown. The page includes a tips section to match.
- Questions inferred: How do I use a whiteboard in Zoom or Meet? Do I need a pen
  tablet? What's the best free online whiteboard for teaching?

### Teacher workflow → `/for-teachers/`

- **note taking software for teachers smartboard**: thin, SMART-dominated
  (Notebook pages, "SMART Board for Dummies", university guides). Low volume.
- Replay/recording searches are served by video recorders (Explain
  Everything, Educreations, ShowMe, Knowmia); none does true stroke replay.
  That's the page's angle.

### Alternatives → `/compare/…`

Priority by opportunity, from the research:

| Page | Who ranks now | Notes |
|---|---|---|
| `/compare/note3/` (+ `/bn/compare/note3/`) | G2 listing (no reviews), an Indian forum, Prestigio's launch news, YouTube "Note 3 vs Note 5", suspicious "free download" sites | Almost no legitimate content. Spell out "Note3 whiteboard software" (the name clashes with the Samsung Galaxy Note 3). No AlternativeTo page exists yet. |
| `/compare/openboard/` | AlternativeTo (many pages), Capterra, GetApp, Software Advice, TrustRadius, itsfoss, SourceForge, Slashdot | No good editorial page. Get listed on AlternativeTo too. |
| `/compare/smart-notebook/` | AlternativeTo, SourceForge, SMART's own pages, Gynzy's "5 alternatives" | Beatable. `.notebook` import would be a strong hook later. |
| `/compare/activinspire/` | AlternativeTo, a school district's migration note, SMART's "replace ActivInspire" post, edtechimpact | Schools are migrating. `.flipchart` import would help. |
| `/compare/jamboard/` | ClickUp, AlternativeTo, Edutopia, Google's help page, MockFlow, TechRepublic, Wooclap, FigJam, Lucid, Padlet, Ditch That Textbook | The most crowded alternatives SERP; demand shrinking since the Dec 2024 shutdown. Angle: bring your Jams back as PDFs. |
| `/compare/explain-everything/` | AlternativeTo, Capterra, G2, Product Hunt | Angle: free, unlimited stroke replay. |
| `/compare/microsoft-whiteboard/` | ClickUp, AlternativeTo, Microsoft Q&A, Capterra, G2 | Business-leaning. Angle: no account after the 2026 retirement. |
| `/compare/onenote/` | Loose results | Low priority. |

Every claim about a competitor on these pages is dated (facts checked
23 Nov 2026) and footnoted to the vendor's own page. Re-check them at least
every six months: prices, editions and platforms change.

### RPK file → `/open/`, `/bn/open/`

- Results: file-extension sites (filext, file.org, fileinfo, which lists only a
  RadLight skin; file-extensions.org; file.tips), Redpanda's `rpk` CLI docs,
  a PyPI package, MAME source, Tizen docs.
- Other formats using `.rpk`: Esri CityEngine rule packages (the most
  prominent current one), Tizen resource packages, MAME/TI-99/4A "rompacks",
  Exanima game assets, old RadLight skins. The FAQ names them honestly and the
  drop zone recognises a file that isn't a Rupok lesson.
- Off-site: submit the Rupok `.rpk` format to fileinfo.com, filext.com and
  file.org (see the launch checklist).

### Bangla → `/bn/`, `/bn/open/`, `/bn/compare/note3/`

- Real Bangla pages are almost all hardware sellers targeting price queries
  ("স্মার্ট বোর্ড দাম"), the government teachers' portal (teachers.gov.bd) and
  news "apps for online class" articles. No Bangla page about smartboard
  software exists, so the niche is empty.
- Spellings in use: "স্মার্ট বোর্ড" (most common), "স্মার্টবোর্ড",
  "স্মার্ট হোয়াইট বোর্ড"; ইন্টারঅ্যাকটিভ / ইন্টারঅ্যাক্টিভ / ইন্টারেক্টিভ.
  `/bn/` uses both smartboard spellings naturally.
- Government terms teachers know: "মাল্টিমিডিয়া ক্লাসরুম" and "ডিজিটাল কনটেন্ট".
  Both appear on `/bn/`.
- `/bn/` (rebuilt 23 Nov 2026 as the full interactive home page): title
  "Rupok: শিক্ষকদের জন্য ফ্রি স্মার্ট বোর্ড সফটওয়্যার ও অনলাইন ক্লাসের
  হোয়াইটবোর্ড"; H1 kicker "শিক্ষকদের জন্য ফ্রি স্মার্ট বোর্ড সফটওয়্যার"; H2
  "অনলাইন ক্লাসের হোয়াইটবোর্ড" (with the Zoom/Meet/Teams steps); the meta
  description names "RPK ফাইল" and the Students section and FAQ explain it,
  linking `/bn/open/` and `/bn/rpk/`. "ডিজিটাল হোয়াইটবোর্ড" and "ডিজিটাল
  কনটেন্ট" were dropped as targets: thin intent, and the copy reads better
  without them.
- Subject terms teachers search with (SSC/HSC পদার্থবিজ্ঞান, রসায়ন, ICT লজিক
  গেট) appear in the Smart shapes section and the FAQ.
- Later: `/bn/online-class-whiteboard/` if Search Console shows demand.

## Listings and outreach targets

See [launch-checklist.md](launch-checklist.md#3-get-listed) for the list
(AlternativeTo pages per competitor, SourceForge, Capterra/GetApp, G2,
Product Hunt, file-extension sites, MCP directories, teacher bloggers and
Bangladeshi channels).
