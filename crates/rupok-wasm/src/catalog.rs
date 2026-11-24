//! The Shapes picker's catalogue: every shape a teacher can insert (basic
//! shapes, lines, teaching diagrams), as geometry for a drag box or a tap.
//!
//! Most entries reuse kinds the engine already draws (`rect`, `ellipse`,
//! `poly`, `line`, `arrow`, and the Smart Ink templates `stadium`, `table`,
//! `numberline`, `axes`). The few the picker adds are box kinds drawn here:
//! `rrect` (rounded rectangle), `callout` (speech bubble), `brace`, `arrow2`
//! (double arrow) and `carrow` (curved arrow). See the Shape geometry section
//! of `spec/rupok-format.md` (proposed kinds, pending RFC).
//!
//! Style that is not geometry also lives here: the fill outline of a closed
//! shape (drawn under the ink, `fill`) and the dash pattern of a dashed
//! outline (`dash`).

use rupok_core::ShapeGeom;
use rupok_ink::StrokePoint;

use std::f64::consts::{FRAC_PI_2, PI, TAU};

type Q = [f64; 2];

/// Largest table the picker makes (rows and columns).
pub const MAX_TABLE: u8 = 10;

/// One entry of the Shapes picker. `Copy`, so it can sit in a tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Rect,
    RoundRect,
    Square,
    Ellipse,
    Circle,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Star,
    Parallelogram,
    Trapezoid,
    Line,
    Arrow,
    DoubleArrow,
    DashedLine,
    CurvedArrow,
    Connector,
    Table { rows: u8, cols: u8 },
    NumberLine,
    Axes,
    Venn2,
    Venn3,
    Terminator,
    Process,
    Decision,
    Data,
    Callout,
    BraceLeft,
    BraceDown,
}

/// Picker ids, in picker order (the table's id is `table:RxC`).
pub const IDS: [&str; 31] = [
    "rect",
    "rrect",
    "square",
    "ellipse",
    "circle",
    "triangle",
    "right-triangle",
    "diamond",
    "pentagon",
    "hexagon",
    "star",
    "parallelogram",
    "trapezoid",
    "line",
    "arrow",
    "double-arrow",
    "dashed-line",
    "curved-arrow",
    "connector",
    "table",
    "numberline",
    "axes",
    "venn2",
    "venn3",
    "terminator",
    "process",
    "decision",
    "data",
    "callout",
    "brace",
    "brace-down",
];

impl Preset {
    /// A picker id: one of [`IDS`], or `table:RxC` (1 ≤ R, C ≤ 10).
    pub fn parse(id: &str) -> Option<Self> {
        use Preset::*;
        if let Some(rc) = id.strip_prefix("table:") {
            let (r, c) = rc.split_once('x')?;
            let (rows, cols) = (r.parse::<u8>().ok()?, c.parse::<u8>().ok()?);
            if !(1..=MAX_TABLE).contains(&rows) || !(1..=MAX_TABLE).contains(&cols) {
                return None;
            }
            return Some(Table { rows, cols });
        }
        Some(match id {
            "rect" => Rect,
            "rrect" => RoundRect,
            "square" => Square,
            "ellipse" => Ellipse,
            "circle" => Circle,
            "triangle" => Triangle,
            "right-triangle" => RightTriangle,
            "diamond" => Diamond,
            "pentagon" => Pentagon,
            "hexagon" => Hexagon,
            "star" => Star,
            "parallelogram" => Parallelogram,
            "trapezoid" => Trapezoid,
            "line" => Line,
            "arrow" => Arrow,
            "double-arrow" => DoubleArrow,
            "dashed-line" => DashedLine,
            "curved-arrow" => CurvedArrow,
            "connector" => Connector,
            "table" => Table { rows: 3, cols: 3 },
            "numberline" => NumberLine,
            "axes" => Axes,
            "venn2" => Venn2,
            "venn3" => Venn3,
            "terminator" => Terminator,
            "process" => Process,
            "decision" => Decision,
            "data" => Data,
            "callout" => Callout,
            "brace" => BraceLeft,
            "brace-down" => BraceDown,
            _ => return None,
        })
    }

    /// The stored `shape` name of the main stroke.
    pub fn shape_name(self) -> &'static str {
        use Preset::*;
        match self {
            Rect => "rect",
            RoundRect => "rounded-rect",
            Square => "square",
            Ellipse => "ellipse",
            Circle => "circle",
            Triangle => "triangle",
            RightTriangle => "right-triangle",
            Diamond => "diamond",
            Pentagon => "pentagon",
            Hexagon => "hexagon",
            Star => "star",
            Parallelogram => "parallelogram",
            Trapezoid => "trapezoid",
            Line | DashedLine => "line",
            Arrow | Connector => "arrow",
            DoubleArrow => "double-arrow",
            CurvedArrow => "curved-arrow",
            Table { .. } => "table",
            NumberLine => "numberline",
            Axes => "axes",
            Venn2 | Venn3 => "venn",
            Terminator => "terminator",
            Process => "process",
            Decision => "decision",
            Data => "data",
            Callout => "callout",
            BraceLeft | BraceDown => "brace",
        }
    }

    /// Dragged as a segment (start → end) rather than a box.
    pub fn is_segment(self) -> bool {
        use Preset::*;
        matches!(
            self,
            Line | Arrow | DoubleArrow | DashedLine | CurvedArrow | Connector
        )
    }

    /// Always as wide as tall (Shift makes any box shape so).
    pub fn keeps_aspect(self) -> bool {
        matches!(self, Preset::Square | Preset::Circle)
    }

    /// The outline starts dashed.
    pub fn dashed(self) -> bool {
        self == Preset::DashedLine
    }

    /// Default size (world units) for a tap, from `unit` (≈ 200 screen px).
    fn default_box(self, unit: f64) -> [f64; 2] {
        use Preset::*;
        match self {
            Square | Circle | Pentagon | Hexagon | Star | Diamond | Decision | Axes => [unit, unit],
            Table { rows, cols } => [f64::from(cols) * unit * 0.55, f64::from(rows) * unit * 0.28],
            NumberLine => [unit * 2.4, unit * 0.2],
            Venn2 => [unit * 1.6, unit],
            Venn3 => [unit * 1.6, unit * 1.5],
            BraceLeft => [unit * 0.25, unit],
            BraceDown => [unit * 1.4, unit * 0.25],
            Terminator => [unit * 1.2, unit * 0.45],
            Callout => [unit * 1.4, unit * 0.8],
            _ => [unit * 1.4, unit],
        }
    }
}

/// A stroke the picker places: its `shape` name and geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub shape: &'static str,
    pub geom: ShapeGeom,
}

/// A text label placed with a shape (number line numbers, axis letters).
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub text: String,
    /// Centre of the text, world units.
    pub at: Q,
    pub size: f64,
}

/// What one placement adds: strokes (several share a group) and labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placement {
    pub parts: Vec<Part>,
    pub labels: Vec<Label>,
}

/// The drag box `a` → `b` with Shift (or the lock, or the preset) keeping
/// it square: the larger side wins and the box grows from `a`. Segments
/// snap to 15° instead.
pub fn constrain(preset: Preset, a: Q, b: Q, lock: bool) -> Q {
    if preset.is_segment() {
        if !lock {
            return b;
        }
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = dx.hypot(dy);
        let step = PI / 12.0;
        let ang = (dy.atan2(dx) / step).round() * step;
        return [a[0] + len * ang.cos(), a[1] + len * ang.sin()];
    }
    if !(lock || preset.keeps_aspect()) {
        return b;
    }
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let s = dx.abs().max(dy.abs());
    [a[0] + s * sign(dx), a[1] + s * sign(dy)]
}

fn sign(v: f64) -> f64 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// The drag `a` → `b` of a tap (no drag): a box of the preset's default
/// size centred on `a`, or a horizontal segment starting there.
pub fn tap_drag(preset: Preset, a: Q, unit: f64) -> (Q, Q) {
    let [w, h] = preset.default_box(unit);
    if preset.is_segment() {
        let len = unit * 1.5;
        return ([a[0] - len / 2.0, a[1]], [a[0] + len / 2.0, a[1]]);
    }
    (
        [a[0] - w / 2.0, a[1] - h / 2.0],
        [a[0] + w / 2.0, a[1] + h / 2.0],
    )
}

fn boxed(kind: &str, a: Q, b: Q, v: Vec<f64>) -> ShapeGeom {
    ShapeGeom {
        kind: kind.into(),
        x: a[0].min(b[0]),
        y: a[1].min(b[1]),
        w: (b[0] - a[0]).abs(),
        h: (b[1] - a[1]).abs(),
        v,
        ..Default::default()
    }
}

fn segment(kind: &str, a: Q, b: Q) -> ShapeGeom {
    ShapeGeom {
        kind: kind.into(),
        x: a[0],
        y: a[1],
        w: b[0] - a[0],
        h: b[1] - a[1],
        ..Default::default()
    }
}

/// A box kind lying along the segment `a` → `b`: `w` its length, `h` its
/// height, turned by the segment's angle. `up` shifts the box above the
/// segment (the curved arrow's chord is the box's bottom edge).
fn along(kind: &str, a: Q, b: Q, h: f64, up: bool) -> ShapeGeom {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy).max(1e-6);
    let rot = dy.atan2(dx);
    let mut c = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    if up {
        // Local (0, -h/2) turned by rot.
        c = [c[0] + h / 2.0 * rot.sin(), c[1] - h / 2.0 * rot.cos()];
    }
    ShapeGeom {
        kind: kind.into(),
        x: c[0] - len / 2.0,
        y: c[1] - h / 2.0,
        w: len,
        h,
        rot,
        ..Default::default()
    }
}

fn regular(n: usize, inner: Option<f64>) -> Vec<f64> {
    let k = if inner.is_some() { 2 * n } else { n };
    let pts: Vec<Q> = (0..k)
        .map(|i| {
            let a = -FRAC_PI_2 + TAU * i as f64 / k as f64;
            let r = match inner {
                Some(r) if i % 2 == 1 => r,
                _ => 1.0,
            };
            [r * a.cos(), r * a.sin()]
        })
        .collect();
    normalise(&pts)
}

/// Points normalised to their bounding box, flat.
fn normalise(pts: &[Q]) -> Vec<f64> {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (w, h) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
    pts.iter()
        .flat_map(|p| [(p[0] - x0) / w, (p[1] - y0) / h])
        .collect()
}

/// Table dividers (`v` of a `table`): evenly spaced rows and columns.
pub fn table_v(rows: u8, cols: u8) -> Vec<f64> {
    let mut v = Vec::new();
    for c in 1..cols {
        v.extend([0.0, f64::from(c) / f64::from(cols)]);
    }
    for r in 1..rows {
        v.extend([1.0, f64::from(r) / f64::from(rows)]);
    }
    v
}

/// Rows and columns of a `table` geometry (its dividers plus one).
pub fn table_size(g: &ShapeGeom) -> (usize, usize) {
    let cols =
        g.v.chunks(2)
            .filter(|c| c.len() == 2 && c[0] == 0.0)
            .count();
    let rows =
        g.v.chunks(2)
            .filter(|c| c.len() == 2 && c[0] != 0.0)
            .count();
    (rows + 1, cols + 1)
}

/// Cell edges of a `table` (box-local fractions, sorted, with 0 and 1).
pub fn table_edges(g: &ShapeGeom) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0, 1.0];
    let mut ys = vec![0.0, 1.0];
    for d in g.v.chunks(2).filter(|c| c.len() == 2) {
        if d[0] == 0.0 {
            xs.push(d[1].clamp(0.0, 1.0));
        } else {
            ys.push(d[1].clamp(0.0, 1.0));
        }
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    (xs, ys)
}

/// What placing `preset` dragged from `a` to `b` adds. `width` is the
/// stroke width (label sizes follow the shape).
pub fn place(preset: Preset, a: Q, b: Q) -> Placement {
    use Preset::*;
    let one = |kind: &str, v: Vec<f64>| Placement {
        parts: vec![Part {
            shape: preset.shape_name(),
            geom: boxed(kind, a, b, v),
        }],
        labels: Vec::new(),
    };
    let poly = |v: Vec<f64>| one("poly", v);
    let mut p = match preset {
        Rect | Square | Process => one("rect", Vec::new()),
        RoundRect => one("rrect", vec![0.18, 0.0]),
        Ellipse | Circle => one("ellipse", Vec::new()),
        Triangle => poly(vec![0.5, 0.0, 1.0, 1.0, 0.0, 1.0]),
        RightTriangle => poly(vec![0.0, 0.0, 1.0, 1.0, 0.0, 1.0]),
        Diamond | Decision => poly(vec![0.5, 0.0, 1.0, 0.5, 0.5, 1.0, 0.0, 0.5]),
        Pentagon => poly(regular(5, None)),
        Hexagon => poly(vec![
            0.25, 0.0, 0.75, 0.0, 1.0, 0.5, 0.75, 1.0, 0.25, 1.0, 0.0, 0.5,
        ]),
        Star => poly(regular(5, Some(0.42))),
        Parallelogram | Data => poly(vec![0.22, 0.0, 1.0, 0.0, 0.78, 1.0, 0.0, 1.0]),
        Trapezoid => poly(vec![0.22, 0.0, 0.78, 0.0, 1.0, 1.0, 0.0, 1.0]),
        Terminator => one("stadium", Vec::new()),
        Callout => one("callout", vec![0.22, 1.4]),
        BraceLeft => one("brace", vec![0.0, 0.0]),
        BraceDown => one("brace", vec![3.0, 0.0]),
        Table { rows, cols } => one("table", table_v(rows, cols)),
        Line | DashedLine => Placement {
            parts: vec![Part {
                shape: preset.shape_name(),
                geom: segment("line", a, b),
            }],
            labels: Vec::new(),
        },
        Arrow | Connector => Placement {
            parts: vec![Part {
                shape: preset.shape_name(),
                geom: segment("arrow", a, b),
            }],
            labels: Vec::new(),
        },
        DoubleArrow => {
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            Placement {
                parts: vec![Part {
                    shape: preset.shape_name(),
                    geom: along("arrow2", a, b, (len * 0.12).clamp(8.0, 40.0), false),
                }],
                labels: Vec::new(),
            }
        }
        CurvedArrow => {
            let len = (b[0] - a[0]).hypot(b[1] - a[1]);
            Placement {
                parts: vec![Part {
                    shape: preset.shape_name(),
                    geom: along("carrow", a, b, len * 0.3, true),
                }],
                labels: Vec::new(),
            }
        }
        NumberLine => number_line(a, b),
        Axes => axes(a, b),
        Venn2 | Venn3 => venn(preset, a, b),
    };
    if preset.dashed() {
        for part in &mut p.parts {
            part.geom.dash = true;
        }
    }
    p
}

fn number_line(a: Q, b: Q) -> Placement {
    let g0 = boxed("numberline", a, b, Vec::new());
    let (x, w) = (g0.x, g0.w.max(40.0));
    let h = g0.h.max(w * 0.06);
    let y = g0.y + g0.h / 2.0 - h / 2.0;
    let mut v = Vec::new();
    let mut labels = Vec::new();
    let size = (w * 0.035).clamp(18.0, 56.0);
    for n in -5i32..=5 {
        let t = 0.06 + 0.88 * f64::from(n + 5) / 10.0;
        v.extend([t, f64::from(n)]);
        labels.push(Label {
            text: if n < 0 {
                format!("−{}", -n)
            } else {
                n.to_string()
            },
            at: [x + t * w, y + h / 2.0 + size * 1.1],
            size,
        });
    }
    Placement {
        parts: vec![Part {
            shape: "numberline",
            geom: ShapeGeom {
                kind: "numberline".into(),
                x,
                y,
                w,
                h,
                v,
                ..Default::default()
            },
        }],
        labels,
    }
}

fn axes(a: Q, b: Q) -> Placement {
    let g = boxed("axes", a, b, vec![0.5, 0.5, 0.09, 0.09]);
    let size = (g.w.min(g.h) * 0.07).clamp(20.0, 56.0);
    let labels = vec![
        Label {
            text: "x".into(),
            at: [g.x + g.w - size * 0.4, g.y + g.h / 2.0 + size * 0.9],
            size,
        },
        Label {
            text: "y".into(),
            at: [g.x + g.w / 2.0 + size * 0.8, g.y + size * 0.5],
            size,
        },
    ];
    Placement {
        parts: vec![Part {
            shape: "axes",
            geom: g,
        }],
        labels,
    }
}

fn venn(preset: Preset, a: Q, b: Q) -> Placement {
    let g = boxed("ellipse", a, b, Vec::new());
    let three = preset == Preset::Venn3;
    let d = if three {
        (g.w / 1.6).min(g.h / 1.5)
    } else {
        (g.w / 1.6).min(g.h)
    }
    .max(8.0);
    let (cx, cy) = (g.x + g.w / 2.0, g.y + g.h / 2.0);
    let centres: Vec<Q> = if three {
        vec![
            [cx - 0.3 * d, cy - 0.25 * d],
            [cx + 0.3 * d, cy - 0.25 * d],
            [cx, cy + 0.25 * d],
        ]
    } else {
        vec![[cx - 0.3 * d, cy], [cx + 0.3 * d, cy]]
    };
    Placement {
        parts: centres
            .into_iter()
            .map(|c| Part {
                shape: "venn",
                geom: ShapeGeom {
                    kind: "ellipse".into(),
                    x: c[0] - d / 2.0,
                    y: c[1] - d / 2.0,
                    w: d,
                    h: d,
                    ..Default::default()
                },
            })
            .collect(),
        labels: Vec::new(),
    }
}

// ------------------------------------------------------------ new kinds

/// Box kinds this module draws (see the module docs).
pub fn is_kind(kind: &str) -> bool {
    matches!(kind, "rrect" | "callout" | "brace" | "arrow2" | "carrow")
}

struct Pen(Vec<Q>);

impl Pen {
    fn to(&mut self, p: Q) {
        self.0.push(p);
    }
    fn here(&self) -> Q {
        self.0[self.0.len() - 1]
    }
    fn arc(&mut self, c: Q, r: f64, a0: f64, a1: f64) {
        let n = ((a1 - a0).abs() / 0.15).ceil().max(2.0) as usize;
        for i in 1..=n {
            let a = a0 + (a1 - a0) * i as f64 / n as f64;
            self.to([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    fn quad(&mut self, c: Q, b: Q) {
        let a = self.here();
        for i in 1..=12 {
            let t = f64::from(i) / 12.0;
            let u = 1.0 - t;
            self.to([
                u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0],
                u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1],
            ]);
        }
    }
    fn spur(&mut self, p: Q) {
        let h = self.here();
        self.to(p);
        self.to(h);
    }
}

fn head(width: f32, len: f64) -> f64 {
    (f64::from(width) * 4.0 + 10.0).min(len * 0.3)
}

fn wings(tip: Q, u: Q, len: f64) -> (Q, Q) {
    let (c, s) = (0.5f64.cos(), 0.5f64.sin());
    let w = |sg: f64| {
        let (rx, ry) = (-u[0] * c + u[1] * s * sg, -u[1] * c - u[0] * s * sg);
        [tip[0] + rx * len, tip[1] + ry * len]
    };
    (w(1.0), w(-1.0))
}

/// A rounded rectangle's outline in the box (corner radius `r`), from the
/// top edge clockwise; `bottom` replaces the straight bottom edge (right to
/// left) when given.
fn rounded(w: f64, h: f64, r: f64, bottom: Option<&[Q]>) -> Vec<Q> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut p = Pen(vec![[r, 0.0]]);
    p.to([w - r, 0.0]);
    p.arc([w - r, r], r, -FRAC_PI_2, 0.0);
    p.to([w, h - r]);
    p.arc([w - r, h - r], r, 0.0, FRAC_PI_2);
    if let Some(b) = bottom {
        for q in b {
            p.to(*q);
        }
    }
    p.to([r, h]);
    p.arc([r, h - r], r, FRAC_PI_2, PI);
    p.to([0.0, r]);
    p.arc([r, r], r, PI, 1.5 * PI);
    p.0
}

/// Local points (box coordinates, before rotation) of one of this module's
/// kinds; closed outlines repeat their first point.
pub fn local_points(g: &ShapeGeom, width: f32) -> Option<Vec<Q>> {
    let (w, h) = (g.w, g.h);
    let v = &g.v;
    Some(match g.kind.as_str() {
        "rrect" => {
            let f = v.first().copied().unwrap_or(0.18).clamp(0.0, 0.5);
            rounded(w, h, f * w.min(h), None)
        }
        "callout" => {
            let tx = v.first().copied().unwrap_or(0.22).clamp(0.0, 1.0) * w;
            let ty = v.get(1).copied().unwrap_or(1.4).max(1.0) * h;
            let r = 0.18 * w.min(h);
            let half = (0.08 * w).max(4.0).min((w - 2.0 * r) / 4.0).max(0.0);
            let base = tx.clamp(r + half, (w - r - half).max(r + half));
            rounded(
                w,
                h,
                r,
                Some(&[[base + half, h], [tx, ty], [base - half, h]]),
            )
        }
        "brace" => {
            let side = v.first().copied().unwrap_or(0.0).round() as i64;
            let vertical = side < 2;
            let (len, depth) = if vertical { (h, w) } else { (w, h) };
            let r = (0.12 * len).min(depth);
            let m = depth / 2.0;
            // (along, across): the cusp at across 0, the arms at `depth`.
            let mut p = Pen(vec![[0.0, depth]]);
            p.quad([0.0, m], [r.min(len / 4.0), m]);
            p.to([len / 2.0 - r.min(len / 4.0), m]);
            p.quad([len / 2.0, m], [len / 2.0, 0.0]);
            p.quad([len / 2.0, m], [len / 2.0 + r.min(len / 4.0), m]);
            p.to([len - r.min(len / 4.0), m]);
            p.quad([len, m], [len, depth]);
            p.0.into_iter()
                .map(|[al, ac]| match side {
                    0 => [ac, al],
                    1 => [w - ac, al],
                    2 => [al, ac],
                    _ => [al, h - ac],
                })
                .collect()
        }
        "arrow2" => {
            let y = h / 2.0;
            let hl = head(width, w / 2.0);
            let (a1, b1) = wings([0.0, y], [-1.0, 0.0], hl);
            let (a2, b2) = wings([w, y], [1.0, 0.0], hl);
            let mut p = Pen(vec![a1]);
            p.to([0.0, y]);
            p.spur(b1);
            p.to([w, y]);
            p.spur(a2);
            p.to(b2);
            p.0
        }
        "carrow" => {
            let (s, c, e) = ([0.0, h], [w / 2.0, -h], [w, h]);
            let mut p = Pen(vec![s]);
            p.quad(c, e);
            let d = [e[0] - c[0], e[1] - c[1]];
            let n = d[0].hypot(d[1]).max(1e-9);
            let (a, b) = wings(e, [d[0] / n, d[1] / n], head(width, w * 0.8));
            p.spur(a);
            p.to(b);
            p.0
        }
        _ => return None,
    })
}

/// Anchors (connector attachment) of this module's kinds, box-local.
pub fn local_anchors(g: &ShapeGeom) -> Vec<(&'static str, Q)> {
    let (w, h) = (g.w, g.h);
    match g.kind.as_str() {
        "rrect" | "callout" => vec![
            ("n", [w / 2.0, 0.0]),
            ("e", [w, h / 2.0]),
            ("s", [w / 2.0, h]),
            ("w", [0.0, h / 2.0]),
        ],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- style

/// Kinds with an inside that a fill can paint.
pub fn fillable(kind: &str) -> bool {
    matches!(
        kind,
        "rect" | "ellipse" | "poly" | "stadium" | "rrect" | "callout" | "table"
    )
}

/// The closed outline a fill paints (world, the stroke's offset applied),
/// or `None` for open shapes, unknown kinds and no fill.
pub fn fill_outline(g: &ShapeGeom, points: &[StrokePoint], offset: [f32; 2]) -> Option<Vec<Q>> {
    g.fill?;
    if !fillable(&g.kind) || !g.is_valid() {
        return None;
    }
    let (ox, oy) = (f64::from(offset[0]), f64::from(offset[1]));
    if g.kind == "table" {
        // The box, not the retraced dividers.
        let (s, c) = g.rot.sin_cos();
        let (cx, cy) = (g.x + g.w / 2.0, g.y + g.h / 2.0);
        return Some(
            [
                [g.x, g.y],
                [g.x + g.w, g.y],
                [g.x + g.w, g.y + g.h],
                [g.x, g.y + g.h],
            ]
            .into_iter()
            .map(|[x, y]| {
                let (dx, dy) = (x - cx, y - cy);
                [cx + dx * c - dy * s + ox, cy + dx * s + dy * c + oy]
            })
            .collect(),
        );
    }
    (points.len() >= 3).then(|| {
        points
            .iter()
            .map(|p| [f64::from(p.x) + ox, f64::from(p.y) + oy])
            .collect()
    })
}

/// SVG path data of `poly` relative to its bounds, and the bounds
/// `[x, y, w, h]`, rounded to 0.1 world units.
pub fn svg_path(poly: &[Q]) -> (String, [f64; 4]) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in poly {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let r1 = |v: f64| (v * 10.0).round() / 10.0;
    let mut d = String::with_capacity(poly.len() * 12);
    for (i, p) in poly.iter().enumerate() {
        d.push(if i == 0 { 'M' } else { 'L' });
        d.push_str(&format!("{} {}", r1(p[0] - x0), r1(p[1] - y0)));
    }
    d.push('Z');
    (d, [r1(x0), r1(y0), r1(x1 - x0), r1(y1 - y0)])
}

/// Dash and gap lengths (world units) for a stroke of `width`.
pub fn dash_pattern(width: f32) -> (f32, f32) {
    (2.5 * width + 8.0, 1.8 * width + 7.0)
}

/// A dashed outline as solid pieces: runs of `points` cut at the dash
/// pattern by arc length (interpolated ends keep pressure and timing).
pub fn dash_pieces(points: &[StrokePoint], width: f32) -> Vec<Vec<StrokePoint>> {
    let (dash, gap) = dash_pattern(width.max(0.5));
    let period = dash + gap;
    let mut out: Vec<Vec<StrokePoint>> = Vec::new();
    let mut cur: Vec<StrokePoint> = Vec::new();
    let mut s = 0.0f32; // arc length at the segment start
    let lerp = |a: &StrokePoint, b: &StrokePoint, t: f32| StrokePoint {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        p: a.p + (b.p - a.p) * t,
        dt: (a.dt as f32 + (b.dt as f32 - a.dt as f32) * t).round() as u32,
    };
    let on = |s: f32| s.rem_euclid(period) < dash;
    if let Some(first) = points.first() {
        cur.push(*first);
    }
    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let len = (b.x - a.x).hypot(b.y - a.y);
        if !(len.is_finite()) || len <= 1e-6 {
            continue;
        }
        let end = s + len;
        // Every pattern boundary inside this segment: (arc length, ends a dash).
        let mut k = (s / period).floor() * period;
        let mut cuts: Vec<(f32, bool)> = Vec::new();
        while k < end && cuts.len() < 4096 {
            for (edge, ends) in [(k, false), (k + dash, true)] {
                if edge > s && edge < end {
                    cuts.push((edge, ends));
                }
            }
            k += period;
        }
        for (c, ends) in cuts {
            let q = lerp(a, b, (c - s) / len);
            if ends {
                cur.push(q);
                if cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            } else {
                cur.clear();
                cur.push(q);
            }
        }
        if on(end - 1e-3) {
            cur.push(*b);
        }
        s = end;
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes::{geom_aabb, geom_points, map_box};

    #[test]
    fn every_preset_places_valid_drawable_geometry() {
        for id in IDS
            .iter()
            .copied()
            .chain(["table:3x4", "table:10x10", "table:1x1"])
        {
            let p = Preset::parse(id).unwrap_or_else(|| panic!("{id}"));
            for (a, b) in [
                ([100.0, 100.0], [420.0, 300.0]),
                ([420.0, 300.0], [100.0, 100.0]),
            ] {
                let pl = place(p, a, b);
                assert!(!pl.parts.is_empty(), "{id}");
                for part in &pl.parts {
                    assert!(part.geom.is_valid(), "{id}: {:?}", part.geom);
                    let pts = geom_points(&part.geom, 4.0, 0.6).unwrap_or_else(|| panic!("{id}"));
                    assert!(pts.len() >= 2, "{id}");
                    assert!(pts.iter().all(|q| q.x.is_finite() && q.y.is_finite()));
                    assert!(geom_aabb(&part.geom).is_some(), "{id}");
                }
            }
        }
        assert_eq!(Preset::parse("table:11x2"), None);
        assert_eq!(Preset::parse("table:0x2"), None);
        assert_eq!(Preset::parse("blob"), None);
    }

    #[test]
    fn shift_keeps_boxes_square_and_snaps_segments() {
        let b = constrain(Preset::Rect, [0.0, 0.0], [100.0, -40.0], true);
        assert_eq!(b, [100.0, -100.0]);
        assert_eq!(
            constrain(Preset::Rect, [0.0, 0.0], [100.0, -40.0], false),
            [100.0, -40.0]
        );
        assert_eq!(
            constrain(Preset::Circle, [0.0, 0.0], [30.0, 50.0], false),
            [50.0, 50.0]
        );
        let s = constrain(Preset::Line, [0.0, 0.0], [100.0, 4.0], true);
        assert!((s[1]).abs() < 1e-9 && (s[0] - 100.08).abs() < 0.01);
    }

    #[test]
    fn a_tap_places_the_default_size_centred() {
        let (a, b) = tap_drag(Preset::Table { rows: 3, cols: 4 }, [500.0, 400.0], 200.0);
        let pl = place(Preset::Table { rows: 3, cols: 4 }, a, b);
        let g = &pl.parts[0].geom;
        assert!((g.x + g.w / 2.0 - 500.0).abs() < 1e-9 && (g.y + g.h / 2.0 - 400.0).abs() < 1e-9);
        assert_eq!(table_size(g), (3, 4));
        assert!((g.w - 4.0 * 110.0).abs() < 1e-9);
    }

    #[test]
    fn tables_report_their_size_and_edges() {
        let g = ShapeGeom {
            kind: "table".into(),
            w: 300.0,
            h: 200.0,
            v: table_v(4, 3),
            ..Default::default()
        };
        assert_eq!(table_size(&g), (4, 3));
        let (xs, ys) = table_edges(&g);
        assert_eq!(xs.len(), 4);
        assert_eq!(ys.len(), 5);
        assert!((ys[1] - 0.25).abs() < 1e-12);
        assert!(g.is_valid());
        // 10 × 10 fits the vertex limit.
        assert!(
            ShapeGeom {
                v: table_v(10, 10),
                ..g
            }
            .is_valid()
        );
    }

    #[test]
    fn new_kinds_resize_keeping_their_kind_and_style() {
        for id in [
            "rrect",
            "callout",
            "brace",
            "brace-down",
            "double-arrow",
            "curved-arrow",
        ] {
            let mut g = place(Preset::parse(id).unwrap(), [0.0, 0.0], [200.0, 100.0]).parts[0]
                .geom
                .clone();
            g.fill = Some(0x11223344);
            g.dash = true;
            let b = geom_aabb(&g).unwrap();
            let n = map_box(&g, b, [10.0, 10.0, 410.0, 310.0]).unwrap();
            assert_eq!(n.kind, g.kind);
            assert_eq!((n.fill, n.dash), (Some(0x11223344), true), "{id}");
            let nb = geom_aabb(&n).unwrap();
            assert!(
                (nb[2] - nb[0] - 400.0).abs() < 1.0 || g.rot != 0.0,
                "{id} {nb:?}"
            );
        }
    }

    #[test]
    fn rotated_polygons_keep_their_style_when_resized() {
        let mut g = place(Preset::Star, [0.0, 0.0], [100.0, 100.0]).parts[0]
            .geom
            .clone();
        g.rot = 0.4;
        g.fill = Some(0xff0000ff);
        g.dash = true;
        let b = geom_aabb(&g).unwrap();
        let n = map_box(&g, b, [0.0, 0.0, 300.0, 300.0]).unwrap();
        assert_eq!((n.fill, n.dash), (Some(0xff0000ff), true));
    }

    #[test]
    fn fills_follow_closed_shapes_only() {
        let mut g = place(Preset::Rect, [0.0, 0.0], [100.0, 50.0]).parts[0]
            .geom
            .clone();
        let pts = geom_points(&g, 4.0, 0.6).unwrap();
        assert!(fill_outline(&g, &pts, [0.0, 0.0]).is_none(), "no fill set");
        g.fill = Some(0xa7f3d0ff);
        let poly = fill_outline(&g, &pts, [10.0, 0.0]).unwrap();
        assert_eq!(poly[0], [10.0, 0.0]);
        let (d, b) = svg_path(&poly);
        assert!(d.starts_with("M0 0L") && d.ends_with('Z'));
        assert_eq!(b, [10.0, 0.0, 100.0, 50.0]);
        let mut line = place(Preset::Line, [0.0, 0.0], [100.0, 0.0]).parts[0]
            .geom
            .clone();
        line.fill = Some(0xa7f3d0ff);
        assert!(fill_outline(&line, &pts, [0.0, 0.0]).is_none());
        let mut t = place(
            Preset::Table { rows: 2, cols: 2 },
            [0.0, 0.0],
            [100.0, 50.0],
        )
        .parts[0]
            .geom
            .clone();
        t.fill = Some(1);
        assert_eq!(fill_outline(&t, &[], [0.0, 0.0]).unwrap().len(), 4);
    }

    #[test]
    fn dashes_cover_about_the_dash_share_of_the_line() {
        let pts: Vec<StrokePoint> = [[0.0, 0.0], [300.0, 0.0], [300.0, 200.0]]
            .iter()
            .map(|p| StrokePoint {
                x: p[0],
                y: p[1],
                p: 0.6,
                dt: 0,
            })
            .collect();
        let width = 4.0;
        let pieces = dash_pieces(&pts, width);
        let (dash, gap) = dash_pattern(width);
        let drawn: f32 = pieces
            .iter()
            .map(|r| {
                r.windows(2)
                    .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                    .sum::<f32>()
            })
            .sum();
        let share = drawn / 500.0;
        let want = dash / (dash + gap);
        assert!((share - want).abs() < 0.06, "{share} vs {want}");
        assert!(pieces.len() >= 10);
        // A dash that turns the corner keeps the corner point.
        assert!(
            pieces
                .iter()
                .any(|r| r.iter().any(|q| q.x == 300.0 && q.y == 0.0))
        );
        assert!(dash_pieces(&pts[..1], width).is_empty());
    }
}
