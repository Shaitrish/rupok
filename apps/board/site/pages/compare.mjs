// Comparison and alternative pages. Every claim about another product is
// dated (facts checked 23 Nov 2026) and footnoted to a source in `sources`
// (^n in the text). Say plainly where the other product is stronger.
import { ROWS, RUPOK } from "./facts.mjs";

const VS_ART = (name) => `<div class="art vs" aria-hidden="true">
            <div class="card-a"><span class="chip">Rupok</span><b>Free · open source</b><span>AI Smart Objects · replay</span></div>
            <div class="vs-dot">vs</div>
            <div class="card-b"><span class="chip ghost">${name}</span><b>Facts, not hype</b><span>Checked Nov 2026</span></div>
          </div>`;

const SWITCH_TAIL = [
  { b: "Open the board.", t: "Go to [rupok.app/board](/board/) in Chrome or Edge, on the panel's PC or your laptop. No install, no account." },
  { b: "Drop the PDF in.", t: "Every page becomes a slide you can write on." },
  { b: "Add the live parts.", t: "Ask your AI for a simulation or quiz and drop it on a slide as a Smart Object ([how](/ai-whiteboard/))." },
  { b: "Save and share.", t: "Save the lesson as a `.rpk` file and send it to students. They replay it at [rupok.app/open](/open/)." },
];

function comparePage(c) {
  const rows = ROWS.filter(([k]) => c.rows[k] !== undefined).map(([k, label]) => [label, RUPOK[k], c.rows[k]]);
  return {
    path: `/compare/${c.slug}/`,
    lang: "en",
    ...(c.alt ? { alt: c.alt } : {}),
    title: c.title,
    description: c.description,
    tag: "Compare",
    h1: c.h1,
    lede: c.lede,
    crumbs: [["Compare", "/compare/"], [c.crumb ?? `Rupok vs ${c.name}`]],
    cta: [{ href: "/board/", label: "Try Rupok free" }, { href: "#at-a-glance", label: "See the table", kind: "ghost" }],
    fine: `Facts as of November 2026, from ${c.name}’s own pages. Sources are at the bottom.`,
    aside: VS_ART(c.name),
    sections: [
      {
        id: "at-a-glance",
        h2: `Rupok vs ${c.name} at a glance`,
        blocks: [
          { table: { hl: "col", caption: `Rupok and ${c.name} compared, as of November 2026`, head: ["", "Rupok", c.name], rows } },
          ...(c.tableNote ? [{ note: c.tableNote }] : []),
        ],
      },
      {
        id: "stronger",
        band: "mint",
        h2: `Where ${c.name} is stronger`,
        lede: "Being honest about this saves you time.",
        blocks: [{ ticks: c.stronger }],
      },
      {
        id: "rupok",
        h2: "Where Rupok is stronger",
        blocks: [{ ticks: c.rupok }],
      },
      {
        id: "pick",
        band: "sun",
        h2: "Which one should you pick?",
        blocks: c.pick,
      },
      {
        id: "switch",
        h2: c.switchTitle ?? `Switching from ${c.name}`,
        blocks: [{ steps: [...c.switchFirst, ...SWITCH_TAIL] }],
      },
      ...(c.extra ?? []),
    ],
    faq: c.faq,
    faqTitle: `Rupok and ${c.name}: questions`,
    sources: c.sources,
    disclaimer: `${c.trademark} Rupok is not affiliated with or endorsed by ${c.owner}.`,
    final: {
      h2: "Try it on your board today.",
      p: "Free, open source, and running in your browser in five seconds.",
      cta: [{ href: "/board/", label: "Open the board" }, { href: "/compare/", label: "All comparisons", kind: "ghost" }],
    },
  };
}

// ---------------------------------------------------------------- Note3
const note3 = comparePage({
  slug: "note3",
  alt: { bn: "/bn/compare/note3/" },
  name: "Note3",
  owner: "Prestigio Solutions",
  trademark: "Note3 and EasiNote are products of their respective owners.",
  title: "Free Note3 Alternative for Teachers: Rupok vs Note3 (2026)",
  description:
    "Looking for a free alternative to Prestigio Note3 (EasiNote) whiteboard software? Compare price, platforms, AI, replay and Bangla support. Facts checked November 2026.",
  h1: "A free Note3 alternative: Rupok vs Note3",
  lede: "Note3 is Prestigio’s paid, Windows-only whiteboard software, the updated EasiNote. Rupok is free and open source, runs in Chrome or Edge on any computer, and adds AI-made lessons and class replay. Here’s an honest comparison.",
  rows: {
    price: "no: Paid, one-time licence per device: €29 on Prestigio’s store (Nov 2026), after a 15-day trial^1^3",
    oss: "no: No",
    runs: "Windows only. Prestigio’s Android panels need a connected Windows PC^1^2",
    account: "Online activation with a serial number^4",
    ai: "no: None documented^2^4",
    replay: "no: No stroke replay documented^4",
    interactive: "yes: Subject tools for maths, physics and chemistry (geometry, circuits, lab apparatus), plus handwriting and formula recognition^2^4",
    pdf: "yes: Imports PDF and PowerPoint; exports PDF, PPTX, DOCX and JPG^4",
    bangla: "no: About 29 interface languages in the installer, no Bangla^4",
    offline: "yes: Installed Windows app (activation needs internet)^4",
    board: "yes: Panels, PCs, laptops and projectors running Windows^2",
    format: "`.enb` (the EasiNote format)^4",
  },
  stronger: [
    "**Ready-made STEM tools.** Geometry and 3D shapes, electric circuits, chemistry apparatus and the periodic table are built in^2^4. In Rupok you ask your AI for these as Smart Objects: more flexible, but it takes a prompt rather than a click.",
    "**Handwriting and maths-formula recognition**^4. Rupok doesn’t have this yet; handwriting to text is on the roadmap.",
    "**Opens PowerPoint files directly** and lets you annotate them^4. In Rupok you save slides as PDF first.",
    "**A native Windows app** that runs without a browser, and without internet once it’s activated^4.",
    "**Document-camera support**^4.",
  ],
  rupok: [
    "**Free on every device.** No licence keys and no activation, so you can put it on every panel and laptop in your school.",
    "**Not just Windows.** It runs in Chrome or Edge on Windows, macOS, Linux and ChromeOS.",
    "**AI Smart Objects.** Ask Claude or ChatGPT for a simulation or a quiz and it lands on your slide, live, ready to write on.",
    "**Class replay.** Every stroke is saved with its timing. Send the `.rpk` file to the class WhatsApp group; students replay it in their browser.",
    "**Bangla.** Bangla text and fonts work in lessons, and the interface is coming. Rupok is made in Bangladesh.",
    "**Open source, open format.** Your lessons are never locked in.",
  ],
  pick: [
    "**Pick Note3** if you teach on Windows, rely on its built-in physics and chemistry tools or formula recognition, and are happy to buy a licence for each device.",
    "**Pick Rupok** if you want one free tool on every board and laptop, interactive lessons made by AI, and replays your students can open on their phones.",
    "You can also use both: export a Note3 lesson to PDF and teach it in Rupok.",
  ],
  switchFirst: [{ b: "Export from Note3.", t: "Note3 exports lessons as PDF^4." }],
  faq: [
    { q: "Is there a free alternative to Note3?", a: "Yes. Rupok is free and open source. It runs in Chrome or Edge on any computer connected to your smartboard or panel, with no licence key, activation or account." },
    { q: "Is Note3 the same as EasiNote?", a: "Prestigio describes Note3 as the updated version of EasiNote, launched in July 2024^1. Its files use EasiNote’s `.enb` format^4." },
    { q: "Can Rupok open Note3 (.enb) files?", a: "No. Export the lesson from Note3 as a PDF and drop the PDF into Rupok; every page becomes a slide you can write on." },
    { q: "Does Rupok work on a Prestigio MultiBoard?", a: "Rupok needs Chrome or Edge with WebGPU or WebGL2. On a panel with a Windows PC module or a connected computer, open rupok.app/board there. Whether a panel’s built-in Android browser is fast enough varies by model, so a PC is the safe choice." },
    { q: "Is Rupok legal to use for free in schools?", a: "Yes. Rupok is open-source software under the AGPL-3.0. Any school can use it on any number of devices at no cost." },
  ],
  sources: [
    "[Unveiling Note3: the return of EasiNote](https://prestigio-solutions.com/news/unveiling-note3-the-return-of-easinote-transforming-education-and-business-presentations/), Prestigio Solutions, 16 July 2024.",
    "[Note3 interactive whiteboard software](https://prestigio-solutions.com/product/note3-interactive-whiteboard-software), Prestigio Solutions product page (checked 23 Nov 2026).",
    "[Note 3 (PSNTE3)](https://prestigioplaza.com/product/note-3-psnte3), Prestigio Plaza store listing, €29 (checked 23 Nov 2026).",
    "Note3 trial installer, version 3.1.4.3735, and the user manual inside it, [downloaded from Prestigio’s CDN](https://cdn0.it4profit.com/s3/cms/prestigio-solutions/NoteSetup_3.1.4.3735.exe) on 23 Nov 2026.",
  ],
});

// ------------------------------------------------------- SMART Notebook
const smart = comparePage({
  slug: "smart-notebook",
  name: "SMART Notebook",
  owner: "SMART Technologies",
  trademark: "SMART Notebook, SMART Board and Lumio are trademarks of SMART Technologies ULC.",
  title: "Free SMART Notebook Alternative: Rupok vs SMART Notebook (2026)",
  description:
    "A free, open-source alternative to SMART Notebook that works on any brand of smartboard. Compare price, platforms, AI, replay and languages, with sources. Checked November 2026.",
  h1: "A free SMART Notebook alternative for any board",
  lede: "SMART Notebook is the most established smartboard software, with a free basic version and paid Plus features. Rupok is free with every feature, open source, and works the same on any brand of panel. Here’s where each one wins.",
  rows: {
    price: "part: Basic Notebook is free; Notebook Plus with Lumio Standard is US$59/year^1. New installs get a 180-day Plus trial^4",
    oss: "no: No",
    runs: "Windows 10/11 and macOS^3. No Linux, iPad or Android app (the iPad app ended in 2020)^5. Lumio runs in a browser^6",
    account: "Online activation for the desktop app^3",
    ai: "part: Lumio AI Assist generates lessons, quizzes and activities, limited on the free plan^6^7. No AI in desktop Notebook release notes through August 2026^4",
    replay: "part: Lesson Recorder (Plus) records on-page actions, without sound^8",
    interactive: "yes: Game templates, assessments and SMART Blocks in Plus^2; Lumio games and widgets^6",
    pdf: "yes: Imports PDF, and PowerPoint on Windows^4",
    bangla: "no: 38 interface languages on Windows, no Bangla^3",
    offline: "yes: Installed desktop app; some features need internet^3",
    board: "part: Without Plus, touch on non-SMART displays is limited; Plus works with any brand^2",
    collab: "part: Student-device activities through Lumio^6",
    format: "`.notebook`; imports ActivInspire `.flipchart` files^4",
    status: "Actively developed: Notebook 26 released May 2026^4",
  },
  stronger: [
    "**Decades of classroom use**, and a huge base of `.notebook` lessons shared between teachers.",
    "**Game-based activities and assessments on student devices** through Lumio, with LMS integrations (Canvas, Schoology, D2L) on the Spark plan^6.",
    "**AI lesson and quiz generation is shipping** in Lumio^7.",
    "**Deep SMART hardware support:** pens, ink and touch tuned for SMART Boards.",
    "**Imports ActivInspire flipcharts**, with bulk conversion^4.",
  ],
  rupok: [
    "**Every feature is free, on any brand.** SMART’s free version limits touch on non-SMART displays^2; Rupok works the same on a SMART, Promethean, BenQ, ViewSonic or Prestigio panel.",
    "**Runs on Linux and ChromeOS too**, in Chrome or Edge. Nothing to install.",
    "**AI that builds real simulations,** not only quizzes: projectile motion, 3D molecules, circuits. It works with the AI you already use, including Claude through MCP.",
    "**Class replay students can open.** They replay every stroke from a small `.rpk` file, in the browser, with no account.",
    "**Bangla text in lessons**, and an open-source codebase and file format.",
  ],
  pick: [
    "**Pick SMART Notebook** if your school runs SMART Boards, uses Lumio’s student-device games or LMS integrations, or has years of `.notebook` lessons.",
    "**Pick Rupok** if your boards are mixed brands, you want every feature free, or you want AI-made simulations and class replays.",
  ],
  switchFirst: [{ b: "Save your Notebook lesson as a PDF.", t: "Export or print it to PDF from SMART Notebook." }],
  faq: [
    { q: "Is SMART Notebook free?", a: "The basic desktop version is free. Notebook Plus with Lumio Standard costs US$59 a year^1, and full touch on non-SMART displays needs Plus^2. Rupok is free with every feature." },
    { q: "What is the best free alternative to SMART Notebook?", a: "For a free desktop app with a long history, OpenBoard. For a free browser app with AI-made interactive lessons and class replay, Rupok. Both are open source." },
    { q: "Can Rupok open .notebook files?", a: "Not yet. Save the lesson as a PDF from SMART Notebook and drop it into Rupok to write on every page." },
    { q: "Does Rupok work on a SMART Board?", a: "Yes. Rupok runs in Chrome or Edge on the computer connected to the board, and works with the board’s touch and pen like any browser app." },
  ],
  sources: [
    "[Lumio and Notebook Plus pricing](https://www.smarttech.com/en-us/education/products/software/smart-notebook/lumio-and-notebook-plus-pricing), SMART Technologies (checked 23 Nov 2026).",
    "[About SMART Notebook](https://support.smarttech.com/docs/software/notebook/current/en/about/about-notebook.cshtml), SMART support (basic and Plus features).",
    "[SMART Notebook specifications](https://support.smarttech.com/docs/software/notebook/current/en/about/specifications/default.cshtml), SMART support.",
    "[SMART Notebook release notes](https://support.smarttech.com/docs/software/notebook/current/en/about/release-notes.cshtml), SMART support (Notebook 26.0, 20 May 2026; service pack 17 Aug 2026).",
    "[Discontinued software](https://support.smarttech.com/en/discontinued-products/discontinued-software), SMART support.",
    "[Lumio pricing](https://smarttech.com/lumio/pricing), SMART Technologies (checked 23 Nov 2026).",
    "[What’s new in Lumio](https://support.smarttech.com/docs/software/lumio/en/whats-new/whats-new.cshtml), SMART support (AI lessons 13 Jan 2026; activities July 2026).",
    "[Using Lesson Recorder](https://support.smarttech.com/docs/software/notebook/current/en/creating-lessons/using-lesson-recorder.cshtml), SMART support.",
  ],
});

// ------------------------------------------------------------- OpenBoard
const openboard = comparePage({
  slug: "openboard",
  name: "OpenBoard",
  owner: "the OpenBoard project",
  trademark: "OpenBoard is developed by its community and the Canton of Geneva’s education department.",
  title: "OpenBoard Alternative: Rupok vs OpenBoard, Two Free Whiteboards (2026)",
  description:
    "Rupok and OpenBoard are both free, open-source whiteboard software for teachers. Compare desktop vs browser, AI, replay, recording and languages. Facts checked November 2026.",
  h1: "Rupok vs OpenBoard: two free, open-source whiteboards",
  lede: "OpenBoard is a mature, free desktop whiteboard maintained in Geneva. Rupok is a newer free whiteboard that runs in the browser, with AI-made lessons and stroke-by-stroke replay. Both are open source, so this is about fit, not price.",
  rows: {
    price: "yes: Free, no registration^1",
    oss: "yes: Yes: GPL-3.0^3",
    runs: "Windows 10+, macOS 12+ and Debian, plus community Linux packages^3^4. No web, ChromeOS or mobile app",
    account: "yes: None^1",
    ai: "no: None",
    replay: "part: Podcast mode records the board and your voice as a video^5",
    interactive: "yes: Web widgets and about 25 built-in activities (memory, word games)^6",
    pdf: "yes: Imports and exports PDF^5; IWB (Common File Format) import and export^3",
    bangla: "no: 35 interface languages, no Bangla translation^3",
    offline: "yes: Installed desktop app^1",
    board: "yes: Any interactive whiteboard, graphics tablet or mouse^1",
    format: "`.ubz`^5",
    status: "Actively maintained: version 1.7.7 released 30 March 2026; 1.8 in development^2",
  },
  stronger: [
    "**A mature native desktop app** for Windows, macOS and Linux that works fully offline, with roots going back to 2003 (Uniboard, then Open-Sankoré)^1.",
    "**Video recording with sound** (Podcast mode)^5. Rupok replays strokes; audio is coming.",
    "**IWB / Common File Format** import and export, for swapping lessons with other whiteboard software^3.",
    "**A library of widgets and activities**, and a widget API^6.",
    "**Institutional backing:** maintained with the Canton of Geneva’s public education department^1.",
  ],
  rupok: [
    "**Nothing to install.** It runs in Chrome or Edge, so it also works on Chromebooks and locked-down school PCs.",
    "**AI Smart Objects.** Live simulations, 3D models and quizzes made by Claude, ChatGPT, Codex or OpenCode, with an MCP server for agents.",
    "**Stroke-by-stroke replay** in a small lesson file, instead of a large video. Students pause, zoom and still use the live objects.",
    "**GPU ink** that stays smooth with 100,000 strokes.",
    "**A student viewer** at [rupok.app/open](/open/): no account, nothing uploaded.",
    "**Bangla text and fonts** in lessons.",
  ],
  pick: [
    "**Pick OpenBoard** if you want a proven desktop app that works fully offline, records video with sound, or exchanges IWB files.",
    "**Pick Rupok** if you want to teach from any browser, add AI-made interactive lessons, or send students replayable classes.",
    "Both are free, so try both on your board for a week.",
  ],
  switchFirst: [{ b: "Export from OpenBoard.", t: "OpenBoard exports documents as PDF^5." }],
  faq: [
    { q: "What is a good OpenBoard alternative?", a: "If you want to stay free and open source, Rupok is the closest: same price (free), plus AI Smart Objects and stroke replay, running in the browser instead of as a desktop app." },
    { q: "Is OpenBoard still maintained?", a: "Yes. Version 1.7.7 came out on 30 March 2026, and 1.8 is in development^2." },
    { q: "Can Rupok open .ubz files?", a: "No. Export the document from OpenBoard as a PDF and drop it into Rupok." },
    { q: "Which licence do they use?", a: "OpenBoard is GPL-3.0^3. Rupok is AGPL-3.0, and its `.rpk` file format spec is Apache-2.0, so anyone can build compatible tools." },
  ],
  sources: [
    "[OpenBoard](https://openboard.ch/), official website (checked 23 Nov 2026).",
    "[OpenBoard releases](https://github.com/OpenBoard-org/OpenBoard/releases), GitHub (1.7.7, 30 Mar 2026).",
    "[OpenBoard source code and README](https://github.com/OpenBoard-org/OpenBoard), GitHub (licence, platforms, translations, IWB import/export in the source).",
    "[OpenBoard downloads](https://github.com/OpenBoard-org/OpenBoard/wiki/Downloads), GitHub wiki.",
    "[OpenBoard 1.6 tutorial](https://openboard.ch/download/Tutoriel_OpenBoard_1.6EN.pdf), official manual, 3 Mar 2022.",
    "[OpenBoard documentation](https://openboard.ch/en/documentation), official website.",
  ],
});

// ---------------------------------------------------------- ActivInspire
const activinspire = comparePage({
  slug: "activinspire",
  name: "ActivInspire",
  owner: "Promethean",
  trademark: "ActivInspire, ActivPanel and ActivSuite are trademarks of Promethean Limited.",
  title: "ActivInspire Alternative: Rupok vs ActivInspire, Free and Open Source (2026)",
  description:
    "A free, open-source alternative to Promethean ActivInspire for any interactive panel. Compare editions, price, platforms, AI and recording. Facts checked November 2026.",
  h1: "A free ActivInspire alternative for any panel",
  lede: "ActivInspire is Promethean’s long-running flipchart software, with a free Personal Edition and a paid Professional Edition. Rupok is free with every feature, runs in the browser, and adds AI-made lessons and class replay.",
  rows: {
    price: "part: Personal Edition is a free download; Professional comes with some ActivPanels or costs US$1,249 per device (US list price, Jan 2026)^1^2",
    oss: "no: No",
    runs: "Windows, macOS and Ubuntu^1. No web, ChromeOS or mobile app",
    ai: "no: None found^1",
    replay: "part: Screen and sound recorder in Professional only^1",
    interactive: "yes: Activities, polls and learner-response devices; question manager and actions in Professional^1",
    pdf: "part: PDF import and export in Professional only^1",
    bangla: "no: About 33 interface languages, no Bangla^1",
    board: "yes: Works with a wide range of interactive whiteboards and panels^1",
    format: "`.flipchart`; imports SMART Notebook files^1; exports `.iwb`^6",
    status: "Still offered. Latest build we could verify: 3.5.18 (Jan 2025)^3. Promethean’s newest software is ActivSuite (launched Jan 2025)^4",
  },
  stronger: [
    "**Years of flipcharts.** Many schools have large libraries of `.flipchart` lessons and Promethean resource packs.",
    "**Deep authoring tools** in Professional: actions, the question manager and self-paced learning^1.",
    "**Learner-response hardware** (ActivVote, ActivExpression) and Express Poll^1.",
    "**Imports SMART Notebook files**^1 and exports IWB files^6.",
    "**A Linux build**, alongside Windows and macOS^1^3.",
  ],
  rupok: [
    "**Every feature is free.** Recording, PDF import and export aren’t locked behind a paid edition.",
    "**Runs in the browser**, on Windows, macOS, Linux and ChromeOS, with nothing to install.",
    "**AI Smart Objects:** live simulations and quizzes from the AI you already use.",
    "**Replay for every student:** a small `.rpk` file they open in any browser.",
    "**Bangla text in lessons** and an open file format, so lessons aren’t locked to one vendor.",
    "**Actively developed** in the open, with a public roadmap.",
  ],
  pick: [
    "**Pick ActivInspire** if your school has a big flipchart library, uses Promethean learner-response devices, or needs Professional’s authoring tools.",
    "**Pick Rupok** if you want every feature free on any panel, AI-made interactive lessons, and replays students can open at home.",
  ],
  switchFirst: [{ b: "Export from ActivInspire.", t: "Professional exports flipcharts to PDF^1; with Personal Edition, print to PDF." }],
  faq: [
    { q: "Is ActivInspire being discontinued?", a: "We found no announced end date as of November 2026. Promethean’s newest classroom software is ActivSuite^4. It retired ClassFlow at the end of 2023, saying that wouldn’t affect ActivInspire^5." },
    { q: "Is ActivInspire free?", a: "The Personal Edition is a free download. The Professional Edition comes with some ActivPanels or costs US$1,249 per device on Promethean’s January 2026 US price list^1^2." },
    { q: "Can Rupok open .flipchart files?", a: "No. Export or print the flipchart to PDF and drop it into Rupok. (SMART Notebook can import flipcharts, if you need the objects kept.)" },
    { q: "Does Rupok work on a Promethean ActivPanel?", a: "Yes, from Chrome or Edge on the panel’s computer. Rupok works with any panel’s touch and pen like any browser app." },
  ],
  sources: [
    "[ActivInspire spec sheet v8.8](https://cdn.shopify.com/s/files/1/0753/1197/2628/files/ActivInspire_SS_1123v8.8_EN_INTL.pdf), Promethean, November 2023 (reseller-hosted copy; earlier [v8.3 on prometheanworld.com](https://www.prometheanworld.com/wp-content/uploads/pdf/ActivInspire_SS_0722v8.3_EN.pdf)).",
    "[Promethean REMC price list, January 2026](https://cdn.prometheanworld.com/docs/price-lists/REMC/REMC%20Price%20List%20-%20January%202026.pdf), Promethean (ActivInspire Professional, MSRP US$1,249 per device).",
    "[Promethean Linux package repository](https://activsoftware.co.uk/linux/repos/ubuntu/dists/focal/non-oss/binary-amd64/Packages) (activinspire 3.5.18, January 2025).",
    "[Promethean ActivSuite expands compatibility](https://d1io3yog0oux5.cloudfront.net/_1295307092f4552e2aa88fc5ad2122ee/mynd/news/2026-01-20_Promethean_ActivSuite_Expands_Compatibility_with_26.pdf), Mynd.ai press release, 20 Jan 2026.",
    "[ClassFlow](https://classflow.com), Promethean (sunset notice, 31 Dec 2023).",
    "[ActivInspire to Explain Everything migration guide](https://learn.prometheanworld.com/wp-content/uploads/dlm_uploads/2024/07/ActivInspire-to-Explain-Everything-Advanced-Migration-Guide_0724.pdf), Promethean, July 2024.",
  ],
});

// --------------------------------------------------- Microsoft Whiteboard
const msWhiteboard = comparePage({
  slug: "microsoft-whiteboard",
  name: "Microsoft Whiteboard",
  owner: "Microsoft",
  trademark: "Microsoft, Microsoft Whiteboard, Teams, OneNote and Copilot are trademarks of the Microsoft group of companies.",
  title: "Microsoft Whiteboard Alternative for Teachers: Rupok (2026)",
  description:
    "Microsoft retired the standalone Whiteboard apps in October 2026 and personal accounts can no longer edit boards. Rupok is a free, no-account whiteboard for teachers. Compare, with sources.",
  h1: "A Microsoft Whiteboard alternative that needs no account",
  lede: "In 2026 Microsoft retired the standalone Whiteboard apps, and personal Microsoft accounts lost the ability to create or edit boards. Whiteboard lives on for work and school accounts, on the web and in Teams. If that leaves you out, here’s how Rupok compares.",
  rows: {
    price: "part: Included with Microsoft 365 work or school accounts. Personal accounts can’t create or edit boards since 25 Sep 2026^2",
    oss: "no: No",
    runs: "Web and Microsoft Teams. The standalone Windows, iOS and Android apps retired on 16 Oct 2026^1^6",
    account: "Microsoft 365 work or school account^1^2",
    ai: "part: Copilot (suggest, categorize, summarize) with a paid Microsoft Copilot licence^3",
    replay: "no: No lesson replay found",
    interactive: "part: Templates, Forms polls, links and video^5",
    pdf: "part: Exports images; no PDF export documented^4",
    board: "yes: Any device with a supported browser",
    collab: "yes: Real-time co-editing with cursors, and live in Teams meetings^5",
    status: "Standalone apps retired 16 Oct 2026; continues on the web and in Teams for work/school accounts^1",
  },
  stronger: [
    "**Real-time collaboration:** many people drawing on one board at once, with cursors, live in Teams meetings^5. Rupok’s live class is still coming.",
    "**Teams and OneDrive integration**, with school identity, compliance and storage managed by IT.",
    "**Copilot** to suggest, sort and summarize ideas, if your school pays for Copilot^3.",
  ],
  rupok: [
    "**No account at all.** Teachers and students just open it; personal Microsoft accounts can no longer edit Whiteboard boards^2.",
    "**Made for presenting lessons:** write on PDF slides, Teach mode for the board, clicker support.",
    "**AI Smart Objects:** live simulations and quizzes on your slide.",
    "**Class replay** students open from a `.rpk` file.",
    "**PDF in and out:** import a PDF to teach from, and download your lesson as a PDF.",
    "**Open source, with Bangla text in lessons.**",
  ],
  pick: [
    "**Pick Microsoft Whiteboard** if your school is on Microsoft 365 and you need live co-editing in Teams meetings.",
    "**Pick Rupok** if you teach from slides on a smartboard, don’t have a work or school Microsoft account, or want AI-made interactive lessons and replays.",
  ],
  switchTitle: "Moving your boards to Rupok",
  switchFirst: [{ b: "Export your board.", t: "Whiteboard exports boards as images^4; drop the images into Rupok, or print the board to PDF." }],
  faq: [
    { q: "Is Microsoft Whiteboard being discontinued?", a: "The standalone apps for Windows, iOS and Android retired on 16 October 2026. For work and school accounts, Whiteboard continues on the web and in Teams^1." },
    { q: "Can I still use Microsoft Whiteboard with a personal account?", a: "Since 25 September 2026, personal Microsoft accounts can no longer create or edit whiteboards^2." },
    { q: "Is Rupok a good free replacement?", a: "For teaching from slides on a board or in an online class, yes: it’s free, needs no account, and adds AI Smart Objects and replay. For live co-editing with a group, it isn’t there yet; that’s coming with Rupok Cloud." },
  ],
  sources: [
    "[Retirement of the standalone Microsoft Whiteboard apps](https://support.microsoft.com/en-us/whiteboard/retirement-standalone-microsoft-whiteboard-apps), Microsoft Support (checked 23 Nov 2026).",
    "[Message center post MC1441775](https://mc.merill.net/message/MC1441775), Microsoft 365 (third-party mirror), first published 28 Jul 2026, updated 25 Aug 2026.",
    "[Welcome to Copilot in Whiteboard](https://support.microsoft.com/en-US/whiteboard/welcome-to-copilot-in-whiteboard), Microsoft Support.",
    "[Export a whiteboard](https://support.microsoft.com/en-US/whiteboard/export-a-whiteboard), Microsoft Support.",
    "[What’s new in Microsoft Whiteboard](https://support.microsoft.com/en-us/whiteboard/what-s-new-in-microsoft-whiteboard), Microsoft Support.",
    "[Message center post MC1469557](https://mc.merill.net/message/MC1469557), Microsoft 365 (third-party mirror), 9 Sep 2026.",
  ],
});

// --------------------------------------------------------------- Jamboard
const jamboard = comparePage({
  slug: "jamboard",
  name: "Jamboard",
  crumb: "Jamboard alternative",
  owner: "Google",
  trademark: "Google and Jamboard are trademarks of Google LLC.",
  title: "Free Jamboard Alternative for Teachers (2026): Rupok",
  description:
    "Google shut Jamboard down on 31 December 2024. Rupok is a free, open-source whiteboard for teachers: write on slides, add AI-made activities, and bring your old Jams back as PDFs.",
  h1: "Jamboard is gone. Here’s a free replacement for teachers.",
  lede: "Google stopped Jamboard editing on 1 October 2024 and wound the app down on 31 December 2024. Your old Jams became PDFs in Google Drive. Rupok is free and open source: drop those PDFs in and keep teaching on them.",
  rows: {
    runs: "no: Nowhere: no editing since 1 Oct 2024, and the app wound down on 31 Dec 2024^1",
    board: "no: The 55-inch Jamboard device reached end of life on 1 Oct 2024^2",
    format: "Old Jams were converted to PDFs in Google Drive^3",
    status: "Discontinued. Google pointed users to FigJam, Lucidspark and Miro^1",
  },
  tableNote: "Jamboard no longer exists, so most rows don’t apply. The rows above are what changed.",
  stronger: [
    "**It was simple and built into Google Workspace,** with real-time collaboration in Drive and Meet. Google’s recommended replacements (FigJam, Lucidspark and Miro)^1 are collaborative whiteboards too. If a whole class co-editing one board is what you need today, try those: Rupok’s live class is still coming.",
  ],
  rupok: [
    "**Free, with no account.** Nothing to sign up for, for you or your students.",
    "**Bring your Jams back.** Google converted Jams to PDFs in Drive^3. Download one and drop it into Rupok: every page becomes a slide you can write on.",
    "**Fast pen ink** on any smartboard, panel, projector or laptop with Chrome or Edge, including the classroom panel that replaced your Jamboard.",
    "**AI Smart Objects:** live simulations and quizzes on the slide.",
    "**Class replay:** students replay your lesson from a small `.rpk` file.",
    "**Open source,** so it can’t be switched off the way Jamboard was.",
  ],
  pick: [
    "**Pick FigJam, Lucidspark or Miro** if you mainly used Jamboard for students posting sticky notes together.",
    "**Pick Rupok** if you used Jamboard to teach: writing on slides at the front of the class, then sharing the result.",
  ],
  switchTitle: "Bringing your Jams to Rupok",
  switchFirst: [{ b: "Find your old Jams in Google Drive.", t: "Google converted them to PDF files^3. Download the ones you want." }],
  faq: [
    { q: "When did Jamboard shut down?", a: "Google announced the wind-down on 28 September 2023. Creating and editing Jams stopped on 1 October 2024, and the app wound down on 31 December 2024^1." },
    { q: "What happened to my Jams?", a: "Google’s help centre says Jams were converted to PDFs and are available in Google Drive^3. You can drop those PDFs into Rupok and write on them." },
    { q: "What did Google recommend instead of Jamboard?", a: "FigJam, Lucidspark and Miro, which integrate with Google Workspace^1." },
    { q: "What can I do with my old Jamboard device?", a: "Google says that since 1 October 2024 the device runs unlicensed: it still works as an offline whiteboard (without saving to Drive) or as an HDMI display^2." },
  ],
  sources: [
    "[The next phase of digital whiteboarding for Google Workspace](https://workspaceupdates.googleblog.com/2023/09/the-next-phase-of-digital-whiteboarding-for-google-workspace.html), Google Workspace Updates, 28 Sep 2023.",
    "[Jamboard device end of life](https://support.google.com/jamboard/answer/14084927?hl=en), Google Jamboard Help (checked 23 Nov 2026).",
    "[What happens to your Jams](https://support.google.com/jamboard/answer/13864841), Google Jamboard Help (checked 23 Nov 2026).",
  ],
});

// ------------------------------------------------------ Explain Everything
const ee = comparePage({
  slug: "explain-everything",
  name: "Explain Everything",
  owner: "Promethean",
  trademark: "Explain Everything is a trademark of Promethean Limited.",
  title: "Explain Everything Alternative: Free Whiteboard with Replay | Rupok",
  description:
    "Compare Rupok with Explain Everything: free plan limits, pricing, platforms, recording vs stroke replay, AI and languages. Facts checked November 2026, with sources.",
  h1: "An Explain Everything alternative with free, unlimited replay",
  lede: "Explain Everything, now part of Promethean, records polished video lessons on tablets, with a limited free plan. Rupok replays every stroke of your class from a small file, free and without limits, and runs on your classroom board.",
  rows: {
    price: "part: Free plan: 3 projects, 1 slide per project, 3-minute videos. Advanced US$69/year (education); Pro US$7.49/month billed yearly^1",
    oss: "no: No",
    runs: "iPad, iPhone, Android, Chromebook and a browser app^1",
    account: "An account to create projects^1",
    ai: "part: AI activities and an AI Companion on paid plans, on Apple devices with Apple Intelligence^2^5",
    replay: "yes: Records narrated video lessons, with editing and captions; exports MP4^2",
    interactive: "part: AI-made crossword, memory and matching activities; polls, timers and spinners in Advanced^1^5",
    pdf: "yes: Imports PDF; exports PDF, video, images and `.explain` files^2",
    bangla: "no: 15 interface languages, no Bangla^2",
    offline: "part: Tablet apps work offline with some features missing; the browser app is online-only^4",
    collab: "yes: Real-time collaboration; one-to-one sessions of 15 minutes on the free plan^1",
    format: "`.explain`^2",
    status: "Actively developed; owned by Promethean since November 2022^3",
  },
  stronger: [
    "**Polished video lessons:** record with your voice, edit, add captions and export MP4^2. Rupok replays strokes; replay with audio is coming.",
    "**Native tablet apps** for iPad, iPhone and Android that work offline^1^4.",
    "**Real-time collaboration** on paid plans^1.",
    "**Many export formats:** video, PDF, images and GIFs^2.",
    "**On-device AI** for activities and lesson plans on recent Apple devices^2^5.",
  ],
  rupok: [
    "**No limits on the free plan:** unlimited lessons, pages and replay length.",
    "**Replay, not video:** students get every stroke in a small file. They can pause, zoom and still use the live Smart Objects, which a video can’t do.",
    "**AI from any assistant**, not tied to an Apple device or a paid plan: Claude, ChatGPT, Codex or OpenCode.",
    "**Built for the classroom board:** fast GPU ink on smartboards and panels in Chrome or Edge.",
    "**Bangla text in lessons**, and open source.",
  ],
  pick: [
    "**Pick Explain Everything** if you make narrated video lessons on an iPad, or need collaboration on tablets.",
    "**Pick Rupok** if you teach live on a board or in an online class and want free, unlimited replays and AI-made interactive lessons.",
  ],
  switchFirst: [{ b: "Export your project as PDF.", t: "Explain Everything exports PDF^2." }],
  faq: [
    { q: "Is Explain Everything free?", a: "There’s a free plan with 3 projects, 1 slide per project and 3-minute videos. Paid plans start at US$69 a year for education (Advanced) or US$7.49 a month billed yearly (Pro)^1." },
    { q: "Who owns Explain Everything?", a: "Promethean acquired Explain Everything in November 2022^3." },
    { q: "How is Rupok’s replay different from a video?", a: "Rupok saves each pen stroke with its timing, so a replay redraws the lesson. The file stays small, students can zoom in, and live Smart Objects keep working. It has no sound yet; replay with audio is coming." },
  ],
  sources: [
    "[Explain Everything pricing](https://explaineverything.com/pricing/), official (checked 23 Nov 2026).",
    "[Explain Everything Whiteboard](https://apps.apple.com/us/app/explain-everything-whiteboard/id1020339980), App Store listing by Promethean Ltd (version 10.0.5; checked 23 Nov 2026).",
    "[Promethean acquires Explain Everything](https://www.nasdaq.com/press-release/promethean-acquires-explain-everything-to-continue-delivering-transformational), press release, 28 Nov 2022.",
    "[Explain Everything in a nutshell](https://explaineverything.com/explain-everything-in-a-nutshell/), official.",
    "[New AI features with Explain Everything Advanced and iOS 26](https://learn.prometheanworld.com/articles/exciting-new-ai-features-with-explain-everything-advanced-and-ios-26/), Promethean.",
  ],
});

// ---------------------------------------------------------------- OneNote
const onenote = comparePage({
  slug: "onenote",
  name: "OneNote",
  owner: "Microsoft",
  trademark: "Microsoft, OneNote and Copilot are trademarks of the Microsoft group of companies.",
  title: "OneNote Alternative for Teaching on a Smartboard: Rupok vs OneNote",
  description:
    "OneNote is a great notebook; Rupok is built for teaching from a smartboard. Compare price, ink replay, AI, Class Notebook, platforms and Bangla. Facts checked November 2026.",
  h1: "Rupok vs OneNote for teaching on a board",
  lede: "Many teachers write on OneNote at the front of the class. OneNote is a superb notebook for every device. Rupok is built for presenting: slides, a Teach mode for the board, live AI-made objects, and a replay students can open anywhere.",
  rows: {
    price: "part: Free to download and use; Ink Replay, Math Assistant and Copilot need Microsoft 365 or Copilot subscriptions^1^5",
    oss: "no: No",
    runs: "Windows, Mac, web, iPhone, iPad and Android^1",
    account: "A Microsoft account (the Windows app can also keep local notebooks)^1",
    ai: "part: Copilot summarizes, rewrites and answers questions, with a paid subscription^5",
    replay: "part: Ink Replay redraws strokes in order, on Windows only^2",
    interactive: "Notes, ink and embedded files",
    pdf: "yes: Insert PDF printouts and ink on them; export to PDF^8",
    bangla: "part: Bangla (Bangladesh) gets proofing tools only; Bangla (India) is partly localized^6",
    offline: "yes: Native apps work offline and sync later^9",
    collab: "yes: Co-editing; Class Notebook gives each student a private section^3",
    status: "Actively developed. OneNote for Windows 10 reached end of support on 14 Oct 2025^4",
  },
  stronger: [
    "**Notebooks on every device**, with offline work and sync^1^9.",
    "**Class Notebook:** private student sections, a read-only content library, a collaboration space, page distribution and feedback, with rosters from Teams or an LMS^3.",
    "**Co-editing** and a huge Microsoft 365 ecosystem; Office 365 A1 is free for eligible schools^7.",
    "**Math Assistant and handwriting search** (Math Assistant needs a qualifying subscription).",
  ],
  rupok: [
    "**Built for presenting:** slide pages, a Teach mode that keeps the slide fitted to the board, and clicker support.",
    "**Live Smart Objects:** simulations and quizzes made by your AI, running on the slide.",
    "**Replay anywhere:** students replay every stroke in any browser. OneNote’s Ink Replay is Windows-only^2.",
    "**No account** for teachers or students.",
    "**Fast GPU ink** that stays smooth through a long class.",
    "**Open source,** with Bangla text in lessons.",
  ],
  pick: [
    "**Pick OneNote** for class notebooks, homework distribution and notes across every device in a Microsoft 365 school.",
    "**Pick Rupok** for teaching at the board: writing on slides, live AI-made objects, and replays for students.",
    "Plenty of teachers use both: OneNote for the notebook, Rupok for the lesson.",
  ],
  switchTitle: "Moving a OneNote lesson to Rupok",
  switchFirst: [{ b: "Export the pages as PDF.", t: "OneNote exports pages, sections or notebooks to PDF on Windows, and single pages on the web and Mac^8." }],
  faq: [
    { q: "Is OneNote free for teachers?", a: "OneNote is free to download and use. Some features, such as Ink Replay and Math Assistant, and Copilot need a Microsoft 365 or Copilot subscription^1^5. Eligible schools can get Office 365 A1 for free^7." },
    { q: "Does OneNote replay handwriting?", a: "Yes, Ink Replay redraws strokes in order, but only on Windows^2. Rupok’s replay works in any browser, including students’ phones." },
    { q: "Should I switch from OneNote to Rupok?", a: "Not necessarily. Keep OneNote for notebooks and use Rupok for presenting at the board, where slides, live objects and replay matter." },
  ],
  sources: [
    "[What’s the difference between the versions of OneNote?](https://support.microsoft.com/topic/a624e692-b78b-4c09-b07f-46181958118f), Microsoft Support.",
    "[Ink Replay](https://support.microsoft.com/en-au/onenote/onenote-help-and-learning/ink-replay), Microsoft Support.",
    "[Using OneNote Class Notebooks with a free Microsoft 365 account](https://support.microsoft.com/en-us/education/onenote/using-onenote-class-notebooks-with-a-free-microsoft-365-account-a-guide-for-teachers-and-educators), Microsoft Support.",
    "[What is happening to OneNote for Windows 10](https://support.microsoft.com/en-US/OneNote/what-is-happening-to-onenote-for-windows-10), Microsoft Support.",
    "[Welcome to Copilot in OneNote](https://support.microsoft.com/en-us/onenote/welcome-to-copilot-in-onenote), Microsoft Support.",
    "[Language Accessory Pack for Microsoft 365](https://support.microsoft.com/en-us/office/language-accessory-pack-for-microsoft-365-82ee1236-0f9a-45ee-9c72-05b026ee809f), Microsoft Support.",
    "[Office 365 Education](https://www.microsoft.com/en-us/education/products/office), Microsoft.",
    "[Export notes from OneNote as a PDF](https://support.microsoft.com/en-US/OneNote/onenote-help-and-learning/export-notes-from-onenote-as-a-pdf), Microsoft Support.",
    "[Work offline in OneNote for iPad or iPhone](https://support.microsoft.com/en-us/onenote/work-offline-in-microsoft-onenote-for-ipad-or-iphone), Microsoft Support.",
  ],
});

// -------------------------------------------------------------------- hub
const HUB_ROWS = [
  ["**Rupok**", "yes: Free", "yes: AGPL-3.0", "Chrome/Edge on any OS", "yes: AI Smart Objects from your own AI", "yes: Stroke-by-stroke replay", "part: Text yes; interface soon"],
  ["[Note3](/compare/note3/)", "no: €29 per device", "no: No", "Windows", "no: None documented", "no: None documented", "no: No"],
  ["[SMART Notebook](/compare/smart-notebook/)", "part: Free basic; Plus US$59/yr", "no: No", "Windows, macOS", "part: In Lumio (web)", "part: Lesson Recorder (Plus)", "no: No"],
  ["[OpenBoard](/compare/openboard/)", "yes: Free", "yes: GPL-3.0", "Windows, macOS, Linux", "no: None", "part: Video podcast", "no: No"],
  ["[ActivInspire](/compare/activinspire/)", "part: Free Personal; Pro US$1,249/device", "no: No", "Windows, macOS, Ubuntu", "no: None found", "part: Recorder (Pro)", "no: No"],
  ["[Microsoft Whiteboard](/compare/microsoft-whiteboard/)", "part: Work/school accounts", "no: No", "Web, Teams", "part: Copilot (paid)", "no: None found", "—"],
  ["[Explain Everything](/compare/explain-everything/)", "part: Free (3 projects); from US$69/yr", "no: No", "Tablets, Chromebook, web", "part: Paid, Apple devices", "yes: Narrated video", "no: No"],
  ["[OneNote](/compare/onenote/)", "part: Free; extras need Microsoft 365", "no: No", "Every major OS, web", "part: Copilot (paid)", "part: Ink Replay (Windows)", "part: Proofing only"],
  ["[Jamboard](/compare/jamboard/)", "no: Discontinued 31 Dec 2024", "no: No", "—", "—", "—", "—"],
];

const hub = {
  path: "/compare/",
  lang: "en",
  title: "Smartboard & Whiteboard Software Compared (2026): Rupok vs the Rest",
  description:
    "An honest comparison of smartboard and whiteboard software for teachers: Rupok, Note3, SMART Notebook, OpenBoard, ActivInspire, Microsoft Whiteboard, Explain Everything, OneNote and Jamboard.",
  tag: "Compare",
  h1: "Rupok compared with other whiteboard software",
  lede: "Price, open source, platforms, AI, replay and Bangla, side by side. Every fact about another product was checked on 23 November 2026 and is sourced on its comparison page, including where they beat us.",
  crumbs: [["Compare"]],
  cta: [{ href: "#table", label: "See the table" }, { href: "/board/", label: "Try Rupok free", kind: "ghost" }],
  aside: VS_ART("8 others"),
  sections: [
    {
      id: "table",
      h2: "The feature table",
      lede: "Swipe sideways on a phone. ✓ yes · ◐ partly or paid · ✗ no · ◷ coming soon.",
      blocks: [
        { table: { wide: true, hl: "row", caption: "Whiteboard and smartboard software for teachers, as of November 2026", head: ["Software", "Price", "Open source", "Runs on", "AI", "Replay / recording", "Bangla interface"], rows: HUB_ROWS } },
        { note: "Sources for every row are on each product’s comparison page. Rupok’s Bangla interface, desktop app, offline install and live class are coming soon; everything else in Rupok’s row ships today." },
      ],
    },
    {
      id: "pages",
      band: "mint",
      h2: "Detailed comparisons",
      blocks: [
        {
          links: [
            { href: "/compare/note3/", b: "Rupok vs Note3", t: "A free alternative to Prestigio’s Windows-only EasiNote successor." },
            { href: "/compare/smart-notebook/", b: "Rupok vs SMART Notebook", t: "Every feature free, on any brand of board." },
            { href: "/compare/openboard/", b: "Rupok vs OpenBoard", t: "Two free, open-source whiteboards: desktop vs browser." },
            { href: "/compare/jamboard/", b: "Jamboard alternative", t: "Jamboard shut down in 2024. Bring your Jams back as PDFs." },
            { href: "/compare/microsoft-whiteboard/", b: "Rupok vs Microsoft Whiteboard", t: "No account needed, after the 2026 app retirement." },
            { href: "/compare/activinspire/", b: "Rupok vs ActivInspire", t: "Free with every feature, in the browser." },
            { href: "/compare/explain-everything/", b: "Rupok vs Explain Everything", t: "Unlimited stroke replay instead of short videos." },
            { href: "/compare/onenote/", b: "Rupok vs OneNote", t: "A notebook vs a tool for presenting at the board." },
          ],
        },
      ],
    },
    {
      id: "how",
      h2: "How we compare",
      blocks: [
        { ticks: [
          "We only use the other product’s own pages (pricing, release notes, help centre, app store listing), and say when a source is a mirror or a reseller copy.",
          "Every claim has a date. Software changes; if we’re out of date, [tell us on GitHub](https://github.com/Shaitrish/rupok/issues) and we’ll fix it.",
          "We say where the other product is stronger. Rupok is young: some features (live class, desktop app, Bangla interface) are still coming, and we label them that way.",
          "No logos, no made-up numbers.",
        ] },
      ],
    },
  ],
  faq: [
    { q: "What is the best smartboard software?", a: "It depends on your boards and what you teach. SMART Notebook and ActivInspire suit schools invested in SMART or Promethean hardware. OpenBoard is a solid free desktop app. Rupok is free, open source and works on any board, with AI-made interactive lessons and class replay." },
    { q: "What is the best free smartboard software?", a: "The free options are Rupok and OpenBoard (both open source), plus the free basic editions of SMART Notebook and ActivInspire. Only Rupok and OpenBoard are free with every feature on any brand of board." },
    { q: "Is there open-source smartboard software?", a: "Yes: Rupok (AGPL-3.0, runs in the browser) and OpenBoard (GPL-3.0, desktop app)." },
    { q: "Which whiteboard software supports Bangla?", a: "Rupok supports Bangla text and fonts in lessons today, and its Bangla interface is coming. Note3, SMART Notebook, OpenBoard, ActivInspire and Explain Everything don’t list Bangla among their interface languages. Microsoft 365 apps such as OneNote offer Bangla proofing tools and partial Bangla (India) localization." },
  ],
  final: {
    h2: "The best test is your own board.",
    p: "Open Rupok on your smartboard and write your first line in five seconds.",
    cta: [{ href: "/board/", label: "Open the board" }],
  },
};

export const COMPARE = [hub, note3, smart, openboard, jamboard, msWhiteboard, activinspire, ee, onenote];
