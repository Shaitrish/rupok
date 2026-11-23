// The RPK file format page: "rpk file", "what is an rpk file", "rpk file
// format", "Rich Presentation Kit", "rpk to pdf", "can ChatGPT read rpk"
// (English) and "আরপিকে ফাইল ফরম্যাট" (Bangla). The viewer itself is /open/.
// Also served to agents as /rpk.md; the agent skill is /skills/read-rpk.md
// (copied from skills/read-rpk/ by site/build.mjs).

import { GITHUB, SITE } from "../lib.mjs";

const SPEC = `${GITHUB}/blob/main/spec/rupok-format.md`;
const SKILL_REPO = `${GITHUB}/tree/main/skills/read-rpk`;

const TREE = `lesson.rpk                 a ZIP archive
├── mimetype               application/vnd.rupok.lesson+zip (first entry)
├── manifest.json          title, dates, course and chapter, page count
├── README.md              "This is a Rupok lesson…" + the outline
├── content.md             the lesson as readable Markdown
├── content.json           the same outline, structured
├── doc.loro               the lesson itself: pages, ink, history (Loro CRDT)
├── assets/<hash>.png      images and media
├── objects/<id>/index.html  interactive Smart Objects (HTML)
└── thumbs/<n>.png         page previews (optional)`;

const EXAMPLE = `# Binary numbers

- Subject: ICT
- Chapter: Number systems
- Pages: 3

## Page 1: Place value in base 2

> Each place is worth twice the place to its right.

$$1011_2 = 8 + 2 + 1 = 11$$

- Handwriting: 3 ink strokes
  - recognised maths (machine reading, may be wrong): $x^2$
- Shapes and diagrams: coordinate axes

## Page 2: Logic gates

- Shapes and diagrams: AND gate, OR gate, 2 × arrow

## Page 3: Try it

- Image: \`assets/6c91…2ee8.png\` (480 × 320)
- Smart Object: **Binary counter** (8 steps, \`objects/binary-counter/index.html\`)`;

const LORO = `pip install loro          # Python
npm install loro-crdt     # JavaScript

python3 read_rpk.py lesson.rpk        # the skill's script: .rpk → Markdown
rupok export --md lesson.rpk          # the Rupok CLI does the same`;

const ld = (lang, path, name, description) => [
  {
    "@type": "DefinedTerm",
    "@id": `${SITE}${path}#term`,
    name: "RPK file",
    alternateName: ["Rich Presentation Kit", ".rpk", "Rupok lesson file"],
    description,
    inLanguage: lang,
    url: SITE + path,
    inDefinedTermSet: { "@type": "DefinedTermSet", name: "Rupok file formats", url: `${SITE}/rpk/` },
  },
  {
    "@type": "CreativeWork",
    "@id": `${SITE}${path}#format`,
    name,
    alternateName: "Rupok Rich Presentation Kit (.rpk) lesson package format",
    about: { "@id": `${SITE}${path}#term` },
    encodingFormat: "application/vnd.rupok.lesson+zip",
    url: SPEC,
    license: "https://www.apache.org/licenses/LICENSE-2.0",
    inLanguage: "en",
    isAccessibleForFree: true,
    creator: { "@id": `${SITE}/#org` },
    isBasedOn: "https://loro.dev",
  },
];

// ----------------------------------------------------------------- English
const en = {
  path: "/rpk/",
  lang: "en",
  alt: { bn: "/bn/rpk/" },
  title: "RPK File Format (Rich Presentation Kit): What Is a .rpk File? | Rupok",
  ogTitle: "RPK file format (Rich Presentation Kit)",
  description:
    "An RPK (Rich Presentation Kit) file is a Rupok lesson: slides, handwriting, live activities and a class replay in one open ZIP package. What's inside, how to open it, convert it to PDF, and read it with AI.",
  crumbs: [["RPK file format"]],
  tag: "File format",
  h1: "RPK file format (Rich Presentation Kit)",
  lede: "An **RPK file** (`.rpk`, short for **Rich Presentation Kit**) is a lesson made with Rupok, the free, open-source whiteboard for teachers. One file holds the slides, everything written on them, live interactive activities and a replay of the class. The format is open, documented and readable by people, scripts and AI.",
  cta: [{ href: "/open/", label: "Open a .rpk file" }, { href: SPEC, label: "Read the spec", kind: "ghost" }],
  fine: "Free · Opens in the browser · Apache-2.0 specification",
  ld: ld("en", "/rpk/", "RPK file format (Rich Presentation Kit)", "A .rpk (Rich Presentation Kit) file is a Rupok lesson: a ZIP package with the lesson's pages, ink with timing, images, interactive Smart Objects and a readable Markdown outline."),
  sections: [
    {
      id: "what",
      split: true,
      h2: "What is an RPK file?",
      copy: [
        "RPK stands for **Rich Presentation Kit**. Teachers make `.rpk` lessons in [Rupok](/board/) and share them with students like slide decks, on WhatsApp, Gmail or Google Drive. Unlike a PDF, the lesson stays alive:",
        { ticks: [
          "**Pages:** slides (often from a PDF) or infinite boards.",
          "**Ink:** every pen stroke, with its timing, so the class can be replayed.",
          "**Smart Objects:** simulations, 3D models and quizzes that still run.",
          "**A readable outline:** the lesson as Markdown and JSON, for search and AI.",
        ] },
      ],
      blocks: [
        { dl: [
          ["Full name", "Rich Presentation Kit (Rupok lesson)"],
          ["Extension", "`.rpk`"],
          ["Media type", "`application/vnd.rupok.lesson+zip`"],
          ["Container", "ZIP, with a `mimetype` first entry (like EPUB)"],
          ["Opens with", "Rupok, free in Chrome or Edge ([rupok.app/open](/open/))"],
          ["Specification", `Open, Apache-2.0 ([spec on GitHub](${SPEC}))`],
        ] },
      ],
    },
    {
      id: "inside",
      band: "mint",
      h2: "What's inside a .rpk file",
      lede: "A `.rpk` is a ZIP archive. Rename a copy to `.zip` and you can look inside:",
      blocks: [
        { code: TREE, lang: "text" },
        "`doc.loro` is the authoritative lesson: a [Loro](https://loro.dev) CRDT document that keeps its whole edit history, so one file is the lesson, its undo history and its class recording. `README.md`, `content.md` and `content.json` are plain-text summaries Rupok rewrites on every save, so anyone can read a lesson without special software.",
      ],
    },
    {
      id: "open",
      h2: "How to open an RPK file",
      blocks: [
        { steps: [
          { b: "Go to rupok.app/open", t: "in Chrome or Edge, on a phone or a computer." },
          { b: "Drop the .rpk file on the page", t: "or tap “Choose a .rpk file”. It opens in Student mode, read-only." },
          { b: "Replay the class,", t: "flip through the pages and try the live activities." },
        ] },
        { cta: [{ href: "/open/", label: "Open the RPK viewer" }] },
        { note: "Nothing is uploaded: the file is opened by your browser on your device. Teachers edit lessons at rupok.app/board." },
      ],
    },
    {
      id: "pdf",
      band: "sun",
      h2: "How to convert an RPK file to PDF",
      blocks: [
        { steps: [
          { b: "Open the file", t: "at [rupok.app/open](/open/)." },
          { b: "Press Download PDF.", t: "Every page is exported with the teacher's ink on it." },
          { b: "Print it or share it.", t: "The PDF is made in your browser; nothing leaves your device." },
        ] },
      ],
    },
    {
      id: "ai",
      h2: "How AI and developers read RPK files",
      lede: "Every `.rpk` describes itself. Give one to ChatGPT, Claude or an agent and it can tell what the lesson contains.",
      blocks: [
        { cols: [
          [
            { h3: "For ChatGPT, Claude and other chatbots" },
            { steps: [
              { b: "Upload the .rpk file", t: "(or a link to it) and ask about the lesson." },
              { b: "The model unzips it", t: "and reads `README.md` and `content.md`: titles, text, maths, diagrams, images and activities, page by page." },
              { b: "Interactive parts", t: "are HTML files in `objects/`, which the model can read too." },
            ] },
          ],
          [
            { h3: "For agents and developers" },
            { ul: [
              "Agent skill: [rupok.app/skills/read-rpk.md](/skills/read-rpk.md), or [download it with its script](" + SKILL_REPO + ").",
              "Markdown twin of this page: [rupok.app/rpk.md](/rpk.md).",
              "Full fidelity: read `doc.loro` with the official Loro packages (`pip install loro`, `npm install loro-crdt`).",
              "Command line: `rupok export --md lesson.rpk` prints the outline.",
            ] },
          ],
        ] },
        { code: LORO },
      ],
    },
    {
      id: "example",
      band: "mint",
      h2: "Example: the outline inside a lesson",
      lede: "This is the `content.md` Rupok wrote for a short ICT lesson ([download the sample .rpk](/skills/sample-lesson.rpk)).",
      blocks: [{ code: EXAMPLE, lang: "markdown" }],
    },
  ],
  faq: [
    { q: "What does RPK stand for?", a: "RPK stands for Rich Presentation Kit. A .rpk file is a lesson made with Rupok, the free, open-source whiteboard for teachers." },
    { q: "What is an RPK file?", a: "An RPK file is one whole lesson: its pages, the teacher's handwriting with timing, images, live Smart Objects (simulations, 3D models, quizzes) and a readable outline. It is a ZIP package with an open, documented format." },
    { q: "How do I open an RPK file?", a: "Go to [rupok.app/open](/open/) in Chrome or Edge and drop the file on the page. It opens free in your browser, with no account and nothing uploaded." },
    { q: "Is there a free RPK file viewer?", a: "Yes. Rupok's student viewer at [rupok.app/open](/open/) is free, runs in the browser on phones and computers, and replays the class stroke by stroke." },
    { q: "How do I convert an RPK file to PDF?", a: "Open the file at [rupok.app/open](/open/), then press Download PDF. Every page is exported with the teacher's ink, on your device." },
    { q: "Can ChatGPT or Claude read an RPK file?", a: "Yes. Every .rpk made by current Rupok versions carries `README.md`, `content.md` and `content.json`: the lesson as plain Markdown and JSON. Upload the file and ask about it; agents can also use the [read-rpk skill](/skills/read-rpk.md)." },
    { q: "Is the RPK format open?", a: `Yes. The specification and the reference reader and writer are Apache-2.0, so anyone can build tools that read or write .rpk files. See the [spec on GitHub](${SPEC}).` },
    { q: "Can I open a .rpk file as a ZIP?", a: "Yes. Rename a copy to .zip, or open it with any ZIP tool. Start with `content.md`; the lesson itself is `doc.loro`, which the Loro libraries for Python and JavaScript read." },
    { q: "My .rpk file isn't a Rupok lesson. What is it?", a: "Other programs also use the .rpk extension, for example Esri CityEngine rule packages, Tizen resource packages and TI-99/4A cartridge files for the MAME emulator. If [rupok.app/open](/open/) says it isn't a Rupok lesson, open it with the program that made it." },
  ],
  final: {
    h2: "Make your own .rpk lessons",
    p: "Rupok is the free, open-source whiteboard for teachers. Write on slides, drop in live lessons from your AI, and share the whole class as one file.",
    cta: [{ href: "/board/", label: "Open the board" }, { href: "/open/", label: "Open a .rpk file", kind: "ghost" }],
  },
};

// ------------------------------------------------------------------ Bangla
const bn = {
  path: "/bn/rpk/",
  lang: "bn",
  alt: { en: "/rpk/" },
  title: "আরপিকে (RPK) ফাইল ফরম্যাট: Rich Presentation Kit কী? | Rupok",
  ogTitle: "আরপিকে (RPK) ফাইল ফরম্যাট",
  description:
    "RPK (Rich Presentation Kit) ফাইল হলো Rupok-এর একটি লেসন: স্লাইড, হাতের লেখা, লাইভ অ্যাক্টিভিটি আর ক্লাসের রিপ্লে, একটি খোলা ZIP প্যাকেজে। ভেতরে কী আছে, কীভাবে খুলবেন, PDF বানাবেন আর এআই দিয়ে পড়বেন।",
  crumbs: [["আরপিকে ফাইল ফরম্যাট"]],
  tag: "ফাইল ফরম্যাট",
  h1: "আরপিকে (RPK) ফাইল ফরম্যাট",
  lede: "**RPK ফাইল** (`.rpk`, পুরো নাম **Rich Presentation Kit**) হলো Rupok দিয়ে বানানো একটি লেসন। Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। একটি ফাইলেই থাকে স্লাইড, বোর্ডের সব লেখা, লাইভ অ্যাক্টিভিটি আর পুরো ক্লাসের রিপ্লে। ফরম্যাটটি খোলা ও নথিভুক্ত; মানুষ, প্রোগ্রাম আর এআই, সবাই পড়তে পারে।",
  cta: [{ href: "/bn/open/", label: ".rpk ফাইল খুলুন" }, { href: SPEC, label: "স্পেসিফিকেশন", kind: "ghost" }],
  fine: "ফ্রি · ব্রাউজারেই খোলে · Apache-2.0 স্পেসিফিকেশন",
  ld: ld("bn", "/bn/rpk/", "RPK file format (Rich Presentation Kit)", "RPK (Rich Presentation Kit) ফাইল হলো Rupok-এর লেসন: পাতা, সময়সহ হাতের লেখা, ছবি, ইন্টারঅ্যাক্টিভ স্মার্ট অবজেক্ট আর পড়ার উপযোগী Markdown সারাংশসহ একটি ZIP প্যাকেজ।"),
  sections: [
    {
      id: "what",
      split: true,
      h2: "RPK ফাইল কী?",
      copy: [
        "RPK মানে **Rich Presentation Kit**। শিক্ষকরা [Rupok](/board/)-এ .rpk লেসন বানান আর হোয়াটসঅ্যাপ, জিমেইল বা গুগল ড্রাইভে শিক্ষার্থীদের পাঠান। PDF-এর মতো স্থির নয়, লেসনটি জীবন্ত থাকে:",
        { ticks: [
          "**পাতা:** স্লাইড (প্রায়ই PDF থেকে) বা অসীম বোর্ড।",
          "**হাতের লেখা:** প্রতিটি টান, সময়সহ, তাই ক্লাসের রিপ্লে দেখা যায়।",
          "**স্মার্ট অবজেক্ট:** সিমুলেশন, থ্রিডি মডেল আর কুইজ, যা এখনো চলে।",
          "**পড়ার উপযোগী সারাংশ:** লেসনটি Markdown আর JSON-এ, সার্চ আর এআই-এর জন্য।",
        ] },
      ],
      blocks: [
        { dl: [
          ["পুরো নাম", "Rich Presentation Kit (Rupok লেসন)"],
          ["এক্সটেনশন", "`.rpk`"],
          ["মিডিয়া টাইপ", "`application/vnd.rupok.lesson+zip`"],
          ["ভেতরে", "একটি ZIP প্যাকেজ"],
          ["যা দিয়ে খোলে", "Rupok, Chrome বা Edge-এ ফ্রি ([rupok.app/bn/open](/bn/open/))"],
          ["স্পেসিফিকেশন", `খোলা, Apache-2.0 ([GitHub-এ](${SPEC}))`],
        ] },
      ],
    },
    {
      id: "inside",
      band: "mint",
      h2: ".rpk ফাইলের ভেতরে কী থাকে",
      lede: ".rpk আসলে একটি ZIP ফাইল। একটি কপির নাম বদলে `.zip` করলেই ভেতরটা দেখা যায়:",
      blocks: [
        { code: TREE, lang: "text" },
        "`doc.loro` হলো মূল লেসন: একটি [Loro](https://loro.dev) CRDT ডকুমেন্ট, যাতে সম্পাদনার পুরো ইতিহাস থাকে। `README.md`, `content.md` আর `content.json` হলো সহজে পড়ার মতো সারাংশ, যা Rupok প্রতিবার সেভ করার সময় নতুন করে লেখে।",
      ],
    },
    {
      id: "open",
      h2: "RPK ফাইল কীভাবে খুলবেন",
      blocks: [
        { steps: [
          { b: "rupok.app/bn/open খুলুন,", t: "মোবাইল বা কম্পিউটারের Chrome বা Edge ব্রাউজারে।" },
          { b: ".rpk ফাইলটি পাতায় ছেড়ে দিন,", t: "অথবা “.rpk ফাইল বেছে নিন”-এ চাপুন। স্টুডেন্ট মোডে খুলবে।" },
          { b: "ক্লাসের রিপ্লে দেখুন,", t: "পাতা উল্টান, লাইভ অ্যাক্টিভিটি চালান।" },
        ] },
        { cta: [{ href: "/bn/open/", label: "আরপিকে ভিউয়ার খুলুন" }] },
      ],
    },
    {
      id: "pdf",
      band: "sun",
      h2: "RPK থেকে PDF বানাবেন কীভাবে",
      blocks: [
        { steps: [
          { b: "ফাইলটি খুলুন,", t: "[rupok.app/bn/open](/bn/open/)-এ।" },
          { b: "Download PDF চাপুন।", t: "শিক্ষকের লেখাসহ সব পাতা PDF হয়ে যাবে।" },
          { b: "প্রিন্ট বা শেয়ার করুন।", t: "PDF আপনার ব্রাউজারেই তৈরি হয়, কিছুই আপলোড হয় না।" },
        ] },
      ],
    },
    {
      id: "ai",
      h2: "এআই দিয়ে RPK ফাইল পড়া",
      lede: "প্রতিটি .rpk নিজের পরিচয় নিজেই দেয়। ChatGPT বা Claude-কে ফাইলটি দিন, সে বুঝে নেবে লেসনে কী আছে।",
      blocks: [
        { steps: [
          { b: ".rpk ফাইলটি আপলোড করুন", t: "আর লেসন নিয়ে প্রশ্ন করুন।" },
          { b: "এআই ফাইলটি আনজিপ করে", t: "`README.md` আর `content.md` পড়ে: পাতাভিত্তিক শিরোনাম, লেখা, গণিত, চিত্র, ছবি আর অ্যাক্টিভিটি।" },
          { b: "ডেভেলপার আর এজেন্টদের জন্য", t: "[read-rpk স্কিল](/skills/read-rpk.md) আর [ইংরেজি পাতা](/rpk/)।" },
        ] },
      ],
    },
  ],
  faqTitle: "আরপিকে ফাইল নিয়ে প্রশ্নোত্তর",
  faq: [
    { q: "RPK-এর পুরো নাম কী?", a: "RPK মানে Rich Presentation Kit। .rpk ফাইল হলো Rupok দিয়ে বানানো লেসন; Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড।" },
    { q: "RPK ফাইল কীভাবে খুলবো?", a: "Chrome বা Edge-এ [rupok.app/bn/open](/bn/open/) খুলে ফাইলটি ছেড়ে দিন। ফ্রি, অ্যাকাউন্ট লাগে না, কিছু আপলোড হয় না।" },
    { q: "RPK ফাইলকে PDF বানাবো কীভাবে?", a: "ফাইলটি [rupok.app/bn/open](/bn/open/)-এ খুলে Download PDF চাপুন।" },
    { q: "ChatGPT বা Claude কি RPK ফাইল পড়তে পারে?", a: "হ্যাঁ। Rupok-এর নতুন সংস্করণে বানানো প্রতিটি .rpk-এ `README.md`, `content.md` আর `content.json` থাকে, অর্থাৎ পুরো লেসনের সহজপাঠ্য সারাংশ। ফাইলটি আপলোড করে প্রশ্ন করুন।" },
    { q: "RPK ফরম্যাট কি খোলা?", a: `হ্যাঁ। স্পেসিফিকেশন আর রেফারেন্স রিডার-রাইটার Apache-2.0 লাইসেন্সে, তাই যে কেউ .rpk পড়ার বা লেখার টুল বানাতে পারে। [GitHub-এ স্পেসিফিকেশন](${SPEC})।` },
  ],
  final: {
    h2: "নিজের .rpk লেসন বানান",
    p: "Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। স্লাইডে লিখুন, এআই-এর বানানো লাইভ লেসন যোগ করুন, আর পুরো ক্লাস একটি ফাইলে পাঠান।",
    cta: [{ href: "/board/", label: "বোর্ড খুলুন" }, { href: "/bn/open/", label: ".rpk ফাইল খুলুন", kind: "ghost" }],
  },
};

export const RPK = [en, bn];
