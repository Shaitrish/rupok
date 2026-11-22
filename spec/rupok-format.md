# The `.rpk` lesson package format

| | |
|---|---|
| **Version** | 1 (package `version` 1, document `meta.format` 1, point codec 1) |
| **Status** | Draft for Phase 0. Changes go through RFCs (see [Change process](#change-process)). |
| **Media type** | `application/vnd.rupok.lesson+zip` |
| **File extension** | `.rpk` |
| **License** | This specification is Apache-2.0 (see `spec/LICENSE`). Anyone may build compatible readers and writers. |
| **Reference implementation** | `crates/rupok-format` (package), `crates/rupok-core` (document), `crates/rupok-ink` (point codec) |

A `.rpk` file is one lesson: its pages, every ink stroke with timing, text,
images and live Smart Objects. The document inside is a
[Loro](https://loro.dev) CRDT snapshot that keeps its full edit history, so
one file is the lesson, its undo history and its class recording.

The key words MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, RECOMMENDED, MAY
and OPTIONAL are to be read as described in RFC 2119 and RFC 8174 when, and
only when, they appear in all capitals.

## Contents

- [Conventions](#conventions)
- [Package layout](#package-layout)
- [manifest.json](#manifestjson)
- [doc.loro](#docloro)
- [Stroke points](#stroke-points)
- [Stroke width and rendering](#stroke-width-and-rendering)
- [Assets](#assets)
- [Smart Object bundles](#smart-object-bundles)
- [Thumbnails and layers](#thumbnails-and-layers)
- [Reader limits and safety](#reader-limits-and-safety)
- [Versioning and forward compatibility](#versioning-and-forward-compatibility)
- [Change process](#change-process)
- [Conformance notes](#conformance-notes)

## Conventions

| Term | Meaning |
|---|---|
| **World unit** | The page coordinate unit. 1 world unit = 1 CSS pixel at 100 % zoom. Origin is the top-left corner of the page frame; x grows right, y grows **down**. |
| **Colour** | A `u32` in `0xRRGGBBAA` order (alpha in the low byte), stored as a Loro `i64` because Loro has no unsigned type. Example: `0x1d2433ff` is stored as `488911871`. |
| **Wall-clock time** | Milliseconds since the Unix epoch (UTC), stored as a Loro `f64` (`Double`). |
| **`i64` / `f64` / `String` / `Binary`** | Loro value types. `Map`, `MovableList` are Loro containers. |
| **Number reading rule** | Where this spec says a key is `f64`, readers MUST also accept `i64`. Where it says `i64`, readers MUST also accept a non-negative, in-range `f64` (truncated). |

## Package layout

A package is a ZIP archive (PKWARE APPNOTE). Entries use the *Stored* or
*Deflate* method. ZIP64 MAY be used.

```text
mimetype                    REQUIRED  first entry, Stored, exact media type bytes
manifest.json               REQUIRED  package metadata (JSON)
doc.loro                    REQUIRED  Loro snapshot with full history
assets/<blake3>.<ext>       optional  content-addressed media
objects/<bundle>/...        optional  Smart Object bundles, one directory each
thumbs/<n>.png              optional  page thumbnails (cache)
layers/<peer>.loro          optional  student annotation layers (reserved)
```

### `mimetype`

- Writers MUST write `mimetype` as the **first** entry, with the *Stored*
  method, no extra field, and content exactly
  `application/vnd.rupok.lesson+zip` (34 bytes, ASCII, no trailing newline).
  The media type then sits at byte offset 38 of the file, so tools can sniff
  it without a ZIP library, as with EPUB and ODF.
- Readers MUST reject a package with no `mimetype` entry or with any other
  content.

### Compression

Writers SHOULD deflate `manifest.json`, `doc.loro`, `layers/*` and
uncompressed media. Writers SHOULD store (not deflate) entries that are
already compressed. The reference writer stores files whose extension is one
of `png jpg jpeg webp avif gif mp3 mp4 m4a opus ogg webm woff woff2 zip pdf`,
and stores all `thumbs/*`. Readers MUST accept either method for any entry
except `mimetype`.

### Entry names

Entry names are UTF-8, use `/` as the separator and MUST satisfy the
[path rules](#path-rules). Names are case-sensitive. Directory entries (names
ending in `/`) carry no meaning; writers SHOULD NOT emit them and readers MUST
skip them. Writers MUST NOT emit two entries with the same name.

## manifest.json

A UTF-8 JSON object. It is a *summary* for file browsers, sync and search
tools that do not want to decode `doc.loro`. Where the two disagree,
`doc.loro` is authoritative.

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `format` | string | yes | MUST be `"rupok-lesson"`. |
| `version` | integer (u32) | yes | Package format version. This spec defines `1`. |
| `title` | string | yes | Lesson title, copied from `meta.title`. |
| `created` | number | yes | Wall-clock creation time, copied from `meta.created`. |
| `modified` | number | yes | Wall-clock time this package was written. |
| `generator` | string | yes | Name and version of the writing software, e.g. `"rupok 0.1.0"`. |
| `pages` | integer | yes | Page count at write time. |
| `course` | object | no | Where the lesson sits in a course. Omitted when unset. |

`course` is an object whose keys are all optional strings. Writers omit empty
strings.

| Key | Meaning |
|---|---|
| `subject` | e.g. `"ICT"` |
| `course` | e.g. `"Grade 9 Computing"` |
| `chapter` | e.g. `"Number systems"` |

Example:

```json
{
  "format": "rupok-lesson",
  "version": 1,
  "title": "Binary numbers",
  "created": 1763870400000.0,
  "modified": 1763874000000.0,
  "generator": "rupok 0.1.0",
  "pages": 2,
  "course": { "subject": "ICT", "chapter": "Number systems" }
}
```

Readers MUST reject a manifest that is not valid JSON or lacks a required
key. Readers MUST ignore unknown keys. Writers SHOULD preserve unknown keys
they read.

## doc.loro

`doc.loro` is a Loro 1.x document exported in **snapshot** mode, including
the full operation history. It MUST NOT be empty.

- Writers MUST NOT export shallow or trimmed snapshots: the history is the
  class recording and the undo stack across sessions.
- Writers SHOULD enable Loro change timestamps (`set_record_timestamp(true)`).
- Each user-visible action (one stroke, one move, one page added) SHOULD be
  exactly one Loro commit, and writers SHOULD NOT merge consecutive changes
  (the reference sets the change merge interval to 0). This keeps undo,
  replay and sync deltas at action granularity.

### Root containers

```text
meta   : Map          { format, title, created }
pages  : MovableList  [ Map page, ... ]      presentation order
```

Other root containers are reserved. Readers MUST ignore unknown root
containers and unknown keys at every level.

### `meta`

| Key | Type | Req. | Meaning |
|---|---|---|---|
| `format` | `i64` | yes | Document schema version. This spec defines `1`. |
| `title` | `String` | yes | Lesson title. |
| `created` | `f64` | yes | Wall-clock creation time. |

Readers MUST reject a document whose `meta.format` is missing or greater than
the highest version they support.

### Pages

Each element of `pages` is a `Map`:

| Key | Type | Req. | Default when missing | Meaning |
|---|---|---|---|---|
| `id` | `String` | yes | `""` | Page id, unique within the document, opaque. |
| `title` | `String` | yes | `""` | Page title (may be empty). |
| `frame_w` | `f64` | no | absent | Slide frame width in world units. |
| `frame_h` | `f64` | no | absent | Slide frame height in world units. |
| `bg_pattern` | `String` | yes | `"plain"` | One of `plain`, `grid`, `dots`, `lines`. Readers MUST treat unknown values as `plain`. |
| `bg_color` | `i64` (colour) | yes | `0xffffffff` | Page background colour. |
| `items` | `Map<String, Map>` | yes | none | Items keyed by item id. |

- `frame_w` and `frame_h` are present together or absent together. When both
  are present the page is a **slide**: the frame spans `[0, frame_w] ×
  [0, frame_h]`, and presenting fits the frame to the screen. When absent, the
  page is an **infinite board**. A reader that finds only one of the two MUST
  treat the page as an infinite board.
- New pages default to a 1920 × 1080 slide, `plain`, white (`0xffffffff`).
- Page order is the order of the movable list. Moving a page is a list move,
  so concurrent edits keep the page identity.

### Items

Every item map has these keys:

| Key | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `kind` | `String` | yes | none | `stroke`, `text`, `image` or `object`. |
| `z` | `f64` | yes | `0` | Stacking order. Higher is drawn on top. |
| `created` | `f64` | yes | `0` | Wall-clock creation time. Drives replay order. |

**Stacking.** Items are drawn in ascending `z`. Ties are broken by item id,
compared as byte strings, so every replica agrees on the order. A new item
gets `z = (max z on the page) + 1`; "bring to front" assigns a new top `z`.

**Box geometry.** `text`, `image` and `object` items share these keys:

| Key | Type | Default | Meaning |
|---|---|---|---|
| `x`, `y` | `f64` | `0` | Top-left corner of the unrotated box, world units. |
| `w`, `h` | `f64` | `0` | Box size, world units. |
| `rot` | `f64` | `0` | Rotation in radians about the box centre. Positive is clockwise on screen (y points down). |

#### `kind: "stroke"`

| Key | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `tool` | `String` | yes | `"pen"` | `pen` or `highlighter`. Readers MUST render unknown tools as `pen`. |
| `color` | `i64` (colour) | yes | `0x000000ff` | Ink colour, including alpha. |
| `width` | `f64` | yes | `2.0` | Nominal full width in world units (see [width](#stroke-width-and-rendering)). |
| `opacity` | `f64` | yes | `1.0` | Extra opacity in `[0, 1]`, multiplied into the colour's alpha. |
| `pts` | `Binary` | yes | empty stroke | Points in the [stroke point codec](#stroke-points). |
| `ox`, `oy` | `f64` | no | `0` | Translation applied at render time. |
| `shape` | `String` | no | absent | Set when a shape tool made the stroke: `rect`, `ellipse`, `line` or `arrow`. |

- Moving a stroke changes `ox`/`oy` and never rewrites `pts`. This keeps moves
  cheap and the recorded timing intact. Writers MAY omit `ox`/`oy` when both
  are zero.
- `shape` is a hint for editing tools. The geometry is always `pts`;
  renderers MUST draw `pts` and MAY ignore `shape`.
- A highlighter stroke is written with pressure `1.0` at every point, so it
  has constant width.

#### `kind: "text"`

Box geometry, plus:

| Key | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `text` | `String` | yes | `""` | Plain text. `\n` is a line break. |
| `size` | `f64` | yes | `32.0` | Font size in world units. |
| `color` | `i64` (colour) | yes | `0x000000ff` | Text colour. |

#### `kind: "image"`

Box geometry, plus:

| Key | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `asset` | `String` | yes | `""` | Asset name `<blake3>.<ext>`; the bytes are at `assets/<asset>`. |

#### `kind: "object"` (Smart Object)

Box geometry, plus:

| Key | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `bundle` | `String` | yes | `""` | Bundle id; the bundle is at `objects/<bundle>/`. |
| `title` | `String` | yes | `""` | Display title (layers panel, accessibility). |
| `step` | `i64` (u32) | yes | `0` | Current build step, `0 ≤ step ≤ steps`. See `spec/smart-object.md`. |
| `state` | `String` | no | absent | JSON text the object saved with `rupok.state.save()`. At most 256 KiB. |

Several items MAY share one bundle. Each item has its own `step` and `state`.

### Ids

Page and item ids are opaque strings. Readers MUST NOT parse them. The
reference writer generates `<base36(peer id)>-<base36(counter)>`, for example
`396ebcagx1dr1-5`. Loro gives each session a random 64-bit peer id, so ids from
different devices do not collide. Writers MUST keep page ids unique within a
document and item ids unique within a page.

### Replay

- The **item timeline** of a page is its items sorted by `created`, ties broken
  by id. Point *i* of a stroke happened at `created + dt[i]` (see
  [Stroke points](#stroke-points)), so a player can redraw a class stroke by
  stroke and point by point.
- Writers SHOULD set a stroke's `created` to the wall-clock time of its first
  point (pen down).
- Erased items are gone from the current state but remain in the Loro history.
  A player that must show ink that was later erased uses the history, for
  example by checking out earlier versions. Loro change timestamps give the
  wall-clock time of each change.

### Example

The JSON view (`get_deep_value`) of a small document. `pts` is shown as a byte
array; in Loro it is a `Binary` value.

```json
{
  "meta": { "format": 1, "title": "Binary numbers", "created": 1763870400000.0 },
  "pages": [
    {
      "id": "396ebcagx1dr1-1", "title": "",
      "frame_w": 1920.0, "frame_h": 1080.0,
      "bg_pattern": "plain", "bg_color": 4294967295,
      "items": {
        "396ebcagx1dr1-2": {
          "kind": "stroke", "z": 1.0, "created": 1763870401000.0,
          "tool": "pen", "color": 488911871, "width": 3.0, "opacity": 1.0,
          "pts": [1, 3, 192, 2, 128, 5, 128, 0, 16, 23, 153, 8, 48, 7, 255, 8]
        },
        "396ebcagx1dr1-3": {
          "kind": "text", "z": 2.0, "created": 1763870402000.0,
          "x": 96.0, "y": 64.0, "w": 800.0, "h": 80.0, "rot": 0.0,
          "text": "Place values", "size": 48.0, "color": 488911871
        },
        "396ebcagx1dr1-4": {
          "kind": "image", "z": 3.0, "created": 1763870403000.0,
          "x": 0.0, "y": 0.0, "w": 320.0, "h": 240.0, "rot": 0.0,
          "asset": "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262.png"
        },
        "396ebcagx1dr1-5": {
          "kind": "object", "z": 4.0, "created": 1763870404000.0,
          "x": 320.0, "y": 270.0, "w": 1280.0, "h": 720.0, "rot": 0.0,
          "bundle": "binary-counter", "title": "Binary counter",
          "step": 0, "state": "{\"value\":5}"
        }
      }
    },
    {
      "id": "396ebcagx1dr1-6", "title": "Board",
      "bg_pattern": "grid", "bg_color": 4261214207,
      "items": {}
    }
  ]
}
```

The second page has no `frame_w`/`frame_h`, so it is an infinite board.

## Stroke points

The `pts` value of a stroke is a compact, versioned byte string. Typical cost
is 4 to 6 bytes per point.

### Layout (codec version 1)

```text
u8             version      = 1
varint         n            number of points, n ≤ 1,048,576 (2^20)
repeat n times:
  zigzag-varint  dx         quantised x, delta from the previous point
  zigzag-varint  dy         quantised y, delta from the previous point
  u8             p          pressure × 255
  varint         ddt        milliseconds since the previous point
```

| Primitive | Definition |
|---|---|
| `varint` | Unsigned LEB128: 7 data bits per byte, least significant group first, high bit set on every byte except the last. At most 10 bytes for a `u64`. |
| `zigzag-varint` | A signed `i64` `v` mapped to `u64` as `(v << 1) ^ (v >> 63)` (arithmetic shift), then written as a `varint`. Decoding: `(u >> 1) ^ -(u & 1)`. So 0→0, −1→1, 1→2, −2→3, … |

### Encoding

For each point, in stroke order, with `qx_prev = qy_prev = 0` and
`dt_prev = 0` before the first point:

1. **Position.** `qx = round(x × 16)`, `qy = round(y × 16)` (quantum
   1/16 world unit; `round` is half away from zero). Write
   `dx = qx − qx_prev` and `dy = qy − qy_prev`. The first point's delta is
   therefore its absolute position.
2. **Pressure.** `p = round(clamp(pressure, 0, 1) × 255)` as one byte.
3. **Time.** `dt` is milliseconds since the stroke started (`u32`). Writers
   MUST make it non-decreasing: `dt' = max(dt, dt_prev)`. Write
   `ddt = dt' − dt_prev`.

Writers MUST write `version = 1` and MUST NOT write bytes after the last
point.

### Decoding

1. Read `version`. An empty buffer or a version other than `1` is an error.
2. Read `n`. Readers MUST reject `n > 1,048,576` **before** allocating.
3. For each point: `x = Σdx / 16`, `y = Σdy / 16`, `pressure = p / 255`,
   `dt = Σddt`, clamped to `u32::MAX`.
4. Running out of bytes inside the header or a point is an error.
5. Bytes after the *n*-th point are reserved for future revisions. Version 1
   readers MUST ignore them.

A stroke whose `pts` fails to decode is corrupt.

### Worked example

Three points:

| # | x | y | pressure | dt (ms) |
|---|---|---|---|---|
| 0 | 10.0 | 20.0 | 0.5 | 0 |
| 1 | 10.5 | 19.25 | 0.6 | 8 |
| 2 | 12.0 | 19.0 | 1.0 | 16 |

| Field | Value | Zigzag | Bytes |
|---|---|---|---|
| version | 1 | | `01` |
| n | 3 | | `03` |
| P0 dx | qx = 160, dx = 160 | 320 | `C0 02` |
| P0 dy | qy = 320, dy = 320 | 640 | `80 05` |
| P0 p | round(127.5) = 128 | | `80` |
| P0 ddt | 0 | | `00` |
| P1 dx | qx = 168, dx = 8 | 16 | `10` |
| P1 dy | qy = 308, dy = −12 | 23 | `17` |
| P1 p | round(153.0) = 153 | | `99` |
| P1 ddt | 8 | | `08` |
| P2 dx | qx = 192, dx = 24 | 48 | `30` |
| P2 dy | qy = 304, dy = −4 | 7 | `07` |
| P2 p | 255 | | `FF` |
| P2 ddt | 8 | | `08` |

Result, 16 bytes:

```text
01 03 C0 02 80 05 80 00 10 17 99 08 30 07 FF 08
```

Decoding gives back the positions and times exactly; P0's pressure becomes
128 / 255 ≈ 0.50196. An empty stroke encodes as `01 00`.

## Stroke width and rendering

Stored strokes MUST look the same in every conforming renderer, so the width
mapping is part of the format (users tune the *input* pressure curve, which is
not stored).

```text
MIN_WIDTH_RATIO = 0.35
radius(p, width) = 0.5 × width × (0.35 + 0.65 × clamp(p, 0, 1))
```

At `p = 0` the line is 35 % of `width`; at `p = 1` it is exactly `width`.
Example: `width = 4` gives radius 0.7 at `p = 0` and radius 2.0 at `p = 1`.

Renderers MUST draw a stroke as:

1. **Geometry.** Point *i* is the disc centred at `(x[i] + ox, y[i] + oy)`
   with radius `radius(p[i], width)`. Consecutive points are joined by the
   convex hull of their two discs (a tapered capsule). A one-point stroke is a
   single disc. The stroke is the union of these shapes.
2. **Colour.** Fill with the stroke colour, alpha = `A × clamp(opacity, 0, 1)`,
   where `A` is the colour's alpha byte.
3. **Coverage.** The union is composited once. A translucent stroke MUST NOT
   get darker where it overlaps itself.

Renderers MAY enforce a minimum on-screen radius (the reference uses about
0.6 device pixels) so zoomed-out ink stays visible.

## Assets

Media referenced by items lives under `assets/`, named by content:

```text
assets/<hash>.<ext>
  hash = lowercase hex of the 32-byte BLAKE3 hash of the file bytes (64 chars)
  ext  = 1–8 lowercase ASCII letters or digits; "bin" when unknown
```

- Writers MUST name assets this way. Identical bytes get the same name, so an
  image used on ten pages is stored once.
- The reference writer derives `ext` from the given extension by keeping
  ASCII letters and digits, taking the first 8 and lowercasing (`"PNG"` →
  `png`).
- Image items reference assets by name without the `assets/` prefix.
- Readers SHOULD NOT trust that the name matches the content. A reader MAY
  verify the hash and MUST then treat a mismatch as a missing asset.
- An asset that no item references is garbage. Writers MAY drop it.
- Readers MUST NOT pick a decoder from the extension alone. They MUST sniff
  the content and refuse types they do not support.

## Smart Object bundles

Each directory `objects/<bundle>/` holds one Smart Object bundle. The format
of a bundle (its `manifest.json`, entry HTML, sandbox, bridge and validation)
is defined in [`spec/smart-object.md`](smart-object.md).

- `<bundle>` is one path segment. Writers SHOULD use `[a-z0-9-]{1,64}`.
- Every bundle directory MUST contain `manifest.json`. A reader that cannot
  parse a bundle's manifest MUST treat that bundle as missing.
- Entries directly under `objects/` (no bundle directory) are not part of any
  bundle. Readers MUST ignore them.
- Object items reference bundles by `<bundle>`.
- Bundles are untrusted code. Hosts MUST validate a bundle and run it only in
  the sandbox described in `spec/smart-object.md`. Reading a package MUST NOT
  execute anything.

## Thumbnails and layers

- `thumbs/<n>.png`: OPTIONAL PNG previews of pages, where `<n>` is the
  0-based page index at write time. They are a cache: readers MUST tolerate
  missing or stale thumbnails and MUST NOT treat them as content.
- `layers/<peer>.loro`: **reserved** for per-student annotation layers (a
  student's ink on top of a teacher's lesson, kept separate from the lesson
  document). Each is a Loro snapshot. Its internal layout will be defined by a
  future revision. Readers MUST NOT fail because of layers. Writers MUST
  preserve layers they read.

## Reader limits and safety

A `.rpk` file can come from anyone (a shared link, an e-mail, a student).
Readers MUST treat every package as untrusted input.

### Path rules

Every entry name MUST satisfy all of the following. Readers MUST reject the
whole package if any non-directory entry violates them.

- not empty, and at most 512 bytes;
- does not start with `/`;
- contains no `\` and no control characters (Unicode `Cc`);
- every `/`-separated segment is non-empty and is neither `.` nor `..`.

Writers SHOULD additionally keep names portable: no `:`, no segment ending in
`.` or a space, and no Windows device names (`CON`, `NUL`, …). A reader that
extracts entries to a file system MUST also refuse names that would escape
the target directory on that platform (for example drive prefixes like `C:`
on Windows).

### Size and count limits

| Limit | Value | Check |
|---|---|---|
| Entries | ≤ 50,000 | central directory count, before reading |
| One entry, uncompressed | ≤ 1 GiB (2^30 bytes) | declared size, and again while reading |
| All entries, uncompressed | ≤ 4 GiB (2^32 bytes) | running total while reading |
| Points per stroke | ≤ 1,048,576 | point codec header |
| Smart Object bundle | ≤ 25 MiB (all files) | Smart Object validation |
| Smart Object `state` | ≤ 256 KiB | bridge |

- Readers MUST NOT trust declared sizes (zip bombs). They MUST cap the number
  of bytes actually inflated per entry and in total, and stop at the limit.
- Readers MUST NOT pre-allocate more than a bounded amount per entry based on
  declared sizes (the reference caps it at 64 MiB).
- Readers MUST reject a package with duplicate entry names, because ZIP tools
  disagree about which copy wins.
- Writers SHOULD stay within these limits so every reader can open the file.

### Other safety rules

- Reading MUST NOT execute code, follow links or fetch from the network.
- Smart Objects run only in the sandbox from `spec/smart-object.md`.
- A corrupt item MUST NOT stop the rest of the document from loading.
  Readers SHOULD skip it and report it.

## Versioning and forward compatibility

There are four independent version gates:

| Gate | Where | Current | Reader rule |
|---|---|---|---|
| Package | `manifest.json` `version` | 1 | Reject if greater than supported. |
| Document | `doc.loro` `meta.format` | 1 | Reject if missing or greater than supported. |
| Point codec | first byte of `pts` | 1 | Treat the stroke as corrupt if unknown. |
| Smart Object | bundle `manifest.json` `schema` | 1 | See `spec/smart-object.md`. |

Rules:

1. **Additive changes keep the version.** New optional keys, new optional
   entries, new `bg_pattern` values and new item kinds are additive.
2. **Readers MUST ignore what they do not understand**: unknown ZIP entries,
   unknown manifest keys, unknown root containers, unknown keys in `meta`,
   pages and items, and items with an unknown `kind` (skip them for rendering
   and hit-testing).
3. **Writers MUST preserve what they do not understand** when they rewrite a
   package or edit a document: unknown entries, unknown manifest keys, and
   unknown keys and items in the Loro document. Loro preserves unknown
   document data automatically as long as the writer edits the document
   in place.
4. **Breaking changes bump a version.** A change that would make a current
   reader misread data (a key's meaning or type changes, a required key is
   added, the codec layout changes) MUST bump the relevant gate.
5. **Defaults are part of the format.** A reader that meets a missing key MUST
   use the default listed in this spec.

## Change process

This spec is a contract between independent implementations, so it changes
deliberately:

1. Open an issue describing the problem.
2. Write an RFC as a pull request (see `CONTRIBUTING.md`). It states the
   change, the version impact, a migration path and test vectors.
3. Change the spec, the reference implementation and the tests **in the same
   pull request**. The spec and `crates/rupok-format`, `crates/rupok-core` and
   `crates/rupok-ink` MUST NOT drift apart.

Editorial fixes (typos, clearer wording with no change in meaning) do not need
an RFC.

## Conformance notes

Known gaps between this spec and the reference implementation at the time of
writing. Each is a bug in the implementation, not in the spec, and is tracked
for fixing:

- **Unknown item kinds** make `Lesson::items()` fail for the whole page
  (`CoreError::Corrupt("unknown kind …")`). A stroke with undecodable `pts`
  does the same. The spec requires skipping them.
- **Duplicate entry names** are accepted; the last copy silently wins.
- **`mimetype`** is checked for presence and content, but not for being the
  first entry.
- **`manifest.format`** is not checked against `"rupok-lesson"`.
- **Unknown ZIP entries and unknown manifest keys** are ignored on read (as
  required) but dropped on rewrite. The spec says writers preserve them.
- **A bundle with a missing or invalid `manifest.json`** makes
  `Package::read` fail for the whole package. The spec says to treat that
  bundle as missing.
- **Writer limits.** `Package::write` does not enforce the reader limits, so it
  can produce a file that `Package::read` rejects (for example an asset over
  1 GiB).
- **Point codec varints.** Overlong varints (more than 64 bits of payload) are
  truncated, not rejected. Trailing bytes are ignored, which matches the
  spec.
- **Thumbnails** are read and written, but nothing generates them yet.
