//! Smart Ink shape recognition: turn a finished hand-drawn stroke into a
//! clean shape (line, arrow, circle, ellipse, rectangle, square, triangle,
//! diamond, pentagon, hexagon or five-point star), or into nothing.
//!
//! Phase 2 adds *templates* for science, maths and ICT classes (axes and
//! fitted graphs, flowcharts, logic gates, number lines, angle marks,
//! tables, Venn diagrams, circuit symbols, benzene, ⇌), most of them
//! *contextual*: they look at the strokes around the new one (see
//! [`registry`]). Each family can be switched off ([`family`]).
//!
//! Pure geometry, no model and no clock. The pipeline:
//!
//! 1. **Size gate** in screen pixels: handwriting is small, shapes are big.
//! 2. **Resample** by arc length, so speed and sampling rate don't matter.
//! 3. **Closure**: the best meeting point of the stroke's ends (overshoot and
//!    small gaps are both normal when drawing a closed shape).
//! 4. **Turning**: a simple closed shape turns once (±360°), a pentagram
//!    twice; loops, figure-eights and letters like `৪` don't.
//! 5. **Corners** from the turning angle over a short window (≈ 3 % of the
//!    perimeter), with non-maximum suppression.
//! 6. **Fit** a polygon (lines through each edge, vertices at their
//!    intersections) or an ellipse (least squares on the principal axes) and
//!    check residuals *relative to the shape's size*.
//! 7. **Regularise** what is close: a circle when the axes are within 12 %,
//!    an equilateral / right / isosceles triangle, a square, axis-aligned
//!    rectangles and lines snapped to 0°/45°/90°.
//!
//! False positives are the worst failure (a teacher's `o`, `0`, `D` or `ব`
//! must never turn into a shape), so every stage errs towards `None`.
//! Thresholds are documented constants; `tests` holds the synthetic shapes
//! and letters they were tuned against.

use crate::StrokePoint;

type P = [f32; 2];

/// Smart Ink families (Settings → Smart ink), as bits of [`Options::families`].
pub mod family {
    /// Axes from a "+" or an "L", curves fitted over them.
    pub const GRAPHS: u32 = 1;
    /// Terminator and input/output shapes, sticky connectors.
    pub const FLOWCHART: u32 = 1 << 1;
    /// AND, OR, NOT, NAND, NOR, XOR, XNOR.
    pub const GATES: u32 = 1 << 2;
    pub const NUMBER_LINE: u32 = 1 << 3;
    /// Angle marks, right-angle marks, parallel / perpendicular lines.
    pub const GEOMETRY: u32 = 1 << 4;
    /// Tables and Venn diagrams.
    pub const TABLES: u32 = 1 << 5;
    /// Circuit symbols, benzene, ⇌.
    pub const SCIENCE: u32 = 1 << 6;
    /// Every family.
    pub const ALL: u32 = (1 << 7) - 1;
    /// On by default: everything but circuits & chemistry, whose symbols
    /// (a zig-zag, a long/short line pair) are the most likely to be meant
    /// as something else in other subjects.
    pub const DEFAULT: u32 = ALL & !SCIENCE;
    /// Names used by settings and the board, in Settings order.
    pub const NAMES: [(&str, u32); 7] = [
        ("graphs", GRAPHS),
        ("flowchart", FLOWCHART),
        ("gates", GATES),
        ("numberline", NUMBER_LINE),
        ("geometry", GEOMETRY),
        ("tables", TABLES),
        ("science", SCIENCE),
    ];

    /// Bits from a comma-separated list of names; unknown names are ignored.
    pub fn parse(list: &str) -> u32 {
        list.split(',')
            .filter_map(|n| NAMES.iter().find(|(k, _)| *k == n.trim()).map(|(_, b)| *b))
            .fold(0, |a, b| a | b)
    }
}

/// Smallest closed shape that snaps on pen-up: its larger side, screen px.
/// Board handwriting is about 40–90 px tall at 1080p.
pub const MIN_CLOSED_PX: f32 = 100.0;
/// Smallest closed shape's smaller side, screen px (a thin loop is a letter).
pub const MIN_CLOSED_THIN_PX: f32 = 28.0;
/// Shortest line or arrow shaft that snaps on pen-up, screen px.
pub const MIN_LINE_PX: f32 = 90.0;
/// Minimum size with hold-to-snap, where the teacher asked explicitly.
pub const MIN_DELIBERATE_PX: f32 = 36.0;
/// A line within this many degrees of 0°/45°/90°/… snaps to it.
pub const LINE_SNAP_DEG: f32 = 7.0;
/// A rectangle within this many degrees of axis-aligned becomes axis-aligned.
pub const RECT_SNAP_DEG: f32 = 8.0;
/// Axis ratio (minor / major) above which an ellipse becomes a circle.
pub const CIRCLE_RATIO: f32 = 0.88;

/// Samples a closed stroke is resampled to.
const N_CLOSED: usize = 128;
/// Samples an open stroke is resampled to.
const N_OPEN: usize = 96;
/// Corner window in samples (≈ 3 % of a closed perimeter).
const CORNER_WINDOW: usize = 4;
/// Turning over the corner window that counts as a corner. A circle turns
/// about 22° over the same window, a hexagon corner 60°.
const CORNER_TURN: f32 = 44.0 * DEG;
/// Largest end gap (fraction of the bounding-box diagonal) for a closed shape.
const CLOSE_GAP: f32 = 0.25;
/// Edge straightness: largest deviation from the fitted line / edge length.
const EDGE_MAX_DEV: f32 = 0.085;
/// Edge straightness: RMS deviation / edge length.
const EDGE_RMS_DEV: f32 = 0.032;
/// Polygon fit: mean distance of the stroke from the polygon / diagonal.
const POLY_MEAN_DEV: f32 = 0.032;
/// Ellipse fit: mean normalised radial error.
const ELLIPSE_MEAN_DEV: f32 = 0.065;
/// Ellipse fit: largest normalised radial error.
const ELLIPSE_MAX_DEV: f32 = 0.2;
/// Line straightness: largest deviation from the chord / length.
const LINE_MAX_DEV: f32 = 0.05;

const DEG: f32 = std::f32::consts::PI / 180.0;
const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;

/// What a stroke was recognised as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Arrow,
    Circle,
    Ellipse,
    Rectangle,
    Square,
    Triangle,
    Diamond,
    Pentagon,
    Hexagon,
    Star,
    // ---- Phase 2 templates
    Axes,
    GraphLine,
    GraphQuad,
    GraphSine,
    GraphExp,
    NumberLine,
    Terminator,
    Parallelogram,
    AndGate,
    OrGate,
    NotGate,
    NandGate,
    NorGate,
    XorGate,
    XnorGate,
    Resistor,
    Cell,
    Bulb,
    Switch,
    Benzene,
    Equilibrium,
    Table,
    Venn,
    Angle,
    RightAngle,
}

impl ShapeKind {
    /// Stored `shape` name (see `spec/rupok-format.md`).
    pub fn as_str(self) -> &'static str {
        match self {
            ShapeKind::Axes => "axes",
            ShapeKind::GraphLine => "graph-line",
            ShapeKind::GraphQuad => "graph-quad",
            ShapeKind::GraphSine => "graph-sine",
            ShapeKind::GraphExp => "graph-exp",
            ShapeKind::NumberLine => "numberline",
            ShapeKind::Terminator => "terminator",
            ShapeKind::Parallelogram => "parallelogram",
            ShapeKind::AndGate => "and-gate",
            ShapeKind::OrGate => "or-gate",
            ShapeKind::NotGate => "not-gate",
            ShapeKind::NandGate => "nand-gate",
            ShapeKind::NorGate => "nor-gate",
            ShapeKind::XorGate => "xor-gate",
            ShapeKind::XnorGate => "xnor-gate",
            ShapeKind::Resistor => "resistor",
            ShapeKind::Cell => "cell",
            ShapeKind::Bulb => "bulb",
            ShapeKind::Switch => "switch",
            ShapeKind::Benzene => "benzene",
            ShapeKind::Equilibrium => "equilibrium",
            ShapeKind::Table => "table",
            ShapeKind::Venn => "venn",
            ShapeKind::Angle => "angle",
            ShapeKind::RightAngle => "right-angle",
            ShapeKind::Line => "line",
            ShapeKind::Arrow => "arrow",
            ShapeKind::Circle => "circle",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Rectangle => "rect",
            ShapeKind::Square => "square",
            ShapeKind::Triangle => "triangle",
            ShapeKind::Diamond => "diamond",
            ShapeKind::Pentagon => "pentagon",
            ShapeKind::Hexagon => "hexagon",
            ShapeKind::Star => "star",
        }
    }
}

/// Clean geometry in world units.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    /// A segment; for arrows `b` is the tip.
    Segment { a: P, b: P },
    /// Ellipse centred at `c` with semi-axes `rx` (along `rot`) and `ry`.
    Ellipse { c: P, rx: f32, ry: f32, rot: f32 },
    /// Rectangle centred at `c`, `w` along `rot`, `h` across it.
    Rect { c: P, w: f32, h: f32, rot: f32 },
    /// Closed polygon through these vertices, in drawing order (a pentagram
    /// lists its tips in star order).
    Polygon { pts: Vec<P> },
    /// A template (stored `geom` kind `kind`): the unrotated box (`x`, `y`
    /// top-left, `w`, `h`) turned by `rot` about its centre, and the kind's
    /// numbers `v` (see `spec/rupok-format.md`, Shape geometry).
    Template {
        kind: &'static str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rot: f32,
        v: Vec<f32>,
    },
}

impl Geometry {
    /// A template from its centre, size and rotation.
    pub fn template(kind: &'static str, c: P, w: f32, h: f32, rot: f32, v: Vec<f32>) -> Self {
        Geometry::Template {
            kind,
            x: c[0] - w / 2.0,
            y: c[1] - h / 2.0,
            w,
            h,
            rot,
            v,
        }
    }

    /// An open polyline template (`path` smooth, `polyline` straight)
    /// through absolute points: `v` holds them normalised to the box.
    pub fn open_path(kind: &'static str, pts: &[P]) -> Self {
        let b = bbox(pts);
        let (w, h) = ((b[2] - b[0]).max(1e-3), (b[3] - b[1]).max(1e-3));
        Geometry::Template {
            kind,
            x: b[0],
            y: b[1],
            w: b[2] - b[0],
            h: b[3] - b[1],
            rot: 0.0,
            v: pts
                .iter()
                .flat_map(|p| [(p[0] - b[0]) / w, (p[1] - b[1]) / h])
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recognized {
    pub kind: ShapeKind,
    pub geometry: Geometry,
    /// 0..=1: how cleanly the stroke fitted (1 = perfect).
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// Screen pixels per world unit (the camera zoom): size gates are in
    /// screen pixels, so writing small while zoomed in still isn't a shape.
    pub px_per_unit: f32,
    /// Hold-to-snap: the teacher asked for a shape, so smaller ones count.
    pub deliberate: bool,
    /// Enabled [`family`] bits.
    pub families: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            px_per_unit: 1.0,
            deliberate: false,
            families: family::DEFAULT,
        }
    }
}

impl Options {
    /// True when family `f` is on.
    pub fn has(&self, f: u32) -> bool {
        self.families & f != 0
    }
    fn px(&self) -> f32 {
        self.px_per_unit.max(1e-4)
    }
    fn min_closed(&self) -> f32 {
        if self.deliberate {
            MIN_DELIBERATE_PX
        } else {
            MIN_CLOSED_PX
        }
    }
    fn min_thin(&self) -> f32 {
        if self.deliberate {
            MIN_DELIBERATE_PX * 0.4
        } else {
            MIN_CLOSED_THIN_PX
        }
    }
    fn min_line(&self) -> f32 {
        if self.deliberate {
            MIN_DELIBERATE_PX
        } else {
            MIN_LINE_PX
        }
    }
}

/// Recognise a finished stroke. `None` means "leave the ink as drawn".
pub fn recognize(points: &[StrokePoint], opts: Options) -> Option<Recognized> {
    let xy: Vec<P> = points.iter().map(|p| [p.x, p.y]).collect();
    recognize_xy(&xy, opts)
}

/// [`recognize`] on bare coordinates.
pub fn recognize_xy(points: &[P], opts: Options) -> Option<Recognized> {
    // Stored strokes are simplified (a straight line keeps 2–3 points);
    // resampling restores an even density, so few points are fine.
    if points.len() < 2
        || points
            .iter()
            .any(|p| !(p[0].is_finite() && p[1].is_finite()))
    {
        return None;
    }
    let px = opts.px();
    let b = bbox(points);
    let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
    let diag = bw.hypot(bh);
    if diag * px < opts.min_line().min(opts.min_closed()) {
        return None;
    }
    let len = path_len(points);
    if len <= f32::EPSILON || len / diag > 12.0 {
        // A dense scribble, never a shape.
        return None;
    }
    let p = resample(points, N_CLOSED);
    if let Some(loop_pts) = close_loop(&p, opts) {
        return closed_shape(&loop_pts, opts);
    }
    open_shape(&resample(points, N_OPEN), opts)
}

// ------------------------------------------------------------------ open

fn open_shape(p: &[P], opts: Options) -> Option<Recognized> {
    line(p, opts).or_else(|| arrow(p, opts))
}

fn line(p: &[P], opts: Options) -> Option<Recognized> {
    let (a, b) = (p[0], p[p.len() - 1]);
    let l = dist(a, b);
    if l * opts.px() < opts.min_line() || path_len(p) / l > 1.12 {
        return None;
    }
    let dev = max_dev(p, a, b) / l;
    if dev > LINE_MAX_DEV {
        return None;
    }
    let (a, b) = snap_segment(a, b);
    Some(Recognized {
        kind: ShapeKind::Line,
        geometry: Geometry::Segment { a, b },
        confidence: (1.0 - dev / LINE_MAX_DEV).clamp(0.0, 1.0),
    })
}

/// One-stroke arrow: a straight shaft with a head (two wings, or a closed
/// triangle) drawn at one end before or after the shaft.
fn arrow(p: &[P], opts: Options) -> Option<Recognized> {
    let c = corners(p, 3, 50.0 * DEG, false);
    if c.is_empty() {
        return None;
    }
    let total = path_len(p);
    let mut best: Option<(f32, P, P)> = None;
    // Shaft first (tail → tip, then the head) or head first (head, then tip → tail).
    for (shaft, head, tail, tip) in [
        (&p[..=c[0]], &p[c[0]..], p[0], p[c[0]]),
        (
            &p[c[c.len() - 1]..],
            &p[..=c[c.len() - 1]],
            p[p.len() - 1],
            p[c[c.len() - 1]],
        ),
    ] {
        if let Some(score) = arrow_fit(shaft, head, tail, tip, total, opts)
            && best.is_none_or(|(s, _, _)| score > s)
        {
            best = Some((score, tail, tip));
        }
    }
    let (score, tail, tip) = best?;
    let (a, b) = snap_segment(tail, tip);
    Some(Recognized {
        kind: ShapeKind::Arrow,
        geometry: Geometry::Segment { a, b },
        confidence: score,
    })
}

fn arrow_fit(shaft: &[P], head: &[P], tail: P, tip: P, total: f32, opts: Options) -> Option<f32> {
    let l = dist(tail, tip);
    if l * opts.px() < opts.min_line() {
        return None;
    }
    let shaft_len = path_len(shaft);
    if shaft_len < 0.4 * total || shaft_len > 0.86 * total || shaft_len / l > 1.1 {
        return None;
    }
    let dev = max_dev(shaft, tail, tip) / l;
    if dev > 0.06 {
        return None;
    }
    let u = [(tip[0] - tail[0]) / l, (tip[1] - tail[1]) / l];
    let (mut left, mut right, mut back) = (0.0f32, 0.0f32, 0.0f32);
    for q in head {
        let d = [q[0] - tip[0], q[1] - tip[1]];
        let along = d[0] * u[0] + d[1] * u[1];
        let across = d[0] * -u[1] + d[1] * u[0];
        if dist(*q, tip) > 0.55 * l || along > 0.12 * l {
            return None;
        }
        back = back.min(along);
        left = left.max(across);
        right = right.max(-across);
    }
    // Wings on both sides of the shaft, reaching back from the tip.
    if left < 0.05 * l || right < 0.05 * l || back > -0.05 * l {
        return None;
    }
    let balance = left.min(right) / left.max(right);
    Some((0.5 + 0.5 * balance) * (1.0 - dev / 0.06).clamp(0.2, 1.0))
}

/// Two-stroke arrow: is `head` a short "V" whose apex sits at one end of the
/// line `a`–`b` and whose arms open back along it? Returns the end it marks:
/// `false` for `a`, `true` for `b`.
pub fn arrow_head(a: P, b: P, head: &[StrokePoint], opts: Options) -> Option<bool> {
    let xy: Vec<P> = head.iter().map(|p| [p.x, p.y]).collect();
    if xy.len() < 3 || xy.iter().any(|p| !(p[0].is_finite() && p[1].is_finite())) {
        return None;
    }
    let line_len = dist(a, b);
    if line_len <= f32::EPSILON {
        return None;
    }
    let p = resample(&xy, 48);
    // The apex: the sharpest turn, well inside the stroke.
    let (mut apex, mut turn) = (0usize, 0.0f32);
    for i in 4..p.len() - 4 {
        let t = turning(&p, i, 4, false).abs();
        if t > turn {
            turn = t;
            apex = i;
        }
    }
    if turn < 50.0 * DEG {
        return None;
    }
    let (e1, e2, v) = (p[0], p[p.len() - 1], p[apex]);
    let (l1, l2) = (dist(e1, v), dist(e2, v));
    let arm = l1.max(l2);
    if l1.min(l2) / arm < 0.35 || arm > 0.6 * line_len || arm * opts.px() < 8.0 {
        return None;
    }
    // Arms roughly straight.
    if max_dev(&p[..=apex], e1, v) > 0.18 * l1 || max_dev(&p[apex..], v, e2) > 0.18 * l2 {
        return None;
    }
    let ang = angle_between([e1[0] - v[0], e1[1] - v[1]], [e2[0] - v[0], e2[1] - v[1]]);
    if !(25.0 * DEG..=150.0 * DEG).contains(&ang) {
        return None;
    }
    for (end, other, at_b) in [(b, a, true), (a, b, false)] {
        let near = (0.3 * arm).max(0.08 * line_len).max(12.0 / opts.px());
        if dist(v, end) > near {
            continue;
        }
        // The arms' bisector points back along the line.
        let back = norm([other[0] - end[0], other[1] - end[1]]);
        let bis = norm([
            norm([e1[0] - v[0], e1[1] - v[1]])[0] + norm([e2[0] - v[0], e2[1] - v[1]])[0],
            norm([e1[0] - v[0], e1[1] - v[1]])[1] + norm([e2[0] - v[0], e2[1] - v[1]])[1],
        ]);
        if bis[0] * back[0] + bis[1] * back[1] >= (40.0 * DEG).cos() {
            return Some(at_b);
        }
    }
    None
}

// ---------------------------------------------------------------- closed

/// The closed loop of a stroke whose ends meet (overshoot trimmed), resampled
/// uniformly around its perimeter, or `None` for an open stroke.
fn close_loop(p: &[P], opts: Options) -> Option<Vec<P>> {
    let n = p.len();
    let win = n / 6;
    let (mut best, mut bi, mut bj) = (f32::INFINITY, 0, n - 1);
    for i in 0..=win {
        for j in (n - 1 - win)..n {
            if j - i < n * 7 / 10 {
                continue;
            }
            // Trimming costs a little, so the full stroke wins near-ties.
            let d = dist(p[i], p[j]) * (1.0 + 0.6 * (i + n - 1 - j) as f32 / n as f32);
            if d < best {
                best = d;
                bi = i;
                bj = j;
            }
        }
    }
    let b = bbox(&p[bi..=bj]);
    let diag = (b[2] - b[0]).hypot(b[3] - b[1]);
    let gap = dist(p[bi], p[bj]);
    let limit = if opts.deliberate { 0.3 } else { CLOSE_GAP };
    (gap <= limit * diag).then(|| resample_closed(&p[bi..=bj], N_CLOSED))
}

fn closed_shape(p: &[P], opts: Options) -> Option<Recognized> {
    let b = bbox(p);
    let (bw, bh) = (b[2] - b[0], b[3] - b[1]);
    let px = opts.px();
    if bw.max(bh) * px < opts.min_closed() || bw.min(bh) * px < opts.min_thin() {
        return None;
    }
    let diag = bw.hypot(bh);
    let (total, abs) = total_turning(p);
    let c = corners(p, CORNER_WINDOW, CORNER_TURN, true);
    let once = (1.7 * PI..=2.35 * PI).contains(&total.abs()) && abs <= 1.3 * total.abs();
    if c.len() >= 3 {
        let poly = polygon(p, &c, total, abs, diag, opts);
        if poly.is_none() && c.len() <= 4 && once && opts.has(family::FLOWCHART) {
            // Kinks where a terminator's straight sides meet its round ends.
            return stadium(p, diag);
        }
        return poly;
    }
    // Smooth: an ellipse turns once, in one direction, with at most one
    // kink where the ends meet (a `D` has two sharp corners, an `8` turns
    // both ways).
    if !once {
        return None;
    }
    if opts.has(family::FLOWCHART)
        && let Some(r) = stadium(p, diag)
    {
        return Some(r);
    }
    if c.len() > 1 {
        return None;
    }
    ellipse(p)
}

/// Flowchart terminator: two straight parallel sides joined by half
/// circles. Taken only when it fits clearly better than an ellipse (an oval
/// stays an ellipse) and the long sides really are straight.
fn stadium(p: &[P], diag: f32) -> Option<Recognized> {
    let n = p.len() as f32;
    let c = p
        .iter()
        .fold([0.0, 0.0], |s, q| [s[0] + q[0] / n, s[1] + q[1] / n]);
    let (_, d) = fit_line(p);
    let rot = angle(d);
    let (s, co) = rot.sin_cos();
    let uv: Vec<P> = p
        .iter()
        .map(|q| {
            let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
            [dx * co + dy * s, -dx * s + dy * co]
        })
        .collect();
    let b = bbox(&uv);
    let (cu, cv) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let (a, r) = ((b[2] - b[0]) / 2.0, (b[3] - b[1]) / 2.0);
    if r <= f32::EPSILON || a / r < 1.6 {
        return None;
    }
    let half = a - r;
    // Distance to the stadium outline and to the inscribed ellipse.
    let d_stadium = |q: P| {
        let (u, v) = (q[0] - cu, q[1] - cv);
        if u.abs() <= half {
            (v.abs() - r).abs()
        } else {
            ((u.abs() - half).hypot(v) - r).abs()
        }
    };
    let ring: Vec<P> = (0..240)
        .map(|i| {
            let t = i as f32 / 240.0 * TAU;
            [cu + a * t.cos(), cv + r * t.sin()]
        })
        .collect();
    let d_ellipse = |q: P| {
        ring.iter()
            .map(|e| dist(*e, q))
            .fold(f32::INFINITY, f32::min)
    };
    let (mut es, mut ee, mut worst, mut side) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for q in &uv {
        let ds = d_stadium(*q);
        es += ds;
        ee += d_ellipse(*q);
        worst = worst.max(ds);
        if (q[0] - cu).abs() <= 0.8 * half {
            side = side.max(ds);
        }
    }
    let (es, ee) = (es / n, ee / n);
    if es / diag > 0.022 || es > 0.6 * ee || worst > 0.3 * r || side > 0.16 * r {
        return None;
    }
    let mut rot = wrap_half_pi(rot);
    if rot.abs() <= 10.0 * DEG {
        rot = 0.0;
    }
    let (s, co) = rot.sin_cos();
    let centre = [c[0] + cu * co - cv * s, c[1] + cu * s + cv * co];
    Some(Recognized {
        kind: ShapeKind::Terminator,
        geometry: Geometry::template("stadium", centre, 2.0 * a, 2.0 * r, rot, Vec::new()),
        confidence: (1.0 - es / diag / 0.022).clamp(0.0, 1.0),
    })
}

fn ellipse(p: &[P]) -> Option<Recognized> {
    let n = p.len() as f32;
    let c = p
        .iter()
        .fold([0.0, 0.0], |s, q| [s[0] + q[0] / n, s[1] + q[1] / n]);
    // Principal axes from the covariance.
    let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
    for q in p {
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let rot = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (s, co) = rot.sin_cos();
    // Least squares for A u² + B v² = 1 on the principal axes.
    let (mut a11, mut a12, mut a22, mut r1, mut r2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let uv: Vec<(f32, f32)> = p
        .iter()
        .map(|q| {
            let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
            (dx * co + dy * s, -dx * s + dy * co)
        })
        .collect();
    for &(u, v) in &uv {
        let (u2, v2) = (f64::from(u * u), f64::from(v * v));
        a11 += u2 * u2;
        a12 += u2 * v2;
        a22 += v2 * v2;
        r1 += u2;
        r2 += v2;
    }
    let det = a11 * a22 - a12 * a12;
    if det.abs() < 1e-12 {
        return None;
    }
    let aa = (r1 * a22 - r2 * a12) / det;
    let bb = (a11 * r2 - a12 * r1) / det;
    if aa <= 0.0 || bb <= 0.0 {
        return None;
    }
    let (rx, ry) = ((1.0 / aa).sqrt() as f32, (1.0 / bb).sqrt() as f32);
    let (mut sum, mut worst) = (0.0f32, 0.0f32);
    for &(u, v) in &uv {
        let r = ((u / rx).powi(2) + (v / ry).powi(2)).sqrt() - 1.0;
        sum += r.abs();
        worst = worst.max(r.abs());
    }
    let mean = sum / n;
    if mean > ELLIPSE_MEAN_DEV || worst > ELLIPSE_MAX_DEV {
        return None;
    }
    let confidence = (1.0 - mean / ELLIPSE_MEAN_DEV).clamp(0.0, 1.0);
    let (major, minor) = (rx.max(ry), rx.min(ry));
    if minor / major >= CIRCLE_RATIO {
        let r = (rx + ry) / 2.0;
        return Some(Recognized {
            kind: ShapeKind::Circle,
            geometry: Geometry::Ellipse {
                c,
                rx: r,
                ry: r,
                rot: 0.0,
            },
            confidence,
        });
    }
    // Major axis along `rot`, in (-90°, 90°]; snap near-axis-aligned.
    let (mut rx, mut ry, mut rot) = (rx, ry, rot);
    if ry > rx {
        std::mem::swap(&mut rx, &mut ry);
        rot += PI / 2.0;
    }
    rot = wrap_half_pi(rot);
    if rot.abs() <= 10.0 * DEG {
        rot = 0.0;
    } else if (rot.abs() - PI / 2.0).abs() <= 10.0 * DEG {
        // Upright ellipse: no rotation, axes swapped.
        std::mem::swap(&mut rx, &mut ry);
        rot = 0.0;
    }
    Some(Recognized {
        kind: ShapeKind::Ellipse,
        geometry: Geometry::Ellipse { c, rx, ry, rot },
        confidence,
    })
}

fn polygon(
    p: &[P],
    c: &[usize],
    total: f32,
    abs: f32,
    diag: f32,
    opts: Options,
) -> Option<Recognized> {
    let n = p.len();
    let k = c.len();
    let turns: Vec<f32> = c
        .iter()
        .map(|&i| turning(p, i, CORNER_WINDOW, true))
        .collect();
    let sign = total.signum();
    // Fit a line through the inner part of every edge.
    let mut lines: Vec<(P, P)> = Vec::with_capacity(k);
    let mut worst_edge = 0.0f32;
    for m in 0..k {
        let (s, e) = (c[m], c[(m + 1) % k]);
        let span = (e + n - s) % n;
        let chord = dist(p[s], p[e]);
        if span < 3 || chord <= f32::EPSILON {
            return None;
        }
        let margin = (span / 6).max(1);
        let pts: Vec<P> = (margin..=span - margin).map(|j| p[(s + j) % n]).collect();
        if pts.len() < 2 {
            return None;
        }
        let (o, d) = fit_line(&pts);
        let (mut max, mut sq) = (0.0f32, 0.0f32);
        for q in &pts {
            let e = perp(*q, o, d);
            max = max.max(e);
            sq += e * e;
        }
        let rms = (sq / pts.len() as f32).sqrt();
        if max / chord > EDGE_MAX_DEV || rms / chord > EDGE_RMS_DEV {
            return None;
        }
        worst_edge = worst_edge.max(max / chord);
        lines.push((o, d));
    }
    // Vertices where neighbouring edge lines meet (fall back to the corner).
    let mut v: Vec<P> = Vec::with_capacity(k);
    for m in 0..k {
        let (prev, next) = (lines[(m + k - 1) % k], lines[m]);
        let corner = p[c[m]];
        let near = 0.25 * dist(p[c[(m + k - 1) % k]], corner).min(dist(corner, p[c[(m + 1) % k]]));
        v.push(match intersect(prev, next) {
            Some(x) if dist(x, corner) <= near => x,
            _ => corner,
        });
    }
    // The stroke must hug the polygon.
    let mean = p
        .iter()
        .map(|q| {
            (0..k)
                .map(|m| seg_dist(*q, v[m], v[(m + 1) % k]))
                .fold(f32::INFINITY, f32::min)
        })
        .sum::<f32>()
        / n as f32;
    if mean / diag > POLY_MEAN_DEV {
        return None;
    }
    let confidence = ((1.0 - mean / diag / POLY_MEAN_DEV) * (1.0 - worst_edge / EDGE_MAX_DEV))
        .sqrt()
        .clamp(0.0, 1.0);
    let same_sign = turns.iter().all(|t| t.signum() == sign);
    let simple = (1.7 * PI..=2.35 * PI).contains(&total.abs());
    let rec = |kind, geometry| {
        Some(Recognized {
            kind,
            geometry,
            confidence,
        })
    };
    match k {
        3 if same_sign && simple => {
            let pts = triangle(&v);
            rec(ShapeKind::Triangle, Geometry::Polygon { pts })
        }
        4 if same_sign && simple => {
            if let Some(pts) = diamond(&v) {
                return rec(ShapeKind::Diamond, Geometry::Polygon { pts });
            }
            if opts.has(family::FLOWCHART)
                && let Some(pts) = parallelogram(&v)
            {
                return rec(ShapeKind::Parallelogram, Geometry::Polygon { pts });
            }
            let (geometry, square) = rectangle(&v)?;
            rec(
                if square {
                    ShapeKind::Square
                } else {
                    ShapeKind::Rectangle
                },
                geometry,
            )
        }
        5 if same_sign && (3.6 * PI..=4.4 * PI).contains(&total.abs()) => {
            // Pentagram: five tips, each turning ~144°, twice around.
            let pts = regular(&v, 5, 0.2)?;
            let star = [0, 2, 4, 1, 3].iter().map(|&i| pts[i]).collect();
            rec(ShapeKind::Star, Geometry::Polygon { pts: star })
        }
        5 | 6 if same_sign && simple => {
            let pts = regular(&v, k, 0.18)?;
            let mean_turn = turns.iter().map(|t| t.abs()).sum::<f32>() / k as f32;
            if (mean_turn - TAU / k as f32).abs() > 28.0 * DEG {
                return None;
            }
            let kind = if k == 5 {
                ShapeKind::Pentagon
            } else {
                ShapeKind::Hexagon
            };
            rec(kind, Geometry::Polygon { pts })
        }
        10 if simple && abs > 2.5 * PI => {
            let pts = star_outline(&v, &turns, sign)?;
            rec(ShapeKind::Star, Geometry::Polygon { pts })
        }
        _ => None,
    }
}

/// Snap a triangle to equilateral, right or isosceles when close, and lay
/// a near-horizontal (or near-vertical) side exactly flat.
fn triangle(v: &[P]) -> Vec<P> {
    let ang: Vec<f32> = (0..3)
        .map(|i| {
            let (a, b, c) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
            angle_between([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]])
        })
        .collect();
    let g = [
        (v[0][0] + v[1][0] + v[2][0]) / 3.0,
        (v[0][1] + v[1][1] + v[2][1]) / 3.0,
    ];
    if ang.iter().all(|a| (a - 60.0 * DEG).abs() <= 10.0 * DEG) {
        let r = v.iter().map(|q| dist(*q, g)).sum::<f32>() / 3.0;
        let turn = orient(v);
        let mut th = angle([v[0][0] - g[0], v[0][1] - g[1]]);
        // Vertex i is opposite side (i+1, i+2): when that side is nearly
        // flat (or upright), point vertex i straight away from it.
        let mut best = 10.0 * DEG;
        for i in 0..3 {
            let (b, c) = (v[(i + 1) % 3], v[(i + 2) % 3]);
            let off = axis_offset(angle([c[0] - b[0], c[1] - b[1]]));
            if off <= best {
                best = off;
                let dir = angle([v[i][0] - g[0], v[i][1] - g[1]]);
                let target = (dir / (PI / 2.0)).round() * (PI / 2.0);
                th = target - turn * i as f32 * TAU / 3.0;
            }
        }
        return (0..3)
            .map(|k| {
                let a = th + turn * k as f32 * TAU / 3.0;
                [g[0] + r * a.cos(), g[1] + r * a.sin()]
            })
            .collect();
    }
    if let Some(i) = (0..3).find(|&i| (ang[i] - 90.0 * DEG).abs() <= 10.0 * DEG) {
        let (vr, p, q) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
        let (l1, l2) = (dist(vr, p), dist(vr, q));
        let a1 = angle([p[0] - vr[0], p[1] - vr[1]]);
        let a2 = angle([q[0] - vr[0], q[1] - vr[1]]);
        let s = wrap_pi(a2 - a1).signum();
        // Average the two legs' directions into one perpendicular pair.
        let mut phi = a1 + wrap_pi(a2 - s * PI / 2.0 - a1) / 2.0;
        if axis_offset(phi) <= 10.0 * DEG {
            phi = (phi / (PI / 2.0)).round() * (PI / 2.0);
        }
        let mut out = vec![[0.0; 2]; 3];
        out[i] = vr;
        out[(i + 1) % 3] = [vr[0] + l1 * phi.cos(), vr[1] + l1 * phi.sin()];
        let phi2 = phi + s * PI / 2.0;
        out[(i + 2) % 3] = [vr[0] + l2 * phi2.cos(), vr[1] + l2 * phi2.sin()];
        return out;
    }
    // Isosceles: the apex whose two sides are most alike.
    let mut apex = None;
    let mut best = 0.9f32;
    for i in 0..3 {
        let (a, b) = (dist(v[i], v[(i + 1) % 3]), dist(v[i], v[(i + 2) % 3]));
        let r = a.min(b) / a.max(b);
        if r >= best {
            best = r;
            apex = Some(i);
        }
    }
    let Some(i) = apex else {
        return v.to_vec();
    };
    let (a, b1, b2) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
    let m = [(b1[0] + b2[0]) / 2.0, (b1[1] + b2[1]) / 2.0];
    let half = dist(b1, b2) / 2.0;
    let mut base = angle([b2[0] - b1[0], b2[1] - b1[1]]);
    if axis_offset(base) <= 10.0 * DEG {
        base = (base / (PI / 2.0)).round() * (PI / 2.0);
    }
    let u = [base.cos(), base.sin()];
    let nrm = [-u[1], u[0]];
    let h = (a[0] - m[0]) * nrm[0] + (a[1] - m[1]) * nrm[1];
    let mut out = vec![[0.0; 2]; 3];
    out[i] = [m[0] + nrm[0] * h, m[1] + nrm[1] * h];
    out[(i + 1) % 3] = [m[0] - u[0] * half, m[1] - u[1] * half];
    out[(i + 2) % 3] = [m[0] + u[0] * half, m[1] + u[1] * half];
    out
}

/// A rhombus whose vertices sit at the midpoints of its bounding box sides.
fn diamond(v: &[P]) -> Option<Vec<P>> {
    let b = bbox(v);
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    if w <= f32::EPSILON || h <= f32::EPSILON {
        return None;
    }
    let mid = [
        [0.5, 0.0], // top
        [1.0, 0.5], // right
        [0.5, 1.0], // bottom
        [0.0, 0.5], // left
    ];
    for m in &mid {
        let hit = v.iter().any(|q| {
            let (nx, ny) = ((q[0] - b[0]) / w, (q[1] - b[1]) / h);
            (nx - m[0]).hypot(ny - m[1]) <= 0.16
        });
        if !hit {
            return None;
        }
    }
    Some(
        mid.iter()
            .map(|m| [b[0] + m[0] * w, b[1] + m[1] * h])
            .collect(),
    )
}

/// Flowchart input/output: level top and bottom, slanted sides (45°–80°),
/// opposite sides parallel. Vertices clockwise from the top left.
fn parallelogram(v: &[P]) -> Option<Vec<P>> {
    let dir = |i: usize| {
        let (a, b) = (v[i], v[(i + 1) % 4]);
        [b[0] - a[0], b[1] - a[1]]
    };
    // Opposite sides parallel (directions opposite as we go round).
    for i in 0..2 {
        let (a, b) = (dir(i), dir(i + 2));
        if angle_between(a, [-b[0], -b[1]]) > 8.0 * DEG {
            return None;
        }
    }
    // Which pair is level?
    let level = |i: usize| {
        axis_offset(angle(dir(i))) <= 10.0 * DEG && {
            let d = dir(i);
            d[0].abs() > d[1].abs()
        }
    };
    let k = (0..2).find(|&i| level(i) && level(i + 2))?;
    let slant = angle_between(dir(k + 1), [1.0, 0.0]);
    let slant = slant.min(PI - slant);
    if !(45.0 * DEG..=80.0 * DEG).contains(&slant) {
        return None;
    }
    let (l1, l2) = (dist(v[k], v[k + 1]), dist(v[k + 2], v[(k + 3) % 4]));
    if l1.min(l2) / l1.max(l2) < 0.8 {
        return None;
    }
    let mut q: Vec<P> = v.to_vec();
    q.sort_by(|a, b| a[1].total_cmp(&b[1]));
    let (mut top, mut bot) = ([q[0], q[1]], [q[2], q[3]]);
    top.sort_by(|a, b| a[0].total_cmp(&b[0]));
    bot.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let len = (top[1][0] - top[0][0] + bot[1][0] - bot[0][0]) / 2.0;
    let shift = (top[0][0] - bot[0][0] + top[1][0] - bot[1][0]) / 2.0;
    if shift.abs() < 0.15 * len {
        // Barely slanted: a rectangle.
        return None;
    }
    let (yt, yb) = ((top[0][1] + top[1][1]) / 2.0, (bot[0][1] + bot[1][1]) / 2.0);
    let cx = (top[0][0] + top[1][0] + bot[0][0] + bot[1][0]) / 4.0;
    let (xt, xb) = (cx + shift / 2.0 - len / 2.0, cx - shift / 2.0 - len / 2.0);
    Some(vec![[xt, yt], [xt + len, yt], [xb + len, yb], [xb, yb]])
}

/// Rectangle (axis-aligned when within [`RECT_SNAP_DEG`]); the flag is true
/// for a square.
fn rectangle(v: &[P]) -> Option<(Geometry, bool)> {
    for i in 0..4 {
        let (a, b, c) = (v[(i + 3) % 4], v[i], v[(i + 1) % 4]);
        let ang = angle_between([a[0] - b[0], a[1] - b[1]], [c[0] - b[0], c[1] - b[1]]);
        if (ang - 90.0 * DEG).abs() > 20.0 * DEG {
            return None;
        }
    }
    // Mean edge direction modulo 90° (4θ averaging).
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    for i in 0..4 {
        let (a, b) = (v[i], v[(i + 1) % 4]);
        let th = angle([b[0] - a[0], b[1] - a[1]]);
        let l = dist(a, b);
        sx += l * (4.0 * th).cos();
        sy += l * (4.0 * th).sin();
    }
    let mut rot = sy.atan2(sx) / 4.0;
    if rot.abs() <= RECT_SNAP_DEG * DEG {
        rot = 0.0;
    }
    let (s, co) = rot.sin_cos();
    let c = [
        v.iter().map(|q| q[0]).sum::<f32>() / 4.0,
        v.iter().map(|q| q[1]).sum::<f32>() / 4.0,
    ];
    // Average extent of the vertices along each axis.
    let (mut wu, mut hv) = (0.0f32, 0.0f32);
    for q in v {
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        wu += (dx * co + dy * s).abs();
        hv += (-dx * s + dy * co).abs();
    }
    let (mut w, mut h) = (wu / 2.0, hv / 2.0);
    let square = w.min(h) / w.max(h) >= 0.9;
    if square {
        let m = (w + h) / 2.0;
        w = m;
        h = m;
    }
    Some((Geometry::Rect { c, w, h, rot }, square))
}

/// Regular k-gon through the fitted vertices (centre, mean radius, phase),
/// with a vertex pointing straight up or a flat top when close. `None` when
/// the vertices are too uneven to call regular.
fn regular(v: &[P], k: usize, max_cv: f32) -> Option<Vec<P>> {
    let kf = k as f32;
    let g = [
        v.iter().map(|q| q[0]).sum::<f32>() / kf,
        v.iter().map(|q| q[1]).sum::<f32>() / kf,
    ];
    let radii: Vec<f32> = v.iter().map(|q| dist(*q, g)).collect();
    let r = radii.iter().sum::<f32>() / kf;
    let cv = (radii.iter().map(|x| (x - r).powi(2)).sum::<f32>() / kf).sqrt() / r;
    if cv > max_cv {
        return None;
    }
    let step = TAU / kf;
    // Average phase of the vertices modulo the symmetry step.
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    for q in v {
        let a = angle([q[0] - g[0], q[1] - g[1]]) * kf;
        sx += a.cos();
        sy += a.sin();
    }
    let mut phase = sy.atan2(sx) / kf;
    // Snap: a vertex up (−90°), or a flat top (vertex at −90° + step/2).
    for target in [-PI / 2.0, -PI / 2.0 + step / 2.0] {
        let d = wrap_pi((phase - target) * kf) / kf;
        if d.abs() <= 10.0 * DEG {
            phase -= d;
            break;
        }
    }
    let turn = orient(v);
    // Start at the vertex nearest the first drawn one, go the drawn way.
    let a0 = angle([v[0][0] - g[0], v[0][1] - g[1]]);
    let start = phase + (wrap_pi(a0 - phase) / step).round() * step;
    Some(
        (0..k)
            .map(|i| {
                let a = start + turn * i as f32 * step;
                [g[0] + r * a.cos(), g[1] + r * a.sin()]
            })
            .collect(),
    )
}

/// Ten corners alternating tip / notch: a five-point star outline.
fn star_outline(v: &[P], turns: &[f32], sign: f32) -> Option<Vec<P>> {
    let outer_first = turns[0].signum() == sign;
    for (i, t) in turns.iter().enumerate() {
        if (t.signum() == sign) != ((i % 2 == 0) == outer_first) {
            return None;
        }
    }
    let g = [
        v.iter().map(|q| q[0]).sum::<f32>() / 10.0,
        v.iter().map(|q| q[1]).sum::<f32>() / 10.0,
    ];
    let off = usize::from(!outer_first);
    let tips: Vec<P> = (0..5).map(|i| v[2 * i + off]).collect();
    let notches: Vec<P> = (0..5).map(|i| v[(2 * i + 1 + off) % 10]).collect();
    let outer = regular(&tips, 5, 0.2)?;
    let ro = dist(outer[0], g).max(f32::EPSILON);
    let ri = notches.iter().map(|q| dist(*q, g)).sum::<f32>() / 5.0;
    let mut ratio = ri / ro;
    if !(0.25..=0.65).contains(&ratio) {
        return None;
    }
    if (ratio - 0.382).abs() <= 0.08 {
        ratio = 0.382;
    }
    let a0 = angle([outer[0][0] - g[0], outer[0][1] - g[1]]);
    let turn = orient(&tips);
    let mut out = Vec::with_capacity(10);
    for i in 0..5 {
        let a = a0 + turn * i as f32 * TAU / 5.0;
        out.push([g[0] + ro * a.cos(), g[1] + ro * a.sin()]);
        let b = a + turn * TAU / 10.0;
        out.push([g[0] + ro * ratio * b.cos(), g[1] + ro * ratio * b.sin()]);
    }
    Some(out)
}

// -------------------------------------------------------------- scratch

/// A scratch-out: a dense zig-zag (six or more sharp back-and-forth passes,
/// closely spaced). Conservative: the engine also requires it to cover ink.
pub fn is_scratch_out(points: &[StrokePoint], opts: Options) -> bool {
    let xy: Vec<P> = points.iter().map(|p| [p.x, p.y]).collect();
    if xy.len() < 4 || xy.iter().any(|p| !(p[0].is_finite() && p[1].is_finite())) {
        return false;
    }
    let len = path_len(&xy);
    let b = bbox(&xy);
    if b[2] - b[0] <= 0.0 && b[3] - b[1] <= 0.0 {
        return false;
    }
    if (b[2] - b[0]).max(b[3] - b[1]) * opts.px() < 24.0 {
        return false;
    }
    let p = resample(&xy, (len * opts.px() / 3.0).clamp(48.0, 600.0) as usize);
    let n = p.len() as f32;
    let c = p
        .iter()
        .fold([0.0, 0.0], |s, q| [s[0] + q[0] / n, s[1] + q[1] / n]);
    let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
    for q in &p {
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    // Passes may run along either principal axis (a wide word scratched
    // with vertical strokes, or with horizontal ones).
    [th, th + PI / 2.0]
        .into_iter()
        .any(|axis| zigzag_along(&p, axis, len))
}

fn zigzag_along(p: &[P], axis: f32, len: f32) -> bool {
    let (s, c) = axis.sin_cos();
    let along: Vec<f32> = p.iter().map(|q| q[0] * c + q[1] * s).collect();
    let across: Vec<f32> = p.iter().map(|q| -q[0] * s + q[1] * c).collect();
    let lo = along.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = along.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let amp = hi - lo;
    if amp <= f32::EPSILON {
        return false;
    }
    // Reversals with hysteresis: a pass must cover half the amplitude.
    let hyst = 0.5 * amp;
    let mut turns: Vec<usize> = Vec::new();
    let (mut dir, mut ext_i) = (0i8, 0usize);
    for i in 1..along.len() {
        let x = along[i];
        let e = along[ext_i];
        match dir {
            0 => {
                if (x - along[0]).abs() >= hyst {
                    dir = if x > along[0] { 1 } else { -1 };
                    ext_i = i;
                } else if (x - along[0]).abs() > (e - along[0]).abs() {
                    ext_i = i;
                }
            }
            1 => {
                if x > e {
                    ext_i = i;
                } else if e - x >= hyst {
                    turns.push(ext_i);
                    dir = -1;
                    ext_i = i;
                }
            }
            _ => {
                if x < e {
                    ext_i = i;
                } else if x - e >= hyst {
                    turns.push(ext_i);
                    dir = 1;
                    ext_i = i;
                }
            }
        }
    }
    if turns.len() < 5 || len < 3.0 * amp {
        return false;
    }
    // Close passes: their spacing is small next to their length.
    let lo2 = across.iter().copied().fold(f32::INFINITY, f32::min);
    let hi2 = across.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let spacing = (hi2 - lo2) / (turns.len() + 1) as f32;
    if spacing > 0.4 * amp {
        return false;
    }
    // Sharp cusps, not rounded humps (cursive `mmm`): the turn within a
    // short window (8 % of the pass length) around each reversal.
    let step = len / (p.len() - 1).max(1) as f32;
    let w = ((0.08 * amp / step.max(f32::EPSILON)).round() as usize).max(2);
    let sharp = turns
        .iter()
        .filter(|&&i| i >= w && i + w < p.len() && turning(p, i, w, false).abs() >= 110.0 * DEG)
        .count();
    sharp * 10 >= turns.len() * 7
}

// -------------------------------------------------------------- helpers

fn dist(a: P, b: P) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn norm(v: P) -> P {
    let l = v[0].hypot(v[1]).max(f32::EPSILON);
    [v[0] / l, v[1] / l]
}

fn angle(v: P) -> f32 {
    v[1].atan2(v[0])
}

fn angle_between(a: P, b: P) -> f32 {
    let (la, lb) = (a[0].hypot(a[1]), b[0].hypot(b[1]));
    if la <= f32::EPSILON || lb <= f32::EPSILON {
        return 0.0;
    }
    ((a[0] * b[0] + a[1] * b[1]) / (la * lb))
        .clamp(-1.0, 1.0)
        .acos()
}

fn wrap_pi(a: f32) -> f32 {
    let mut a = a % TAU;
    if a > PI {
        a -= TAU;
    } else if a <= -PI {
        a += TAU;
    }
    a
}

fn wrap_half_pi(a: f32) -> f32 {
    let mut a = wrap_pi(a);
    if a > PI / 2.0 {
        a -= PI;
    } else if a <= -PI / 2.0 {
        a += PI;
    }
    a
}

/// Distance of an angle from the nearest multiple of 90°.
fn axis_offset(a: f32) -> f32 {
    let q = PI / 2.0;
    (a - (a / q).round() * q).abs()
}

/// +1 when the vertices run clockwise on screen (y down), −1 otherwise.
fn orient(v: &[P]) -> f32 {
    let mut area = 0.0f32;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[(i + 1) % v.len()]);
        area += a[0] * b[1] - b[0] * a[1];
    }
    if area >= 0.0 { 1.0 } else { -1.0 }
}

fn bbox(p: &[P]) -> [f32; 4] {
    let mut b = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for q in p {
        b = [
            b[0].min(q[0]),
            b[1].min(q[1]),
            b[2].max(q[0]),
            b[3].max(q[1]),
        ];
    }
    b
}

fn path_len(p: &[P]) -> f32 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

/// `n` points evenly spaced along an open polyline (ends kept).
fn resample(p: &[P], n: usize) -> Vec<P> {
    let total = path_len(p);
    if p.len() < 2 || total <= f32::EPSILON || n < 2 {
        return p.to_vec();
    }
    let step = total / (n - 1) as f32;
    let mut out = Vec::with_capacity(n);
    out.push(p[0]);
    let mut acc = 0.0f32;
    let mut prev = p[0];
    let mut i = 1;
    while i < p.len() && out.len() < n - 1 {
        let d = dist(prev, p[i]);
        if acc + d >= step && d > 0.0 {
            let t = (step - acc) / d;
            let q = [
                prev[0] + t * (p[i][0] - prev[0]),
                prev[1] + t * (p[i][1] - prev[1]),
            ];
            out.push(q);
            prev = q;
            acc = 0.0;
        } else {
            acc += d;
            prev = p[i];
            i += 1;
        }
    }
    while out.len() < n {
        out.push(p[p.len() - 1]);
    }
    out
}

/// `n` points evenly spaced around a polyline closed back to its start.
fn resample_closed(p: &[P], n: usize) -> Vec<P> {
    let mut ring = p.to_vec();
    ring.push(p[0]);
    let mut out = resample(&ring, n + 1);
    out.truncate(n);
    out
}

/// Signed turning (radians) at sample `i` between the directions over the
/// `w` samples before and after it.
fn turning(p: &[P], i: usize, w: usize, cyclic: bool) -> f32 {
    let n = p.len();
    let (a, c) = if cyclic {
        (p[(i + n - w % n) % n], p[(i + w) % n])
    } else {
        if i < w || i + w >= n {
            return 0.0;
        }
        (p[i - w], p[i + w])
    };
    let b = p[i];
    let d1 = [b[0] - a[0], b[1] - a[1]];
    let d2 = [c[0] - b[0], c[1] - b[1]];
    if d1[0].hypot(d1[1]) <= f32::EPSILON || d2[0].hypot(d2[1]) <= f32::EPSILON {
        return 0.0;
    }
    wrap_pi(angle(d2) - angle(d1))
}

/// Total signed and absolute turning around a closed ring, measured on every
/// fourth sample so pen jitter doesn't count.
fn total_turning(p: &[P]) -> (f32, f32) {
    let ring: Vec<P> = p.iter().step_by(4).copied().collect();
    let n = ring.len();
    if n < 3 {
        return (0.0, 0.0);
    }
    let dir: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            angle([b[0] - a[0], b[1] - a[1]])
        })
        .collect();
    let (mut total, mut abs) = (0.0f32, 0.0f32);
    for i in 0..n {
        let d = wrap_pi(dir[(i + 1) % n] - dir[i]);
        total += d;
        abs += d.abs();
    }
    (total, abs)
}

/// Corner indices: local maxima of |turning| ≥ `min_turn`, at least `w`
/// samples apart, in index order.
fn corners(p: &[P], w: usize, min_turn: f32, cyclic: bool) -> Vec<usize> {
    let n = p.len();
    if n < 2 * w + 1 {
        return Vec::new();
    }
    let t: Vec<f32> = (0..n).map(|i| turning(p, i, w, cyclic).abs()).collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| t[i] >= min_turn).collect();
    order.sort_by(|&a, &b| t[b].total_cmp(&t[a]).then(a.cmp(&b)));
    let mut picked: Vec<usize> = Vec::new();
    for i in order {
        let clash = picked.iter().any(|&j| {
            let d = i.abs_diff(j);
            let d = if cyclic { d.min(n - d) } else { d };
            d <= w + 1
        });
        if !clash {
            picked.push(i);
        }
    }
    picked.sort_unstable();
    picked
}

/// Least-squares line: a point on it and its unit direction.
fn fit_line(p: &[P]) -> (P, P) {
    let n = p.len() as f32;
    let c = p
        .iter()
        .fold([0.0, 0.0], |s, q| [s[0] + q[0] / n, s[1] + q[1] / n]);
    let (mut sxx, mut syy, mut sxy) = (0.0f32, 0.0f32, 0.0f32);
    for q in p {
        let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    (c, [th.cos(), th.sin()])
}

fn perp(q: P, o: P, d: P) -> f32 {
    ((q[0] - o[0]) * -d[1] + (q[1] - o[1]) * d[0]).abs()
}

fn intersect(l1: (P, P), l2: (P, P)) -> Option<P> {
    let ((o1, d1), (o2, d2)) = (l1, l2);
    let den = d1[0] * d2[1] - d1[1] * d2[0];
    if den.abs() < 0.15 {
        // Nearly parallel (< ~9°): no reliable vertex.
        return None;
    }
    let t = ((o2[0] - o1[0]) * d2[1] - (o2[1] - o1[1]) * d2[0]) / den;
    Some([o1[0] + t * d1[0], o1[1] + t * d1[1]])
}

fn seg_dist(q: P, a: P, b: P) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = vx * vx + vy * vy;
    let t = if l2 <= f32::EPSILON {
        0.0
    } else {
        (((q[0] - a[0]) * vx + (q[1] - a[1]) * vy) / l2).clamp(0.0, 1.0)
    };
    (a[0] + t * vx - q[0]).hypot(a[1] + t * vy - q[1])
}

/// Largest distance of the points from the segment `a`–`b`.
fn max_dev(p: &[P], a: P, b: P) -> f32 {
    p.iter().map(|q| seg_dist(*q, a, b)).fold(0.0, f32::max)
}

/// Rotate a segment about its midpoint onto the nearest multiple of 45°
/// when within [`LINE_SNAP_DEG`].
fn snap_segment(a: P, b: P) -> (P, P) {
    let th = angle([b[0] - a[0], b[1] - a[1]]);
    let q = PI / 4.0;
    let target = (th / q).round() * q;
    if (th - target).abs() > LINE_SNAP_DEG * DEG {
        return (a, b);
    }
    let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let h = dist(a, b) / 2.0;
    let (s, c) = target.sin_cos();
    ([m[0] - c * h, m[1] - s * h], [m[0] + c * h, m[1] + s * h])
}

pub mod registry;
pub use registry::{Context, Label, Neighbour, Proposal, Recognizer, Registry};

pub mod diagram;
pub mod geometry;
pub mod graph;
pub mod logic;
pub mod science;

/// A stroke that is a straight segment of any length: its ends.
fn straight(p: &[P], max_dev_ratio: f32) -> Option<(P, P)> {
    if p.len() < 2 {
        return None;
    }
    let (a, b) = (p[0], p[p.len() - 1]);
    let l = dist(a, b);
    if l <= f32::EPSILON || path_len(p) / l > 1.15 || max_dev(p, a, b) / l > max_dev_ratio {
        return None;
    }
    Some((a, b))
}

/// A neighbour as a straight segment: its stored line or arrow, or its ink
/// when that is straight.
fn as_segment(n: &Neighbour) -> Option<(P, P)> {
    match &n.geometry {
        Some(Geometry::Segment { a, b }) => Some((*a, *b)),
        Some(_) => None,
        None => straight(&n.points, 0.07),
    }
}

/// A neighbour as a circle (centre, radius): a stored circle (or a nearly
/// round ellipse), or round ink.
fn as_circle(n: &Neighbour, min_ratio: f32) -> Option<(P, f32)> {
    match &n.geometry {
        Some(Geometry::Ellipse { c, rx, ry, .. }) => {
            (rx.min(*ry) / rx.max(*ry) >= min_ratio).then_some((*c, (rx + ry) / 2.0))
        }
        Some(_) => None,
        None => circle_of(&n.points, min_ratio),
    }
}

/// Round ink: a closed loop that fits an ellipse with axes within `min_ratio`.
fn circle_of(points: &[P], min_ratio: f32) -> Option<(P, f32)> {
    if points.len() < 6 {
        return None;
    }
    // Fewer samples on a small loop, so pen jitter doesn't read as turning.
    let n = ((path_len(points) / 2.0) as usize).clamp(32, N_CLOSED);
    let p = resample(points, n);
    let loop_pts = close_loop(
        &p,
        Options {
            deliberate: true,
            ..Options::default()
        },
    )?;
    // Pen jitter adds a lot of absolute turning on a small loop; the ellipse
    // fit's residuals keep letters out.
    let (total, abs) = total_turning(&loop_pts);
    if !(1.7 * PI..=2.35 * PI).contains(&total.abs()) || abs > 1.8 * total.abs() {
        return None;
    }
    match ellipse(&loop_pts)?.geometry {
        Geometry::Ellipse { c, rx, ry, .. } if rx.min(ry) / rx.max(ry) >= min_ratio => {
            Some((c, (rx + ry) / 2.0))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
