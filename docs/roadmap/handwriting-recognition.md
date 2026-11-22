# Handwriting recognition (ink to text and math)

`Status: 📋 planned · Phase 3 · Difficulty: advanced`

Select handwritten ink and convert it to clean text or typeset maths, on-device where possible, with the teacher's AI agent as a fallback.

## Goal

Let a teacher lasso some handwriting and tap **Convert**: words become a text
item, formulas become typeset maths. Recognition must work offline in a
classroom, support English and Bangla first, and never lose the original ink
(undo brings it back).

## User story

- *As a teacher*, I write "Photosynthesis" quickly on the board, lasso it and
  tap **To text**; it becomes neat typed text my students can read from the
  back.
- *As a maths teacher*, I write `x² + 3x − 4 = 0` and tap **To math**; it
  becomes a typeset formula I can keep solving next to.
- *As a Bangla-medium teacher*, I write in Bangla and the conversion keeps
  conjuncts and vowel signs correct.
- *As a teacher without internet*, conversion still works.

## Scope

- Recognizer abstraction with pluggable backends.
- Backends: (1) on-device model (ONNX) for text, (2) on-device model for
  maths, (3) platform recognizers where available, (4) the teacher's AI agent
  through MCP as a fallback.
- Ink → text item; ink → math rendered as a KaTeX Smart Object.
- One undoable step per conversion.

## Non-goals

- Live recognition while writing (shape/word suggestions as you write).
- Recognising whole pages or diagrams.
- A new `math` item kind in Phase 3 (maths uses a Smart Object with
  `rupoklib://katex`; a native kind can come later via RFC).
- Training our own models from scratch.

## Design

### Pipeline

```text
lasso selection ─▶ Engine::selection_strokes() ─▶ normalise (offset, scale, order by created)
                 ─▶ Recognizer (text | math)     ─▶ candidates [{text|latex, score}]
                 ─▶ UI picker (top 3)            ─▶ replace: remove strokes + add item (one commit)
```

### Engine (`crates/rupok-wasm`)

New module `crates/rupok-wasm/src/recognize.rs`:

```rust
pub struct InkSample { pub strokes: Vec<Vec<[f32; 3]>> }  // per stroke: [x, y, t_ms] with offsets applied

impl<S: InkSink> Engine<S> {
    pub fn selection_ink(&self) -> Option<InkSample>;                 // strokes only, in timeline order
    pub fn replace_selection_with_text(&mut self, text: &str, size: f64, now_ms: f64) -> Option<ItemId>;
    pub fn replace_selection_with_object(&mut self, html: &str, title: &str, now_ms: f64) -> Result<ItemId, String>;
}
```

- `selection_ink` reads `StrokeData::points` (`x`, `y`, `dt`) plus `offset`
  from the scene; absolute time is `created + dt` (see "Open questions").
- The replace functions must be **one Loro commit**: today
  `Lesson::remove_items` and `Lesson::add_item` each commit. Add a
  `Lesson::replace_items(page, remove: &[ItemId], add: &ItemData, now_ms)` in
  `crates/rupok-core/src/doc.rs` that does both and commits once, so one undo
  restores the ink.
- Text placement: the text item's geometry is the selection bounds
  (`Scene::bounds_of`); size from the median stroke height.

### Recognizer backends (TypeScript side, `apps/board/src/recognize/`, new)

```ts
interface Recognizer {
  id: string;
  kinds: ("text" | "math")[];
  languages: string[];             // BCP-47, e.g. "en", "bn"
  available(): Promise<boolean>;
  recognize(ink: InkSample, kind: "text" | "math", lang: string): Promise<Candidate[]>;
}
```

1. **On-device ONNX** (default when downloaded): `onnxruntime-web` with the
   WASM/WebGPU execution provider in the board; the same model through `ort` in
   the desktop app if the web runtime is too slow. Models are optional
   downloads (not in the installer), cached on disk, licence-checked
   (must be open source and allow redistribution).
   - Text: an online (trajectory) handwriting model trained on IAM-OnDB-style
     data; Bangla needs a model trained on Bangla online handwriting.
   - Maths: a handwritten-math-to-LaTeX model trained on CROHME data (ink is
     rendered to a grayscale image first if the model is image-based).
2. **Platform recognizers** (desktop shell): Windows Ink handwriting
   recognition via the Windows Runtime, exposed by a Tauri command; Android ML
   Kit Digital Ink Recognition in the [Android](./android.md) build (on-device,
   includes Bangla).
3. **AI agent fallback**: render the selection to PNG (canvas in the board, or
   headless `rupok-render` natively) and expose it through a new MCP tool in
   `crates/rupok-cli` (`recognize_selection` returning the image, the agent
   answers with text/LaTeX). Works with any multimodal agent the teacher
   already uses; needs no bundled model.

### Math output

`replace_selection_with_object` places a small single-file Smart Object that
loads `rupoklib://katex/katex.min.js` and `katex.min.css` and renders the LaTeX
from `rupok.state.load()`. The LaTeX is the object's saved state, so editing
it later is a state change. Validation runs as for any object
(`spec/smart-object.md#validation`).

## Implementation tasks

1. `Lesson::replace_items` (single commit) with undo test in `crates/rupok-core`. (intermediate)
2. `Engine::selection_ink` and `replace_selection_with_text` with `MockSink` tests. (intermediate)
3. Board API wrappers and the **Convert** button in the selection toolbar with a candidate picker. (good-first-issue)
4. Math Smart Object template (KaTeX, state-driven) + `replace_selection_with_object`; validate it in CI. (good-first-issue)
5. Recognizer interface and registry in `apps/board/src/recognize/`. (good-first-issue)
6. AI-agent fallback: PNG rendering of a selection + MCP tool `recognize_selection` (coordinate with the CLI owner). (intermediate)
7. ONNX text backend (English), model download manager with checksum verification. (advanced)
8. ONNX math backend. (advanced)
9. Windows Ink backend via a Tauri command. (advanced)
10. Bangla text backend (model selection, evaluation set of 500 teacher-written samples). (advanced)
11. Evaluation harness: `tools/recognize-eval/` with labelled ink samples stored as `.rpk` files and an accuracy report. (intermediate)

## Acceptance criteria and tests

- Converting then undoing restores the exact original strokes (ids, points,
  `created`) in one undo step.
- On the evaluation set: English word accuracy ≥ 90 % (top-1) and ≥ 97 %
  (top-3); maths expression exact-match ≥ 70 % (top-1) on simple school
  algebra.
- Bangla: correct rendering of conjuncts and vowel signs in the resulting text
  item (visual test in the board); accuracy targets set after model choice.
- With no network and a downloaded model, conversion works.
- The math object passes `ObjectBundle::validate` with zero errors.

## Performance budget

- One handwritten line (≤ 30 strokes): result ≤ 1.5 s on a mid-range laptop,
  ≤ 3 s on an Android panel.
- Model downloads: ≤ 30 MB per language per kind; nothing downloads without
  the teacher's consent.
- Recognition runs off the pen path (worker); `rupok-bench` wet frame p99 ≤ 4 ms
  is unaffected.

## Open questions

- Stroke `created` is currently the **pen-up** time (`Engine::pointer_up`
  passes `now_ms` to `commit_stroke`), while `spec/rupok-format.md` says writers
  SHOULD use pen-down time. Trajectory models need correct stroke order and
  timing; fix the engine first or reconstruct start = `created - last dt`.
- Which open models have licences compatible with redistribution and good
  enough accuracy, especially for Bangla?
- Should converted text keep a link to the original ink ("show ink" toggle)?

## References

- `spec/rupok-format.md` — "Items", "Stroke points", "Replay"
- `spec/smart-object.md` — "Offline libraries" (`katex`), "Validation"
- `crates/rupok-core/src/scene.rs` — `Scene::lasso`, `Scene::bounds_of`
- [subject-tools](./subject-tools.md), [i18n](./i18n.md), [android](./android.md), [acp-agent-panel](./acp-agent-panel.md)
