//! JavaScript binding for the offline handwritten-maths recognizer
//! ([`rupok_ink::math`]).
//!
//! Strokes cross the boundary as flat `Float32Array`s of `x, y` pairs in
//! world units; a batch of strokes is one flat array plus a `Uint32Array` of
//! point counts. Results are JSON:
//!
//! ```json
//! [{"latex":"x^2","confidence":0.93,
//!   "symbols":[{"symbol":"x","latex":"x","bbox":[x0,y0,x1,y1],"strokes":[0,1],"confidence":0.97}, …]}]
//! ```
//!
//! The UI must never convert silently below
//! [`rupok_ink::math::AUTO_CONVERT_CONFIDENCE`]; see
//! `docs/design/math-recognition.md`.

use rupok_ink::StrokePoint;
use rupok_ink::math::Candidate;

/// One stroke from flat `x, y` pairs (a trailing odd value is ignored).
pub fn stroke_from_xy(xy: &[f32]) -> Vec<StrokePoint> {
    xy.as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .map(|(i, p)| StrokePoint {
            x: p[0],
            y: p[1],
            p: 0.5,
            dt: (i as u32).saturating_mul(8),
        })
        .collect()
}

/// Strokes from flat pairs and per-stroke point counts. Counts that run
/// past the data are clipped.
pub fn strokes_from_xy(xy: &[f32], counts: &[u32]) -> Vec<Vec<StrokePoint>> {
    let mut out = Vec::with_capacity(counts.len());
    let mut at = 0usize;
    for &n in counts {
        let end = at.saturating_add(2 * n as usize).min(xy.len());
        out.push(stroke_from_xy(&xy[at..end]));
        at = end;
    }
    out
}

/// Candidates as the JSON documented above.
pub fn candidates_json(cands: &[Candidate]) -> String {
    let v: Vec<serde_json::Value> = cands
        .iter()
        .map(|c| {
            serde_json::json!({
                "latex": c.latex,
                "confidence": c.confidence,
                "symbols": c.symbols.iter().map(|s| serde_json::json!({
                    "symbol": s.symbol,
                    "latex": s.latex,
                    "bbox": s.bbox,
                    "strokes": s.strokes,
                    "confidence": s.confidence,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::Value::Array(v).to_string()
}

#[cfg(target_arch = "wasm32")]
mod web {
    use rupok_ink::math::{self, MathRecognizer};
    use wasm_bindgen::prelude::*;

    use super::{candidates_json, stroke_from_xy, strokes_from_xy};

    /// Live maths recognizer for Smart Ink: push strokes as they finish,
    /// read the n-best list, pop on undo.
    #[wasm_bindgen]
    pub struct MathInk {
        rec: MathRecognizer,
    }

    impl Default for MathInk {
        fn default() -> Self {
            Self::new()
        }
    }

    #[wasm_bindgen]
    impl MathInk {
        #[wasm_bindgen(constructor)]
        pub fn new() -> MathInk {
            MathInk {
                rec: MathRecognizer::new(),
            }
        }

        /// Add a finished stroke (`x, y` pairs); returns its id.
        pub fn push_stroke(&mut self, xy: &[f32]) -> u32 {
            self.rec.push_stroke(&stroke_from_xy(xy)) as u32
        }

        pub fn pop_stroke(&mut self) -> bool {
            self.rec.pop_stroke()
        }

        pub fn clear(&mut self) {
            self.rec.clear();
        }

        pub fn stroke_count(&self) -> u32 {
            self.rec.stroke_count() as u32
        }

        /// n-best JSON for the pushed strokes.
        pub fn candidates_json(&mut self) -> String {
            candidates_json(&self.rec.candidates())
        }

        /// n-best JSON for a batch of strokes (does not touch the live buffer).
        pub fn recognize_json(&self, xy: &[f32], counts: &[u32]) -> String {
            candidates_json(&self.rec.recognize(&strokes_from_xy(xy, counts)))
        }

        /// Teach the user's form of `symbol` ("x", "\\theta", "√"…).
        pub fn add_user_template(
            &mut self,
            symbol: &str,
            xy: &[f32],
            counts: &[u32],
        ) -> Result<(), JsValue> {
            self.rec
                .add_user_template(symbol, &strokes_from_xy(xy, counts))
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }

        /// Forget the user's templates for `symbol`, or all when absent.
        pub fn clear_user_templates(&mut self, symbol: Option<String>) -> Result<(), JsValue> {
            self.rec
                .clear_user_templates(symbol.as_deref())
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }

        /// Confidence below which the UI must not convert without a tap.
        pub fn auto_convert_confidence() -> f32 {
            math::AUTO_CONVERT_CONFIDENCE
        }

        /// Supported symbols as JSON `[[text, latex], …]`.
        pub fn symbols_json() -> String {
            let v: Vec<[&str; 2]> = math::supported_symbols().map(|(a, b)| [a, b]).collect();
            serde_json::to_string(&v).unwrap_or_else(|_| "[]".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_strokes_round_trip() {
        let s = strokes_from_xy(&[0.0, 0.0, 1.0, 1.0, 5.0, 5.0], &[2, 1, 9]);
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].len(), 2);
        assert_eq!(s[1].len(), 1);
        assert!(s[2].is_empty());
        assert_eq!(stroke_from_xy(&[1.0, 2.0, 3.0]).len(), 1);
    }

    #[test]
    fn json_shape() {
        let rec = rupok_ink::math::MathRecognizer::new();
        let one: Vec<f32> = (0..=10).flat_map(|i| [0.0, i as f32 * 4.0]).collect();
        let json = candidates_json(&rec.recognize(&strokes_from_xy(&one, &[11])));
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v[0]["latex"].is_string());
        assert!(v[0]["symbols"][0]["bbox"].is_array());
    }
}
