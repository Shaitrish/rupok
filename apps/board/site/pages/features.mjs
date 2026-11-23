// Feature and use-case pages, one search intent each (see docs/seo/keywords.md):
//   /smartboard-software/      best / free / open-source smartboard & IWB software
//   /ai-whiteboard/            AI whiteboard for teachers, AI smartboard software, MCP
//   /online-class-whiteboard/  online class whiteboard, whiteboard for online teaching
//   /for-teachers/             teacher workflow: plan, teach, replay, share; note-taking

const ART_BOARD = `<div class="art board" aria-hidden="true">
            <div class="panel">
              <div class="slide">
                <b>Ohm’s law</b>
                <svg viewBox="0 0 300 120" class="scribble"><path d="M10 90 C 60 20, 110 120, 160 60 S 250 10, 290 50" /></svg>
                <div class="mini-obj"><span class="chip">Live</span><span>V = I × R</span><em>12 V · 4 Ω → 3 A</em></div>
              </div>
            </div>
            <div class="stand"></div>
            <div class="cursor c1"><svg viewBox="0 0 24 24"><path d="M4 2l16 7-7 2.6L10 20z" /></svg><span>Any board</span></div>
          </div>`;

const ART_AI = `<div class="art ai" aria-hidden="true">
            <div class="bubble"><span class="who">You → Claude</span><p>“A projectile simulation for Class 9, with angle and speed sliders.”</p></div>
            <div class="arrow">↓</div>
            <div class="obj-card">
              <div class="obj-head"><span class="chip">Physics · live</span><span class="steps">2 / 4</span></div>
              <svg viewBox="0 0 260 120" class="traj"><path d="M14 110 H246" class="ground" /><path d="M20 108 C 70 0, 150 0, 200 108" class="arc" /><circle cx="110" cy="30" r="6" class="ball" /></svg>
              <div class="obj-foot">range <b>40.8 m</b></div>
            </div>
          </div>`;

const ART_ONLINE = `<div class="art online" aria-hidden="true">
            <div class="call">
              <div class="call-board"><b>y = x²</b><svg viewBox="0 0 200 90" class="scribble"><path d="M10 10 Q 100 160 190 10" /></svg></div>
              <div class="tiles"><span class="t1">R</span><span class="t2">S</span><span class="t3">A</span><span class="t4">+28</span></div>
            </div>
            <div class="rp-mini"><span class="play">▶</span><span class="bar"><i></i></span><span>12:40</span></div>
          </div>`;

const ART_TEACHER = `<div class="art teacher" aria-hidden="true">
            <div class="note sun"><b>Tonight</b><span>Prepare</span></div>
            <div class="note mint"><b>9:00</b><span>Teach</span></div>
            <div class="note coral"><b>After class</b><span>Replay</span></div>
          </div>`;

// ---------------------------------------------------------- smartboard
const smartboard = {
  path: "/smartboard-software/",
  lang: "en",
  title: "Free Smartboard Software for Teachers: Open Source, Any Board | Rupok",
  description:
    "Rupok is free, open-source smartboard software for any interactive panel, smart board or projector. Write on PDF slides, add AI-made activities, replay every class. No licence.",
  tag: "Smartboard software",
  h1: "Free smartboard software that works on any board",
  lede: "Rupok turns any interactive flat panel, smart board or projector into a teaching board: fast pen ink on your slides, live interactive lessons, and a replay of every class. Free and open source, with no licence keys and no account.",
  crumbs: [["Smartboard software"]],
  cta: [{ href: "/board/", label: "Open the board, it’s free" }, { href: "/compare/", label: "Compare software", kind: "ghost" }],
  fine: "Runs in Chrome, Edge or Safari on the computer connected to your board.",
  aside: ART_BOARD,
  sections: [
    {
      id: "any-board",
      h2: "Interactive whiteboard software, minus the licence",
      lede: "Most smartboard software is free only on its maker’s own hardware, or keeps the best features for a paid edition. Rupok gives every feature to every board.",
      blocks: [
        { cards: [
          { b: "Any brand", t: "SMART, Promethean, BenQ, ViewSonic, Prestigio or no-name panels, projectors with a pen, laptops and tablets." },
          { b: "Every feature free", t: "No “Plus” or “Professional” edition. Recording, PDF and AI aren’t locked away." },
          { b: "Nothing to install", t: "Open rupok.app/board in Chrome, Edge or Safari. Works on Windows, macOS, Linux and ChromeOS. Prefer an app? There’s a desktop app for Windows, Mac and Linux." },
        ] },
      ],
    },
    {
      id: "features",
      band: "mint",
      h2: "Everything a smartboard lesson needs",
      blocks: [
        { cols: [
          [
            { h3: "Write" },
            { ticks: [
              "GPU ink that draws a stroke in about a millisecond and stays smooth with 100,000 strokes.",
              "Pressure, palm rejection, highlighter, laser pointer, eraser and lasso. Rub with your palm to erase; swipe to change slides; tap with two fingers to undo.",
              "Smart shapes, offline: hand-drawn shapes, arrows, graphs, flowcharts, logic gates, number lines and tables snap clean. Handwritten maths turns into typeset LaTeX.",
              "The Smart Dock: a big, touch-friendly toolbar you drag anywhere and shrink to a bubble; keyboard shortcuts on a laptop.",
            ] },
            { h3: "Present" },
            { ticks: [
              "Import a PDF and write on every page. (Using PowerPoint? Save it as PDF first.)",
              "Teach mode keeps the slide fitted to the screen with minimal chrome.",
              "Step through slides and live objects with a presentation clicker.",
            ] },
          ],
          [
            { h3: "Bring lessons to life" },
            { ticks: [
              "Smart Objects: live simulations, 3D models and quizzes on the slide. Your AI makes them ([see how](/ai-whiteboard/)).",
              "Write on top of a running simulation with the pen.",
            ] },
            { h3: "Keep the class" },
            { ticks: [
              "Every stroke is saved with its timing: replay any page at 1–8×.",
              "Save the lesson as one `.rpk` file and share it; students open it at [rupok.app/open](/open/).",
              "Export the lesson as PDF slides, or as handouts with 2 or 3 slides per page and lines for notes.",
              "Keep every lesson in the Library, in folders by subject, course and chapter.",
            ] },
          ],
        ] },
      ],
    },
    {
      id: "open-source",
      h2: "Open-source smartboard software, so it stays free",
      blocks: [
        "Rupok’s code is open source under the AGPL-3.0, and its `.rpk` lesson format is published under Apache-2.0. Nobody can take the free version away, put features behind a paywall, or lock your lessons into one company’s app. A school can run it on every device forever.",
        "Rupok is made by Shaitrish in Bangladesh, and built in the open on [GitHub](https://github.com/Shaitrish/rupok) with a public roadmap.",
      ],
    },
    {
      id: "setup",
      band: "sun",
      h2: "Set it up on your board in two minutes",
      blocks: [
        { steps: [
          { b: "Open Chrome, Edge or Safari on the board’s computer.", t: "The built-in PC (OPS module) of an interactive flat panel, or the laptop connected to your projector." },
          { b: "Go to rupok.app/board.", t: "Pin it to the taskbar or install it from the browser’s menu so it opens like an app." },
          { b: "Drop in your slides.", t: "Drag a PDF onto the board; every page becomes a slide." },
          { b: "Teach.", t: "Switch to Teach mode, pick up the pen, and press the clicker to move on." },
        ] },
        { note: "Panels running only their built-in Android system vary: Rupok needs a browser with WebGPU or WebGL2. A connected Windows, Linux or ChromeOS computer is the safe choice." },
      ],
    },
    {
      id: "compare",
      h2: "How Rupok compares",
      blocks: [
        { table: {
          hl: "row",
          caption: "Smartboard software for teachers, as of November 2026 (details and sources on each comparison page)",
          head: ["", "Price", "Any brand of board", "AI", "Replay"],
          rows: [
            ["**Rupok**", "yes: Free", "yes: Yes", "yes: AI Smart Objects", "yes: Stroke-by-stroke"],
            ["[SMART Notebook](/compare/smart-notebook/)", "part: Free basic; Plus US$59/yr", "part: Full touch needs Plus", "part: In Lumio (web)", "part: Plus only, no sound"],
            ["[ActivInspire](/compare/activinspire/)", "part: Free Personal; Pro US$1,249", "yes: Yes", "no: None found", "part: Professional only"],
            ["[OpenBoard](/compare/openboard/)", "yes: Free", "yes: Yes", "no: None", "part: Video podcast"],
            ["[Note3](/compare/note3/)", "no: €29 per device", "part: Windows devices", "no: None documented", "no: None documented"],
          ],
        } },
        { cta: [{ href: "/compare/", label: "See the full comparison", kind: "ghost" }] },
      ],
    },
  ],
  faq: [
    { q: "What is the best smartboard software for teachers?", a: "If your school is invested in one brand, its own software (SMART Notebook, ActivInspire) fits that hardware best. If you have mixed boards, want every feature free, or want AI-made interactive lessons and class replay, Rupok is built for that. See the [comparison](/compare/)." },
    { q: "Is Rupok really free?", a: "Yes. The whole app is free and open source, with no paid edition of the software. A hosted add-on, Rupok Cloud (sync, share links and live classes), is coming with a free plan for every teacher." },
    { q: "Do I need to download smartboard software for my PC?", a: "No download is needed: Rupok runs in Chrome, Edge or Safari. You can install it from the browser menu so it opens in its own window. There is also a desktop app for Windows, Mac and Linux: see [Download](/download/)." },
    { q: "Does it work with SMART, Promethean, BenQ, ViewSonic or Prestigio boards?", a: "Yes. Rupok runs on the computer connected to the board and uses its touch and pen like any browser app, so the brand doesn’t matter." },
    { q: "Does Rupok work offline?", a: "Yes, with the [desktop app](/download/) for Windows, Mac and Linux: drawing, slides, replay, PDF export and handwritten-maths recognition work without internet (the AI features need it). In the browser, the board runs on your device once it’s open." },
    { q: "Can I use my PowerPoint slides?", a: "Save the presentation as a PDF (File → Save As → PDF in PowerPoint), then drop the PDF onto the board. Every slide becomes a page you can write on." },
    { q: "Does Rupok support Bangla?", a: "Yes, for lesson content: Bangla text and fonts work today. A Bangla interface is coming. [বাংলায় পড়ুন](/bn/)." },
  ],
  final: { h2: "Your board, minus the licence.", p: "Open Rupok on your smartboard and write your first line in five seconds.", cta: [{ href: "/board/", label: "Open the board" }] },
};

// ------------------------------------------------------------------ AI
const ai = {
  path: "/ai-whiteboard/",
  lang: "en",
  title: "AI Whiteboard for Teachers: Live AI-Made Lessons on Your Board | Rupok",
  description:
    "Ask Claude, ChatGPT, Codex or OpenCode for a simulation or quiz and teach it on your smartboard a minute later. Rupok is the free, open-source AI whiteboard for teachers, with an MCP server.",
  tag: "AI whiteboard",
  h1: "The AI whiteboard for teachers",
  lede: "Ask your AI for a simulation, a 3D model or a quiz. It lands on your slide as a live Smart Object you can write on with the pen and step through with your clicker. No new panel to buy: Rupok brings AI to the board you already have.",
  crumbs: [["AI whiteboard"]],
  cta: [{ href: "/board/", label: "Open the board" }, { href: "#mcp", label: "Connect your AI", kind: "ghost" }],
  fine: "Works with the AI you already use. No Rupok AI credits needed.",
  aside: ART_AI,
  sections: [
    {
      id: "how",
      h2: "From prompt to lesson in a minute",
      blocks: [
        { steps: [
          { b: "Ask.", t: "“Make a projectile-motion simulation for Class 9 with angle and speed sliders, Earth vs Moon vs Mars.”" },
          { b: "It lands on your slide.", t: "Your AI writes a Smart Object: one self-contained HTML file that runs live on the board." },
          { b: "Teach it.", t: "Use it with touch, write on top of it with the pen, and press Next on your clicker to reveal each step." },
          { b: "It’s in the replay.", t: "Students who open the lesson file get the same live object, still working." },
        ] },
      ],
    },
    {
      id: "examples",
      band: "mint",
      h2: "What teachers make with it",
      lede: "These all ship with Rupok as examples. Try them on the [home page](/#lessons).",
      blocks: [
        { cols: [
          [{ ticks: ["**Physics:** projectile motion, Ohm’s law circuits with moving electrons.", "**Chemistry:** water, methane and benzene in 3D; methane burning at particle level.", "**Maths:** y = a·sin(bx + c) with sliders."] }],
          [{ ticks: ["**ICT:** a binary counter that reveals place values one by one.", "**History:** a timeline of computing, one era per click.", "**Quizzes:** four questions with instant feedback and a score."] }],
        ] },
      ],
    },
    {
      id: "any-ai",
      h2: "Bring your own AI",
      blocks: [
        { cards: [
          { b: "Any chatbot", t: "Ask ChatGPT, Claude or Gemini for “a single self-contained HTML file”, then paste it onto the board or drop the .html file in." },
          { b: "Agents over MCP", t: "Claude Code, Codex and OpenCode build whole lessons through Rupok’s MCP server, and check every object before saving it." },
          { b: "Claude connector", t: "Manage lessons from Claude itself, with no install: coming soon with Rupok Cloud." },
        ] },
      ],
    },
    {
      id: "on-board",
      band: "mint",
      h2: "Your AI, on the board",
      lede: "Rupok works with your own AI subscription. Nothing is sent to Rupok.",
      blocks: [
        { ticks: [
          "**Chat inside Rupok** with your own Claude, Codex or OpenCode: pick the model, @mention pages, use /commands, and keep your chat history. In the desktop app there’s no terminal to open.",
          "**Live edits.** Your AI sees the open page, points at things and edits the board while you teach. Every change can be undone.",
          "**Solve this.** Select a problem and the AI writes a step-by-step solution in handwriting below it. By default you preview it before the class sees it.",
          "**Handwriting to text.** Handwritten maths becomes LaTeX offline; other handwriting becomes text through Claude, with your own API key.",
          "**Voice input** in text boxes, the chat and search, in English or Bangla.",
        ] },
      ],
    },
    {
      id: "mcp",
      band: "sun",
      h2: "Connect Claude, Codex or OpenCode (MCP)",
      lede: "The `rupok` command-line tool includes an MCP server. Your agent can create lessons, add pages and add validated Smart Objects, but only inside the folder you choose.",
      blocks: [
        { code: "git clone https://github.com/Shaitrish/rupok && cd rupok\ncargo install --path crates/rupok-cli      # installs the `rupok` command\nclaude mcp add rupok -- rupok mcp --workspace ~/Lessons   # Claude Code" },
        { ul: [
          "**Codex CLI:** in `~/.codex/config.toml`, add `[mcp_servers.rupok]` with `command = \"rupok\"` and `args = [\"mcp\", \"--workspace\", \"/home/me/Lessons\"]`.",
          "**OpenCode:** in `opencode.json`, add `\"mcp\": { \"rupok\": { \"type\": \"local\", \"command\": [\"rupok\", \"mcp\", \"--workspace\", \"/home/me/Lessons\"] } }`.",
          "**Any MCP client:** command `rupok`, arguments `mcp --workspace <folder>`, transport stdio.",
        ] },
        "Then ask: *“Make me a 6-slide Rupok lesson on binary numbers for Grade 7, with an interactive counter.”* The [agent guide](https://github.com/Shaitrish/rupok/blob/main/skills/AGENTS.md) lists all 12 tools. You need [Rust](https://rustup.rs) to build the CLI today; release builds are coming.",
      ],
    },
    {
      id: "safe",
      h2: "Safe in a classroom",
      blocks: [
        { ticks: [
          "**Sandboxed.** Every Smart Object runs in a locked-down frame. It can’t read your files, your other tabs or the rest of the lesson.",
          "**Offline by default.** Objects can’t reach the internet unless the lesson explicitly allows a specific site.",
          "**Checked before it lands.** The MCP server validates every object and rejects broken or unsafe ones, so the agent fixes them first.",
          "**Open spec.** The [Smart Object spec](https://github.com/Shaitrish/rupok/blob/main/spec/smart-object.md) is public (Apache-2.0), so anyone can audit it or build objects.",
        ] },
      ],
    },
  ],
  faq: [
    { q: "What is an AI whiteboard?", a: "A whiteboard where AI helps make the lesson. In Rupok, an AI writes interactive Smart Objects (simulations, 3D models, quizzes) that run live on your slide while you teach with the pen." },
    { q: "Do I need to buy an AI smartboard?", a: "No. Rupok runs on the board you already have, in Chrome, Edge or Safari. The AI is the assistant you already use." },
    { q: "Which AI works with Rupok?", a: "Any AI that can write an HTML file: ChatGPT, Claude or Gemini (paste the file onto the board). Claude Code, Codex and OpenCode can build whole lessons through Rupok’s MCP server, chat with you inside Rupok, and edit the open board live." },
    { q: "Does it cost anything?", a: "Rupok is free. You use your own AI subscription, or a free AI plan. Optional Rupok AI credits are planned for the paid Pro plan." },
    { q: "Can a Smart Object steal data or go online?", a: "No. Objects run in a sandboxed frame with no access to your files or the rest of the page, and network access is blocked unless the lesson allows a specific site." },
  ],
  final: { h2: "Ask for a lesson. Teach it a minute later.", p: "Open the board, paste in what your AI made, and pick up the pen.", cta: [{ href: "/board/", label: "Open the board" }] },
};

// ------------------------------------------------------------- online
const online = {
  path: "/online-class-whiteboard/",
  lang: "en",
  title: "Online Class Whiteboard: Free Whiteboard for Online Teaching | Rupok",
  description:
    "Teach online classes on Zoom, Google Meet or Teams with a free whiteboard: write on slides with a pen tablet, add live AI activities, then send students a replay they open in the browser.",
  tag: "Online classes",
  h1: "A whiteboard for online classes that students can replay",
  lede: "Share Rupok in Zoom, Google Meet or Teams and teach with a pen tablet, a touchscreen or a mouse. After class, send one small file to the class group: students replay every stroke in their browser, with the live activities still working.",
  crumbs: [["Online class whiteboard"]],
  cta: [{ href: "/board/", label: "Open the board" }, { href: "#setup", label: "How to set it up", kind: "ghost" }],
  fine: "Free. No account for you or your students.",
  aside: ART_ONLINE,
  sections: [
    {
      id: "setup",
      h2: "Set up your online class in three minutes",
      blocks: [
        { steps: [
          { b: "Open the board.", t: "Go to [rupok.app/board](/board/) in Chrome, Edge or Safari and drop in your PDF slides." },
          { b: "Share the window.", t: "In Zoom, Meet or Teams, share the Rupok tab or window. Teach mode keeps the slide fitted with minimal chrome." },
          { b: "Teach with a pen.", t: "A pen tablet (Wacom, XP-Pen, Huion), a pen-enabled laptop or an Android tablet with a stylus gives you pressure-sensitive ink. A touchscreen or a mouse works too." },
          { b: "Send the replay.", t: "Save the lesson as a `.rpk` file and post it in the class WhatsApp group, Google Classroom or email." },
        ] },
      ],
    },
    {
      id: "replay",
      band: "mint",
      split: true,
      h2: "A replay instead of a recording",
      copy: [
        "A screen recording is a heavy video your students have to download. Rupok saves your strokes, not pixels, so the lesson file is usually much smaller than a video of the same class.",
        "Students open it at [rupok.app/open](/open/), press play and watch the class unfold at 1–8× speed. They can pause, zoom in on a diagram, and use the simulations themselves.",
      ],
      blocks: [
        { cards: [
          { b: "Small files", t: "Strokes, not video, so it works on mobile data." },
          { b: "Still interactive", t: "Simulations and quizzes keep working in the replay." },
          { b: "No account", t: "Students just open the file in their browser." },
        ] },
      ],
    },
    {
      id: "tips",
      h2: "Tips for teaching online with a digital whiteboard",
      blocks: [
        { ticks: [
          "**Use a pen.** Even a cheap pen tablet makes handwriting and diagrams far clearer than a mouse.",
          "**Share one window, not your whole screen,** so notifications stay private.",
          "**Use the laser pointer** to show where you are without leaving marks.",
          "**Prepare live objects in advance:** ask your AI for the simulation the night before, so class time is for teaching.",
          "**Keep slides simple:** big text survives video compression on slow connections.",
        ] },
      ],
    },
    {
      id: "live",
      band: "sun",
      h2: "Coming soon: students follow your board live",
      blocks: [
        "With Rupok Cloud, students will follow your board live in their browser from a link, sharp at any zoom instead of a blurry video, and the class becomes a replay automatically. Until then, screen sharing plus a replay file works well.",
      ],
    },
  ],
  faq: [
    { q: "What is the best whiteboard for online teaching?", a: "One you can write on comfortably with a pen, that shares cleanly over video, and that leaves students something to review. Rupok does all three for free, and adds live AI-made activities and a replay file." },
    { q: "Does Rupok work with Zoom, Google Meet and Microsoft Teams?", a: "Yes. Share the Rupok tab or window like any other app. Nothing needs to be installed in the meeting." },
    { q: "Do I need a pen tablet?", a: "No, but it helps a lot. Rupok supports pen pressure from pen tablets and pen-enabled screens in Chrome, Edge or Safari; touch and mouse work too." },
    { q: "How do students watch the class later?", a: "Send them the `.rpk` lesson file. They open it at rupok.app/open on a phone or computer and press play. No account is needed." },
    { q: "Is Rupok a digital whiteboard or an interactive whiteboard?", a: "Both: a digital whiteboard you can use on a laptop for online classes, and interactive whiteboard software for a smartboard in a classroom." },
  ],
  final: { h2: "Teach online with a pen tonight.", p: "Free, in your browser, with nothing for students to install.", cta: [{ href: "/board/", label: "Open the board" }] },
};

// ----------------------------------------------------------- teachers
const teachers = {
  path: "/for-teachers/",
  lang: "en",
  title: "Whiteboard & Note-Taking App for Teachers: Plan, Teach, Replay | Rupok",
  description:
    "Rupok is a free whiteboard and note-taking app for teachers: prepare lessons the night before, teach on the smartboard, and share a replay with students. Open source, with Bangla text.",
  tag: "For teachers",
  h1: "Plan it, teach it, replay it",
  lede: "Rupok follows a teacher’s day. Prepare slides and live activities at home, teach on the classroom board, and send students the class, stroke by stroke, so nothing you write is wiped away.",
  crumbs: [["For teachers"]],
  cta: [{ href: "/board/", label: "Open the board" }, { href: "/open/", label: "How students open lessons", kind: "ghost" }],
  fine: "Free and open source. No account.",
  aside: ART_TEACHER,
  sections: [
    {
      id: "day",
      h2: "A teaching day with Rupok",
      blocks: [
        { cols: [
          [
            { h3: "The night before: Prepare" },
            "Drop in your PDF slides, add text and images, and ask your AI for the interactive parts: a circuit, a titration, a quiz. Prepare mode has the full toolbar and page list.",
            { h3: "In class: Teach" },
            "Teach mode keeps the slide fitted to the board and the chrome out of the way. Write with the pen, use the live objects with touch, press the clicker to move on.",
          ],
          [
            { h3: "After class: Share" },
            "Save the lesson as one `.rpk` file and post it in the class group. Students open it at [rupok.app/open](/open/) in Student mode: read-only, with replay.",
            { h3: "Next year: Reuse" },
            "Open last year’s lesson, replay how you explained it, and build on it. Your lessons are files you own, in an open format.",
          ],
        ] },
      ],
    },
    {
      id: "notes",
      band: "mint",
      h2: "Notes that don’t get wiped",
      blocks: [
        "On a chalkboard, the best explanation of the year is gone by the next period. In Rupok every page is kept, and every stroke is stored with its timing, so a page can be replayed exactly as you wrote it.",
        { cards: [
          { b: "Every page kept", t: "Add as many pages as the class needs; reorder and duplicate them." },
          { b: "Replay at 1–8×", t: "Students who missed class watch it unfold in order." },
          { b: "PDF and handouts", t: "Print the slides with your ink, or handouts with lines for notes." },
        ] },
      ],
    },
    {
      id: "subjects",
      h2: "For every subject",
      blocks: [
        { cols: [
          [{ ticks: ["**Maths:** graphs with sliders, step-by-step working revealed with the clicker.", "**Physics and chemistry:** simulations and 3D molecules you can write on.", "**ICT:** binary, networks and logic, made interactive."] }],
          [{ ticks: ["**Languages:** Bangla and English text side by side on the same slide.", "**History and geography:** timelines and maps, one step per click.", "**Any subject:** quizzes with instant feedback."] }],
        ] },
      ],
    },
    {
      id: "bangladesh",
      band: "sun",
      h2: "Made in Bangladesh, for every classroom",
      blocks: [
        "Rupok is made by Shaitrish in Bangladesh. Bangla text and fonts work in lessons today, voice typing works in Bangla, and a full Bangla interface is coming. It runs on the multimedia-classroom PCs and interactive panels schools already have. [বাংলায় পড়ুন →](/bn/)",
      ],
    },
  ],
  faq: [
    { q: "Is Rupok a note-taking app for teachers?", a: "It’s a whiteboard first, made for presenting at the board, but it keeps everything: pages, ink, live objects and the timing of every stroke. Many teachers use it as their teaching notebook." },
    { q: "Can students edit my lesson?", a: "No. Students open lessons in Student mode, which is read-only. They can replay it, use the live objects, and save their own copy to their Library." },
    { q: "Where are my lessons saved?", a: "On your device, in the Library: folders by subject, course and chapter. In the browser they’re kept in browser storage (or a folder you pick in Chrome or Edge); in the desktop app they’re plain `.rpk` files in a real folder, which Google Drive or OneDrive can sync. Cloud sync is coming with Rupok Cloud." },
    { q: "Does it work on a laptop without a smartboard?", a: "Yes. Use a mouse, touchscreen or pen tablet, and present through a projector or in an online class." },
  ],
  final: { h2: "Mind over matter.", p: "Open the board and write your first line in five seconds.", cta: [{ href: "/board/", label: "Open the board" }] },
};

export const FEATURES = [smartboard, ai, online, teachers];
