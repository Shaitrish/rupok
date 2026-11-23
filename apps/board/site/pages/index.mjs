// Every page the site builder knows. Order = order in llms-full.txt.
import { BN } from "./bn.mjs";
import { COMPARE } from "./compare.mjs";
import { FEATURES } from "./features.mjs";
import { OPEN } from "./open.mjs";
import { RPK } from "./rpk.mjs";
import { DOWNLOAD } from "./download.mjs";

/** The home page. Its HTML is hand-written in index.html; the builder only
 * injects the FAQ and JSON-LD below (between <!-- site:… --> markers) and
 * writes /index.md. Keep `faq` in step with what the page says. */
export const HOME = {
  path: "/",
  lang: "en",
  alt: { bn: "/bn/" },
  html: false,
  title: "Rupok — Free Open-Source Smartboard & AI Whiteboard for Teachers",
  description:
    "Rupok is free, open-source smartboard and whiteboard software for teachers. Write on your slides with fast GPU ink, drop in live lessons your AI made, and replay every class. Works on any board.",
  h1: "Rupok: teach with ink that comes alive",
  lede: "Rupok is free, open-source smartboard and whiteboard software for teachers. Write on your slides, drop in live lessons your AI made, and replay every class, stroke by stroke. It runs in Chrome or Edge on any smartboard, interactive panel, laptop or tablet, with no account. Open it at [rupok.app/board](/board/).",
  sections: [
    {
      h2: "Draw: a pen that keeps up with you",
      blocks: ["GPU ink draws a stroke in under a millisecond, even two hours and thousands of strokes into a class. Pressure, palm rejection, highlighter, shapes and a laser pointer, on an 86-inch panel or a laptop."],
    },
    {
      h2: "AI lessons: ask for a lesson, teach it a minute later",
      blocks: ["Connect the AI you already use (Claude, Codex, OpenCode) and ask for a simulation, a 3D model or a quiz. It lands on your slide as a live Smart Object you can write on and step through with your clicker. See [the AI whiteboard](/ai-whiteboard/)."],
    },
    {
      h2: "Replay: every class, on replay",
      blocks: ["Every stroke is saved with its timing. Students who missed the class open the lesson and watch it unfold exactly as it was taught, objects, ink and all."],
    },
    {
      h2: "Students: open the lesson, replay the class",
      blocks: ["Got an [.rpk file (Rich Presentation Kit)](/rpk/) from your teacher? Open it at [rupok.app/open](/open/): no account, no download, with every live object still working."],
    },
    {
      h2: "Pricing: the app is free, forever",
      blocks: [
        "Rupok is open source. Rupok Cloud adds sync, sharing and live classes, starting with a free plan for every teacher.",
        { ul: [
          "**Free (৳0):** the full app and every feature. Coming soon: 500 MB cloud sync, a Claude connector, student share links.",
          "**Pro (coming soon):** large cloud storage, live class for students, Rupok AI credits, quiz and attendance insights.",
          "**School (coming soon):** admin for all teachers, classes and student lists, a shared course library.",
        ] },
      ],
    },
    {
      h2: "More",
      blocks: [{ links: [
        { href: "/smartboard-software/", b: "Smartboard software", t: "Free software for any interactive panel or smart board." },
        { href: "/online-class-whiteboard/", b: "Online class whiteboard", t: "Teach on Zoom, Meet or Teams; send a replay." },
        { href: "/for-teachers/", b: "For teachers", t: "Plan, teach, replay, share." },
        { href: "/compare/", b: "Compare", t: "Rupok vs Note3, SMART Notebook, OpenBoard, Jamboard and more." },
        { href: "/bn/", b: "বাংলা", t: "Rupok: শিক্ষকদের জন্য ফ্রি স্মার্ট বোর্ড সফটওয়্যার।" },
      ] }],
    },
  ],
  faqTitle: "Questions teachers ask",
  faq: [
    { q: "Is Rupok really free?", a: "Yes. The whole app is free and open source, with every feature, on every device. Rupok Cloud, a hosted add-on for sync, sharing and live classes, is coming with a free plan for every teacher." },
    { q: "Does it work with my smartboard or interactive panel?", a: "Yes, with any brand: SMART, Promethean, BenQ, ViewSonic, Prestigio and others. Rupok runs in Chrome or Edge on the computer connected to the board, and uses its touch and pen like any browser app." },
    { q: "Do I need to install anything?", a: "No. Open [rupok.app/board](/board/) in Chrome or Edge. You can install it from the browser menu so it opens in its own window. A desktop app for Windows and Linux is coming soon." },
    { q: "How does the AI part work?", a: "Ask the AI you already use for a simulation, 3D model or quiz as a single HTML file, and paste it onto the board. Claude Code, Codex and OpenCode can build whole lessons through Rupok’s MCP server. [More about the AI whiteboard](/ai-whiteboard/)." },
    { q: "How do students open my lessons?", a: "Save the lesson as a `.rpk` file and send it on WhatsApp, Gmail or Google Drive. Students open it at [rupok.app/open](/open/) on a phone or computer, with no account, and replay the class." },
    { q: "What is an RPK file?", a: "An RPK (Rich Presentation Kit) file, `.rpk`, is a Rupok lesson: its slides, the teacher’s ink with timing, live Smart Objects and a readable outline that AI can read, in one open ZIP package. Open one free at [rupok.app/open](/open/); see [the RPK file format](/rpk/)." },
    { q: "Is Rupok open source?", a: "Yes. The app is AGPL-3.0, and the `.rpk` file format, the Smart Object SDK and the AI skills are Apache-2.0. The code is on [GitHub](https://github.com/Shaitrish/rupok)." },
    { q: "Does Rupok support Bangla?", a: "Bangla text and fonts work in lessons today, and a Bangla interface is coming. [বাংলায় পড়ুন](/bn/)." },
    { q: "How is Rupok different from SMART Notebook, OpenBoard or Note3?", a: "Rupok is free with every feature on any board, runs in the browser, and adds AI-made Smart Objects and class replay. Those apps are stronger in other ways, such as mature desktop tools and content libraries. See [the honest comparison](/compare/)." },
  ],
};

const tag = (source) => (page) => ({ page, source });

export const PAGES = [
  tag("index.mjs")(HOME),
  ...BN.map(tag("bn.mjs")),
  ...OPEN.map(tag("open.mjs")),
  ...RPK.map(tag("rpk.mjs")),
  ...DOWNLOAD.map(tag("download.mjs")),
  ...FEATURES.map(tag("features.mjs")),
  ...COMPARE.map(tag("compare.mjs")),
];
