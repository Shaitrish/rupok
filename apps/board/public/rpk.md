# RPK file format (Rich Presentation Kit)

> An RPK (Rich Presentation Kit) file is a Rupok lesson: slides, handwriting, live activities and a class replay in one open ZIP package. What's inside, how to open it, convert it to PDF, and read it with AI.

Canonical: https://rupok.app/rpk/ · Language: en · Updated: 2026-11-23

An **RPK file** (`.rpk`, short for **Rich Presentation Kit**) is a lesson made with Rupok, the free, open-source whiteboard for teachers. One file holds the slides, everything written on them, live interactive activities and a replay of the class. The format is open, documented and readable by people, scripts and AI.

## What is an RPK file?

RPK stands for **Rich Presentation Kit**. Teachers make `.rpk` lessons in [Rupok](https://rupok.app/board/) and share them with students like slide decks, on WhatsApp, Gmail or Google Drive. Unlike a PDF, the lesson stays alive:

- **Pages:** slides (often from a PDF) or infinite boards.
- **Ink:** every pen stroke, with its timing, so the class can be replayed.
- **Smart Objects:** simulations, 3D models and quizzes that still run.
- **A readable outline:** the lesson as Markdown and JSON, for search and AI.

- **Full name**: Rich Presentation Kit (Rupok lesson)
- **Extension**: `.rpk`
- **Media type**: `application/vnd.rupok.lesson+zip`
- **Container**: ZIP, with a `mimetype` first entry (like EPUB)
- **Opens with**: Rupok, free in Chrome or Edge ([rupok.app/open](https://rupok.app/open/))
- **Specification**: Open, Apache-2.0 ([spec on GitHub](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md))

## What's inside a .rpk file

A `.rpk` is a ZIP archive. Rename a copy to `.zip` and you can look inside:

```text
lesson.rpk                 a ZIP archive
├── mimetype               application/vnd.rupok.lesson+zip (first entry)
├── manifest.json          title, dates, course and chapter, page count
├── README.md              "This is a Rupok lesson…" + the outline
├── content.md             the lesson as readable Markdown
├── content.json           the same outline, structured
├── doc.loro               the lesson itself: pages, ink, history (Loro CRDT)
├── assets/<hash>.png      images and media
├── objects/<id>/index.html  interactive Smart Objects (HTML)
└── thumbs/<n>.png         page previews (optional)
```

`doc.loro` is the authoritative lesson: a [Loro](https://loro.dev) CRDT document that keeps its whole edit history, so one file is the lesson, its undo history and its class recording. `README.md`, `content.md` and `content.json` are plain-text summaries Rupok rewrites on every save, so anyone can read a lesson without special software.

## How to open an RPK file

1. **Go to rupok.app/open** in Chrome or Edge, on a phone or a computer.
2. **Drop the .rpk file on the page** or tap “Choose a .rpk file”. It opens in Student mode, read-only.
3. **Replay the class,** flip through the pages and try the live activities.

[Open the RPK viewer](https://rupok.app/open/)

*Nothing is uploaded: the file is opened by your browser on your device. Teachers edit lessons at rupok.app/board.*

## How to convert an RPK file to PDF

1. **Open the file** at [rupok.app/open](https://rupok.app/open/).
2. **Press Download PDF.** Every page is exported with the teacher's ink on it.
3. **Print it or share it.** The PDF is made in your browser; nothing leaves your device.

## How AI and developers read RPK files

Every `.rpk` describes itself. Give one to ChatGPT, Claude or an agent and it can tell what the lesson contains.

### For ChatGPT, Claude and other chatbots

1. **Upload the .rpk file** (or a link to it) and ask about the lesson.
2. **The model unzips it** and reads `README.md` and `content.md`: titles, text, maths, diagrams, images and activities, page by page.
3. **Interactive parts** are HTML files in `objects/`, which the model can read too.

### For agents and developers

- Agent skill: [rupok.app/skills/read-rpk.md](https://rupok.app/skills/read-rpk.md), or [download it with its script](https://github.com/Shaitrish/rupok/tree/main/skills/read-rpk).
- Markdown twin of this page: [rupok.app/rpk.md](https://rupok.app/rpk.md).
- Full fidelity: read `doc.loro` with the official Loro packages (`pip install loro`, `npm install loro-crdt`).
- Command line: `rupok export --md lesson.rpk` prints the outline.

```sh
pip install loro          # Python
npm install loro-crdt     # JavaScript

python3 read_rpk.py lesson.rpk        # the skill's script: .rpk → Markdown
rupok export --md lesson.rpk          # the Rupok CLI does the same
```

## Example: the outline inside a lesson

This is the `content.md` Rupok wrote for a short ICT lesson ([download the sample .rpk](https://rupok.app/skills/sample-lesson.rpk)).

```markdown
# Binary numbers

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

- Image: `assets/6c91…2ee8.png` (480 × 320)
- Smart Object: **Binary counter** (8 steps, `objects/binary-counter/index.html`)
```

## Questions people ask

### What does RPK stand for?

RPK stands for Rich Presentation Kit. A .rpk file is a lesson made with Rupok, the free, open-source whiteboard for teachers.

### What is an RPK file?

An RPK file is one whole lesson: its pages, the teacher's handwriting with timing, images, live Smart Objects (simulations, 3D models, quizzes) and a readable outline. It is a ZIP package with an open, documented format.

### How do I open an RPK file?

Go to [rupok.app/open](https://rupok.app/open/) in Chrome or Edge and drop the file on the page. It opens free in your browser, with no account and nothing uploaded.

### Is there a free RPK file viewer?

Yes. Rupok's student viewer at [rupok.app/open](https://rupok.app/open/) is free, runs in the browser on phones and computers, and replays the class stroke by stroke.

### How do I convert an RPK file to PDF?

Open the file at [rupok.app/open](https://rupok.app/open/), then press Download PDF. Every page is exported with the teacher's ink, on your device.

### Can ChatGPT or Claude read an RPK file?

Yes. Every .rpk made by current Rupok versions carries `README.md`, `content.md` and `content.json`: the lesson as plain Markdown and JSON. Upload the file and ask about it; agents can also use the [read-rpk skill](https://rupok.app/skills/read-rpk.md).

### Is the RPK format open?

Yes. The specification and the reference reader and writer are Apache-2.0, so anyone can build tools that read or write .rpk files. See the [spec on GitHub](https://github.com/Shaitrish/rupok/blob/main/spec/rupok-format.md).

### Can I open a .rpk file as a ZIP?

Yes. Rename a copy to .zip, or open it with any ZIP tool. Start with `content.md`; the lesson itself is `doc.loro`, which the Loro libraries for Python and JavaScript read.

### My .rpk file isn't a Rupok lesson. What is it?

Other programs also use the .rpk extension, for example Esri CityEngine rule packages, Tizen resource packages and TI-99/4A cartridge files for the MAME emulator. If [rupok.app/open](https://rupok.app/open/) says it isn't a Rupok lesson, open it with the program that made it.
