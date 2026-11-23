# Handwritten maths to LaTeX (offline)

`Status: recognizer core done (crates/rupok-ink/src/math); wired into the board (live preview, To text, typeset maths items, PDF export)`

A teacher writes `x²` or `a² + b² = c²` on the board. With Smart Ink on, a
typeset preview appears above the ink while they write, and one tap converts
it. This recognizer does that **without a model, a download or a network**:
classic geometry, a point-cloud classifier and dynamic programming, in pure
Rust that builds for `wasm32-unknown-unknown`. Messy or complex input can
still go to the AI fallback (`apps/board/src/handwriting.ts`). This page
covers the offline core only.

- Code: `crates/rupok-ink/src/math/` (module docs in `mod.rs`)
- JS binding: `crates/rupok-wasm/src/math_ink.rs` (`MathInk`)
- Report and bench: `cargo run -p rupok-ink --release --example math_eval`

## API

```rust
use rupok_ink::math::{MathRecognizer, Candidate};

let mut rec = MathRecognizer::new();            // built-in templates, ~10 ms
let all: Vec<Candidate> = rec.recognize(&strokes); // strokes: &[Vec<StrokePoint>], time order

// Live preview: push strokes as they finish, read the n-best list.
let id = rec.push_stroke(&points);
let cands = rec.candidates();                    // groups seen before are cached
rec.pop_stroke();                                // undo
rec.clear();

// "Teach it my handwriting"
rec.add_user_template("x", &my_x_strokes)?;     // text or LaTeX name: "x", "\\theta", "√"
rec.clear_user_templates(Some("x"))?;
```

A `Candidate` has `latex` (KaTeX-ready), `confidence` (0–1) and `symbols`:
each `RecognizedSymbol` has `symbol` (`"x"`, `"×"`, `"√"`, `"─"` for a
fraction bar), `latex`, `bbox` (input coordinates), `strokes` (input stroke
indices) and `confidence`. Candidates come best first; the list is at most
`n_best` long (default 5) with distinct LaTeX.

From JavaScript (`MathInk`): `push_stroke(Float32Array xy)`, `pop_stroke()`,
`clear()`, `candidates_json()`, `recognize_json(xy, counts)`,
`add_user_template(symbol, xy, counts)`, `clear_user_templates(symbol?)`,
`MathInk.auto_convert_confidence()`, `MathInk.symbols_json()`.

## How it works

```text
strokes ─▶ deslant ─▶ reorder delayed strokes ─▶ k-best grouping DP ──▶ layout ──▶ context ──▶ LaTeX + n-best
                                                   │  ($Q classifier)     (radicals,   (Viterbi:
                                                   ▼                       fractions,   grammar,
                                             template library             scripts)     size, position)
```

1. **Scale and slant** (`segment.rs`). `unit` is the median stroke size,
   ignoring dots. The writing slant is the length-weighted mean slope of
   near-vertical chords; it is sheared out (quantised to 0.05 so the cache
   survives small changes), because templates are upright.
2. **Delayed strokes** (`segment.rs`). Grouping follows time order, but a
   stroke that lands on an earlier symbol and not on the stroke just before
   it moves next to that symbol: the dot of an `i` added at the end of a
   word, a `t` crossed later, `−` turned into `+`, the second line of `=`
   added after the next symbol.
3. **Grouping** (`segment.rs`). A k-best dynamic programme over the
   reordered strokes. A symbol is a run of 1–4 strokes that pass cheap
   geometric checks: close together, overlapping horizontally or touching,
   dots only above or below the other ink (`i`, `j`, `÷`). Cost of a group =
   classifier distance / τ + λ (per symbol) + largest gap + a penalty for
   each stroke beyond the symbol's usual count + a penalty for a small
   stroke sitting outside the rest without touching it (a script, not part
   of the symbol), for a small closed loop at the edge of a bigger symbol (a
   degree sign) and for a flat line floating above the rest (a fraction
   bar). Three structures are recognised directly, because they are common
   and distortion pulls them far from their templates: `+` (a horizontal and
   a vertical line crossing near both middles), `=` (two stacked horizontal
   lines) and a radical drawn as a tick plus a separate overbar. A clear
   radical never groups with the strokes under its bar. The 4 best
   segmentations are kept.
4. **Classification** (`classify.rs`). The $Q recognizer (Vatavu, Anthony,
   Wobbrock, MobileHCI 2018; successor of $P, ICMI 2012), implemented from
   the paper: resample, scale uniformly, centre on the centroid, greedy
   weighted cloud matching with $Q's lookup-table lower bound and early
   abandoning. It ignores stroke order, direction and count but not
   rotation (`6`/`9`, `u`/`n`, `<`/`>`). Changes: 24 points and 4 start
   offsets instead of 32 and 6 (half the cost, accuracy within noise:
   91.4 % vs 92.4 % on an earlier version of the full set); templates are
   visited best-first by their bound and the search stops once no template
   can enter the top 3; each stroke gets points in proportion to
   max(length, 8 % of the diagonal), so dots keep points; multi-stroke
   symbols must (`+ t f ± ≠`) or must not (`= ÷ i j ≤ ≥`) have crossing
   strokes; each stroke beyond a symbol's usual count costs a little; a
   two-stroke `θ` needs a flat bar. Look-alikes that templates cannot
   separate (`x`/`×`, `.`/`·`, `0`/`o`/`O`/`°`, `c`/`C`, `s`/`S`, `v`/`V`,
   `p`/`P`, `1`/`l`/`|`, `,`/`′`) are added as *twins* of the winner for the
   context stage to decide.
5. **Layout** (`layout.rs`), per region: radicals take the symbols inside
   them; the widest flat line with symbols above **and** below becomes a
   fraction (otherwise it stays a minus); the rest is walked left to right.
   Each symbol has an x-height band from its profile (`x` fills it, `2`
   stands on it and is taller, `y` hangs below it, `+` is centred on it),
   and a following symbol is a superscript or subscript when its baseline
   and centre sit clearly above or below the base's band, with smaller
   symbols needing less offset. Only operands, `)` and big operators take
   scripts. Close calls are recorded as *doubts* and tried the other way
   for the n-best list.
6. **Context** (`context.rs`). Viterbi over each row. Emission = classifier
   distance + rarity prior + size and baseline fit of the candidate's
   profile on that row. Transitions forbid or penalise two binary operators
   in a row, an operator at the end, a relation at the start, a digit right
   after a letter on the same line; a minus after `=` or `(` is unary and
   fine; an integral needs an integrand. A capital letter on the row (`H`,
   `N`…) makes `O` likelier than `0`. Then fixed rules: a tiny raised circle
   (or dot) after a number is `^\circ`, a tiny raised tick (or dot) after a
   letter is a prime, and letter runs that spell `sin cos tan log ln`
   become `\sin` and friends. Whole-parse costs are added for unmatched
   brackets (`[x` is likelier `∫x`, `)(` likelier `x`) and for scripts that
   are not lowered or raised relative to their base (`c` with an `O`
   "subscript" at the same height is really `CO`).
7. **n-best and confidence** (`pipeline.rs`). Candidates come from the top 3
   segmentations, the two best alternative labels for every symbol
   (re-decoded, so neighbours adapt, and laid out again when the label sits
   differently on the line: `×` vs `x`, `μ` vs `H`), and the two closest
   script decisions flipped. Each
   has a total cost; `confidence = softmax(−cost / 0.6) × geometric mean of
   per-symbol shape quality`, where shape quality is a logistic function of
   the classifier distance (0.5 at 0.03).

### Templates and their licence

| Source | What | Licence |
|---|---|---|
| Hershey fonts, Roman simplex (`futural.jhf`) | digits, Latin letters, `+ − = < > ( ) / . , ′ |` | Hershey font licence (below) |
| Hershey fonts, Greek simplex (`greek.jhf`) | `α β θ λ μ π Δ Σ` | Hershey font licence |
| Hershey fonts, maths (`mathlow.jhf`) | `± ≠ ∞ ° · ÷` | Hershey font licence |
| Hand-authored (`templates.rs`) | one- and two-stroke `x` (`)(`), open `4`, `7` with bar, plain and based `1`, two-stroke `5`, straight-stem `9`, round-top `3`, straight `t`, curly and straight `y`, barred `z`, two-storey and one-stroke `a`, narrow `θ`, handwritten `λ`, three-stroke `π`, `α`, `[ ] ≤ ≥ ∫`, straight prime | AGPL-3.0-or-later, as the rest of the crate |
| Generated (`templates.rs`) | radicals with 8 overbar lengths (with and without lead tick, bar joined or separate), arrows of 4 lengths | AGPL-3.0-or-later |

The Hershey records are copied verbatim from the `.jhf` files of
<https://github.com/kamalmostafa/hershey-fonts> (glyph data only, not that
project's GPL code). The Hershey font licence allows any use and
redistribution provided this acknowledgement travels with the data: *The
Hershey Fonts were originally created by Dr. A. V. Hershey while working at
the U. S. National Bureau of Standards. The format of the Font data in this
distribution was originally created by James Hurt, Cognition, Inc., 900
Technology Park Drive, Billerica, MA 01821 (mit-eddie!ci-dandelion!hurt).*
It also forbids redistributing the data in the U.S. NTIS format, which we
don't. Hershey strokes that meet end to end (`v`, `z`, `Σ`, `M`…) are joined
into the single stroke a pen would draw. No handwriting dataset is bundled
(CROHME's licence is unclear); tests synthesise their data.

Template memory: 113 templates of 24 points plus a 1 KiB lookup table each,
about 140 KiB (`MathRecognizer::template_bytes()`); building them takes a
few milliseconds.

## Symbol set (v1)

- Digits `0–9`.
- Lowercase `a–z` (including `l` and `o`, needed for `log`, `cos` and the
  `1`/`l`, `0`/`o` decisions); uppercase `A B C E F H M N O P R S T V`
  (`O` and `S` added for chemistry: H₂O, CO₂, SO₄).
- Operators and relations `+ − × ÷ · ± / = ≠ < > ≤ ≥ →`, delimiters
  `( ) [ ] |`, punctuation and marks `. , ′ °`.
- `√` (with its extent), `∫`, `∞`, Greek `α β θ λ μ π Δ Σ`.
- Structures: superscripts, subscripts (nested), fractions (nested),
  radicals, functions `\sin \cos \tan \log \ln`, degrees, primes, decimal
  points, implicit multiplication (juxtaposition), chemistry
  (`2H_2+O_2\rightarrow 2H_2O`, output with upright element symbols:
  `2\mathrm{H}_2+\mathrm{O}_2\rightarrow 2\mathrm{H}_2\mathrm{O}`).

`supported_symbols()` lists them (text, LaTeX) for a calibration screen.

### Adding a symbol or a template

1. Add a row to `SYMBOLS` in `symbols.rs`: name (the Unicode text), LaTeX,
   profile (where it sits on the line), grammar category, stroke counts
   `(min, usual, max)`, crossing rule, rarity prior. If it looks exactly like
   an existing symbol, add both to `TWINS` instead of a template.
2. Give it templates: a Hershey record in `hershey.rs` (copy the record from
   the `.jhf` file verbatim) and/or a path in `AUTHORED` in `templates.rs`
   (font units: baseline y = 9, x-height −5, capitals −12, descender 16).
   Add a template for every common *shape*; stroke order and direction don't
   matter, stroke count matters only through the `(min, usual, max)` counts.
3. If it changes grammar (a new operator class), extend `trans()` in
   `context.rs`.
4. Add an expression using it to `SIMPLE` or `STRUCTURED` in `synth.rs` and
   run `math_eval`. `cargo test -p rupok-ink` checks that every template
   parses, that its stroke count fits the table and that it classifies as
   itself.

For a single user, `add_user_template` is enough: user templates count with
a 0.8 distance factor and widen the symbol's stroke counts.

## Measured accuracy

Measured with `cargo run -p rupok-ink --release --example math_eval` on a
synthetic test set: the 70 expressions of `synth::SIMPLE` (56, no fractions
or radicals) and `synth::STRUCTURED` (14), each rendered with 6 seeds, so
420 expressions. Rendering uses the built-in templates with random
perturbation: writing slant ±15°, per-glyph size ±10 % and rotation ±3°,
smooth elastic distortion (4 % of the glyph), point noise (σ = 1.5 % of the
x-height) and pen sampling, spacing and baseline wobble, stroke order and
direction shuffles, strokes joined without lifting the pen (25 %), and one
out-of-order stroke or exponent in 15 % of expressions. The *hard* set
doubles the distortion. Because the glyphs come from the templates, this
measures robustness to perturbation, not to unseen writing styles; expect
real handwriting to score lower until users add their own templates.

Release build, `cargo run -p rupok-ink --release --example math_eval`
(2026-11-23):

| | Standard | Hard |
|---|---|---|
| Expressions | 420 | 420 |
| **Expression top-1** | **96.9 %** (407) | 75.0 % (315) |
| – simple (no fractions or radicals, 336) | **96.4 %** | 72.9 % |
| – structured (fractions, radicals, 84) | 98.8 % | 83.3 % |
| Expression in the n-best list (top 5) | 98.3 % | 81.4 % |
| Symbols grouped exactly right | 99.6 % | 97.2 % |
| Symbols grouped and labelled right | 99.2 % | 91.8 % |

The brief's target, at least 85 % expression-level on simple expressions,
is met on the standard set (96.4 %). The 17 expressions named in the brief
(each rendered 6 times, 102 in all) score 97.1 %: all 6/6 except `2x+3=7`
(one `3` read as `5`), `CO_2` (`O₂` merged into `a`) and `x_{n+1}` (the `+`
of the subscript split), each 5/6. `cargo test -p rupok-ink` checks a
350-expression slice (≥ 90 % simple, ≥ 85 % structured) so regressions fail
CI.

**Confusions** (correctly grouped, wrongly labelled). Standard: `1→j`,
`7→1`, `E→6`, `f→t`, `z→2`, `Σ→2`, `Σ→F`, `∫→1`, once each; the other
standard failures are grouping errors (a symbol merged with its script, an
`m` split). Hard: `2→1` 7, `3→5` 7, `O→0` 7, `)→|` 6, `(→|` 4, `7→1` 3,
`8→6` 3, then `)→1`, `2→z`, `C→0`, `a→g`, `u→y`, `x→×`, `z→2`, `×→x` twice
each.

**Calibration** (share of top-1 answers that were right, by confidence):

| Confidence | Standard | Hard |
|---|---|---|
| 0.3–0.4 | 80 % (15) | 44 % (43) |
| 0.4–0.5 | 90 % (42) | 65 % (66) |
| 0.5–0.6 | 97 % (86) | 78 % (77) |
| 0.6–0.7 | 100 % (80) | 86 % (86) |
| 0.7–0.8 | 96 % (79) | 88 % (56) |
| 0.8–0.9 | 100 % (72) | 87 % (45) |
| 0.9–1.0 | 100 % (46) | 75 % (40) |

At `AUTO_CONVERT_CONFIDENCE` (0.85) and above: 100 % right on the standard
set, about 80 % on the hard set. Hard-set errors made with high confidence
are mostly shape confusions (`3`/`5`, `O`/`0`) where no close alternative
existed.

## Speed and size

Release build on a shared 12-core Linux machine under heavy load (load
average 20–40 from other builds), each sample the best of 5 runs:

| | p50 | p99 |
|---|---|---|
| `recognize`, 9–11 symbols (13.7 strokes) | 3.0 ms | 4.5 ms |
| Live preview: `push_stroke` + `candidates` per stroke | 0.8 ms | 3.7 ms |

Building the recognizer (templates and lookup tables) takes 8 ms; the
templates take about 141 KiB. Cost grows roughly linearly with strokes (each
stroke starts at most 4 candidate groups; each group is classified once and
cached for live preview). The same code is compiled to wasm32
(`MathInk` in `rupok-wasm`); expect 1.5–3× the native time in a browser,
still well inside one frame for live preview. Run the bench yourself with
`cargo run -p rupok-ink --release --example math_eval -- --speed`.

Where the time goes: classification is about 90 %. A candidate cloud costs
about 2 µs, ranking all 113 templates by their $Q bound about 12 µs, and the
greedy matching of the ~30 templates that survive the bound the rest
(roughly 150 µs per group).

## Limitations (honest list)

The synthetic numbers above are an upper bound: test glyphs come from the
same templates. These are the failure modes seen in the reports and by
construction:

- **Look-alikes without context.** `3`/`5`, `2`/`z`/`1`, `7`/`1`, `8`/`6`,
  `O`/`0`, `Σ`/`2`/`z`, `θ`/`B`, `f`/`t`, `E`/`6`, `∫`/`1`/`[`, `)`/`|` are the
  commonest confusions on messy ink. A lone symbol (nothing around it for
  the grammar) falls back to the shape match. `0` vs `O` needs a capital
  letter on the line (chemistry) to become `O`; `l` is only read inside
  `log`/`ln`; `|` (absolute value) is never preferred over `1`.
- **A script touching its base can merge with it** (`a₁` → `q`, `O₂` →
  `a`, `x₁` → `H`), and a symbol whose strokes don't touch where they
  should can split (an `m` whose arches separate). The n-best list usually
  has the right reading.
- **Scripts are judged by geometry only.** A superscript written at full
  size on the line reads as juxtaposition (`x2`), a barely lowered
  subscript as a normal symbol; close calls appear as the second candidate.
- **One line only.** No multi-line work, matrices, aligned equations or
  limits stacked above/below `Σ`/`∫` (they read as scripts). Fraction bars
  must be clearly wider than what they divide; a short bar is a minus.
  A radical's body is what lies under its overbar: a short overbar cuts
  the body off.
- **Slant is corrected, rotation is not.** A baseline tilted by more than a
  few degrees disturbs script detection.
- **Delayed strokes** are re-attached only when they land on their symbol
  (dots, crosses, the second stroke of `x`, `+`, `=`); finishing a symbol
  after writing others in between is not supported.
- **Symbols outside the v1 set** (other capitals, other Greek letters,
  `% ! : ; { }`, `∂ ∇ ≈ ≡ ∈`, Bangla digits) are read as the nearest known
  symbol.
- **Confidence is calibrated on synthetic ink.** On messy (hard) ink even
  confidence ≥ 0.9 was right only 75 % of the time; treat it as a ranking
  and a threshold for *suggesting*, never for replacing ink silently.
- **Not yet measured on real handwriting.** Before tuning further, collect a
  small labelled set of teacher ink (the handwriting-recognition plan's
  evaluation harness) and report on it next to the synthetic set.

## How the UI should use it

- **Preview, don't replace.** Show the top candidate typeset above the ink
  as the teacher writes (call `candidates()` after each stroke; it is cheap
  thanks to the cache). Never replace ink without an explicit tap.
- **Confidence threshold.** Use `AUTO_CONVERT_CONFIDENCE` (0.85) for
  anything automatic: below it, show the preview dimmed and keep the convert
  button a button. Above it, the preview may be shown at full strength, but
  conversion still needs the tap. On the synthetic sets top-1 accuracy rises
  with confidence (see the calibration table above), but on messy ink even
  ≥ 0.9 was right only 75 % of the time: confidence is a ranking signal, not
  a calibrated probability on real handwriting.
- **n-best.** Offer the other candidates in a small picker (long-press or a
  chevron on the preview). Alternatives differ by one symbol, one
  segmentation or one script decision, so they are exactly the corrections a
  teacher needs (`x^2` vs `x2`, `5` vs `S`, `0` vs `O`).
- **Show what was read.** Use `symbols[].bbox` and `symbols[].strokes` to
  tint the ink of a low-confidence symbol (`symbols[].confidence < 0.5`) and
  let a tap on it cycle that symbol's alternatives.
- **Undo restores ink.** Conversion must be one undoable step that removes
  the strokes and adds the maths item (see the handwriting-recognition plan:
  `Lesson::replace_items`). Undo brings back the exact original strokes.
- **Calibration.** A "teach my handwriting" screen can ask for each symbol a
  few times (`supported_symbols()`) and store the strokes; re-add them with
  `add_user_template` on start-up. Offer it when the teacher corrects the
  same symbol twice.
- **Fallback.** When the best confidence stays low or the teacher asks, send
  the ink to the AI fallback (`apps/board/src/handwriting.ts`).

## Board wiring

- **Live preview** (`apps/board/src/components/MathPreview.tsx`): with Smart
  shapes on and Settings → Pen & ink → Smart ink → **Maths** on (default), the
  pen strokes around the last one (`Board.math_cluster_json`: plain pen
  strokes of writing size, chained by closeness, written within two
  minutes) are read after every stroke. When the best reading looks like
  maths (`mathInk.ts` `looksLikeMath`: a digit, operator, script, fraction or
  root, two or more symbols) and its confidence is at least 0.55, it floats
  above the ink, typeset. One tap converts; the arrow shows up to three
  readings; × dismisses. The board never converts on its own, so the 0.85
  bar is never crossed silently.
- **Typeset maths items**: a text item with `latex: true` (spec,
  `kind: "text"`), shown with KaTeX from `public/rupoklib/katex`, edited as
  LaTeX source (double-click), resized like text, in the ink's colour.
  `Board.replace_selection_with_latex` makes one as a single undo step.
- **To text** (`ToText.tsx`) tries this recogniser first, offline: it
  converts maths read with confidence ≥ 0.7 (≥ 0.45 when no AI reader is set
  up); anything else goes to the AI reader, whose LaTeX now also becomes a
  typeset item.
- **PDF**: `export-pdf.ts` `rasterLatex` lays the maths out with KaTeX and
  draws each glyph run, rule and SVG part into the text mask, in the item's
  colour.
- **The teacher's handwriting**: one store for every feature,
  `rupok.handwriting.v1` (`glyphs()` in `apps/board/src/solve/state.ts`),
  written by the "Teach Rupok my handwriting" screen (Solve this). The
  recogniser learns every sample with `add_user_template`; Settings → Pen &
  ink → Smart ink opens the same screen.

## Writing, the other way round

`crates/rupok-ink/src/write/` turns text with inline LaTeX into handwriting
strokes for **Solve this** (`docs/ai-agents.md#solve-this`). It reuses this
module's Hershey parser and generator, and the same calibration: the board's
"Teach Rupok my handwriting" samples become both user templates here
(`add_user_template`) and the writer's glyphs (`GlyphSet::add_user`), in the
same writing units (baseline y = 0, x-height 14). Render a sample with
`cargo run -p rupok-ink --example write_svg -- out.svg "$x^2 = 4$"`.
