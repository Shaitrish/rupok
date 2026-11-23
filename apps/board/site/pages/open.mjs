// The RPK viewer page: "open rpk file", "rpk viewer", "what is an rpk file",
// "rpk to pdf" (English) and "rpk ফাইল খুলুন", "আরপিকে ফাইল" (Bangla).
// The drop zone is wired up by src/pages.ts: it stashes the file in IndexedDB
// (src/pending.ts) and opens /board/?view&pending=1.

const FILE_ICON = `<svg class="file-ico" viewBox="0 0 96 112" aria-hidden="true"><path class="sheet" d="M10 6h50l26 26v70a4 4 0 0 1-4 4H10a4 4 0 0 1-4-4V10a4 4 0 0 1 4-4z"/><path class="fold" d="M60 6v22a4 4 0 0 0 4 4h22"/><rect class="label" x="16" y="62" width="64" height="28" rx="8"/><text x="48" y="82" text-anchor="middle">.rpk</text><path class="ink" d="M20 46 C 32 30, 44 52, 56 38 S 70 34, 76 42"/></svg>`;

function drop(t) {
  return `<div class="drop" data-drop data-t-opening="${t.opening}" data-t-notrpk="${t.notrpk}" data-t-other="${t.other}" data-t-blocked="${t.blocked}" data-t-empty="${t.empty}">
            <input class="sr-only" type="file" id="rpk-file" data-file tabindex="-1" aria-hidden="true" />
            ${FILE_ICON}
            <p class="drop-title">${t.title}</p>
            <p class="drop-or">${t.or}</p>
            <button class="pill dark lg" type="button" data-pick>${t.button}</button>
            <p class="drop-msg" role="status" aria-live="polite" data-msg></p>
            <p class="drop-fine">${t.fine}</p>
          </div>`;
}

// ----------------------------------------------------------------- English
const en = {
  path: "/open/",
  lang: "en",
  alt: { bn: "/bn/open/" },
  title: "Open .rpk Files: Free Online RPK Viewer | Rupok",
  ogTitle: "Open .rpk files: free online RPK viewer",
  description:
    "Got a .rpk (Rich Presentation Kit) file from your teacher? Open it free in your browser: replay the class stroke by stroke, use the live activities, and download a PDF. No account, no app, nothing uploaded.",
  bodyClass: "open-page",
  h1: "Open .rpk files: free online RPK viewer",
  lede: "Got a lesson file from your teacher on WhatsApp, Gmail or Google Drive? Drop it here. It opens right in your browser, with the class replay and the live activities still working.",
  fine: "No account · No app to install · Your file never leaves your device",
  aside: drop({
    title: "Drop your .rpk file here",
    or: "or",
    button: "Choose a .rpk file",
    fine: "Works on phones and computers. Nothing is uploaded.",
    opening: "Opening {name}…",
    notrpk: "{name} isn’t a .rpk lesson file. Choose the file your teacher sent; its name ends in .rpk.",
    other: "{name} isn’t a Rupok lesson. Other programs also use the .rpk extension (see the FAQ below); open it with the app that made it.",
    blocked: "Your browser blocked local storage, so this page can’t hand the file to the viewer. <a href='/board/?view'>Open the viewer</a> and choose the file there.",
    empty: "That file is empty. Download it again from your teacher’s message.",
  }),
  sections: [
    {
      id: "what",
      split: true,
      h2: "What is an .rpk file?",
      copy: [
        "An **RPK (Rich Presentation Kit) file is a Rupok lesson.** Rupok is a free, open-source whiteboard for teachers, and a `.rpk` file holds one whole class in a single file:",
        { ticks: [
          "**Slides:** every page the teacher prepared, including PDF slides.",
          "**Ink:** everything written on them with the pen.",
          "**Live Smart Objects:** simulations, 3D models and quizzes that still work.",
          "**A replay:** the timing of every stroke, so you can watch the class unfold.",
        ] },
      ],
      blocks: [
        { dl: [
          ["File type", "Rupok lesson (RPK: Rich Presentation Kit)"],
          ["Extension", "`.rpk`"],
          ["Media type", "`application/vnd.rupok.lesson+zip`"],
          ["Inside", "A ZIP package: the lesson document with its full history, images and Smart Objects"],
          ["Opens with", "Rupok, free in Chrome, Edge or Safari, on this page or at rupok.app/board"],
          ["Format", "Open and documented ([RPK file format](/rpk/), Apache-2.0 [spec](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md))"],
          ["Readable by AI", "Yes: each file carries a Markdown outline ChatGPT or Claude can read"],
        ] },
      ],
    },
    {
      id: "how",
      band: "mint",
      h2: "How to open an .rpk file on a phone or computer",
      blocks: [
        { cols: [
          [
            { h3: "On a phone (Android or iPhone)" },
            { steps: [
              { b: "Save the file.", t: "In WhatsApp, tap the file and download it (on iPhone: Share → Save to Files). In Gmail, tap the attachment’s download icon. In Google Drive, tap ⋮ → Download." },
              { b: "Open rupok.app/open", t: "in your phone’s browser. Chrome works best." },
              { b: "Tap “Choose a .rpk file”", t: "and pick it from Downloads or Files. The lesson opens in Student mode." },
            ] },
          ],
          [
            { h3: "On a computer" },
            { steps: [
              { b: "Download the file", t: "from WhatsApp Web, Gmail or Google Drive." },
              { b: "Drag it onto this page,", t: "or click “Choose a .rpk file”. Use Chrome, Edge or Safari." },
              { b: "Replay the class.", t: "Press play, flip through the pages and try the live objects." },
            ] },
          ],
        ] },
        { note: "Student mode is read-only: you can’t change your teacher’s lesson by accident." },
      ],
    },
    {
      id: "replay",
      split: "rev",
      h2: "What you can do with the lesson",
      copy: [
        "The lesson opens exactly as your teacher left it. Press play to watch it being written, stroke by stroke, at 1–8× speed. Pause on a diagram, zoom in, and use the simulations and quizzes yourself.",
      ],
      blocks: [
        { cards: [
          { b: "Replay at 1–8×", t: "Watch every stroke appear in order." },
          { b: "Try the objects", t: "Simulations and quizzes still work." },
          { b: "Zoom and page through", t: "Every slide, sharp at any size." },
        ] },
      ],
    },
    {
      id: "pdf",
      band: "sun",
      h2: "How to turn an .rpk file into a PDF",
      blocks: [
        { steps: [
          { b: "Open the .rpk file here.", t: "Drop it in the box at the top of this page." },
          { b: "Press Download PDF.", t: "You get every page with the teacher’s ink on it." },
          { b: "Print it or keep it.", t: "Good for revision, printing, or reading where there’s no internet." },
        ] },
        { note: "The PDF is made in your browser. Nothing is uploaded." },
      ],
    },
  ],
  faq: [
    { q: "What is an .rpk file?", a: "An .rpk file is a lesson made with Rupok, a free whiteboard for teachers. It holds the slides, the teacher’s ink, live interactive Smart Objects and a replay of the class. Technically it’s a ZIP package with an open, documented format: see [the RPK file format](/rpk/)." },
    { q: "What does RPK stand for?", a: "RPK stands for Rich Presentation Kit, the name of Rupok’s lesson file format." },
    { q: "How do I open an .rpk file?", a: "Drop it on this page, or tap “Choose a .rpk file”. It opens in your browser in Rupok’s Student mode. You don’t need an account or an app." },
    { q: "Can I open an .rpk file on my phone?", a: "Yes. Save the file from WhatsApp, Gmail or Google Drive, open rupok.app/open in your phone’s browser (Chrome works best), and choose the file." },
    { q: "How do I convert an .rpk file to PDF?", a: "Open the file on this page, then press Download PDF in the viewer. The PDF has every page with the teacher’s ink, and it’s made on your device." },
    { q: "Can ChatGPT or Claude read my .rpk file?", a: "Yes. Upload the file and ask about the lesson: every .rpk from current Rupok versions includes a plain Markdown outline of its pages (`content.md`) that AI models read. See [how AI reads RPK files](/rpk/#ai)." },
    { q: "Is my file uploaded anywhere?", a: "No. The file is opened by your browser on your device. This page hands it to the viewer through your browser’s local storage, and nothing is sent to a server." },
    { q: "Can I change the lesson?", a: "Student mode is read-only, so you can replay the class and use the live objects without changing anything. Teachers edit lessons at [rupok.app/board](/board/)." },
    { q: "My .rpk file won’t open. What can I do?", a: [
      "First download it again: a half-finished download is the most common cause. Make sure the name ends in `.rpk` (some apps add `.zip` or a number; that’s fine, this page checks the file itself).",
      "Other programs also use the .rpk extension, for example Esri CityEngine rule packages, Tizen resource packages and TI-99/4A cartridge files for the MAME emulator. Those aren’t Rupok lessons; open them with the program that made them.",
    ] },
    { q: "How do I make my own .rpk lessons?", a: "Open [rupok.app/board](/board/), the free whiteboard for teachers. Write on slides, add live AI-made activities, and save the lesson as a .rpk file to share." },
  ],
  final: {
    h2: "Teaching? Make your own .rpk lessons.",
    p: "Rupok is the free, open-source whiteboard for teachers. Write on slides, drop in live lessons from your AI, and send the whole class as one file.",
    cta: [{ href: "/board/", label: "Open the board" }, { href: "/for-teachers/", label: "Rupok for teachers", kind: "ghost" }],
  },
};

// ------------------------------------------------------------------ Bangla
const bn = {
  path: "/bn/open/",
  lang: "bn",
  alt: { en: "/open/" },
  title: ".rpk ফাইল খুলুন: ফ্রি অনলাইন আরপিকে ভিউয়ার | Rupok",
  ogTitle: ".rpk ফাইল খুলুন: ফ্রি অনলাইন আরপিকে ভিউয়ার",
  description:
    "শিক্ষক .rpk ফাইল পাঠিয়েছেন? ব্রাউজারেই ফ্রিতে খুলুন: পুরো ক্লাসের রিপ্লে দেখুন, সিমুলেশন আর কুইজ চালান, PDF নামান। কোনো অ্যাকাউন্ট বা অ্যাপ লাগবে না।",
  bodyClass: "open-page",
  h1: ".rpk ফাইল খুলুন, একদম ফ্রিতে",
  lede: "হোয়াটসঅ্যাপ, জিমেইল বা গুগল ড্রাইভে শিক্ষকের পাঠানো লেসন ফাইল এখানে ছেড়ে দিন। ফাইলটি আপনার ব্রাউজারেই খুলবে, ক্লাসের রিপ্লে আর লাইভ অ্যাক্টিভিটিসহ।",
  fine: "অ্যাকাউন্ট লাগবে না · অ্যাপ নামাতে হবে না · ফাইল আপনার ডিভাইসের বাইরে যায় না",
  aside: drop({
    title: ".rpk ফাইলটি এখানে টেনে আনুন",
    or: "অথবা",
    button: ".rpk ফাইল বেছে নিন",
    fine: "মোবাইল আর কম্পিউটার, দুটোতেই চলে। কিছুই আপলোড হয় না।",
    opening: "{name} খোলা হচ্ছে…",
    notrpk: "{name} কোনো .rpk লেসন ফাইল নয়। শিক্ষকের পাঠানো ফাইলটি বেছে নিন; নামের শেষে .rpk থাকে।",
    other: "{name} Rupok-এর লেসন নয়। অন্য কিছু সফটওয়্যারও .rpk এক্সটেনশন ব্যবহার করে (নিচের প্রশ্নোত্তর দেখুন); যে সফটওয়্যারে তৈরি, সেটিতেই খুলুন।",
    blocked: "আপনার ব্রাউজার লোকাল স্টোরেজ বন্ধ রেখেছে, তাই ফাইলটি ভিউয়ারে পাঠানো গেল না। <a href='/board/?view'>ভিউয়ার খুলুন</a> আর সেখানে ফাইলটি বেছে নিন।",
    empty: "ফাইলটি খালি। শিক্ষকের মেসেজ থেকে আবার ডাউনলোড করুন।",
  }),
  sections: [
    {
      id: "what",
      split: true,
      h2: ".rpk ফাইল কী?",
      copy: [
        "**.rpk ফাইল হলো Rupok-এর একটি লেসন।** Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। একটি .rpk ফাইলে পুরো একটি ক্লাস থাকে:",
        { ticks: [
          "**স্লাইড:** শিক্ষক যত পাতা বানিয়েছেন, PDF স্লাইডসহ।",
          "**হাতের লেখা:** কলম দিয়ে বোর্ডে যা লেখা হয়েছে।",
          "**লাইভ স্মার্ট অবজেক্ট:** সিমুলেশন, থ্রিডি মডেল আর কুইজ, যা এখনো চলে।",
          "**রিপ্লে:** প্রতিটি টানের সময়, তাই ক্লাসটি শুরু থেকে আবার দেখা যায়।",
        ] },
      ],
      blocks: [
        { dl: [
          ["ফাইলের ধরন", "Rupok লেসন (RPK: Rich Presentation Kit)"],
          ["এক্সটেনশন", "`.rpk`"],
          ["ভেতরে যা থাকে", "একটি ZIP প্যাকেজ: পুরো ইতিহাসসহ লেসন, ছবি আর স্মার্ট অবজেক্ট"],
          ["যা দিয়ে খোলে", "Rupok, Chrome, Edge বা Safari ব্রাউজারে ফ্রি: এই পাতায় বা rupok.app/board-এ"],
          ["ফরম্যাট", "খোলা ও নথিভুক্ত ([আরপিকে ফাইল ফরম্যাট](/bn/rpk/), [স্পেসিফিকেশন](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md))"],
        ] },
      ],
    },
    {
      id: "how",
      band: "mint",
      h2: "মোবাইল বা কম্পিউটারে .rpk ফাইল কীভাবে খুলবেন",
      blocks: [
        { cols: [
          [
            { h3: "মোবাইলে (অ্যান্ড্রয়েড বা আইফোন)" },
            { steps: [
              { b: "ফাইলটি সেভ করুন।", t: "হোয়াটসঅ্যাপে ফাইলে চাপ দিয়ে ডাউনলোড করুন (আইফোনে: Share → Save to Files)। জিমেইলে অ্যাটাচমেন্টের ডাউনলোড আইকনে চাপুন। গুগল ড্রাইভে ⋮ → Download।" },
              { b: "rupok.app/bn/open খুলুন,", t: "মোবাইলের ব্রাউজারে। Chrome সবচেয়ে ভালো চলে।" },
              { b: "“.rpk ফাইল বেছে নিন”-এ চাপুন", t: "আর Downloads বা Files থেকে ফাইলটি বাছুন। লেসনটি স্টুডেন্ট মোডে খুলবে।" },
            ] },
          ],
          [
            { h3: "কম্পিউটারে" },
            { steps: [
              { b: "ফাইলটি ডাউনলোড করুন,", t: "হোয়াটসঅ্যাপ ওয়েব, জিমেইল বা গুগল ড্রাইভ থেকে।" },
              { b: "এই পাতায় টেনে আনুন,", t: "অথবা “.rpk ফাইল বেছে নিন”-এ ক্লিক করুন। Chrome, Edge বা Safari ব্যবহার করুন।" },
              { b: "ক্লাসের রিপ্লে দেখুন।", t: "প্লে চাপুন, পাতা উল্টান, লাইভ অবজেক্টগুলো চালিয়ে দেখুন।" },
            ] },
          ],
        ] },
        { note: "স্টুডেন্ট মোডে শুধু দেখা যায়, তাই ভুল করেও শিক্ষকের লেসন বদলে যাবে না।" },
      ],
    },
    {
      id: "replay",
      split: "rev",
      h2: "লেসন খুলে যা যা করতে পারবেন",
      copy: [
        "শিক্ষক যেভাবে রেখেছেন, লেসনটি ঠিক সেভাবেই খুলবে। প্লে চাপলে দেখবেন বোর্ডে কীভাবে একটার পর একটা লেখা হয়েছিল, ১ থেকে ৮ গুণ গতিতে। কোনো চিত্রে থেমে জুম করুন, সিমুলেশন আর কুইজ নিজে চালান।",
      ],
      blocks: [
        { cards: [
          { b: "১–৮× গতিতে রিপ্লে", t: "প্রতিটি লেখা ক্রম অনুযায়ী ফুটে ওঠে।" },
          { b: "অবজেক্ট চালান", t: "সিমুলেশন আর কুইজ এখনো কাজ করে।" },
          { b: "জুম করুন, পাতা উল্টান", t: "যেকোনো আকারে ঝকঝকে স্লাইড।" },
        ] },
      ],
    },
    {
      id: "pdf",
      band: "sun",
      h2: ".rpk থেকে PDF বানাবেন কীভাবে",
      blocks: [
        { steps: [
          { b: "এখানে .rpk ফাইলটি খুলুন।", t: "এই পাতার ওপরের বাক্সে ফাইলটি ছেড়ে দিন।" },
          { b: "Download PDF বোতামে চাপুন।", t: "শিক্ষকের লেখাসহ সব পাতা একটি PDF-এ পাবেন।" },
          { b: "প্রিন্ট করুন বা রেখে দিন।", t: "পড়া ঝালাই, প্রিন্ট, বা ইন্টারনেট ছাড়া পড়ার জন্য কাজে লাগবে।" },
        ] },
        { note: "PDF আপনার ব্রাউজারেই তৈরি হয়, কিছুই আপলোড হয় না।" },
      ],
    },
  ],
  faqTitle: "আরপিকে ফাইল নিয়ে প্রশ্নোত্তর",
  faq: [
    { q: ".rpk (আরপিকে) ফাইল কী?", a: ".rpk ফাইল হলো Rupok দিয়ে বানানো একটি লেসন। Rupok শিক্ষকদের জন্য ফ্রি হোয়াইটবোর্ড। এই ফাইলে স্লাইড, শিক্ষকের হাতের লেখা, লাইভ স্মার্ট অবজেক্ট আর পুরো ক্লাসের রিপ্লে থাকে। ভেতরে এটি একটি ZIP প্যাকেজ, যার ফরম্যাট খোলা ও নথিভুক্ত।" },
    { q: "RPK-এর পুরো নাম কী?", a: "RPK মানে Rich Presentation Kit, Rupok-এর লেসন ফাইলের ফরম্যাট। বিস্তারিত: [আরপিকে ফাইল ফরম্যাট](/bn/rpk/)।" },
    { q: "rpk ফাইল কীভাবে খুলবো?", a: "ফাইলটি এই পাতায় ছেড়ে দিন, অথবা “.rpk ফাইল বেছে নিন”-এ চাপুন। Rupok-এর স্টুডেন্ট মোডে ব্রাউজারেই খুলে যাবে। অ্যাকাউন্ট বা অ্যাপ লাগবে না।" },
    { q: "মোবাইলে কি rpk ফাইল খোলা যায়?", a: "হ্যাঁ। হোয়াটসঅ্যাপ, জিমেইল বা গুগল ড্রাইভ থেকে ফাইলটি সেভ করুন, মোবাইলের ব্রাউজারে rupok.app/bn/open খুলুন (Chrome সবচেয়ে ভালো), তারপর ফাইলটি বেছে নিন।" },
    { q: "rpk ফাইলকে PDF বানাবো কীভাবে?", a: "ফাইলটি এই পাতায় খুলুন, তারপর ভিউয়ারে Download PDF চাপুন। শিক্ষকের লেখাসহ সব পাতা PDF হয়ে যাবে, আপনার ডিভাইসেই।" },
    { q: "আমার ফাইল কি কোথাও আপলোড হয়?", a: "না। ফাইলটি আপনার ডিভাইসে, আপনার ব্রাউজারেই খোলে। এই পাতা ব্রাউজারের লোকাল স্টোরেজের মাধ্যমে ফাইলটি ভিউয়ারে পাঠায়; কোনো সার্ভারে কিছু যায় না।" },
    { q: "লেসনে কি কিছু বদলাতে পারবো?", a: "স্টুডেন্ট মোডে শুধু দেখা যায়: ক্লাসের রিপ্লে দেখা আর লাইভ অবজেক্ট চালানো যায়, কিছু বদলায় না। শিক্ষকরা লেসন সম্পাদনা করেন [rupok.app/board](/board/)-এ।" },
    { q: "rpk ফাইল খুলছে না, কী করবো?", a: [
      "প্রথমে ফাইলটি আবার ডাউনলোড করুন; অর্ধেক ডাউনলোড হওয়াই সবচেয়ে বড় কারণ। কোনো অ্যাপ নামের শেষে .zip বা একটি সংখ্যা জুড়ে দিলেও সমস্যা নেই, এই পাতা ফাইলের ভেতরটা দেখে চিনে নেয়।",
      "অন্য কিছু সফটওয়্যারও .rpk এক্সটেনশন ব্যবহার করে, যেমন Esri CityEngine-এর রুল প্যাকেজ, Tizen-এর রিসোর্স প্যাকেজ বা MAME ইমুলেটরের TI-99/4A কার্ট্রিজ। সেগুলো Rupok-এর লেসন নয়; যে সফটওয়্যারে তৈরি, সেটিতেই খুলুন।",
    ] },
    { q: "নিজে কীভাবে .rpk লেসন বানাবো?", a: "[rupok.app/board](/board/) খুলুন, শিক্ষকদের জন্য ফ্রি হোয়াইটবোর্ড। স্লাইডে লিখুন, এআই দিয়ে বানানো লাইভ অ্যাক্টিভিটি যোগ করুন, তারপর .rpk ফাইল হিসেবে সেভ করে শেয়ার করুন।" },
  ],
  final: {
    h2: "আপনি শিক্ষক? নিজের .rpk লেসন বানান।",
    p: "Rupok শিক্ষকদের জন্য ফ্রি, ওপেন সোর্স হোয়াইটবোর্ড। স্লাইডে লিখুন, এআই-এর বানানো লাইভ লেসন যোগ করুন, আর পুরো ক্লাস একটি ফাইলে পাঠিয়ে দিন।",
    cta: [{ href: "/board/", label: "বোর্ড খুলুন" }, { href: "/bn/", label: "Rupok সম্পর্কে জানুন", kind: "ghost" }],
  },
};

export const OPEN = [en, bn];
