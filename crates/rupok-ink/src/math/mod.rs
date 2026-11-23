//! Offline handwritten maths → LaTeX.
//!
//! A teacher writes `x²` or `a² + b² = c²` on the board; with Smart Ink on,
//! the UI shows a typeset preview above the ink and converts it on a tap.
//! Everything here is classic geometry and dynamic programming: no model
//! download, no network, no clock, a few milliseconds per expression, and it
//! builds for `wasm32-unknown-unknown`.
//!
//! # Pipeline
//!
//! 1. **Grouping** ([`segment`]): strokes are deslanted, delayed strokes
//!    (an `i` dot added later, a late `t` cross) are moved next to their
//!    symbol, and a k-best dynamic programme over the time order splits the
//!    strokes into symbols of 1–4 strokes.
//! 2. **Classification** ([`classify`]): each candidate group is matched
//!    against templates with the $Q point-cloud recognizer (Vatavu et al.,
//!    2018), which ignores stroke order, direction and count but not
//!    rotation.
//! 3. **Layout** ([`layout`]): radicals, fraction bars, then a baseline walk
//!    that attaches superscripts and subscripts by relative position and
//!    size.
//! 4. **Context** ([`context`]): Viterbi over each row with a small grammar
//!    plus size and baseline priors picks between look-alikes (`x`/`×`,
//!    `0`/`o`/`O`, `1`/`l`/`|`, `2`/`z`, `5`/`s`, `.`/`·`, `c`/`C`), and fixed
//!    rules handle degrees, primes and `\sin`-style function names.
//! 5. **LaTeX** ([`tree`]) that KaTeX renders, with an n-best list and a
//!    confidence for each candidate.
//!
//! # Templates and their licence
//!
//! Built-in templates come from the Hershey vector fonts (Roman simplex,
//! Greek simplex and the maths symbol set) plus hand-authored variants for
//! common handwritten forms (one- and two-stroke `x`, open `4`, barred `7`,
//! straight `t`, curly `y`, barred `z`, two-storey `a`, …) and generated
//! radicals and arrows of several lengths. The Hershey glyph data may be
//! used for any purpose with this acknowledgement: *The Hershey Fonts were
//! originally created by Dr. A. V. Hershey while working at the U. S.
//! National Bureau of Standards. The format of the Font data in this
//! distribution was originally created by James Hurt, Cognition, Inc., 900
//! Technology Park Drive, Billerica, MA 01821 (mit-eddie!ci-dandelion!hurt).*
//! See [`hershey`] for details. No handwriting dataset is bundled.
//!
//! # Use
//!
//! ```
//! use rupok_ink::math::{MathRecognizer, synth::{Synth, SynthStyle}};
//!
//! let rec = MathRecognizer::new();
//! // Synthetic ink for "x^2" (a real caller passes the board's strokes).
//! let sample = Synth::new().render("x^2", 7, &SynthStyle::clean()).unwrap();
//! let best = &rec.recognize(&sample.strokes)[0];
//! assert_eq!(best.latex, "x^2");
//! ```
//!
//! For live preview, feed strokes as they finish with
//! [`MathRecognizer::push_stroke`] and call [`MathRecognizer::candidates`];
//! classification of unchanged stroke groups is cached.
//!
//! See `docs/design/math-recognition.md` for accuracy figures, limitations
//! and how the UI should use confidence.

pub mod classify;
pub mod context;
pub mod geom;
pub mod hershey;
pub mod layout;
mod pipeline;
pub mod segment;
pub mod symbols;
pub mod synth;
pub mod templates;
pub mod tree;

#[cfg(test)]
mod tests;

use crate::{Aabb, StrokePoint};
use classify::Library;
use geom::Pt;
use segment::GroupCache;
pub use symbols::SymId;

/// Below this confidence the UI should show the preview but never convert
/// without the teacher's tap (see the design doc for the calibration).
pub const AUTO_CONVERT_CONFIDENCE: f32 = 0.85;

/// Strokes beyond this many are ignored (one expression, not a page).
pub const MAX_STROKES: usize = 160;

/// One reading of the ink.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// LaTeX that KaTeX can render, e.g. `a^2+b^2=c^2`.
    pub latex: String,
    /// 0..=1: how sure the recognizer is that this is what was written.
    pub confidence: f32,
    /// Every symbol read, in reading order, with its ink.
    pub symbols: Vec<RecognizedSymbol>,
}

/// One symbol of a [`Candidate`].
#[derive(Clone, Debug, PartialEq)]
pub struct RecognizedSymbol {
    /// The symbol as text (`"x"`, `"×"`, `"√"`; `"─"` for a fraction bar).
    pub symbol: &'static str,
    /// Its LaTeX (`"\\times"`, `"\\frac"` for a bar).
    pub latex: &'static str,
    /// Box of its ink, in the input coordinates.
    pub bbox: Aabb,
    /// Ids (indices into the input) of its strokes, ascending.
    pub strokes: Vec<usize>,
    /// 0..=1.
    pub confidence: f32,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MathError {
    #[error("unknown maths symbol {0:?}")]
    UnknownSymbol(String),
    #[error("a template needs at least one stroke with points")]
    EmptyTemplate,
    #[error("a template has at most {max} strokes", max = segment::MAX_GROUP)]
    TooManyStrokes,
}

/// Every supported symbol as (text, LaTeX), e.g. for a calibration screen.
pub fn supported_symbols() -> impl Iterator<Item = (&'static str, &'static str)> {
    symbols::SYMBOLS.iter().map(|s| (s.name, s.latex))
}

fn to_pts(points: &[StrokePoint]) -> Vec<Pt> {
    points
        .iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| Pt::new(p.x, p.y))
        .collect()
}

/// Handwritten maths recognizer: built-in templates, the user's own
/// templates, and an incremental stroke buffer for live preview.
#[derive(Clone)]
pub struct MathRecognizer {
    lib: Library,
    strokes: Vec<Vec<Pt>>,
    n_best: usize,
    cache: GroupCache,
}

impl Default for MathRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MathRecognizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MathRecognizer")
            .field("templates", &self.lib.len())
            .field("user_templates", &self.lib.user_count())
            .field("strokes", &self.strokes.len())
            .finish()
    }
}

impl MathRecognizer {
    /// A recognizer with the built-in templates (a few milliseconds to
    /// build; build it once).
    pub fn new() -> Self {
        MathRecognizer {
            lib: Library::new(&templates::builtin_glyphs()),
            strokes: Vec::new(),
            n_best: 5,
            cache: Default::default(),
        }
    }

    /// Length of the n-best list (default 5).
    pub fn set_n_best(&mut self, n: usize) {
        self.n_best = n.max(1);
    }

    /// Teach the recognizer how this user writes `symbol` (its text, e.g.
    /// `"x"`, or LaTeX, e.g. `"\\theta"`). User templates win slightly over
    /// built-in ones; at most 16 are kept per symbol (oldest dropped). The
    /// caller stores the strokes and adds them again on start-up.
    pub fn add_user_template(
        &mut self,
        symbol: &str,
        strokes: &[Vec<StrokePoint>],
    ) -> Result<(), MathError> {
        let id = symbols::find(symbol).ok_or_else(|| MathError::UnknownSymbol(symbol.into()))?;
        let pts: Vec<Vec<Pt>> = strokes
            .iter()
            .map(|s| to_pts(s))
            .filter(|s| !s.is_empty())
            .collect();
        if pts.is_empty() {
            return Err(MathError::EmptyTemplate);
        }
        if pts.len() > segment::MAX_GROUP {
            return Err(MathError::TooManyStrokes);
        }
        let refs: Vec<&[Pt]> = pts.iter().map(|s| s.as_slice()).collect();
        if !self.lib.add_user(id, &refs) {
            return Err(MathError::EmptyTemplate);
        }
        self.reset_cache();
        Ok(())
    }

    /// Forget the user's templates for one symbol, or all of them.
    pub fn clear_user_templates(&mut self, symbol: Option<&str>) -> Result<(), MathError> {
        let id = match symbol {
            Some(s) => Some(symbols::find(s).ok_or_else(|| MathError::UnknownSymbol(s.into()))?),
            None => None,
        };
        self.lib.clear_user(id);
        self.reset_cache();
        Ok(())
    }

    pub fn user_template_count(&self) -> usize {
        self.lib.user_count()
    }

    pub fn template_count(&self) -> usize {
        self.lib.len()
    }

    /// Approximate memory used by the templates, in bytes.
    pub fn template_bytes(&self) -> usize {
        self.lib.heap_bytes()
    }

    fn reset_cache(&mut self) {
        self.cache.clear();
    }

    /// Recognise a whole expression. Strokes are in time order; a stroke's
    /// index is its id in the result. Best candidate first; empty when there
    /// is no ink.
    pub fn recognize(&self, strokes: &[Vec<StrokePoint>]) -> Vec<Candidate> {
        let pts: Vec<Vec<Pt>> = strokes
            .iter()
            .take(MAX_STROKES)
            .map(|s| to_pts(s))
            .collect();
        pipeline::run(&pts, &self.lib, None, self.n_best)
    }

    /// Add a finished stroke to the live buffer; returns its id.
    pub fn push_stroke(&mut self, points: &[StrokePoint]) -> usize {
        self.strokes.push(to_pts(points));
        self.strokes.len() - 1
    }

    /// Remove the last stroke (undo); false when the buffer is empty.
    pub fn pop_stroke(&mut self) -> bool {
        match self.strokes.pop() {
            Some(_) => {
                self.cache.forget_stroke(self.strokes.len());
                true
            }
            None => false,
        }
    }

    /// Empty the live buffer.
    pub fn clear(&mut self) {
        self.strokes.clear();
        self.cache.clear();
    }

    pub fn stroke_count(&self) -> usize {
        self.strokes.len()
    }

    /// Candidates for the live buffer. Groups already classified on an
    /// earlier call are not classified again.
    pub fn candidates(&mut self) -> Vec<Candidate> {
        let n = self.strokes.len().min(MAX_STROKES);
        pipeline::run(
            &self.strokes[..n],
            &self.lib,
            Some(&mut self.cache),
            self.n_best,
        )
    }
}
