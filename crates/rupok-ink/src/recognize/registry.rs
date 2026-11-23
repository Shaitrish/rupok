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

use super::{Geometry, Options, P, Recognized, ShapeKind, arrow_head, family, recognize_xy};
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

/// A text label that goes with a result (axis names, an equation, number
/// line values, an angle's degrees).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    /// Centre of the text, world units.
    pub at: P,
    /// Font size, world units.
    pub size: f32,
}

/// A recogniser's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub recognized: Recognized,
    /// Neighbours (indices into [`Context::neighbours`]) the result replaces.
    /// Empty: the result replaces the new stroke. Otherwise the result is
    /// drawn in place of the first absorbed neighbour (keeping its style and
    /// replay time) and the new stroke and the other absorbed ones go
    /// (unless [`Proposal::onto_stroke`]).
    pub absorbs: Vec<usize>,
    /// The recogniser's name.
    pub by: &'static str,
    /// The result replaces the new stroke even though it absorbs
    /// neighbours (which all go).
    pub onto_stroke: bool,
    /// More strokes in the result's style, grouped with it: parts one
    /// polyline can't draw (a cell's two plates, Venn circles).
    pub extra: Vec<Recognized>,
    /// Text labels grouped with the result.
    pub labels: Vec<Label>,
    /// Put the result in this neighbour's group (a curve joins its axes).
    pub join: Option<usize>,
    /// Remove the labels grouped with these absorbed neighbours (a result
    /// that redraws its labels).
    pub relabel: Vec<usize>,
}

impl Proposal {
    pub fn new(recognized: Recognized, absorbs: Vec<usize>, by: &'static str) -> Self {
        Self {
            recognized,
            absorbs,
            by,
            onto_stroke: false,
            extra: Vec::new(),
            labels: Vec::new(),
            join: None,
            relabel: Vec::new(),
        }
    }

    /// Every neighbour index the proposal refers to.
    fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.absorbs
            .iter()
            .chain(self.join.iter())
            .chain(self.relabel.iter())
            .copied()
    }
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
    /// True when it runs once the pen has rested (a number line is read
    /// when its last tick is drawn, not at the fourth).
    fn deferred(&self) -> bool {
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
        let mut r = recognize_xy(stroke, ctx.opts)?;
        if ctx.opts.has(family::GEOMETRY) {
            super::geometry::align_line(&mut r, ctx);
        }
        Some(Proposal::new(r, Vec::new(), self.name()))
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
            Some(Proposal::new(
                Recognized {
                    kind: ShapeKind::Arrow,
                    geometry: Geometry::Segment { a: tail, b: tip },
                    confidence: 1.0,
                },
                vec![i],
                self.name(),
            ))
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
    /// Composites first (they are the more specific reading), then the
    /// single-stroke templates, then shapes. Each Phase 2 recogniser checks
    /// its own family bit.
    pub fn standard() -> Self {
        use super::{diagram, geometry, graph, logic, science};
        Self {
            list: vec![
                Box::new(ArrowHead),
                Box::new(graph::CurveFit),
                Box::new(graph::AxesPlus),
                Box::new(logic::Gates),
                Box::new(geometry::RightAngleMark),
                Box::new(geometry::AngleMark),
                Box::new(diagram::Table),
                Box::new(diagram::Venn),
                Box::new(science::Bulb),
                Box::new(science::Cell),
                Box::new(science::Switch),
                Box::new(science::Benzene),
                Box::new(science::Equilibrium),
                Box::new(diagram::NumberLine),
                Box::new(graph::AxesL),
                Box::new(science::Resistor),
                Box::new(Shapes),
            ],
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
    /// `standalone` is false. Deferred recognisers don't run here (see
    /// [`Registry::recognize_deferred`]). Proposals that refer to a missing
    /// neighbour are dropped.
    pub fn recognize(
        &self,
        stroke: &[StrokePoint],
        ctx: &Context,
        standalone: bool,
    ) -> Option<Proposal> {
        let xy: Vec<P> = stroke.iter().map(|p| [p.x, p.y]).collect();
        self.list
            .iter()
            .filter(|r| !r.deferred() && (standalone || r.contextual()))
            .find_map(|r| r.recognize(&xy, ctx))
            .filter(|p| p.indices().all(|i| i < ctx.neighbours.len()))
    }

    /// True when some deferred recogniser is enabled: the engine then reads
    /// the last stroke again once the pen rests.
    pub fn has_deferred(&self, opts: &Options) -> bool {
        self.list.iter().any(|r| r.deferred()) && opts.has(family::NUMBER_LINE)
    }

    /// The first proposal of the deferred recognisers for a stroke that is
    /// already on the board (not among `ctx.neighbours`).
    pub fn recognize_deferred(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        self.list
            .iter()
            .filter(|r| r.deferred())
            .find_map(|r| r.recognize(stroke, ctx))
            .filter(|p| p.indices().all(|i| i < ctx.neighbours.len()))
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
        assert_eq!(reg.names()[0], "arrow-head");
        assert_eq!(*reg.names().last().unwrap(), "shapes");
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
                Some(Proposal::new(
                    Recognized {
                        kind: ShapeKind::Line,
                        geometry: Geometry::Segment {
                            a: [0.0, 0.0],
                            b: [1.0, 1.0],
                        },
                        confidence: 1.0,
                    },
                    // A bad index is never applied.
                    vec![3],
                    "test",
                ))
            }
        }
        let mut reg = Registry::empty();
        reg.push(Box::new(Everything));
        assert_eq!(reg.recognize(&circle, &ctx, true), None);
    }
}
