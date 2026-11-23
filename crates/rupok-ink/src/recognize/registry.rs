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

/// A small "+" is writing: strokes under this size, screen px, that cross a
/// recent stroke of the same size, one level and one upright, stay ink.
pub const SMALL_CROSS_PX: f32 = 110.0;
/// The two strokes of a small cross come within this many ms.
pub const SMALL_CROSS_WINDOW_MS: f64 = 4000.0;

/// Does `stroke` cross a recent small neighbour through both their middles,
/// one level and the other upright (a handwritten "+")? An arrow head's "V"
/// meets its line only at the line's end, so it never counts; a "×" (a
/// bulb's cross) is left to the recognisers. Returns the neighbour's index.
pub fn small_cross(stroke: &[P], ctx: &Context) -> Option<usize> {
    let px = ctx.opts.px();
    let extent = |b: [f32; 4]| (b[2] - b[0]).max(b[3] - b[1]) * px;
    if stroke.len() < 2 || extent(super::bbox(stroke)) >= SMALL_CROSS_PX {
        return None;
    }
    ctx.neighbours.iter().position(|n| {
        if !(0.0..=SMALL_CROSS_WINDOW_MS).contains(&n.gap_ms) || extent(n.bounds) >= SMALL_CROSS_PX
        {
            return false;
        }
        let seg;
        let other: &[P] = match &n.geometry {
            Some(Geometry::Segment { a, b }) => {
                seg = [*a, *b];
                &seg
            }
            Some(_) => return false,
            None => &n.points,
        };
        if other.len() < 2 {
            return false;
        }
        match (level_mid(other), level_mid(stroke)) {
            (Some(x), Some(y)) if x != y => crosses_mid(stroke, other),
            _ => false,
        }
    })
}

/// [`level`] of a polyline's middle (from 20 % to 80 % of its length), so a
/// hook at either end doesn't tilt it.
fn level_mid(p: &[P]) -> Option<bool> {
    let total = super::path_len(p);
    let at = |f: f32| {
        let want = f * total;
        let mut run = 0.0;
        for w in p.windows(2) {
            let l = super::dist(w[0], w[1]);
            if run + l >= want && l > f32::EPSILON {
                let t = (want - run) / l;
                return [
                    w[0][0] + t * (w[1][0] - w[0][0]),
                    w[0][1] + t * (w[1][1] - w[0][1]),
                ];
            }
            run += l;
        }
        p[p.len() - 1]
    };
    level(at(0.2), at(0.8))
}

/// Do polylines `p` and `q` cross somewhere in the middle 60 % of both?
fn crosses_mid(p: &[P], q: &[P]) -> bool {
    let (lp, lq) = (super::path_len(p), super::path_len(q));
    if lp <= f32::EPSILON || lq <= f32::EPSILON {
        return false;
    }
    let mid = |f: f32| (0.2..=0.8).contains(&f);
    let mut rp = 0.0;
    p.windows(2).any(|w| {
        let l = super::dist(w[0], w[1]);
        let mut rq = 0.0;
        let hit = q.windows(2).any(|v| {
            let m = super::dist(v[0], v[1]);
            let hit = crossing(w[0], w[1], v[0], v[1])
                .is_some_and(|(t, u)| mid((rp + t * l) / lp) && mid((rq + u * m) / lq));
            rq += m;
            hit
        });
        rp += l;
        hit
    })
}

/// `Some(true)` for a segment within 25° of level, `Some(false)` within 25°
/// of upright (handwriting slants), `None` for a diagonal.
fn level(a: P, b: P) -> Option<bool> {
    let th = (b[1] - a[1]).atan2(b[0] - a[0]).abs();
    let off = th.min(super::PI - th);
    let tol = 25.0 * super::DEG;
    if off <= tol {
        Some(true)
    } else if off >= super::PI / 2.0 - tol {
        Some(false)
    } else {
        None
    }
}

/// Where segments `a`–`b` and `c`–`d` cross: the parameters along each.
fn crossing(a: P, b: P, c: P, d: P) -> Option<(f32, f32)> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() <= f32::EPSILON {
        return None;
    }
    let q = [c[0] - a[0], c[1] - a[1]];
    let t = (q[0] * s[1] - q[1] * s[0]) / den;
    let u = (q[0] * r[1] - q[1] * r[0]) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
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
        if small_cross(&xy, ctx).is_some() {
            return None;
        }
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
