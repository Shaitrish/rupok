# Smart Objects

| | |
|---|---|
| **Version** | 1 (bundle `schema` 1, bridge protocol `rupok: 1`, SDK `"1"`) |
| **Status** | Draft for Phase 0. Changes go through RFCs (see `spec/rupok-format.md#change-process`). |
| **License** | Apache-2.0 (see `spec/LICENSE`) |
| **Reference implementation** | `crates/rupok-format/src/object.rs` (bundle, validation), `packages/object-sdk/sdk.js` (in-object SDK) |

A **Smart Object** is live HTML/CSS/JS placed on a lesson page: a diagram, a
3D model, a simulation, a quiz, a game. Teachers' AI agents usually write
them. They run in a locked-down iframe, the teacher writes with the pen on top
of them, and the presenter's Next button steps through them like slide builds.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be read as in
RFC 2119 and RFC 8174 when they appear in all capitals.

## Contents

- [Bundle layout](#bundle-layout)
- [Manifest](#manifest)
- [Sandbox](#sandbox)
- [Offline libraries (`rupoklib://`)](#offline-libraries-rupoklib)
- [Bridge protocol](#bridge-protocol)
- [SDK API (`window.rupok`)](#sdk-api-windowrupok)
- [Steps](#steps)
- [State](#state)
- [Theme](#theme)
- [Maths (LaTeX)](#maths-latex)
- [Modes](#modes)
- [Validation](#validation)
- [Design guidance for AI authors](#design-guidance-for-ai-authors)
- [Complete minimal example](#complete-minimal-example)
- [Conformance notes](#conformance-notes)

## Bundle layout

A bundle is a directory (on disk, or `objects/<bundle>/` inside a `.rpk`
package):

```text
manifest.json     REQUIRED  this spec
index.html        REQUIRED  the entry file (name set by manifest "entry")
...               optional  more files: scripts, styles, images, data
```

- Paths follow the `.rpk` path rules (`spec/rupok-format.md#path-rules`):
  relative, `/`-separated, no empty, `.` or `..` segments, no `\` or
  control characters, at most 512 bytes.
- All files together MUST NOT exceed 25 MiB (26,214,400 bytes).
- Relative URLs in the entry file resolve to files inside the bundle.
- Symbolic links are never followed when a bundle is loaded from disk.

**Single-file objects** are the common case for AI output: one
self-contained HTML document. A host wraps it as a bundle with
`entry = "index.html"`, `libs` detected from its `rupoklib://` references, and
the title, size and step count supplied by the caller (for example the
`add_smart_object` MCP tool). The reference function is
`ObjectBundle::from_single_html(html, title, w, h, steps)`.

## Manifest

`manifest.json` is a UTF-8 JSON object.

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `schema` | integer | yes | none | MUST be `1`. |
| `title` | string | yes | none | Human title. Not blank; SHOULD be ≤ 200 characters. |
| `description` | string | no | `""` | One or two sentences: what it shows, for whom. |
| `entry` | string | no | `"index.html"` | Path of the entry HTML file. MUST end in `.html`. |
| `size` | object | yes | none | `{ "w": number, "h": number }`: nominal size in world units (CSS px at 100 %). Each in `[50, 8000]`. |
| `steps` | integer | no | `0` | Build steps the presenter can click through. `0` means static. At most 500. |
| `libs` | string[] | no | `[]` | `rupoklib://` libraries used (names from [the list](#offline-libraries-rupoklib)). Informative; hosts MAY preload them. |
| `permissions` | object | no | `{}` | See below. |
| `permissions.network` | string[] | no | `[]` | Plain hostnames the object may contact, e.g. `"api.open-meteo.com"`. |
| `theme` | string[] | no | `[]` | Theme tokens the object reads, e.g. `"color.accent"`. Informative, for brand kits. |
| `generator` | string | no | absent | What produced the object, e.g. `"claude-code"`. |

- `permissions.network` entries MUST be plain hostnames: no scheme, port,
  path or wildcard, and at least one dot. An entry also allows its
  subdomains: `example.com` allows `api.example.com`.
- Readers MUST ignore unknown fields.
- The reference writer omits `description`, `libs`, `theme` and `generator`
  when empty, and always writes `entry`, `steps` and `permissions`.

Example:

```json
{
  "schema": 1,
  "title": "Binary counter",
  "description": "Eight tappable bits with place values, for Grade 7 ICT.",
  "entry": "index.html",
  "size": { "w": 1280, "h": 720 },
  "steps": 10,
  "permissions": {}
}
```

## Sandbox

Smart Objects are untrusted code: an AI wrote them, or someone shared a
lesson. Hosts MUST run them as follows.

### iframe

```html
<iframe sandbox="allow-scripts" ...></iframe>
```

The `sandbox` attribute MUST be exactly `allow-scripts`. Hosts MUST NOT add
`allow-same-origin`, `allow-top-navigation*`, `allow-popups*`, `allow-forms`,
`allow-modals`, `allow-downloads` or `allow-pointer-lock`, and MUST NOT grant
powerful features through the `allow` attribute (camera, microphone,
geolocation, clipboard, USB…). Hosts MAY grant `fullscreen`. For the object
this means:

| Works | Does not work |
|---|---|
| Scripts, inline `<script>` and `<style>`, ES modules | `localStorage`, `sessionStorage`, `indexedDB`, cookies (opaque origin) |
| Canvas, WebGL, SVG, CSS animation, Web Audio | Reaching the parent: `window.top`, `parent.document`, `top.location` |
| Pointer, touch and keyboard events inside the frame | Navigating the top window, opening popups or new tabs |
| `postMessage` to the parent (through the SDK) | `alert()`, `confirm()`, `prompt()`, form submission |
| Files inside the bundle, `rupoklib://` libraries | Any network host not listed in `permissions.network` |

Use `rupok.state.save()` instead of browser storage.

### Network

- Network access is **blocked by default**. Hosts MUST enforce this at run
  time (Content-Security-Policy, request interception, or both). The
  [validator](#validation) is a static, best-effort check and is not the
  security boundary.
- When `permissions.network` lists hosts, the host MAY allow requests to
  those hosts and their subdomains. Hosts SHOULD allow only `https:` and
  `wss:` to them, and SHOULD show the teacher which hosts an object contacts.
- Hosts MUST NOT let objects reach `localhost`, private address ranges or the
  host application's own origin.

A recommended policy for an object with no network permission:

```text
default-src 'none';
script-src  'unsafe-inline' 'unsafe-eval' <bundle> <libs> blob:;
style-src   'unsafe-inline' <bundle> <libs>;
img-src     <bundle> <libs> data: blob:;
font-src    <bundle> <libs> data:;
media-src   <bundle> data: blob:;
connect-src 'none';
```

`<bundle>` and `<libs>` are the origins the host serves bundle files and
`rupoklib` libraries from (they may be the same).

### Host duties

- **Inject the SDK.** Before any of the object's own scripts run, the host
  MUST make `window.rupok` available, for example by inserting the SDK as
  the first `<script>` in `<head>` when it serves the entry file. Objects
  SHOULD NOT ship their own copy. If two copies load, the first one wins.
- **Tell the SDK where the libraries are.** Hosts MUST write
  `<meta name="rupok-lib" content="<library base URL>/">` into the object
  document before the SDK script. The SDK loads KaTeX for
  [built-in maths](#maths-latex) from there. It is a meta tag, not an `init`
  field, so maths can start loading before `init` arrives and in standalone
  previews.
- **Serve `rupoklib://`.** `rupoklib:` is a syntactically valid URL scheme,
  so browsers parse `rupoklib://three/x.js` as an absolute URL everywhere
  (`src`, `href`, CSS `url()`, `import`) and cannot fetch it: no browser has
  a handler for it. Hosts MUST therefore rewrite the literal string
  `rupoklib://` to their library base URL (for example
  `https://example.org/rupoklib/`) in the bundle's HTML, JS and CSS before
  the object loads them. Hosts MAY also inject an import map before any
  module script, `<script type="importmap">{"imports": {"rupoklib://": "<library base>/"}}</script>`,
  which also resolves module specifiers assembled at run time. Objects SHOULD
  write `rupoklib://` URLs as literal strings so the rewrite finds them.
  Library responses MUST carry the right MIME type and
  `Access-Control-Allow-Origin: *`, because module scripts and fonts loaded
  from an opaque-origin frame need CORS.
- **Provide the theme fonts.** Hosts SHOULD make the [theme fonts](#fonts)
  available inside the object (the object cannot see the host's own fonts).
- **Ink stays on top.** The pen layer is drawn above every object. While a
  pen or eraser tool is active, pointer events go to the ink layer, not the
  iframe.
- **Keyboard focus.** After the user interacts with an object in `present`
  mode, the host SHOULD take keyboard focus back, so the presenter's clicker
  (PageDown/PageUp, arrows) keeps working.
- **Validate first.** Hosts MUST run validation before placing a bundle.
  Errors block placement; warnings are shown to the teacher or agent.

## Offline libraries (`rupoklib://`)

Classrooms often have bad internet, so popular libraries ship with rupok.
Reference them as `rupoklib://<name>/<file>`. Library names are exactly the
`LIBS` list in `crates/rupok-format/src/object.rs`:

| Name | Entry file(s) | Use as |
|---|---|---|
| `three` | `rupoklib://three/three.module.js` | ES module: `import * as THREE from "rupoklib://three/three.module.js"` (it imports its sibling `three.core.js`). Extras, not guaranteed: addons under `rupoklib://three/addons/` (`controls/OrbitControls.js`, `controls/TrackballControls.js`, `renderers/CSS2DRenderer.js`, `environments/RoomEnvironment.js`) with their `three` import rewritten to a relative path, so they need no import map |
| `d3` | `rupoklib://d3/d3.min.js` | classic script, global `d3` |
| `katex` | `rupoklib://katex/katex.min.js` + `rupoklib://katex/katex.min.css` | global `katex`; the CSS loads fonts from `rupoklib://katex/fonts/`. Extra: `auto-render.min.js` |
| `mermaid` | `rupoklib://mermaid/mermaid.min.js` | global `mermaid` |
| `chart.js` | `rupoklib://chart.js/chart.umd.min.js` | global `Chart` |
| `gsap` | `rupoklib://gsap/gsap.min.js` | global `gsap` |
| `p5` | `rupoklib://p5/p5.min.js` | global `p5` (p5.js 2.x) |
| `matter-js` | `rupoklib://matter-js/matter.min.js` | global `Matter` |
| `animejs` | `rupoklib://animejs/anime.min.js` | global `anime` (Anime.js 4 API: `anime.animate()`, `anime.createTimeline()`, `anime.stagger()`) |
| `tailwind` | `rupoklib://tailwind/tailwind.js` | Tailwind's browser build (v4): generates CSS for the classes on the page |
| `lucide` | `rupoklib://lucide/lucide.min.js` | global `lucide`; call `lucide.createIcons()` |

- The entry files above are guaranteed in v1. Other files of a library MAY be
  present but are not guaranteed.
- Library versions are pinned per Rupok release. A version change that
  breaks the listed usage needs an RFC.
- A `rupoklib://` name not in the list is a validation warning (`lib`) and will
  not load.
- Any other library: inline it in the HTML or ship it as a file in the
  bundle. Never load it from a CDN.

## Bridge protocol

Version 1. The object and the host talk with `postMessage`. The SDK
implements the object side; you only need this section to write a host or
your own SDK.

### Rules

- Every message is a plain, JSON-serialisable object with `rupok: 1` and a
  string `type`.
- **Object → host:** `window.parent.postMessage(msg, "*")`.
- **Host → object:** `iframe.contentWindow.postMessage(msg, "*")`. The target
  origin must be `"*"` because a sandboxed frame has an opaque origin.
- The host identifies the sender by `event.source === iframe.contentWindow`.
  It MUST NOT rely on `event.origin` (it is `"null"`).
- The object accepts messages only when `event.source === window.parent`.
- Both sides MUST ignore messages with an unknown `type`, a missing or
  different `rupok`, or malformed fields.
- Hosts SHOULD rate-limit each object (for example drop messages beyond 100 per
  second) and MUST drop any message larger than 300 KiB, except a `snapshot`
  reply, which MUST be dropped above 16 MiB.

### Object → host

| Message | Fields | Meaning |
|---|---|---|
| `hello` | `sdk: "1"` | Sent as soon as the SDK loads. The host replies with `init`. |
| `state` | `state`: JSON value | Persist this state. The serialised JSON MUST be ≤ 256 KiB (UTF-8). |
| `goto` | `step`: integer | Ask the host to change step (for example the object's own Next button). The host replies with `step`. |
| `steps` | `count`: integer `0..500` | Declare or change the step count at run time. |
| `resize` | `w`, `h`: numbers | Ask for a new size in world units. |
| `quiz` | `payload`: `{ questionId, answer, correct?, score? }` | Record an answer. |
| `log` | `level`: `"info"` \| `"warn"` \| `"error"`, `message`: string | Diagnostic text for the teacher or agent. |
| `key` | `key`: `"PageDown"` \| `"PageUp"` \| `"ArrowRight"` \| `"ArrowLeft"` \| `"ArrowDown"` \| `"ArrowUp"` \| `"Escape"` | A presenter key the object received while focused and did **not** handle (`defaultPrevented` stayed false). The SDK sends it so a clicker keeps working after the teacher touches an object. Objects that use these keys themselves MUST call `event.preventDefault()`. Keys typed into inputs are never forwarded. |
| `snapshot` | `id`: the request's `id`; then `dataUrl`: string, or `error`: string | Reply to a host `snapshot` request: a `data:image/png;base64,…` picture of the object's viewport, or a short reason it could not be drawn. See [Snapshots](#snapshots). |

`quiz.payload` fields: `questionId` string; `answer` any JSON value (the
option id, text or number the learner chose); `correct` optional boolean;
`score` optional number (running score so far).

### Host → object

| Message | Fields | Meaning |
|---|---|---|
| `init` | `id`, `step`, `steps`, `state`, `theme`, `mode` | Reply to `hello`. |
| `step` | `step`: integer | The current step changed. |
| `theme` | `theme`: object | The theme changed. Carries the full theme. |
| `mode` | `mode`: string | The mode changed. |
| `snapshot` | `id`: string (`[A-Za-z0-9._:-]{1,64}`) or number; `scale`: number, optional | Ask for a PNG picture of the object as it looks now. `scale` is device pixels per CSS pixel (default 2, clamped to `[0.25, 4]`). The object replies with `snapshot` and the same `id`. |

`init` fields:

| Field | Type | Meaning |
|---|---|---|
| `id` | string | The object's item id on the page. |
| `step` | integer | Current step, `0 ≤ step ≤ steps`. |
| `steps` | integer | Current step count (the manifest value unless the object declared another). |
| `state` | JSON value or `null` | The last saved state, `null` if none. |
| `theme` | object | Map of theme token → string value. Tokens it omits use the SDK defaults. |
| `mode` | string | `"edit"`, `"present"`, `"replay"` or `"student"`. |

### Sequence

```text
object (iframe)                         host
   │── hello {sdk:"1"} ─────────────────▶│
   │◀──────────── init {id, step, ...} ──│
   │── steps {count:8} ─────────────────▶│  optional: runtime count
   │◀──────────────────── step {step:1} ─│  presenter pressed Next
   │── goto {step:2} ───────────────────▶│  object's own button
   │◀──────────────────── step {step:2} ─│  host confirms and stores it
   │── state {state:{...}} ─────────────▶│  debounced by the SDK
```

### Host behaviour

- On `goto`, the host clamps the step to `[0, count]`, stores it in the
  item's `step` key (one commit), and replies with `step`. In `replay` mode it
  MAY refuse and reply with the unchanged step.
- On `steps`, if the stored step is greater than the new count, the host MUST
  clamp it and send `step`.
- On `state`, the host stores the JSON text in the item's `state` key (one
  commit). It MUST reject state over 256 KiB. In `replay` mode it SHOULD
  ignore `state`.
- On `resize`, the host MAY apply the size in `edit` mode and SHOULD ignore it
  in other modes. Values outside `[50, 8000]` MUST be ignored.
- On `quiz`, the host records the answer (class results are planned).
- On `key`, the host performs its presenter action as if the key were pressed on the board: next/previous step or page, and `Escape` leaves "use objects" mode and returns focus to the board.
- On `log`, the host shows the message to the teacher (for example in a
  developer panel) and MAY pass it to the agent that made the object.

### Snapshots

A host that needs a still picture of an object (the reference host uses it
for **Export PDF**) sends `snapshot`. The object answers with a PNG data URL
of its viewport (`innerWidth` × `innerHeight` CSS pixels at `scale`).

The SDK answers automatically; authors do nothing:

1. Inside the next animation frame (or after 150 ms if the frame never
   paints) it copies every visible `<canvas>`. Copying right after the
   object's own frame callbacks lets it read WebGL canvases created without
   `preserveDrawingBuffer`.
2. It clones the document, freezes form values, replaces each canvas with an
   image of its copy, inlines images it can read, removes scripts and
   frames, and draws the clone through an SVG `<foreignObject>`.
   Stylesheets and fonts loaded from other URLs (including `rupoklib://`) do
   not load inside an SVG image, so inline styles and inline SVG render best.
3. If the browser refuses that picture (a tainted canvas, no
   `XMLSerializer`), it draws only the canvas copies over the page
   background. With no canvas either, it replies with `error`.
4. Output is capped at 4096 × 4096 pixels; while the data URL exceeds 16 MiB
   the scale is halved (up to three times).

Hosts MUST treat a missing or late reply as normal. The reference host asks
once the frame has loaded, initialised and the page is idle, waits about
1.5 s, asks once more (the first snapshot in a fresh frame pays a one-off
warm-up), and after another 1.5 s draws a placeholder card with the object's
title. Hosts SHOULD take snapshots in a separate, hidden iframe of the same
bundle, given the item's saved `step` and `state` in `init`, and MUST NOT
store `state`, `goto`, `steps` or `resize` messages from such a frame
(exporting must not change the lesson).

## SDK API (`window.rupok`)

`packages/object-sdk/sdk.js` (npm: `@rupok/object-sdk`) is a
dependency-free script that the host injects. Types are in
`packages/object-sdk/index.d.ts`. **No method ever throws**, including before
`ready`.

| Member | Description |
|---|---|
| `version` | `"1"` |
| `ready` | `Promise<InitInfo>`. Resolves on `init`, or with defaults in standalone mode. Never rejects. |
| `mode` | Current mode. `"present"` until `init`. |
| `onModeChange(cb)` | Calls `cb(mode)` when the mode changes. Returns an unsubscribe function. |
| `steps.current` | Current step. `0` until `init`. |
| `steps.count` | Step count. |
| `steps.onChange(cb)` | Subscribe to step changes; see [Steps](#steps). Returns an unsubscribe function. |
| `steps.next()` / `steps.prev()` | Ask for the next or previous step. No-op at the ends. |
| `steps.goto(n)` | Ask for step `n`, clamped to `[0, count]`. |
| `steps.setCount(n)` | Declare the step count (integer, clamped to `[0, 500]`). |
| `state.load()` | The saved state (from `init`, or the last value saved in this session), or `undefined`. A fresh copy each call. |
| `state.save(value)` | Persist a JSON-serialisable value. Debounced; see [State](#state). |
| `theme.get(token, fallback)` | A theme value, or `fallback` if the token is unknown. |
| `theme.all()` | A copy of the whole theme. |
| `theme.onChange(cb)` | Calls `cb(theme)` with the initial theme once known, then on every change. Returns an unsubscribe function. |
| `quiz.submit(payload)` | Send a `quiz` message. `questionId` is required. |
| `resize(w, h)` | Send a `resize` message. |
| `log(level, ...args)` | Log to the console and send a `log` message. Non-string args are JSON-encoded; the message is cut at 4,000 characters. |
| `tex(el, latex, options)` | Typeset LaTeX into `el` with KaTeX, replacing its content; `options.display` for display style. Resolves to `true` once rendered. The same formula again is a no-op. See [Maths](#maths-latex). |
| `math.render(root)` | Typeset `$…$`, `$$…$$`, `\(…\)`, `\[…\]` and `[data-tex]` under `root` (default: the body). Resolves to the number of formulas rendered. |
| `math.load()` | Load KaTeX now. Resolves to whether maths can be rendered. |
| *(automatic)* | Answers host `snapshot` requests; see [Snapshots](#snapshots). |

`InitInfo` is `{ id, step, steps, state, theme, mode, standalone }`. In
standalone mode `id` is `"standalone"`.

### Lifecycle

1. On load the SDK applies the default theme as CSS variables, installs its
   message listener and, if it has a parent, posts `hello`.
2. Calls to `steps.next/prev/goto/setCount`, `quiz.submit`, `resize` and
   `log` made before `ready` are queued and run, in order, right after `init`.
3. `state.save()` before `ready` is ignored with a console warning, so a
   startup default cannot overwrite the teacher's saved state. Load first,
   then save.
4. Uncaught errors and unhandled promise rejections are forwarded as `log`
   messages with level `error` (at most 20 per page load), so the host can
   show them to the teacher or agent.

### Standalone mode

If the window has no parent (`window.parent === window`), or no `init`
arrives within **600 ms** of loading, the SDK resolves `ready` with defaults
(`step 0`, `steps 0`, `state undefined`, default theme, mode `"present"`,
`standalone: true`) and drives steps itself:

| Key | Action |
|---|---|
| ArrowRight, Space, PageDown | `steps.next()` |
| ArrowLeft, PageUp | `steps.prev()` |

Keys are ignored while focus is in a text field, select or editable element,
when Ctrl/Alt/Meta is held, and Space is ignored on a focused button. Saved
state stays in memory. This lets authors preview an object in a normal
browser. Declare the count with `rupok.steps.setCount(n)` at startup so
standalone previews know how many steps there are.

Outside Rupok nothing injects the SDK. To preview a file by opening it
directly, either add `<script src="…/object-sdk/sdk.js"></script>` while
developing (remove it before placing; a host-injected copy wins anyway), or
use the small `window.rupok || {…}` fallback shown in `examples/objects/`.

If a late `init` arrives after the fallback, the SDK adopts it: it applies the
values, notifies listeners and stops handling keys.

## Steps

Steps are slide builds inside an object.

- `current` is an integer in `[0, count]`. `0` is the initial state. `count`
  is fully revealed.
- `count = 0` means the object is static: Next goes straight to the next page.
- The presenter's **Next** advances the object's step until `count`, then
  moves to the next page. **Prev** goes back until `0`, then to the previous
  page. If a page has several stepped objects, the host SHOULD advance them
  in timeline order (item `created`, then id), each to its `count` before the
  next one.
- The host stores the step in the item (`spec/rupok-format.md`), so it survives
  reloads, syncs to other devices and appears in class replays.
- **Render the step as state, not as a sequence of events.** The object can
  receive any step at any time: a jump from 0 to 5, a step back, or a reload at
  step 3. `render(step)` must draw exactly what step `step` looks like. Do not
  rely on having seen the steps before it.

`steps.onChange(cb)` calls `cb(current, info)`:

- once with the initial step when `ready` resolves (or straight away, as a
  microtask, if you subscribe after `ready`), with `info.source = "init"`;
- then whenever `current` or `count` changes.

`info` is `{ previous, count, source }`, where `source` is `"init"`, `"host"`
(a `step` message) or `"local"` (a change made inside the object:
`setCount`, or stepping in standalone mode).

In host mode, `next()`, `prev()` and `goto()` only *ask*: they post `goto`, and
`current` changes when the host's `step` reply arrives. Several quick `next()`
calls count from the last requested step, so two taps move two steps.

## State

State is a small JSON value that lives in the lesson: a quiz's answers, a
toggled switch, a slider position.

- `state.save(value)` serialises `value` immediately (later changes to the
  object do not leak in) and sends it after **200 ms** without another save.
  Only the last value is sent.
- If the JSON text is over **256 KiB** (262,144 UTF-8 bytes) or cannot be
  serialised (cycles, `BigInt`), the save is dropped with a console warning.
- Every stored save is a document commit and part of the class recording.
  Save on meaningful changes, never per animation frame.
- `state.load()` returns `undefined` when nothing is saved.
- Store what the learner did, not what can be derived. The step is already
  stored by the host; don't duplicate it.

## Theme

The teacher's theme (and later, school brand kits) reaches every object as
tokens. The SDK mirrors each token to a CSS custom property on `:root`:
`--rupok-` + the token with `.` replaced by `-`.

| Token | CSS variable | SDK default |
|---|---|---|
| `scheme` | `--rupok-scheme`, plus `data-scheme` on `<html>` | `light` (`light` \| `dark`) |
| `color.bg` | `--rupok-color-bg` | `#ffffff` |
| `color.surface` | `--rupok-color-surface` | `#f4f5f4` |
| `color.line` | `--rupok-color-line` | `rgba(17, 20, 24, 0.12)` |
| `color.fg` | `--rupok-color-fg` | `#111418` |
| `color.accent` | `--rupok-color-accent` | `#0ca678` |
| `color.accent-text` | `--rupok-color-accent-text` | `#087f5b` |
| `color.on-accent` | `--rupok-color-on-accent` | `#04241b` |
| `color.accent-soft` | `--rupok-color-accent-soft` | `#e6fcf5` |
| `color.accent2` | `--rupok-color-accent2` | `#1c7ed6` |
| `color.muted` | `--rupok-color-muted` | `#5f6670` |
| `color.success` | `--rupok-color-success` | `#2b8a3e` |
| `color.danger` | `--rupok-color-danger` | `#e03131` |
| `font.body` | `--rupok-font-body` | `"Inter Variable", Inter, "Hind Siliguri", system-ui, …` |
| `font.heading` | `--rupok-font-heading` | same as `font.body` |
| `font.mono` | `--rupok-font-mono` | `ui-monospace, "Cascadia Mono", "SF Mono", Menlo, Consolas, "Liberation Mono", monospace` |
| `radius` | `--rupok-radius` | `12px` |

### Light and dark

Objects sit on the slide, so the host derives `scheme` from the **page
colour**, not from the app's chrome theme: a white or paper page sends
`light`, a chalkboard or blackboard page sends `dark`, and `color.bg` is the
page colour itself. Every object MUST render correctly in both schemes:

- Paint the document background with `var(--rupok-color-bg)` and text with
  `var(--rupok-color-fg)`; use `--rupok-color-surface` for cards and
  `--rupok-color-line` for rules and gridlines.
- Never hard-code white or black surfaces. For colours that need a different
  value per scheme, use `:root[data-scheme="dark"] { … }`.
- Canvas and WebGL code reads `rupok.theme.get(...)` and redraws in
  `rupok.theme.onChange` (the scheme changes when the teacher recolours the
  page).
- **Accent contrast.** The host picks the accent tokens for the page's
  scheme. `color.accent` is for fills and strokes (at least 3:1 on the page).
  Text in the accent colour uses `color.accent-text` (at least 4.5:1), never
  `color.accent`. Labels and icons on an accent fill (a button) use
  `color.on-accent`. A highlight is `color.accent-soft` behind
  `color.accent-text`.

- Defaults apply as soon as the SDK loads, before `init`.
- `init.theme` and `theme` messages carry the full theme; tokens they omit
  fall back to the defaults. Hosts MAY send extra tokens; the SDK mirrors any
  token made of letters, digits, `.`, `_` and `-`.
- In CSS, always give a fallback so the file also renders without the SDK:
  `color: var(--rupok-color-fg, #1d2433);`
- In canvas or WebGL code, read `rupok.theme.get("color.accent", "#2563eb")`
  and redraw in `theme.onChange`.

### Fonts

An object runs in an opaque-origin frame and cannot see the fonts the host
app loaded, and the teacher's machine may not have them installed. So the
host provides the theme fonts itself. The reference host injects
`@font-face` rules for these files into every object document, serving them
from the library base (`<libs>`, so the recommended `font-src` already
allows them):

| Family | Weights | Scripts |
|---|---|---|
| `Inter Variable` (also answers to `Inter`) | 100–900, variable | Latin |
| `Hind Siliguri` | 400, 500, 600, 700 | Bengali, Latin |

- Use the theme variables, `font-family: var(--rupok-font-body)`. Bangla
  text falls through Inter (Latin only) to Hind Siliguri by itself.
- Other weights snap to the nearest face. There is no italic face; the
  browser slants the upright one.
- A font loads when something first uses it. Canvas text drawn before that
  uses the fallback and stays wrong until the next redraw, so await
  `document.fonts.load('700 1em "Inter Variable"')` (and `"Hind Siliguri"`
  for Bangla) before the first canvas draw, or redraw on
  `document.fonts.onloadingdone`.
- Keep the fallbacks in the stack: other hosts may not provide these fonts.
- Never load fonts from the network. For another font, ship a subset woff2
  in the bundle and declare it with your own `@font-face`.

## Maths (LaTeX)

Maths is built in. Write LaTeX in the page and the SDK typesets it with
KaTeX, which the host ships with its libraries, so it works offline. Write
formulas as LaTeX, not as hand-styled HTML (`<sup>`, `<sub>`, Unicode
look-alikes).

- **Delimiters.** `$…$` and `\(…\)` inline; `$$…$$` and `\[…\]` display.
  `<span data-tex="x^2">x²</span>` typesets its attribute (add `data-display`
  for display style), and its content shows until KaTeX has loaded.
- **When.** The SDK renders once the document is ready, and again whenever
  text or a `data-tex` attribute changes later (a `MutationObserver`), so
  `el.innerHTML = "$v_x = 3$"` just works.
- **Not rendered:** `<script>`, `<style>`, `<textarea>`, `<pre>`, `<code>`,
  `<svg>`, `<canvas>`, editable elements and anything under `[data-no-math]`.
  `$5 and $10` stays text: an inline `$` formula must not start or end with a
  space, and its closing `$` must not be followed by a digit. `\$` is a
  literal dollar. A formula must sit in one text node (no tags inside).
- **Live values.** `rupok.tex(el, latex, { display })` typesets into `el` and
  resolves to `true` when done. Calling it again with the same formula does
  nothing, so call it on every change. `rupok.math.render(root)` typesets
  everything under `root`; `rupok.math.load()` loads KaTeX ahead of time.
- **Highlight.** `\hl{…}` marks the term that changes, in
  `color.accent-text` on `color.accent-soft`:
  `R = \dfrac{v^2 \hl{\sin 2\theta}}{g}`. It is the only HTML extension
  enabled (`\htmlClass{rupok-hl}` with a narrowly scoped `trust`); `\href`,
  `\url`, `\htmlStyle`, `\htmlData` and other classes stay disabled.
- **Colour.** Maths inherits the text colour, so it follows light and dark
  pages.
- **Cost.** KaTeX (script, stylesheet and the fonts it uses) loads lazily,
  only when the page contains maths or calls the API. Opt out of automatic
  rendering with `<meta name="rupok-math" content="off">`; the API keeps
  working.
- **Canvas** cannot hold KaTeX. Put formula labels in HTML positioned over
  the canvas.
- **Hosts** name the library base URL in `<meta name="rupok-lib">` (see
  [Host duties](#host-duties)) and serve `katex/katex.min.js`,
  `katex/katex.min.css` and `katex/fonts/` from it with CORS. Without the
  meta tag (and without a `window.katex` the page loaded itself) maths stays
  as source text and `rupok.tex()` resolves to `false`.

## Modes

| Mode | Context | Object guidance |
|---|---|---|
| `edit` | The teacher is building the lesson. | Fully interactive. May show extra hints. |
| `present` | Teaching live, usually on a projector or panel. | Big, clear, touch-first. |
| `replay` | Playing back a recorded class. | Steps and state are driven by the recording. Avoid side effects. |
| `student` | Running on a learner's own device (planned). | Interactive; quiz answers go to the teacher. |

## Validation

`ObjectBundle::validate()` returns diagnostics with a severity and a stable
code. **Errors** block placement. **Warnings** are shown to the teacher or
agent. Agents run it through `rupok validate <file.html>` or the
`validate_smart_object` MCP tool and fix every error.

| Code | Severity | Triggered when |
|---|---|---|
| `schema` | error | `schema` is not `1`. |
| `title` | error | `title` is empty or only whitespace. |
| `title` | warning | `title` is longer than 200 characters. |
| `size` | error | `size.w` or `size.h` is not a finite number in `[50, 8000]`. |
| `steps` | error | `steps` is greater than 500. |
| `steps` | warning | `steps > 0` but neither the entry HTML nor any `.js` file contains the text `rupok`. |
| `path` | error | A file path breaks the path rules. |
| `size_bytes` | error | The files total more than 25 MiB. |
| `permissions` | error | A `permissions.network` entry contains `/`, `:` or `*`, or has no `.`. |
| `entry` | error | The entry file is missing or not valid UTF-8. Validation stops here. |
| `entry` | error | `entry` does not end in `.html`. |
| `network` | error | A remote resource is loaded from a host not in `permissions.network` (see below). |
| `lib` | warning | A `rupoklib://<name>` reference uses a name not in the library list. |
| `sandbox` | warning | Source text contains `window.top`, `parent.document` or `top.location`. |
| `storage` | warning | Source text contains `localStorage` or `indexedDB`. |

The `network`, `lib`, `sandbox` and `storage` checks scan the entry HTML and
every `.js`, `.mjs` and `.css` file in the bundle. They are text heuristics:

- **Resource URLs** are `http://`, `https://` and protocol-relative `//`
  URLs in a loading position:
  - a quoted `src=` or `srcset=` attribute value;
  - a quoted `href=` inside a `<link>` tag;
  - CSS `url(…)` or `@import`;
  - a quoted string right after `import`, `from`, `import(`, `fetch`,
    `WebSocket` or `open("GET",` (XHR).
- Not reported: URLs in prose ("data from https://…"), plain `<a href>`
  links (navigation, allowed), and `localhost` (blocked at run time anyway).
  Protocol-relative URLs count only right after `src=`, `href=` or `url(`.
- A URL is allowed when its host equals a `permissions.network` entry or is a
  subdomain of one.
- Because the checks are textual, the validator misses URLs built at run time
  (`fetch(base + path)`). The run-time sandbox is the real boundary.

## Design guidance for AI authors

Objects are shown on classroom projectors and touch panels, viewed from the
back of the room, and written on with a pen.

**Size and layout**

- Slides are **1920 × 1080** world units. Most objects are between
  **960 × 540** and **1920 × 1080**; 16:9 fits best.
- The object **must work at any size**: the teacher resizes it, and screens
  differ. Size everything relative to the frame, never in fixed pixels. A
  simple pattern: make `rem` scale with the frame, then use `rem` everywhere.

  ```css
  /* Designed at 1280 × 720: 1rem = 16px at that size, scales with the frame. */
  html { font-size: min(calc(100vw * 16 / 1280), calc(100vh * 16 / 720)); }
  ```

  For canvas or WebGL, use a `ResizeObserver` and redraw at the new size and
  `devicePixelRatio`.
- Fill the frame: `html, body { margin: 0; height: 100%; overflow: hidden; }`.
  No scrollbars.

**Readability**

- Body text **≥ 24px** at the nominal size (≥ 1.5rem with the pattern above);
  headings 48px or more. Keep to about 40 words per step.
- High contrast: at least 4.5:1 for text (WCAG AA). Use the theme
  variables, which default to dark text on white.
- Never rely on colour alone: add labels, icons or patterns.

**Interaction**

- **Touch first.** Interactive boards and tablets have no hover. Nothing may
  depend on `:hover` or `mouseover`. Use `pointer` events and `click`.
- Tap targets at least 64px at nominal size, with space between them.
- Use `touch-action: manipulation` on controls to avoid double-tap zoom.
- Keyboard is a bonus, not a requirement. Don't capture keys the presenter
  uses (arrows, Space, PageUp/PageDown) unless an input has focus.

**Steps and state**

- Call `rupok.steps.setCount(n)` at startup and render from
  `rupok.steps.onChange`. Render each step from scratch (see [Steps](#steps)).
- Animate transitions with CSS transitions or the Web Animations API, and keep
  them short (≤ 600 ms). Respect `prefers-reduced-motion`.
- Save learner actions with `rupok.state.save()`, after `rupok.ready`.

**The pen draws on top**

- Teachers annotate over objects. Leave calm, empty space (margins, room
  around diagrams) for handwriting. Avoid busy full-bleed backgrounds and
  constant motion behind where the teacher writes.

**Offline and safe**

- No CDNs. Inline everything or use `rupoklib://`. No web fonts from the
  internet; use the [theme fonts](#fonts).
- No `localStorage`, cookies, `alert()` or popups.
- Validate, fix every error, and re-validate.

## Complete minimal example

A three-step "water cycle" object. Save it as `water-cycle.html`.

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>The water cycle</title>
<style>
  html { font-size: min(calc(100vw * 16 / 1280), calc(100vh * 16 / 720)); }
  html, body { margin: 0; height: 100%; overflow: hidden; }
  body {
    display: grid; place-content: center; gap: 2rem; text-align: center;
    background: var(--rupok-color-bg, #ffffff); color: var(--rupok-color-fg, #1d2433);
    font-family: var(--rupok-font-body, system-ui, sans-serif);
  }
  h1 { font-size: 3.5rem; margin: 0; }
  ol { display: flex; gap: 2rem; list-style: none; margin: 0; padding: 0; }
  li {
    width: 18rem; padding: 2rem 1rem; font-size: 1.75rem;
    border-radius: var(--rupok-radius, 12px);
    background: var(--rupok-color-accent, #2563eb); color: #fff;
    opacity: 0; transform: translateY(1rem);
    transition: opacity .4s, transform .4s;
  }
  li.on { opacity: 1; transform: none; }
</style>
</head>
<body>
  <h1>The water cycle</h1>
  <ol>
    <li>1. Evaporation</li>
    <li>2. Condensation</li>
    <li>3. Precipitation</li>
  </ol>
<script>
  const items = document.querySelectorAll("li");
  function render(step) {
    items.forEach((li, i) => li.classList.toggle("on", i < step));
  }
  rupok.steps.setCount(items.length);
  rupok.steps.onChange(render);
</script>
</body>
</html>
```

Validate and place it:

```sh
rupok validate water-cycle.html
```

Wrapped as a bundle, its manifest is:

```json
{
  "schema": 1,
  "title": "The water cycle",
  "entry": "index.html",
  "size": { "w": 1280, "h": 720 },
  "steps": 3,
  "permissions": {}
}
```

More complete examples (binary counter, computing timeline, networking quiz)
are in `examples/objects/`.

## Conformance notes

Known gaps between this spec and the reference implementation at the time of
writing:

- **`steps` warning.** The check looks for the text `rupok` anywhere, not
  for use of `rupok.steps`.
- **Unknown manifest fields** are ignored on read but dropped when a bundle
  is rewritten.
- **Bundle size** is enforced by `validate()`, not by `Package::read`.
