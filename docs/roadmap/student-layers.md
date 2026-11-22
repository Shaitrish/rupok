# Student annotation layers

`Status: 📋 planned · Phase 2 · Difficulty: advanced`

Students write on top of a teacher's lesson in their own layer, without ever changing the lesson itself.

## Goal

Give every student a private ink layer over a shared lesson. The teacher's
lesson stays read-only for students; their notes, answers and Smart Object
state live in a separate Loro document stored under the reserved
`layers/<peer>.loro` entry of the `.rpk` package. The teacher can later open a
lesson with any number of student layers and toggle them on and off.

## User story

- *As a teacher*, I share today's lesson with my class. Each student opens it on
  a tablet and takes notes directly on my slides. When I fix a typo on slide 3
  and re-share, their notes are still there.
- *As a teacher*, I collect the lesson back from 30 students and flip through
  "Rahim's layer", "Nusrat's layer" on the same slide to see who solved the
  problem, without 30 copies of my slides.
- *As a teacher*, I can hide all student layers with one switch before I
  present, so my clean slides are never polluted.
- *As a student*, my answers inside a quiz Smart Object are saved in my layer,
  not in the teacher's lesson.

## Scope

- Layer data model (Loro document) and its package entry.
- Engine support: a "base + one editable layer" mode (student) and a
  "base + N read-only layers" mode (teacher review).
- Rendering layer ink above base ink, with per-layer visibility.
- Smart Object state per layer in `student` mode.
- Import/export: attach a layer to a package, extract a layer from a package.

## Non-goals

- Real-time collection of layers during class (that is
  [live-broadcast](./live-broadcast.md) and submissions in Phase 3).
- Rosters, grading and gradebook export (Phase 3).
- Students editing base items (moving the teacher's text, deleting slides).
- Merging a student layer *into* the base lesson (possible later as "copy
  layer ink to lesson", out of scope here).

## Design

### Data model (proposed; requires an RFC)

The `.rpk` spec reserves `layers/<peer>.loro` but leaves the layout undefined
(`spec/rupok-format.md`, section "Thumbnails and layers"). This plan defines it.
**Requires an RFC** amending that section and "Versioning and forward
compatibility". The change is additive (no version bump): v1 readers already
preserve layers and must not fail on them. `Package.layers: BTreeMap<String, Vec<u8>>`
in `crates/rupok-format/src/package.rs` already reads and writes the bytes.

A layer is a Loro snapshot with this root layout:

```text
meta   : Map  { format: i64 = 1, kind: "layer", author: String, author_id: String,
                base_lesson: String, created: f64 }
pages  : Map<page_id, Map { items: Map<id, Map> }>     keyed by base page id
objects: Map<item_id, String>                           Smart Object state JSON per base object item
```

- `pages` is a `Map` keyed by the **base** page id, not a movable list: a
  student cannot reorder the teacher's pages. Notes on a page the teacher later
  deletes are kept but hidden ("orphaned layer pages").
- Item maps reuse the item schema from `spec/rupok-format.md#items` exactly
  (`kind`, `z`, `created`, stroke keys, text keys). Layer `z` values are local
  to the layer; layers always draw above base items.
- `base_lesson` identifies which lesson the layer belongs to. Today the format
  has no lesson identity, so this plan also proposes an additive
  `meta.lesson_id: String` (random 128-bit, base36) in the base document,
  written by `Lesson::new` (**requires an RFC**, `spec/rupok-format.md#meta`).
- Entry name: `layers/<author_id>.loro`, where `author_id` is a stable,
  filesystem-safe id (`[a-z0-9-]{1,64}`). The spec's `<peer>` placeholder is
  renamed to `<author>` in the RFC; Loro peer ids are per-session and random,
  so they are not stable author ids.

### Core (`crates/rupok-core`)

New module `crates/rupok-core/src/layer.rs`:

```rust
pub struct Layer { doc: LoroDoc, undo: UndoManager, counter: u64 }

impl Layer {
    pub fn new(base_lesson: &str, author: &str, author_id: &str, now_ms: f64) -> Self;
    pub fn load(bytes: &[u8]) -> Result<Self>;
    pub fn save(&self) -> Vec<u8>;                       // ExportMode::Snapshot
    pub fn add_item(&mut self, page_id: &str, data: &ItemData, now_ms: f64) -> Result<ItemId>;
    pub fn remove_items(&mut self, page_id: &str, ids: &[ItemId]) -> Result<usize>;
    pub fn translate_items(&mut self, page_id: &str, ids: &[ItemId], dx: f64, dy: f64) -> Result<()>;
    pub fn items(&self, page_id: &str) -> Result<Vec<Item>>;
    pub fn object_state(&self, item_id: &str) -> Option<String>;
    pub fn set_object_state(&mut self, item_id: &str, json: &str) -> Result<()>;
    pub fn undo(&mut self) -> Result<bool>;
    pub fn redo(&mut self) -> Result<bool>;
}
```

- Reuse `write_item_data`, `read_item`, `write_geom` from
  `crates/rupok-core/src/doc.rs`. They are private today; move them to a
  `pub(crate)` module (`crates/rupok-core/src/items.rs`) so `Lesson` and `Layer`
  share one implementation.
- Same rules as `Lesson`: every mutation commits, `set_change_merge_interval(0)`,
  record timestamps, wall-clock time passed in as `now_ms`.

### Engine (`crates/rupok-wasm/src/engine.rs`)

- New field `layers: Vec<LayerView>` where
  `struct LayerView { id: String, layer: Layer, visible: bool, editable: bool }`.
- New enum `EditTarget { Base, Layer(usize) }`. `commit_stroke`,
  `commit_erase`, `move_items`, `delete_selection` and `set_object_state`
  route to the target. In student mode the target is the student's layer and
  the base is read-only: `erase_at` and `Scene::lasso` must only return layer
  item ids.
- Scene: keep one `Scene` per layer (`scene_layers: Vec<Scene>`) so hit-tests
  can be restricted to the editable layer; the base `scene` is unchanged.
- `load_page` / `resync` add visible layer strokes to the sink after base
  strokes.

### Renderer (`crates/rupok-render/src/ink.rs`)

Layer ink must draw above all base ink. `InkRenderer` resolves stacking with a
per-stroke depth (`depth_of(idx)`, `DEPTH_STEP`, `MAX_DEPTH_IDX`) and a `Less`
depth test. Proposed: add a depth *band* per stroke group.

```rust
pub fn add_stroke_in(&mut self, band: u8, id: &str, s: &StrokeData); // band 0 = base, 1..=15 = layers
pub fn set_band_visible(&mut self, band: u8, visible: bool);          // hides via radius < 0 tombstone trick, no rebuild
```

`depth = 1.0 - (band_offset(band) + idx + 1) * DEPTH_STEP`, with bands taking
disjoint index ranges. `compact()` renumbers within bands. `add_stroke` keeps
its current signature and means band 0.

### Board API (`crates/rupok-wasm/src/lib.rs`, new methods)

```text
layer_new(author: &str, author_id: &str) -> String       // returns layer id, becomes edit target
layer_attach(bytes: &[u8]) -> Result<String, JsValue>    // read-only layer from bytes
layer_list_json() -> String                              // [{id, author, visible, editable, strokes}]
layer_set_visible(id: &str, visible: bool)
layer_set_edit_target(id: &str)                          // "" = base
layer_export(id: &str) -> Vec<u8>
```

`save()` writes every layer into `pkg.layers` under `<author_id>.loro`.

### Smart Objects

In `student` mode (`spec/smart-object.md#modes`) the host sends the layer's
state for the object (`Layer::object_state`) in `init`, and stores `state`
messages in the layer. The base `state` key is never written by a student.
No bridge change is needed.

## Implementation tasks

1. **RFC**: layer layout, `<author>` entry naming, `meta.lesson_id`. (intermediate)
2. Move item read/write helpers out of `doc.rs` into `pub(crate) mod items`, no behaviour change; existing tests must pass. (good-first-issue)
3. Write `meta.lesson_id` in `Lesson::new`; expose `Lesson::lesson_id()`; tolerate missing ids in old files. (good-first-issue)
4. Add `crates/rupok-core/src/layer.rs` with `Layer::{new, load, save, add_item, items, remove_items}` and unit tests (round trip, undo per stroke, orphaned page ids). (intermediate)
5. Add `Layer` object state map and tests. (good-first-issue)
6. Renderer depth bands: `add_stroke_in`, `set_band_visible`, band-aware `compact()`; a headless test that a layer stroke covers a base stroke at the same pixel (extend the probe in `bench/src/main.rs` or add `crates/rupok-render/tests/bands.rs`). (advanced)
7. Engine: `EditTarget`, per-layer scenes, routing of commits/erase/lasso, read-only base in student mode; tests with `MockSink`. (advanced)
8. Board API methods and `pkg.layers` integration in `Engine::save`/`Engine::load`. (intermediate)
9. UI (`apps/board`): layers panel (list, eye toggle, "edit my layer"), student-mode banner. (intermediate)
10. Smart Object host: per-layer `init.state` and `state` routing in student mode. (intermediate)
11. Docs: update `spec/rupok-format.md` after the RFC; add a "Using layers" section to the README. (good-first-issue)

## Acceptance criteria and tests

- `cargo test -p rupok-core layer` covers: create, save/load round trip, undo,
  items keyed by base page id, unknown page ids kept.
- A package with three layers survives `Package::write` → `Package::read` with
  byte-identical layer entries (`crates/rupok-format` test).
- Engine test: in student mode, the eraser never removes a base stroke; lasso
  never selects base items; a student stroke lands in the layer document.
- Engine test: hiding a layer removes its strokes from the sink without
  touching base strokes; showing it again restores them.
- Opening a v1 file without `lesson_id` still works; a layer whose
  `base_lesson` does not match shows a warning but still renders.
- Manual: 30 attached layers on a 1080p page, toggling one is instant.

## Performance budget

- Attaching a layer with 5,000 strokes: ≤ 150 ms (load + sink upload).
- Toggling a layer's visibility: ≤ 1 frame (no vertex buffer rebuild).
- All `rupok-bench` budgets unchanged with zero layers; add a bench case with
  10 layers × 2,000 strokes and keep pan frame p99 ≤ 8.3 ms. Run
  `cargo run -p rupok-bench --release`.

## Open questions

- Should a student's layer ship as a full `.rpk` (base + layer, easy to open
  anywhere) or as a small "layer file" (`.37l`) that needs the base lesson? The
  first is simpler; the second saves bandwidth for 40 students.
- Do layers need their own pages (a blank "my notes" page after slide 5)?
  This would need `pages` to allow layer-only pages.
- How many depth bands are enough? 15 layers visible at once seems plenty for
  review; more could be paged.

## References

- `spec/rupok-format.md` — sections "Thumbnails and layers", "Items", "meta", "Versioning and forward compatibility"
- `spec/smart-object.md` — "Modes", "State"
- [live-broadcast](./live-broadcast.md), [live-quiz](./live-quiz.md), [web-viewer](./web-viewer.md), [google-drive-sync](./google-drive-sync.md)
