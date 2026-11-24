//! Points and anchors of the Smart Ink Phase 2 templates (`axes`,
//! `numberline`, `stadium`, `arc`, `table`, `gate-*`, `resistor`, `cell`,
//! `bulb`, `switch`, `harpoon`, `path`, `polyline`). See the Shape geometry
//! section of `spec/rupok-format.md` for each kind's numbers.
//!
//! A stroke is one polyline, so parts that a pen would draw separately
//! (tick marks, pins, table dividers) are reached by retracing lines already
//! drawn: the retraced stretch is invisible, and replay still draws each
//! part in turn. Parts that touch nothing (a cell's second plate, XOR's back
//! curve) are separate strokes in the same group, told apart by `v[0]`.

use rupok_core::ShapeGeom;
use rupok_ink::recognize::logic::gate_layout;

type Q = [f64; 2];

/// Box kinds: `x`, `y`, `w`, `h` is the unrotated box turned by `rot`.
pub fn is_box_kind(kind: &str) -> bool {
    matches!(
        kind,
        "axes"
            | "numberline"
            | "stadium"
            | "arc"
            | "table"
            | "resistor"
            | "cell"
            | "bulb"
            | "switch"
    ) || kind.starts_with("gate-")
        || crate::catalog::is_kind(kind)
}

/// Open polylines through `v` normalised to the box (like `poly`).
pub fn is_path_kind(kind: &str) -> bool {
    matches!(kind, "path" | "polyline")
}

/// Template kinds this build draws.
pub fn is_template(kind: &str) -> bool {
    is_box_kind(kind) || is_path_kind(kind) || kind == "harpoon"
}

/// Local box coordinates → world (rotation about the box centre).
struct Frame {
    x: f64,
    y: f64,
    c: Q,
    s: f64,
    co: f64,
}

impl Frame {
    fn of(g: &ShapeGeom) -> Self {
        let (s, co) = g.rot.sin_cos();
        Self {
            x: g.x,
            y: g.y,
            c: [g.x + g.w / 2.0, g.y + g.h / 2.0],
            s,
            co,
        }
    }
    fn at(&self, lx: f64, ly: f64) -> Q {
        let (dx, dy) = (self.x + lx - self.c[0], self.y + ly - self.c[1]);
        [
            self.c[0] + dx * self.co - dy * self.s,
            self.c[1] + dx * self.s + dy * self.co,
        ]
    }
}

/// Builds one polyline in local coordinates.
struct Pen(Vec<Q>);

impl Pen {
    fn new(p: Q) -> Self {
        Pen(vec![p])
    }
    fn to(&mut self, p: Q) {
        self.0.push(p);
    }
    fn here(&self) -> Q {
        self.0[self.0.len() - 1]
    }
    /// Out to `p` and back (a tick, a pin).
    fn spur(&mut self, p: Q) {
        let h = self.here();
        self.to(p);
        self.to(h);
    }
    fn arc(&mut self, c: Q, rx: f64, ry: f64, a0: f64, a1: f64) {
        let n = ((a1 - a0).abs() / 0.12).ceil().max(2.0) as usize;
        for i in 1..=n {
            let a = a0 + (a1 - a0) * i as f64 / n as f64;
            self.to([c[0] + rx * a.cos(), c[1] + ry * a.sin()]);
        }
    }
    fn quad(&mut self, c: Q, b: Q) {
        let a = self.here();
        for i in 1..=16 {
            let t = f64::from(i) / 16.0;
            let u = 1.0 - t;
            self.to([
                u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0],
                u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1],
            ]);
        }
    }
}

/// An arrowhead's wings at `tip` pointing along unit `u`.
fn wings(tip: Q, u: Q, head: f64) -> (Q, Q) {
    let (c, s) = (0.5f64.cos(), 0.5f64.sin());
    let w = |sg: f64| {
        let (rx, ry) = (-u[0] * c + u[1] * s * sg, -u[1] * c - u[0] * s * sg);
        [tip[0] + rx * head, tip[1] + ry * head]
    };
    (w(1.0), w(-1.0))
}

fn head_len(width: f32, len: f64) -> f64 {
    (f64::from(width) * 4.0 + 10.0).min(len * 0.3)
}

/// Local points of a box kind (closed shapes repeat their first point).
fn box_points(g: &ShapeGeom, width: f32) -> Option<Vec<Q>> {
    let (w, h) = (g.w, g.h);
    let v = &g.v;
    let wd = f64::from(width);
    let pts = match g.kind.as_str() {
        "axes" => {
            let (ox, oy) = (v.first()? * w, v.get(1)? * h);
            let (ux, uy) = (v.get(2)? * w, v.get(3)? * h);
            if ux <= 0.0 || uy <= 0.0 {
                return None;
            }
            let t = (1.5 * wd + 3.0).min(0.3 * ux.min(uy));
            let hx = head_len(width, w - ox);
            let hy = head_len(width, oy);
            let mut p = Pen::new([0.0, oy]);
            // x axis left to right, ticks on the way, arrow at the end.
            let mut k = ((0.0 - ox) / ux).ceil() as i64;
            loop {
                let x = ox + k as f64 * ux;
                if x > w - 1.3 * hx {
                    break;
                }
                if k != 0 && x > 0.5 * t {
                    p.to([x, oy]);
                    p.to([x, oy - t]);
                    p.to([x, oy + t]);
                    p.to([x, oy]);
                }
                k += 1;
            }
            p.to([w, oy]);
            let (a, b) = wings([w, oy], [1.0, 0.0], hx);
            p.spur(a);
            p.spur(b);
            // Back to the origin (retraced), down the y axis and up again.
            p.to([ox, oy]);
            let mut k = 1;
            while oy + k as f64 * uy < h - 0.5 * t {
                let y = oy + k as f64 * uy;
                p.to([ox, y]);
                p.spur([ox - t, y]);
                p.spur([ox + t, y]);
                k += 1;
            }
            p.to([ox, h]);
            p.to([ox, oy]);
            let mut k = 1;
            while oy - k as f64 * uy > 1.3 * hy {
                let y = oy - k as f64 * uy;
                p.to([ox, y]);
                p.spur([ox - t, y]);
                p.spur([ox + t, y]);
                k += 1;
            }
            p.to([ox, 0.0]);
            let (a, b) = wings([ox, 0.0], [0.0, -1.0], hy);
            p.spur(a);
            p.to(b);
            p.0
        }
        "numberline" => {
            let ticks: Vec<f64> = v
                .chunks(2)
                .filter(|c| c.len() == 2)
                .map(|c| c[0] * w)
                .collect();
            let gap = ticks
                .windows(2)
                .map(|w| w[1] - w[0])
                .fold(f64::INFINITY, f64::min);
            let t = (0.18 * gap).clamp(2.0 * wd + 3.0, (2.0 * wd + 3.0).max(16.0));
            let y = h / 2.0;
            let head = head_len(width, w / 2.0);
            let (a, b) = wings([0.0, y], [-1.0, 0.0], head);
            let mut p = Pen::new(a);
            p.to([0.0, y]);
            p.spur(b);
            for x in ticks {
                p.to([x, y]);
                p.spur([x, y - t]);
                p.spur([x, y + t]);
            }
            p.to([w, y]);
            let (a, b) = wings([w, y], [1.0, 0.0], head);
            p.spur(a);
            p.to(b);
            p.0
        }
        "stadium" => {
            let r = w.min(h) / 2.0;
            let mut p;
            if w >= h {
                p = Pen::new([r, 0.0]);
                p.to([w - r, 0.0]);
                p.arc([w - r, r], r, r, -FRAC_PI_2, FRAC_PI_2);
                p.to([r, h]);
                p.arc([r, r], r, r, FRAC_PI_2, 1.5 * PI);
            } else {
                p = Pen::new([w, r]);
                p.to([w, h - r]);
                p.arc([r, h - r], r, r, 0.0, PI);
                p.to([0.0, r]);
                p.arc([r, r], r, r, PI, TAU);
            }
            p.0
        }
        "arc" => {
            let (a0, sweep) = (*v.first()?, *v.get(1)?);
            let mut p = Pen::new([w / 2.0 + w / 2.0 * a0.cos(), h / 2.0 + h / 2.0 * a0.sin()]);
            p.arc([w / 2.0, h / 2.0], w / 2.0, h / 2.0, a0, a0 + sweep);
            p.0
        }
        "table" => {
            let mut p = Pen::new([0.0, 0.0]);
            p.to([w, 0.0]);
            p.to([w, h]);
            p.to([0.0, h]);
            p.to([0.0, 0.0]);
            let mut cols: Vec<f64> = Vec::new();
            let mut rows: Vec<f64> = Vec::new();
            for d in v.chunks(2).filter(|c| c.len() == 2) {
                if d[0] == 0.0 {
                    cols.push(d[1] * w);
                } else {
                    rows.push(d[1] * h);
                }
            }
            cols.sort_by(f64::total_cmp);
            rows.sort_by(f64::total_cmp);
            for x in cols {
                p.to([x, 0.0]);
                p.spur([x, h]);
            }
            p.to([0.0, 0.0]);
            for y in rows {
                p.to([0.0, y]);
                p.spur([w, y]);
            }
            p.0
        }
        "resistor" => {
            let lead = v.first().copied().unwrap_or(0.2).clamp(0.05, 0.45) * w;
            let teeth = v.get(1).copied().unwrap_or(6.0).clamp(2.0, 12.0) as usize;
            let mut p = Pen::new([0.0, h / 2.0]);
            p.to([lead, h / 2.0]);
            let zig = w - 2.0 * lead;
            for k in 0..teeth {
                let t = (k as f64 + 0.5) / teeth as f64;
                p.to([lead + t * zig, if k % 2 == 0 { 0.0 } else { h }]);
            }
            p.to([w - lead, h / 2.0]);
            p.to([w, h / 2.0]);
            p.0
        }
        "cell" => {
            let lead = v.get(1).copied().unwrap_or(0.4).clamp(0.05, 0.48) * w;
            if v.first().copied().unwrap_or(0.0) < 0.5 {
                let mut p = Pen::new([0.0, h / 2.0]);
                p.to([lead, h / 2.0]);
                p.to([lead, 0.0]);
                p.to([lead, h]);
                p.0
            } else {
                let x = w - lead;
                let mut p = Pen::new([x, 0.25 * h]);
                p.to([x, 0.75 * h]);
                p.to([x, h / 2.0]);
                p.to([w, h / 2.0]);
                p.0
            }
        }
        "bulb" => {
            let r = h / 2.0;
            let c = [w / 2.0, h / 2.0];
            let at = |a: f64| [c[0] + r * a.cos(), c[1] + r * a.sin()];
            let mut p = Pen::new([0.0, c[1]]);
            p.to(at(PI));
            p.arc(c, r, r, PI, 3.0 * PI);
            p.arc(c, r, r, PI, 1.25 * PI);
            p.to(at(0.25 * PI));
            p.arc(c, r, r, 0.25 * PI, 0.0);
            p.spur([w, c[1]]);
            p.arc(c, r, r, 0.0, -0.25 * PI);
            p.to(at(0.75 * PI));
            p.0
        }
        "switch" => {
            let part = v.first().copied().unwrap_or(0.0);
            let pivot = v.get(1).copied().unwrap_or(0.4).clamp(0.0, 1.0) * w;
            let contact = v.get(2).copied().unwrap_or(0.7).clamp(0.0, 1.0) * w;
            let flip = v.get(3).copied().unwrap_or(0.0) >= 0.5;
            let (wire, tip) = if flip { (0.0, h) } else { (h, 0.0) };
            if part < 0.5 {
                let mut p = Pen::new([0.0, wire]);
                p.to([pivot, wire]);
                p.to([contact, tip]);
                p.0
            } else {
                let mut p = Pen::new([contact, wire]);
                p.to([w, wire]);
                p.0
            }
        }
        k if k.starts_with("gate-") => {
            gate_points(k, w, h, v.get(1).copied().unwrap_or(0.0) >= 0.5)?
        }
        k if crate::catalog::is_kind(k) => crate::catalog::local_points(g, width)?,
        _ => return None,
    };
    Some(pts)
}

use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Input pins' heights (fractions of the box height) for `n` inputs.
fn input_ys(n: usize) -> Vec<f64> {
    if n <= 1 {
        vec![0.5]
    } else {
        (0..n)
            .map(|i| 0.28 + 0.44 * i as f64 / (n - 1) as f64)
            .collect()
    }
}

/// A gate facing right in its box; `arc_part` draws XOR's back curve and
/// its input pins instead of the body.
fn gate_points(kind: &str, w: f64, h: f64, arc_part: bool) -> Option<Vec<Q>> {
    let lay = gate_layout(kind)?;
    let unit = w / f64::from(lay.total());
    let f = |x: f32| f64::from(x) * unit;
    let bx0 = f(lay.pin_in + lay.gap);
    let bx1 = bx0 + f(lay.body);
    let bub = f(lay.bubble);
    let ys: Vec<f64> = input_ys(if kind == "gate-not" { 1 } else { 2 })
        .into_iter()
        .map(|t| t * h)
        .collect();
    let or_like = matches!(kind, "gate-or" | "gate-nor" | "gate-xor" | "gate-xnor");
    // OR's concave back: x of the back curve at height y.
    let back_bow = 0.22 * (bx1 - bx0);
    let back_x = |x0: f64, y: f64| {
        let t = y / h;
        x0 + back_bow * 4.0 * t * (1.0 - t)
    };
    if arc_part {
        let ax = f(lay.pin_in);
        let (y0, y1) = (ys[0], ys[ys.len() - 1]);
        let mut p = Pen::new([0.0, y0]);
        p.to([back_x(ax, y0), y0]);
        for i in 1..=8 {
            let y = y0 * (1.0 - f64::from(i) / 8.0);
            p.to([back_x(ax, y), y]);
        }
        for i in 1..=32 {
            let y = h * f64::from(i) / 32.0;
            p.to([back_x(ax, y), y]);
        }
        for i in 1..=8 {
            let y = h - (h - y1) * f64::from(i) / 8.0;
            p.to([back_x(ax, y), y]);
        }
        p.to([0.0, y1]);
        return Some(p.0);
    }
    let tip = [bx1, h / 2.0];
    // Output pin (and the bubble), drawn when the outline reaches the tip.
    let output = |p: &mut Pen| {
        if bub > 0.0 {
            let c = [bx1 + bub / 2.0, h / 2.0];
            p.arc(c, bub / 2.0, bub / 2.0, PI, 2.0 * PI);
            p.spur([w, h / 2.0]);
            p.arc(c, bub / 2.0, bub / 2.0, 0.0, PI);
        } else {
            p.spur([w, h / 2.0]);
        }
    };
    let mut p;
    if kind == "gate-not" {
        p = Pen::new([0.0, ys[0]]);
        p.to([bx0, ys[0]]);
        p.to([bx0, 0.0]);
        p.to(tip);
        output(&mut p);
        p.to([bx0, h]);
        p.to([bx0, ys[0]]);
    } else if or_like {
        let y0 = ys[0];
        p = Pen::new([0.0, y0]);
        p.to([back_x(bx0, y0), y0]);
        for i in (0..=8).rev() {
            let y = y0 * f64::from(i) / 8.0;
            p.to([back_x(bx0, y), y]);
        }
        let len = bx1 - bx0;
        p.quad([bx0 + 0.55 * len, 0.0], tip);
        output(&mut p);
        p.quad([bx0 + 0.55 * len, h], [bx0, h]);
        let y1 = ys[1];
        for i in 0..=8 {
            let y = h - (h - y1) * f64::from(i) / 8.0;
            p.to([back_x(bx0, y), y]);
        }
        p.spur([0.0, y1]);
        for i in 0..=16 {
            let y = y1 - (y1 - y0) * f64::from(i) / 16.0;
            p.to([back_x(bx0, y), y]);
        }
    } else {
        // AND: flat back, flat top, round front.
        let r = (h / 2.0).min(bx1 - bx0);
        let flat = bx1 - r;
        p = Pen::new([0.0, ys[0]]);
        p.to([bx0, ys[0]]);
        p.to([bx0, 0.0]);
        p.to([flat, 0.0]);
        p.arc([flat, h / 2.0], r, h / 2.0, -FRAC_PI_2, 0.0);
        output(&mut p);
        p.arc([flat, h / 2.0], r, h / 2.0, 0.0, FRAC_PI_2);
        p.to([bx0, h]);
        p.to([bx0, ys[1]]);
        p.spur([0.0, ys[1]]);
        p.to([bx0, ys[0]]);
    }
    Some(p.0)
}

/// Uniform Catmull–Rom through `c`, densely sampled.
fn spline(c: &[Q]) -> Vec<Q> {
    if c.len() < 3 {
        return c.to_vec();
    }
    let mut out = vec![c[0]];
    for i in 0..c.len() - 1 {
        let p0 = c[i.saturating_sub(1)];
        let (p1, p2) = (c[i], c[i + 1]);
        let p3 = c[(i + 2).min(c.len() - 1)];
        let seg = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
        let n = (seg / 6.0).ceil().clamp(1.0, 12.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, cc: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + cc) * t
                    + (2.0 * a - 5.0 * b + 4.0 * cc - d) * t2
                    + (-a + 3.0 * b - 3.0 * cc + d) * t3)
            };
            out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out
}

/// World points of a template, or `None` when the kind is unknown or its
/// numbers are unusable.
pub fn points(g: &ShapeGeom, width: f32) -> Option<Vec<Q>> {
    if is_path_kind(&g.kind) {
        let v: Vec<Q> =
            g.v.as_chunks::<2>()
                .0
                .iter()
                .map(|q| [g.x + q[0] * g.w, g.y + q[1] * g.h])
                .collect();
        if v.len() < 2 {
            return None;
        }
        return Some(if g.kind == "path" { spline(&v) } else { v });
    }
    if g.kind == "harpoon" {
        let (a, b) = ([g.x, g.y], [g.x + g.w, g.y + g.h]);
        let len = g.w.hypot(g.h).max(1e-6);
        let u = [g.w / len, g.h / len];
        let head = head_len(width, len);
        // One barb, on the left of travel (above for a rightward arrow).
        let (c, s) = (0.5f64.cos(), 0.5f64.sin());
        let barb = [
            b[0] + (-u[0] * c + u[1] * s) * head,
            b[1] + (-u[1] * c - u[0] * s) * head,
        ];
        return Some(vec![a, b, barb]);
    }
    let local = box_points(g, width)?;
    let f = Frame::of(g);
    Some(local.into_iter().map(|q| f.at(q[0], q[1])).collect())
}

/// Named attachment points (world) of a template, for connectors.
pub fn anchors(g: &ShapeGeom) -> Vec<(&'static str, Q)> {
    let f = Frame::of(g);
    let (w, h) = (g.w, g.h);
    let sides = || {
        vec![
            ("n", f.at(w / 2.0, 0.0)),
            ("e", f.at(w, h / 2.0)),
            ("s", f.at(w / 2.0, h)),
            ("w", f.at(0.0, h / 2.0)),
        ]
    };
    match g.kind.as_str() {
        "stadium" | "table" => sides(),
        "resistor" | "cell" | "bulb" => vec![("in", f.at(0.0, h / 2.0)), ("out", f.at(w, h / 2.0))],
        "switch" => {
            let wire = if g.v.get(3).copied().unwrap_or(0.0) >= 0.5 {
                0.0
            } else {
                h
            };
            vec![("in", f.at(0.0, wire)), ("out", f.at(w, wire))]
        }
        k if k.starts_with("gate-") => {
            let n = if k == "gate-not" { 1 } else { 2 };
            let mut out: Vec<(&'static str, Q)> = input_ys(n)
                .into_iter()
                .map(|t| ("in", f.at(0.0, t * h)))
                .collect();
            out.push(("out", f.at(w, h / 2.0)));
            out
        }
        "harpoon" => vec![("start", [g.x, g.y]), ("end", [g.x + g.w, g.y + g.h])],
        k if crate::catalog::is_kind(k) => crate::catalog::local_anchors(g)
            .into_iter()
            .map(|(n, q)| (n, f.at(q[0], q[1])))
            .collect(),
        _ => Vec::new(),
    }
}
