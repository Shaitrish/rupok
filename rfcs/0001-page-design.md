# RFC 0001: Page design (background images and branding)

| | |
|---|---|
| Status | Draft, not yet opened for comment |
| Created | 2026-11-24 |
| Author | mdraselislam1944 |
| Spec | [`spec/rupok-format.md` § Page design](../spec/rupok-format.md#page-design) (marked "proposed, pending RFC") |
| Reference | Rupok v0.2.2 (PR #17): `crates/rupok-core` (`model.rs`, `doc.rs`), `crates/rupok-format` (`readable.rs`, `tests/page_design.rs`), `apps/board/src` (`design.ts`, `components/PageBackdrop.tsx`, `components/PageThumb.tsx`, `export-pdf.ts`) |
| Roadmap | Phase 1, "Pages and page design" |

## Summary

Add three optional keys to `doc.loro`:

- the page key `background`: an image drawn under the ink, with a fit, an
  opacity and a "dim for readability" wash;
- `meta.branding`: a logo and a line of text (school, teacher, class, date)
  shown on every page;
- the page key `branding`: one page's own branding, which replaces the
  lesson's on that page or hides it.

All three hold a Loro `Map` value. The change is additive. No version gate
changes, and readers that do not know the keys show the page as they do
today.

The spec already carries this text as *proposed*, and Rupok has read and
written these keys since v0.2.2. This RFC asks to accept that text, with
five amendments (A–E below) that the review of the reference implementation
turned up.

## Motivation

A teacher at a school in Dhaka prepares 30 lessons a term. The school wants
its logo and name on every slide students see and every PDF sent home.
Without page design the teacher pastes the logo onto each page as an image
item, and then:

- the logo can be selected, moved and erased by accident in class;
- replay draws the logo as if the teacher had placed it during the lesson;
- a new page starts blank, so the teacher pastes the logo again;
- changing the class name means editing every page.

The same teacher wants a chalkboard texture or ruled exam paper behind the
writing, and a dimmed photo behind a title slide that stays readable under
ink.

Background and branding are part of the page, not part of the content. A
lesson must carry them, so that a shared `.rpk` looks the same on another
device and in the PDF. They are not items, so they must not take part in
selection, erasing or replay.

## Design

### Storage

The three keys hold a `Map` **value**, not a map container. One edit
replaces the whole map in one op, so one undo step restores the previous
design exactly. Concurrent edits to the same key resolve as Loro's
last-writer-wins on that key: the whole design wins or loses together.

Readers MUST also accept a `Map` container under these keys. Readers MUST
ignore unknown keys inside the maps, so later RFCs can add fields
additively.

### Page key `background`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `image` | `String` | none | Asset name `<blake3>.<ext>` (see Assets). Required: a map without a valid name (1–128 bytes, one path segment, no control characters) is no background. |
| `fit` | `String` | `"fit"` | `fit` (whole image, as large as fits, centred), `fill` (covers the page, centred, cropped), `stretch` (exactly the page), `tile` (repeated from the top-left at the image's size, one pixel = one world unit), `center` (the image's size, centred). Unknown values read as `fit`. |
| `opacity` | `f64` | `1` | 0–1, clamped. |
| `dim` | `bool` | `false` | "Dim for readability": the page colour at 50 % opacity over the image. |

### Branding: `meta.branding` and the page key `branding`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `logo` | `String` | absent | Logo asset name. An invalid name reads as no logo. |
| `logo_x`, `logo_y` | `f64` | `0.025`, `0.035` | The logo's top-left corner as fractions (0–1, clamped) of the page frame's width and height. |
| `logo_w` | `f64` | `0.1` | The logo's width as a fraction of the frame width, clamped to 0.02–0.6. Its height follows the image's aspect ratio. |
| `logo_opacity` | `f64` | `1` | 0–1, clamped. |
| `school`, `teacher`, `class` | `String` | `""` | Text, at most 120 characters each (see amendment B). |
| `date` | `bool` | `false` | Show the date of the day the page is shown, printed or exported. |
| `text_pos` | `String` | `"bottom"` | `top` (header) or `bottom` (footer). Unknown values read as `bottom`. |
| `text_align` | `String` | `"center"` | `left`, `center` or `right`. Unknown values read as `center`. |
| `off` | `bool` | `false` | Page key only: no branding on this page. |

Non-finite numbers read as the default.

**Which branding a page shows.** A page shows its own `branding` when it is
present, and none when that has `off: true`. Otherwise the page shows
`meta.branding`.

**Placeholders.** `school`, `teacher` and `class` may contain `{name}`
tokens. Renderers fill them in where the line is drawn, so one design fits
every lesson and page. Names are lowercase ASCII letters.

| Token | Becomes |
|---|---|
| `{lesson}` | `meta.title` |
| `{page}`, `{pages}` | the page's 1-based number and the lesson's page count |
| `{date}` | the date of showing, printing or exporting |
| `{teacher}` | the teacher's name as the app knows it (empty if unknown) |
| `{school}` | the `school` value (empty inside `school` itself) |
| `{subject}`, `{course}`, `{chapter}` | the lesson's course: `manifest.json` `course`, or where the app keeps the lesson (empty if unknown) |

Unknown tokens stay as written. A part that is empty after filling in is
left out of the line, and runs of spaces collapse to one. Readable entries
show the text as stored, tokens included.

### Drawing

- **Layers**, bottom to top: page colour; background image (clipped to the
  frame) and its dim wash; the `bg_pattern`; DOM items (images, text, Smart
  Objects); the branding (logo, then text); ink. Branding sits above the
  items so that a logo stays visible on imported PDF slides.
- **Text line.** The set parts of `school`, `teacher`, `class` and the date,
  joined by ` · `. It sits in a box inset 2.5 % of the frame width from the
  sides and 1.8 % of the frame height from the top or bottom edge. The type
  is `0.026 × frame height` high (28 units on a 1080-high slide), in a box
  1.6 times that. It is dark on light page colours and light on dark ones.
- A page without a frame (an infinite board) uses a 1920 × 1080 frame at the
  origin for these layouts.
- Background and branding are not items. They are not selected, moved,
  erased or replayed. Only a page design editor changes them.

### Assets

Every asset named by `background.image` or a branding `logo` is referenced.
Writers MUST keep it in `assets/` like an image item's asset, and store it
once however many pages use it. This changes the existing Assets rule from
"an asset that no item references is garbage" to "an asset that nothing
references (no item, page `background` or branding `logo`) is garbage".

### New pages

A new page added after another SHOULD copy its `bg_pattern`, `bg_color`,
frame, `background` and `branding`. The reference does this.

### Amendments to the current spec text

The review of the reference implementation found these gaps. The reference
already behaves as written below. The implementation PR adds the text to
`spec/rupok-format.md`.

**A. A missing asset draws nothing.** A `background` whose image is not in
the package draws nothing: the page shows its colour and pattern. A logo
that is not in the package is left out, and the text line still draws.
Readers MUST NOT fail the page or the lesson. (This matters for files
re-saved by older writers; see Compatibility.)

**B. What a "character" is.** The 120-character cap counts Unicode scalar
values. Readers trim leading and trailing whitespace first, then keep the
first 120 scalar values. A writer in a language whose strings count UTF-16
code units (JavaScript) must not cut a surrogate pair. Bangla text is
counted the same way as Latin text.

**C. `off` in `meta.branding`.** Writers MUST NOT write `off` in
`meta.branding`. A reader that finds `off: true` there treats the lesson as
having no branding.

**D. A page's branding replaces the lesson's; it does not merge.** A page
`branding` with only `school` set shows that school and no logo, even when
`meta.branding` has a logo. Fields are not merged one by one.

**E. Branding text is plain text.** Renderers MUST NOT interpret `school`,
`teacher` or `class` as HTML or any other markup. The reference draws them
as DOM text, canvas text and PDF text.

## Compatibility

**Version gates.** None change. Package `version` stays 1 and
`meta.format` stays 1. Under rule 1 of Versioning, new optional keys are
additive.

**Old readers, new files.** Under rule 2, readers ignore unknown keys in
`meta` and pages. A lesson with page design opens in Rupok v0.1.0–v0.2.1
and in any third-party reader with its page colours and patterns only: no
background image, logo or text line. Ink and items are unchanged.

**Old writers rewriting new files.** Under rule 3, Loro keeps the unknown
keys when an old writer edits the document in place. But the old Assets
text allowed writers to drop any asset that *no item* references, and
Rupok v0.1.0–v0.2.1 do: the board's save keeps only assets used by image
and object items (`crates/rupok-wasm/src/engine.rs`,
`assets.retain(|k, _| used.contains(k))`). So a designed lesson opened and
saved in v0.2.1 keeps its `background` and `branding` keys but loses the
image bytes they point to. Amendment A makes this safe for new readers: the
page shows its colour and pattern and the text line, and nothing fails. The
images are lost until the teacher adds them again. See unresolved question 1.

**New readers, old files.** Nothing to migrate. A lesson without the keys
has no background image and no branding.
`a_lesson_from_before_page_design_opens_unchanged_and_takes_one` in
`crates/rupok-format/tests/page_design.rs` opens a pre-v0.2.2 sample, checks
that it reads the same, and adds branding to it.

**Readable entries.** `content.md` and `content.json` describe the branding
and backgrounds. Readable entries are themselves proposed and pending their
own RFC. If that RFC changes, the lines this RFC adds there follow it.

## Test vectors

Asset names are shortened here as `<logo>` (a PNG) and `<chalk>` (an SVG).
In a file they are 64 hex characters plus the extension.

### 1. Document (JSON view, `get_deep_value`)

```json
{
  "meta": {
    "format": 1, "title": "Fractions", "created": 1763956800000.0,
    "branding": {
      "logo": "<logo>.png", "logo_x": 0.025, "logo_y": 0.035, "logo_w": 0.1,
      "logo_opacity": 1.0,
      "school": "Dhaka Model School", "teacher": "{teacher}",
      "class": "Class 8 · page {page}/{pages}", "date": true,
      "text_pos": "bottom", "text_align": "center"
    }
  },
  "pages": [
    {
      "id": "p1", "title": "", "frame_w": 1920.0, "frame_h": 1080.0,
      "bg_pattern": "grid", "bg_color": 523972863,
      "background": { "image": "<chalk>.svg", "fit": "tile", "opacity": 0.5, "dim": true },
      "items": {}
    },
    {
      "id": "p2", "title": "Quiz", "frame_w": 1920.0, "frame_h": 1080.0,
      "bg_pattern": "plain", "bg_color": 4294967295,
      "branding": { "off": true },
      "items": {}
    },
    {
      "id": "p3", "title": "Science fair", "frame_w": 1920.0, "frame_h": 1080.0,
      "bg_pattern": "plain", "bg_color": 4294967295,
      "branding": { "school": "Science fair 2026", "text_pos": "top" },
      "items": {}
    }
  ]
}
```

`523972863` is `0x1f3b30ff` (a dark green board). The package holds
exactly two assets, `assets/<logo>.png` and `assets/<chalk>.svg`.

- `p1` shows the tiled chalk texture at 50 % opacity, the dim wash, the
  grid, the lesson's logo and the footer line.
- `p2` shows no logo and no text.
- `p3` shows the header "Science fair 2026" and **no logo** (amendment D).

### 2. Reader normalisation

| Stored map | What a reader uses |
|---|---|
| `background: {}` | no background |
| `background: {"image": "../x.png"}` | no background (not one path segment) |
| `background: {"image": "<chalk>.svg", "fit": "cover"}` | `fit` |
| `background: {"image": "<chalk>.svg", "opacity": 7}` | opacity 1 |
| `background: {"image": "<chalk>.svg", "opacity": -1}` | opacity 0 |
| `background: {"image": "<absent>.png"}` with no such asset | draws nothing (amendment A) |
| `branding: {"logo_w": 0.9}` | `logo_w` 0.6 |
| `branding: {"logo_w": 0}` | `logo_w` 0.02 |
| `branding: {"logo": "a/b.png"}` | no logo |
| `branding: {"text_pos": "middle", "text_align": "justify"}` | `bottom`, `center` |
| `branding: {"school": "  Dhaka Model School  "}` | `"Dhaka Model School"` |
| `branding: {"school": <200 × "ক">}` | the first 120 `ক` (amendment B) |
| `meta.branding: {"off": true}` | no lesson branding (amendment C) |
| a page `branding` stored as a map container | read like the map value |

### 3. Placeholders

The app knows the lesson "Fractions", the teacher "Ms. Rahman", the subject
"Maths", the chapter "Fractions" and no course. The page is page 3 of 12,
shown on 24 November 2026.

| Field | Stored | Becomes |
|---|---|---|
| `class` | `Class 8 · page {page}/{pages}` | `Class 8 · page 3/12` |
| `teacher` | `{teacher}` | `Ms. Rahman` |
| `teacher` | `{teacher}`, teacher unknown | empty: left out of the line |
| `class` | `{course}` | empty: left out of the line |
| `teacher` | `{school}` | the `school` value, filled in |
| `school` | `{school} Annex` | `Annex` |
| `class` | `{Lesson}` | `{Lesson}` (not a token) |
| `class` | `{room}` | `{room}` (unknown token) |
| `class` | `Class   8` | `Class 8` |

With `school` "Dhaka Model School", `teacher` "{teacher}", `class`
"Class 8" and `date` true, the line is
`Dhaka Model School · Ms. Rahman · Class 8 · <date>`.

### 4. Layout on a 1920 × 1080 frame, default branding

| What | Value (world units) |
|---|---|
| Logo box | left 48, top 37.8, width 192; height 96 for a 2:1 image |
| Type size | 0.026 × 1080 = 28.08 (the reference rounds to 28) |
| Text box height | 1.6 × 28 = 44.8 |
| Text box sides | 48 to 1872 (width 1824) |
| Footer box top | 1080 − 19.44 − 44.8 = 1015.76 |
| Header box top | 19.44 |

An infinite board uses the same numbers on a 1920 × 1080 frame at the
origin.

### Executable checks

`crates/rupok-format/tests/page_design.rs` round-trips a lesson like
vector 1 (a background on every page, lesson branding, one page with
`off`). It checks
that one logo and one texture are stored once for four pages, and checks
the readable outline. The implementation PR adds unit tests for vector 2
(amendments B–D) and a board test for amendment A.

## Security

- Background and logo images are drawn as images: `<img>` and CSS
  `background-image` on the board, image XObjects in the PDF. An SVG drawn
  this way runs no script and loads nothing external. Readers MUST still
  sniff the content and refuse unsupported types (Assets).
- Agents can set page design (MCP tools `set_page_design` and
  `set_branding`, and the matching live ops). `rupok-live`
  (`design.rs`, `sanitize_svg`) rejects agent SVG that contains scripts,
  event handlers, external references or animation. That check is app
  behaviour and not part of the format.
- Branding text is plain text (amendment E).
- The 120-character cap and the 128-byte asset name cap bound what a reader
  allocates per page.

## Alternatives

1. **A locked image item** (the `locked` key that
   [pdf-import-native](../docs/roadmap/pdf-import-native.md) proposes).
   Old readers would show the logo and keep its asset, which is the
   strongest argument for it. But the logo would be content: replayed, in
   the z-order, copied with "select all", and stored as one item per page.
   Lesson-wide branding and placeholders would need a new mechanism anyway.
2. **Map containers** instead of map values. These give per-field merging
   of concurrent edits. But an undo step would then restore fields one by
   one, and "replace this design with that one" would no longer be one
   operation. Readers accept containers, so a later RFC can switch if live
   co-editing needs it.
3. **Burn the branding into the background image.** There would be no new
   keys for text. But nothing would be editable, there would be no
   placeholders or per-page numbers, and the readable outline and search
   would have no text.
4. **Merge a page's branding over the lesson's, field by field.** This
   needs a way to tell "unset" from "empty" for every field. Replace plus
   `off` covers the cases teachers asked for.
5. **Bump `meta.format` to 2** to stop old writers dropping design assets.
   Every old reader would then refuse every new lesson, including lessons
   with no design. That cost is far larger than the loss it prevents.

## Unresolved questions

1. **Asset loss in Rupok ≤ v0.2.1.** Is losing the images when an old app
   re-saves (Compatibility) acceptable, given amendment A and the in-app
   update notices in v0.2.3? Or should something prevent it?
2. **Date language and format.** The reference always formats the date as
   `en-GB` ("24 November 2026"). Should `date` and `{date}` follow
   `meta.lang` (the [i18n](../docs/roadmap/i18n.md) plan), the device
   locale, or a new branding key? Bangla-medium schools are the first case.
3. **Separator spacing.** The spec says ` · `. The reference board joins
   with two spaces on each side (`apps/board/src/design.ts`, `brandLine`).
   The DOM collapses the extra spaces, but the thumbnails and the PDF draw
   them. Should the spec or the code change?
4. **Text colour.** "Dark on light page colours and light on dark ones" is
   normative but not exact. The reference uses luma (0.299 R + 0.587 G +
   0.114 B) below 128 as dark, with `#1c2024` at 72 % on light pages and
   white at 78 % on dark ones. Should the threshold be normative, or stay a
   renderer choice?
5. **PDF pages as backgrounds.** Should imported PDF pages become a page
   `background` instead of locked image items? The layer order (branding
   above items) assumes they stay items. This needs reconciling with
   pdf-import-native.
6. **Brand kit.** The roadmap's brand kit idea (school colours and fonts as
   theme tokens) could add keys inside `meta.branding` later. Unknown keys
   are ignored, so that would stay additive. Is `meta.branding` the right
   home for them?
