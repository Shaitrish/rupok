# Subject tools

`Status: 📋 planned · Phase 3 · Difficulty: intermediate (several good-first-issues)`

On-screen ruler, protractor, set square and compass that guide the pen, plus built-in graphing, number line and periodic table objects.

## Goal

Maths and science teachers need drawing instruments and reference tools that
paper and a chalkboard give them for free. This plan adds two families:

1. **Drawing guides**: ruler, set square, protractor and compass that float
   over the board and constrain the pen, producing ordinary strokes.
2. **Content tools**: graphing calculator, number line, periodic table,
   fraction bars, unit circle. These are **built-in Smart Objects** shipped
   with the app, so they reuse the sandbox, steps, theme and the pen-on-top
   behaviour instead of new item kinds.

Handwriting to text/math is a separate plan:
[handwriting-recognition](./handwriting-recognition.md).

## User story

- *As a maths teacher*, I drag the ruler onto the board, rotate it with two
  fingers to 30°, and draw a perfectly straight line along its edge with my pen.
- *As a geometry teacher*, I measure an angle with the protractor and draw an
  arc with the compass for a construction, live in front of the class.
- *As a science teacher*, I insert the periodic table, tap "Na" to enlarge it,
  and circle the alkali metals with my pen.
- *As a maths teacher*, I type `y = x^2 - 3` into the graphing tool and the
  class watches the parabola appear; I annotate the roots.

## Scope

- Guides: ruler (with cm/inch ticks), set square (45° and 30/60°),
  protractor (180° and 360°), compass.
- Pen snapping to a guide edge; compass arcs.
- Built-in Smart Objects: graphing (functions, points), number line,
  periodic table, fraction bars, unit circle, coordinate grid.
- An "Insert tool" menu in `apps/board`.

## Non-goals

- A full CAS or dynamic geometry system (GeoGebra-class). Teachers can still
  embed AI-made Smart Objects for advanced cases.
- Measuring real-world units on screen (the ruler measures world units with a
  configurable "1 cm" scale, not physical centimetres on every display).
- New `.rpk` item kinds for these tools.

## Design

### Guides (engine)

Guides are **transient tool state**, like the laser pointer, not document
items: placing a ruler is not something to undo or replay. Strokes drawn along
a guide are ordinary strokes, so the eraser, lasso, replay and file format need
no change.

New module `crates/rupok-wasm/src/guides.rs`:

```rust
pub enum GuideKind { Ruler, SetSquare45, SetSquare3060, Protractor180, Protractor360, Compass }

pub struct Guide {
    pub id: u32,
    pub kind: GuideKind,
    pub origin: [f32; 2],   // world units
    pub angle: f32,         // radians
    pub length: f32,        // world units (ruler, set square)
    pub radius: f32,        // compass opening, protractor size
}

impl Guide {
    /// If `p` (world) is within `snap` of an edge, return the edge as a line segment.
    pub fn edge_near(&self, p: [f32; 2], snap: f32) -> Option<([f32; 2], [f32; 2])>;
    /// Project a point onto an edge segment.
    pub fn project(edge: ([f32; 2], [f32; 2]), p: [f32; 2]) -> [f32; 2];
    pub fn hit(&self, p: [f32; 2]) -> bool;   // for dragging the guide itself
}
```

Engine changes in `crates/rupok-wasm/src/engine.rs`:

- New field `guides: Vec<Guide>`.
- In `pointer_down` with the pen or highlighter tool: if the first point is
  within `snap = 12 / camera.zoom` of a guide edge, start a
  `Gesture::Ink` with a new `snap_edge: Option<(...)>`. In `pointer_move`,
  project each sample onto the edge **before** `StrokeBuilder::push`, keeping
  pressure and timing. The result is a straight, pressure-sensitive stroke.
- Touch on a guide body (not the pen) drags or rotates it: one finger moves,
  two fingers rotate (reuse the centroid/distance math in
  `navigate_touches`).
- Compass: a new `Gesture::Arc { center, radius, start_angle, last_angle }`.
  On pen up, commit a stroke whose points come from a new
  `shapes::arc_points(center, r, a0, a1)` in `crates/rupok-wasm/src/shapes.rs`,
  with `shape: Some("arc")`.

The `shape` key is a hint and readers must draw `pts` regardless
(`spec/rupok-format.md`, section "Items", `kind: "stroke"`), so adding the value
`"arc"` does not break v1 readers. It still **requires an RFC** to list the
new value in the spec.

Board API (new, `crates/rupok-wasm/src/lib.rs`):

```text
guide_add(kind: &str) -> u32          // placed at the view centre
guide_set(id: u32, x: f32, y: f32, angle: f32, length: f32, radius: f32)
guide_remove(id: u32)
guides_json() -> String               // for the overlay renderer
set_guide_unit(world_units_per_cm: f32)
```

Guides are drawn by the UI as an SVG overlay (`apps/board/src/guides/`, new),
not by the GPU renderer: they are few, need crisp tick labels, and must not
enter the dry ink cache.

### Content tools (built-in Smart Objects)

- Location: `apps/board/src/builtin/<tool>.html` (new), each a single
  self-contained file following `spec/smart-object.md` (design guidance,
  `rupok` SDK, no network). They are validated in CI with the same test as
  `crates/rupok-format/tests/examples.rs`.
- Inserted with the existing `Board.add_object_html(html, title, w, h, steps)`.
- Graphing: a small expression parser in plain JS (numbers, `x`, `+ - * / ^`,
  parentheses, `sin cos tan sqrt abs log ln exp pi e`), plotted on a canvas
  with `ResizeObserver`. Expressions are saved with `rupok.state.save()`.
  Steps reveal one function per step.
- Periodic table: data inlined as JSON (118 elements: symbol, name, number,
  group, period, category, atomic mass). Tap opens a detail card. Steps
  highlight groups (alkali metals, halogens, noble gases…).
- Number line and fraction bars: steps reveal marks; tap to place points.

## Implementation tasks

1. `guides.rs`: types, `edge_near`, `project`, `hit` with unit tests (angles, edge ends). (good-first-issue)
2. Ruler snapping in the engine's ink gesture; `MockSink` test that a wobbly pen path along the ruler commits a straight stroke. (intermediate)
3. Guide drag/rotate with touch, pen draws through. (intermediate)
4. SVG overlay for guides with ticks and degree labels in `apps/board`. (good-first-issue)
5. Set square and protractor shapes and edges. (good-first-issue)
6. Compass gesture and `shapes::arc_points`, `shape: "arc"`; RFC to list the value. (intermediate)
7. Built-in graphing object. (intermediate)
8. Built-in periodic table object (data + layout). (good-first-issue)
9. Built-in number line, fraction bars, unit circle (one PR each). (good-first-issue)
10. "Insert tool" menu and a CI test that validates every `apps/board/src/builtin/*.html` with `ObjectBundle::validate`. (good-first-issue)

## Acceptance criteria and tests

- `cargo test -p rupok-wasm guides` and engine tests: a stroke started within the
  snap distance lies on the edge (every point within 0.01 world units of the
  line); a stroke started farther away is untouched.
- The compass produces a stroke with `shape = "arc"` whose points are within
  0.5 world units of the circle.
- Guides never appear in saved files, undo history or replay.
- All built-in objects pass validation with zero errors and render correctly at
  960×540 and 1920×1080, touch-only.
- Graphing: `y = x^2` plots through (0,0), (1,1), (−2,4) within one pixel.

## Performance budget

- Snapping adds ≤ 0.05 ms per input sample; `rupok-bench` wet frame p99 ≤ 4 ms
  holds with a ruler on screen.
- Built-in objects: first render ≤ 300 ms on a mid-range Chromebook-class
  device; periodic table bundle ≤ 150 KB.

## Open questions

- Should a placed guide be saved per page so it is still there after a page
  switch (UI state), or always start fresh?
- Do teachers want a "measure" readout (length/angle of the last stroke)?
- Which content tools matter most for the first release? Ask teachers.

## References

- `spec/rupok-format.md` — "Items" (`kind: "stroke"`, `shape`), "Change process"
- `spec/smart-object.md` — "Design guidance for AI authors", "Steps", "State"
- `crates/rupok-wasm/src/shapes.rs`, `crates/rupok-wasm/src/engine.rs`
- [handwriting-recognition](./handwriting-recognition.md), [i18n](./i18n.md)
