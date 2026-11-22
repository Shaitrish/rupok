---
name: rupok-smart-objects
description: Create Rupok lessons, lesson slides and interactive class content (Smart Objects: live HTML diagrams, simulations, timelines, quizzes, games) for teachers, through the Rupok MCP tools or the `rupok` CLI. Use whenever the user asks for slides, a lesson, a class activity, an interactive explainer or a quiz for Rupok or a `.rpk` file, or mentions Smart Objects, `rupok`, `rupoklib://` or the Rupok board.
---

# Rupok lessons and Smart Objects

Rupok is a whiteboard and presentation app for teachers. A lesson is a `.rpk`
file of pages (1920 × 1080 slides). A page holds ink, text, images and
**Smart Objects**: self-contained HTML documents that run live in a sandboxed
iframe on the classroom board. The teacher writes on top of them with a pen,
and the presenter's Next button steps through them like slide builds.

You build lessons with the **Rupok MCP tools** (preferred) or the
**`rupok` CLI**. Authoritative specs: `spec/smart-object.md` (call
`read_object_spec`) and `spec/rupok-format.md`.

## Workflow

1. **Read the spec once.** Call `read_object_spec` before writing any object
   HTML. It returns `spec/smart-object.md`.
2. **Plan the lesson** with the teacher's goal, subject and grade in mind:
   one idea per page, 5–12 pages, a mix of text slides and objects. Say the
   plan in a few lines before you build it.
3. **Create or inspect.** `create_lesson` for a new file, or
   `get_lesson_outline` before editing an existing one.
4. **Write each object** as one complete HTML file (see the template below).
5. **Validate, fix, repeat.** `validate_smart_object` (or
   `rupok validate file.html --steps N`). Fix **every error**; fix warnings
   unless you have a reason. Repeat until valid.
6. **Place it.** `add_smart_object` validates again and saves. On errors it
   saves nothing and returns diagnostics: fix and call again.
7. **Add titles and notes** with `add_text` (size ≥ 32).
8. **Report back**: file path, what is on each page, how many Next presses
   each object takes, and anything the teacher should check.

## MCP tools

The server is `rupok mcp --workspace <dir>`. Paths are relative to that
workspace. Pages are **1-based**. Coordinates are slide pixels from the
top-left of the 1920 × 1080 frame.

| Tool | Parameters | Use |
|---|---|---|
| `list_lessons` | none | Find `*.rpk` files in the workspace. |
| `create_lesson` | `path`, `title`, `pages?` (1–200, default 1) | New lesson of blank slides. Fails if the file exists. |
| `get_lesson_outline` | `path` | Pages and items (ids, kinds, geometry, bundle ids, steps). Call before editing. |
| `add_page` | `path`, `title?`, `index?` (1-based), `pattern?` | Add a blank slide. |
| `add_smart_object` | `path`, `page`, `html`, `title`, `width?` (1280), `height?` (720), `steps?` (0), `x?`, `y?` (centred), `network_hosts?` | Validate and place an object. Returns `item_id` and `bundle_id`. |
| `update_smart_object` | `path`, `bundle_id`, `html`, `steps?`, `network_hosts?` | Replace an object's HTML (all placements update). |
| `add_text` | `path`, `page`, `text`, `x`, `y`, `w?`, `size?` (32), `color?` (`#rrggbb`) | Text box. Keep `size` ≥ 24, titles 48–72. |
| `delete_item` | `path`, `page`, `item_id` | Remove one item. |
| `set_page_background` | `path`, `page`, `pattern` (`plain`/`grid`/`dots`/`lines`), `color?` | Page background. |
| `validate_smart_object` | `html`, `steps?`, `network_hosts?`, `width?`, `height?` | Check without saving. |
| `read_object_spec` | none | The authoring spec (Markdown). |

**CLI equivalents** (when MCP is not connected):

```sh
rupok new lesson.rpk --title "Binary numbers" --pages 6
rupok validate counter.html --steps 10        # exit 1 on errors; --json for machine output
rupok add-object lesson.rpk counter.html --page 2 --steps 10 --w 1280 --h 720
rupok info lesson.rpk
```

## The object contract

- **One HTML file**, everything inline (CSS, JS, SVG, data). No build step.
- **Offline.** No CDNs, no web fonts, no remote images. Use `rupoklib://`
  libraries or inline code. Any remote resource is a validation error unless
  its host is in `network_hosts` (avoid; lessons must work without Wi-Fi).
- **Sandboxed.** `sandbox="allow-scripts"`, opaque origin: no
  `localStorage`/`sessionStorage`/`indexedDB`/cookies, no `alert`/`confirm`,
  no popups, no `window.top`/`parent.document`.
- **The host injects the SDK** as `window.rupok` before your scripts run.
  Do not include it yourself.
- **Fills its frame** at any size: the iframe viewport *is* the object.
  No scrolling.

## SDK quick reference (`window.rupok`)

```js
rupok.steps.setCount(n)            // declare build steps (0 = static); call at startup
rupok.steps.onChange((step, info) => render(step))  // initial call + every change
rupok.steps.current / .count
rupok.steps.next() / .prev() / .goto(n)   // e.g. your own Next button
rupok.ready.then((info) => {...})  // init done (info.standalone in a plain browser)
rupok.state.load()                 // saved JSON value or undefined (after ready)
rupok.state.save(value)            // JSON value, ≤ 256 KiB, debounced 200 ms; only after ready
rupok.theme.get("color.accent", "#2563eb")   // also CSS vars: var(--rupok-color-accent, #2563eb)
rupok.quiz.submit({ questionId: "q1", answer: "b", correct: true, score: 3 })
rupok.mode                         // "edit" | "present" | "replay" | "student"
rupok.log("info", "anything")      // shows up for the teacher/agent
```

Theme CSS variables: `--rupok-color-bg`, `--rupok-color-fg`, `--rupok-color-accent`,
`--rupok-color-accent2`, `--rupok-color-muted`, `--rupok-color-success`,
`--rupok-color-danger`, `--rupok-font-body`, `--rupok-font-heading`,
`--rupok-font-mono`, `--rupok-radius`. Always write a fallback:
`var(--rupok-color-fg, #1d2433)`.

## Steps: render state, not events

Step `0` is the initial view; step `count` is fully revealed. The presenter
can jump, go back or reload at any step, so `render(step)` must draw exactly
that step from scratch:

```js
function render(step) {
  parts.forEach((el, i) => el.classList.toggle("shown", i < step));
}
rupok.steps.setCount(parts.length);
rupok.steps.onChange(render);
```

Pass the same number as `steps` to `add_smart_object`. The validator warns
(`steps`) when `steps > 0` but the HTML never mentions `rupok`.

## Template

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Photosynthesis</title>
<style>
  :root {
    --bg: var(--rupok-color-bg, #ffffff);
    --fg: var(--rupok-color-fg, #1d2433);
    --accent: var(--rupok-color-accent, #2563eb);
    --muted: var(--rupok-color-muted, #64748b);
    --font: var(--rupok-font-body, system-ui, sans-serif);
  }
  /* Designed at 1280 x 720; 1rem = 16px there and scales with the frame. */
  html { font-size: min(calc(100vw * 16 / 1280), calc(100vh * 16 / 720)); }
  html, body { margin: 0; height: 100%; overflow: hidden; }
  body { background: var(--bg); color: var(--fg); font-family: var(--font);
         padding: 3rem 4rem; box-sizing: border-box; }
  h1 { font-size: 3rem; margin: 0 0 1.5rem; }
  .part { font-size: 1.75rem; opacity: 0; transition: opacity .4s; }
  .part.shown { opacity: 1; }
  button { font: inherit; min-height: 4rem; padding: 0 1.5rem; touch-action: manipulation; }
</style>
</head>
<body>
  <h1>Photosynthesis</h1>
  <p class="part">Light + water + carbon dioxide</p>
  <p class="part">make glucose and oxygen.</p>
<script>
  const parts = document.querySelectorAll(".part");
  function render(step) {
    parts.forEach((el, i) => el.classList.toggle("shown", i < step));
  }
  rupok.steps.setCount(parts.length);
  rupok.steps.onChange(render);
</script>
</body>
</html>
```

Complete, tested examples live in `examples/objects/` (binary counter,
computing timeline, networking quiz). They also show an optional
`window.rupok || {…}` fallback that lets a file preview when opened directly
in a browser; it is not needed inside rupok.

## Design rules (projectors and touch boards)

- **Size**: slides are 1920 × 1080. Objects are usually 960 × 540 to
  1920 × 1080 (16:9). Use 1920 × 1080 at `x: 0, y: 0` for a full slide.
- **Scale with the frame**: `rem` tied to the viewport (template above),
  `vw`/`vh`, flex or grid. For canvas or WebGL, use `ResizeObserver` and
  `devicePixelRatio`.
- **Text**: body ≥ 24px at nominal size (≥ 1.5rem in the template), headings
  ≥ 48px, at most about 40 words per step.
- **Contrast**: at least 4.5:1. Use the theme variables. Never rely on colour
  alone: add labels or icons.
- **Touch first**: no hover-only behaviour, tap targets ≥ 64px, `click` or
  pointer events, `touch-action: manipulation`. Don't capture the
  presenter's keys (arrows, Space, PageUp/PageDown).
- **The pen draws on top**: leave calm, empty space for the teacher's
  handwriting. No busy full-bleed backgrounds; no endless motion behind text.
- **Motion**: short transitions (≤ 600 ms); honour `prefers-reduced-motion`.
- **Accuracy**: dates, facts and formulas must be correct for the stated
  grade. Prefer fewer, true statements.

## Libraries (`rupoklib://`, offline)

| Name | Entry | Use |
|---|---|---|
| `three` | `rupoklib://three/three.module.js` | `import * as THREE from "rupoklib://three/three.module.js"` |
| `d3` | `rupoklib://d3/d3.min.js` | global `d3` |
| `katex` | `rupoklib://katex/katex.min.js` + `katex.min.css` | global `katex` (maths) |
| `mermaid` | `rupoklib://mermaid/mermaid.min.js` | global `mermaid` (diagrams) |
| `chart.js` | `rupoklib://chart.js/chart.umd.min.js` | global `Chart` |
| `gsap` | `rupoklib://gsap/gsap.min.js` | global `gsap` |
| `p5` | `rupoklib://p5/p5.min.js` | global `p5` |
| `matter-js` | `rupoklib://matter-js/matter.min.js` | global `Matter` (physics) |
| `animejs` | `rupoklib://animejs/anime.min.js` | global `anime` |
| `tailwind` | `rupoklib://tailwind/tailwind.js` | utility classes |
| `lucide` | `rupoklib://lucide/lucide.min.js` | global `lucide`, `lucide.createIcons()` |

Prefer plain HTML, CSS and SVG when they are enough.

## Fixing validation diagnostics

| Code | Severity | Fix |
|---|---|---|
| `network` | error | Remove the CDN/remote URL. Use `rupoklib://` or inline the code/data. |
| `entry` | error | The HTML is missing or not UTF-8; send the full document. |
| `title` | error | Give the object a non-empty title (≤ 200 characters). |
| `size` | error | `width`/`height` must be 50–8000. |
| `steps` | error / warning | ≤ 500 steps. Warning: you declared steps but never use `rupok`. |
| `permissions` | error | `network_hosts` entries are plain hostnames (`api.example.org`). |
| `size_bytes` | error | Bundle over 25 MiB: shrink or drop embedded media. |
| `lib` | warning | Unknown `rupoklib://` name; use one from the table. |
| `storage` | warning | Replace `localStorage`/`indexedDB` with `rupok.state`. |
| `sandbox` | warning | Remove `window.top` / `parent.document` / `top.location`. |

**Known validator bug (until fixed):** the validator can crash on a `//`
sequence (for example a `// comment`) that comes shortly after non-ASCII text
such as Bangla or accented letters. In object HTML, write JavaScript comments
as `/* … */` and avoid `//` outside URLs.

## Checklist before placing

- [ ] Valid: zero errors, warnings understood.
- [ ] `setCount(n)` matches the `steps` you pass; every step renders from
      scratch; step 0 makes sense on its own.
- [ ] Readable from the back of the room; works by touch only.
- [ ] Looks right at half size and at full slide size.
- [ ] Saves only after `rupok.ready`; no storage, no network, no `alert`.
- [ ] Facts checked.
