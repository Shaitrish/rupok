//! The recogniser registry. Smart Ink runs a list of recognisers over each
//! finished stroke, in order; the first proposal wins.
//!
//! A recogniser sees the stroke and its [`Context`]: nearby strokes and shapes
//! with their clean geometry and how long ago they were drawn. *Standalone*
//! recognisers look at the stroke alone (circle, triangle, …). *Contextual*
//! ones may also **absorb** neighbours into a composite: an arrow head drawn
//! right after its line today; axes from a "+", Venn diagrams, flowchart
//! connectors and circuit symbols later. Adding one is a new type that
//! implements [`Recognizer`] plus a [`Registry::push`]; the engine applies
//! every proposal the same way (one undo step, the first undo brings the
//! hand-drawn stroke back).

use super::{Geometry, Options, P, Recognized, ShapeKind, arrow_head, recognize_xy};
use crate::StrokePoint;

/// A stroke or shape near the new stroke (world units).
#[derive(Clone, Debug, PartialEq)]
pub struct Neighbour {
    /// The caller's id (an item id).
    pub id: String,
    /// Points with the stroke's offset applied.
    pub points: Vec<P>,
    /// Stored `shape` name, for shape strokes.
    pub shape: Option<String>,
    /// Clean geometry, for shape strokes that store it.
    pub geometry: Option<Geometry>,
    pub bounds: [f32; 4],
    /// Milliseconds from the neighbour's pen-up to the new stroke's pen-down
    /// (negative when it was drawn later, e.g. by another device).
    pub gap_ms: f64,
}

/// What a recogniser may look at besides the stroke.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub opts: Options,
    /// Nearby strokes, most recent first.
    pub neighbours: Vec<Neighbour>,
}

/// A recogniser's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub recognized: Recognized,
    /// Neighbours (indices into [`Context::neighbours`]) the result replaces.
    /// Empty: the result replaces the new stroke. Otherwise the result is
    /// drawn in place of the first absorbed neighbour (keeping its style and
    /// replay time) and the new stroke and the other absorbed ones go.
    pub absorbs: Vec<usize>,
    /// The recogniser's name.
    pub by: &'static str,
}

pub trait Recognizer: Send + Sync {
    fn name(&self) -> &'static str;
    /// True when it needs (and may absorb) neighbours. Contextual
    /// recognisers also run where standalone snapping is off (hold-to-snap
    /// mode on pen-up, handwriting in progress), because they only fire on
    /// an unmistakable pattern next to a shape the teacher already made.
    fn contextual(&self) -> bool {
        false
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal>;
}

/// The single-stroke shapes of [`recognize_xy`].
pub struct Shapes;

impl Recognizer for Shapes {
    fn name(&self) -> &'static str {
        "shapes"
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        Some(Proposal {
            recognized: recognize_xy(stroke, ctx.opts)?,
            absorbs: Vec::new(),
            by: self.name(),
        })
    }
}

/// A short "V" drawn within [`ArrowHead::WINDOW_MS`] of a line, at one of its
/// ends, turns the line into an arrow (see [`arrow_head`]).
pub struct ArrowHead;

impl ArrowHead {
    pub const WINDOW_MS: f64 = 1200.0;
}

impl Recognizer for ArrowHead {
    fn name(&self) -> &'static str {
        "arrow-head"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        let head: Vec<StrokePoint> = stroke
            .iter()
            .map(|q| StrokePoint {
                x: q[0],
                y: q[1],
                p: 0.5,
                dt: 0,
            })
            .collect();
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if n.shape.as_deref() != Some("line") || !(0.0..=Self::WINDOW_MS).contains(&n.gap_ms) {
                return None;
            }
            let Some(Geometry::Segment { a, b }) = n.geometry else {
                return None;
            };
            let at_b = arrow_head(a, b, &head, ctx.opts)?;
            let (tail, tip) = if at_b { (a, b) } else { (b, a) };
            Some(Proposal {
                recognized: Recognized {
                    kind: ShapeKind::Arrow,
                    geometry: Geometry::Segment { a: tail, b: tip },
                    confidence: 1.0,
                },
                absorbs: vec![i],
                by: self.name(),
            })
        })
    }
}

/// Recognisers in priority order.
pub struct Registry {
    list: Vec<Box<dyn Recognizer>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::standard()
    }
}

impl Registry {
    /// Composites first (they are the more specific reading), then shapes.
    pub fn standard() -> Self {
        Self {
            list: vec![Box::new(ArrowHead), Box::new(Shapes)],
        }
    }

    pub fn empty() -> Self {
        Self { list: Vec::new() }
    }

    /// Add a recogniser after the existing ones.
    pub fn push(&mut self, r: Box<dyn Recognizer>) {
        self.list.push(r);
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.list.iter().map(|r| r.name()).collect()
    }

    /// The first proposal, trying only contextual recognisers when
    /// `standalone` is false. Proposals that absorb a missing neighbour are
    /// dropped.
    pub fn recognize(
        &self,
        stroke: &[StrokePoint],
        ctx: &Context,
        standalone: bool,
    ) -> Option<Proposal> {
        let xy: Vec<P> = stroke.iter().map(|p| [p.x, p.y]).collect();
        self.list
            .iter()
            .filter(|r| standalone || r.contextual())
            .find_map(|r| r.recognize(&xy, ctx))
            .filter(|p| p.absorbs.iter().all(|&i| i < ctx.neighbours.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(p: &[P]) -> Vec<StrokePoint> {
        let mut out = Vec::new();
        for w in p.windows(2) {
            for k in 0..20 {
                let t = k as f32 / 20.0;
                out.push(StrokePoint {
                    x: w[0][0] + t * (w[1][0] - w[0][0]),
                    y: w[0][1] + t * (w[1][1] - w[0][1]),
                    p: 0.5,
                    dt: 0,
                });
            }
        }
        out
    }

    fn line_neighbour(gap_ms: f64) -> Neighbour {
        Neighbour {
            id: "line-1".into(),
            points: vec![[100.0, 300.0], [500.0, 300.0]],
            shape: Some("line".into()),
            geometry: Some(Geometry::Segment {
                a: [100.0, 300.0],
                b: [500.0, 300.0],
            }),
            bounds: [100.0, 298.0, 500.0, 302.0],
            gap_ms,
        }
    }

    #[test]
    fn a_v_next_to_a_recent_line_is_absorbed_into_an_arrow() {
        let reg = Registry::standard();
        assert_eq!(reg.names(), ["arrow-head", "shapes"]);
        let v = pts(&[[460.0, 265.0], [501.0, 300.0], [458.0, 337.0]]);
        let ctx = Context {
            opts: Options::default(),
            neighbours: vec![line_neighbour(300.0)],
        };
        let p = reg.recognize(&v, &ctx, false).expect("composite");
        assert_eq!((p.by, p.absorbs.as_slice()), ("arrow-head", &[0usize][..]));
        assert_eq!(
            p.recognized.geometry,
            Geometry::Segment {
                a: [100.0, 300.0],
                b: [500.0, 300.0]
            }
        );
        // Too long ago, or drawn later elsewhere: no composite.
        for gap in [5000.0, -10.0] {
            let ctx = Context {
                opts: Options::default(),
                neighbours: vec![line_neighbour(gap)],
            };
            assert_eq!(reg.recognize(&v, &ctx, true), None, "gap {gap}");
        }
    }

    #[test]
    fn standalone_shapes_run_only_when_allowed_and_custom_recognisers_plug_in() {
        let circle: Vec<StrokePoint> = (0..=120)
            .map(|i| {
                let a = i as f32 / 120.0 * std::f32::consts::TAU * 1.02;
                StrokePoint {
                    x: 400.0 + 150.0 * a.cos(),
                    y: 300.0 + 146.0 * a.sin(),
                    p: 0.5,
                    dt: 0,
                }
            })
            .collect();
        let ctx = Context::default();
        let reg = Registry::standard();
        assert_eq!(reg.recognize(&circle, &ctx, false), None);
        let p = reg.recognize(&circle, &ctx, true).unwrap();
        assert_eq!((p.by, p.recognized.kind), ("shapes", ShapeKind::Circle));

        struct Everything;
        impl Recognizer for Everything {
            fn name(&self) -> &'static str {
                "test"
            }
            fn contextual(&self) -> bool {
                true
            }
            fn recognize(&self, _: &[P], _: &Context) -> Option<Proposal> {
                Some(Proposal {
                    recognized: Recognized {
                        kind: ShapeKind::Line,
                        geometry: Geometry::Segment {
                            a: [0.0, 0.0],
                            b: [1.0, 1.0],
                        },
                        confidence: 1.0,
                    },
                    // A bad index is never applied.
                    absorbs: vec![3],
                    by: "test",
                })
            }
        }
        let mut reg = Registry::empty();
        reg.push(Box::new(Everything));
        assert_eq!(reg.recognize(&circle, &ctx, true), None);
    }
}
