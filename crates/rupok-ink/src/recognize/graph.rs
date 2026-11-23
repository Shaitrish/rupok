//! Graphs: clean x/y axes from a big "+" (two strokes) or an "L" (one
//! stroke), and a curve sketched over axes fitted to a line, a parabola, a
//! sine wave or an exponential, with its equation as a label.
//!
//! The axes' tick spacing is the unit: a curve's equation is in ticks.
//! A fit is refused when no family fits well (a squiggle stays ink), and the
//! simplest family that fits about as well as the best one wins (a slightly
//! bent line is a line, not a parabola).

use super::registry::{Context, Label, Proposal, Recognizer};
use super::{
    DEG, Geometry, P, PI, Recognized, ShapeKind, TAU, as_segment, bbox, corners, dist, family,
    max_dev, path_len, resample, straight,
};

/// Shortest axis, screen px.
pub const MIN_AXIS_PX: f32 = 150.0;
/// Shortest arm of an "L", screen px.
pub const MIN_L_ARM_PX: f32 = 160.0;
/// The second stroke of a "+" must come this soon (ms) after the first.
pub const AXES_WINDOW_MS: f64 = 4000.0;
/// Axes within this many degrees of level / upright.
const AXIS_TOL_DEG: f32 = 12.0;
/// A fit's mean distance from the ink, as a share of the ink's diagonal.
const FIT_MEAN: f64 = 0.03;
/// A fit's worst distance from the ink, as a share of the diagonal.
const FIT_MAX: f64 = 0.12;

/// The axes' frame: origin (world), world units per math unit along x and
/// y, and the box `[x0, y0, x1, y1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub o: P,
    pub ux: f32,
    pub uy: f32,
    pub b: [f32; 4],
}

impl Frame {
    /// The frame of an `axes` template.
    pub fn of(g: &Geometry) -> Option<Self> {
        let Geometry::Template {
            kind: "axes",
            x,
            y,
            w,
            h,
            v,
            ..
        } = g
        else {
            return None;
        };
        if v.len() < 4 || *w <= 0.0 || *h <= 0.0 || v[2] <= 0.0 || v[3] <= 0.0 {
            return None;
        }
        Some(Self {
            o: [x + v[0] * w, y + v[1] * h],
            ux: v[2] * w,
            uy: v[3] * h,
            b: [*x, *y, x + w, y + h],
        })
    }
    fn to_math(self, q: P) -> [f64; 2] {
        [
            f64::from((q[0] - self.o[0]) / self.ux),
            f64::from((self.o[1] - q[1]) / self.uy),
        ]
    }
    fn to_world(self, m: [f64; 2]) -> P {
        [
            self.o[0] + m[0] as f32 * self.ux,
            self.o[1] - m[1] as f32 * self.uy,
        ]
    }
}

/// `Some(true)` for a level segment, `Some(false)` for an upright one.
fn level(a: P, b: P) -> Option<bool> {
    let th = (b[1] - a[1]).atan2(b[0] - a[0]).abs();
    let off = th.min(PI - th);
    if off <= AXIS_TOL_DEG * DEG {
        Some(true)
    } else if (off - PI / 2.0).abs() <= AXIS_TOL_DEG * DEG {
        Some(false)
    } else {
        None
    }
}

/// Axes from a level segment `h` and an upright one `v` (any order of
/// ends). `None` unless they cross (or meet) with a positive x half to the
/// right of the origin and a positive y half above it.
fn axes_from(h: (P, P), v: (P, P), px: f32) -> Option<Proposal> {
    let hy = (h.0[1] + h.1[1]) / 2.0;
    let vx = (v.0[0] + v.1[0]) / 2.0;
    let (mut x0, mut x1) = (h.0[0].min(h.1[0]), h.0[0].max(h.1[0]));
    let (mut y0, mut y1) = (v.0[1].min(v.1[1]), v.0[1].max(v.1[1]));
    let (lh, lv) = (x1 - x0, y1 - y0);
    // The axes meet: the origin lies on both (a little short is fine).
    if vx < x0 - 0.08 * lh || vx > x1 + 0.08 * lh || hy < y0 - 0.08 * lv || hy > y1 + 0.08 * lv {
        return None;
    }
    // An end within 12 % of the origin is the origin (an "L", no stubs).
    if vx - x0 < 0.12 * lh {
        x0 = vx;
    }
    if y1 - hy < 0.12 * lv {
        y1 = hy;
    }
    x1 = x1.max(vx);
    y0 = y0.min(hy);
    let (px_half, py_half) = (x1 - vx, hy - y0);
    if px_half < 0.35 * lh || py_half < 0.35 * lv {
        // A "T", a "⊥" or an "L" facing the wrong way: not axes.
        return None;
    }
    let short = px_half.min(py_half);
    let unit = (short / 5.5).max(18.0 / px);
    let (w, hgt) = (x1 - x0, y1 - y0);
    let size = (0.5 * unit).clamp(16.0 / px, 30.0 / px);
    let mut p = Proposal::new(
        Recognized {
            kind: ShapeKind::Axes,
            geometry: Geometry::Template {
                kind: "axes",
                x: x0,
                y: y0,
                w,
                h: hgt,
                rot: 0.0,
                v: vec![(vx - x0) / w, (hy - y0) / hgt, unit / w, unit / hgt],
            },
            confidence: 1.0,
        },
        Vec::new(),
        "axes",
    );
    p.labels = vec![
        Label {
            text: "x".into(),
            at: [x1 + 0.75 * size, hy],
            size,
        },
        Label {
            text: "y".into(),
            at: [vx, y0 - 0.8 * size],
            size,
        },
    ];
    Some(p)
}

/// The second stroke of a big "+" (or an "L" in two strokes), drawn right
/// after the first, perpendicular to it and crossing it.
pub struct AxesPlus;

impl Recognizer for AxesPlus {
    fn name(&self) -> &'static str {
        "axes"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GRAPHS) || stroke.len() < 2 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let (sa, sb) = straight(&resample(stroke, 64), 0.06)?;
        let ls = dist(sa, sb);
        if ls * px < MIN_AXIS_PX {
            return None;
        }
        let s_level = level(sa, sb)?;
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if !(0.0..=AXES_WINDOW_MS).contains(&n.gap_ms)
                || n.shape
                    .as_deref()
                    .is_some_and(|s| s != "line" && s != "arrow")
            {
                return None;
            }
            let (na, nb) = as_segment(n)?;
            let ln = dist(na, nb);
            if ln * px < MIN_AXIS_PX || ln.min(ls) / ln.max(ls) < 0.25 || level(na, nb)? == s_level
            {
                return None;
            }
            let (h, v) = if s_level {
                ((sa, sb), (na, nb))
            } else {
                ((na, nb), (sa, sb))
            };
            let mut p = axes_from(h, v, px)?;
            p.absorbs = vec![i];
            Some(p)
        })
    }
}

/// One stroke shaped like an "L": up the y axis and along the x axis (or
/// the other way round), both arms long and straight.
pub struct AxesL;

impl Recognizer for AxesL {
    fn name(&self) -> &'static str {
        "axes-l"
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GRAPHS) || stroke.len() < 3 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let p = resample(stroke, 96);
        let c = corners(&p, 3, 40.0 * DEG, false);
        let [k] = c.as_slice() else {
            return None;
        };
        let (a, m, b) = (p[0], p[*k], p[p.len() - 1]);
        let (l1, l2) = (dist(a, m), dist(m, b));
        if l1.min(l2) * px < MIN_L_ARM_PX
            || l1.min(l2) / l1.max(l2) < 0.3
            || max_dev(&p[..=*k], a, m) > 0.05 * l1
            || max_dev(&p[*k..], m, b) > 0.05 * l2
        {
            return None;
        }
        let (h_end, v_end) = match (level(a, m)?, level(m, b)?) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            _ => return None,
        };
        // First quadrant only: x to the right of the corner, y above it.
        if h_end[0] <= m[0] || v_end[1] >= m[1] {
            return None;
        }
        axes_from((m, h_end), (v_end, m), px)
    }
}

// ------------------------------------------------------------------ fits

/// A fitted family and its parameters (math units: one tick is 1).
///
/// - `GraphLine`: `[m, c]`, y = mx + c
/// - `GraphQuad`: `[a, b, c]`, y = ax² + bx + c
/// - `GraphSine`: `[a, b, c, d]`, y = a sin(bx + c) + d
/// - `GraphExp`: `[a, b, d]`, y = a·bˣ + d
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub kind: ShapeKind,
    pub params: Vec<f64>,
}

impl Fit {
    pub fn eval(&self, x: f64) -> f64 {
        let p = &self.params;
        match self.kind {
            ShapeKind::GraphLine => p[0] * x + p[1],
            ShapeKind::GraphQuad => (p[0] * x + p[1]) * x + p[2],
            ShapeKind::GraphSine => p[0] * (p[1] * x + p[2]).sin() + p[3],
            ShapeKind::GraphExp => p[0] * p[1].powf(x) + p[2],
            _ => f64::NAN,
        }
    }

    /// The equation, e.g. `y = x² − 2x − 3`, `y = 2 sin(x + π/2)`, `y = 3·2ˣ − 1`.
    pub fn label(&self) -> String {
        let p = &self.params;
        let body = match self.kind {
            ShapeKind::GraphLine => poly_terms(&[(p[0], "x"), (p[1], "")]),
            ShapeKind::GraphQuad => poly_terms(&[(p[0], "x²"), (p[1], "x"), (p[2], "")]),
            ShapeKind::GraphSine => {
                let inner = poly_terms_with(&[(p[1], "x")], &[(p[2], true)]);
                let s = format!("sin({inner})");
                let mut out = if (p[0].abs() - 1.0).abs() < 1e-9 {
                    coef(p[0], &s, true)
                } else {
                    format!("{} {s}", num(p[0]))
                };
                out.push_str(&constant(p[3], false));
                out
            }
            ShapeKind::GraphExp => {
                let base = format!("{}ˣ", num(p[1]));
                let head = if (p[0] - 1.0).abs() < 1e-9 {
                    base
                } else if (p[0] + 1.0).abs() < 1e-9 {
                    format!("−{base}")
                } else {
                    format!("{}·{base}", num(p[0]))
                };
                format!("{head}{}", constant(p[2], false))
            }
            _ => String::new(),
        };
        format!("y = {body}")
    }
}

/// A number with at most two decimals, a real minus sign, no trailing zeros.
fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round().abs() as i64)
    } else {
        let t = format!("{:.2}", r.abs());
        t.trim_end_matches('0').to_string()
    };
    if r < 0.0 { format!("−{s}") } else { s }
}

/// `c·sym` as a leading term: 1 and −1 hide, 0 is empty.
fn coef(c: f64, sym: &str, lead: bool) -> String {
    if c.abs() < 1e-9 {
        return String::new();
    }
    let sign = if c < 0.0 {
        if lead { "−" } else { " − " }
    } else if lead {
        ""
    } else {
        " + "
    };
    let mag = c.abs();
    let m = if (mag - 1.0).abs() < 1e-9 && !sym.is_empty() {
        String::new()
    } else {
        num(mag)
    };
    format!("{sign}{m}{sym}")
}

fn constant(c: f64, lead: bool) -> String {
    coef(c, "", lead)
}

/// `a x² + b x + c` style sums, dropping zero terms.
fn poly_terms(terms: &[(f64, &str)]) -> String {
    poly_terms_with(terms, &[])
}

/// Sums whose trailing constants may be written as multiples of π.
fn poly_terms_with(terms: &[(f64, &str)], consts: &[(f64, bool)]) -> String {
    let mut out = String::new();
    for &(c, sym) in terms {
        out.push_str(&coef(c, sym, out.is_empty()));
    }
    for &(c, pi) in consts {
        if c.abs() < 1e-9 {
            continue;
        }
        let lead = out.is_empty();
        let sign = match (c < 0.0, lead) {
            (true, true) => "−",
            (true, false) => " − ",
            (false, true) => "",
            (false, false) => " + ",
        };
        let mag = if pi { pi_frac(c.abs()) } else { num(c.abs()) };
        out.push_str(&format!("{sign}{mag}"));
    }
    if out.is_empty() { "0".into() } else { out }
}

/// A multiple of π/12 as `π/4`, `2π/3`, …; anything else as a decimal.
fn pi_frac(v: f64) -> String {
    let k = v / (std::f64::consts::PI / 12.0);
    if (k - k.round()).abs() > 1e-6 || k.round() == 0.0 {
        return num(v);
    }
    let k = k.round() as i64;
    let g = gcd(k, 12);
    let (n, d) = (k / g, 12 / g);
    let n = if n == 1 { String::new() } else { n.to_string() };
    if d == 1 {
        format!("{n}π")
    } else {
        format!("{n}π/{d}")
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// Least squares for `y ≈ Σ c_k · basis_k(x)` (at most 4 terms).
fn lsq(xs: &[f64], ys: &[f64], basis: &dyn Fn(f64) -> [f64; 4], k: usize) -> Option<Vec<f64>> {
    let mut m = [[0.0f64; 5]; 4];
    for (&x, &y) in xs.iter().zip(ys) {
        let b = basis(x);
        for i in 0..k {
            for j in 0..k {
                m[i][j] += b[i] * b[j];
            }
            m[i][4] += b[i] * y;
        }
    }
    // Gaussian elimination with partial pivoting.
    for col in 0..k {
        let piv = (col..k).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[piv][col].abs() < 1e-12 {
            return None;
        }
        m.swap(col, piv);
        for r in 0..k {
            if r != col {
                let f = m[r][col] / m[col][col];
                for c in col..5 {
                    m[r][c] -= f * m[col][c];
                }
            }
        }
    }
    Some((0..k).map(|i| m[i][4] / m[i][i]).collect())
}

fn rss(xs: &[f64], ys: &[f64], f: &Fit) -> f64 {
    xs.iter()
        .zip(ys)
        .map(|(&x, &y)| (f.eval(x) - y).powi(2))
        .sum()
}

/// Least-squares fits of each family (vertical residuals in math units).
fn fits(xs: &[f64], ys: &[f64], wavy: bool) -> Vec<Fit> {
    let mut out = Vec::new();
    if let Some(c) = lsq(xs, ys, &|x| [x, 1.0, 0.0, 0.0], 2) {
        out.push(Fit {
            kind: ShapeKind::GraphLine,
            params: c,
        });
    }
    if let Some(c) = lsq(xs, ys, &|x| [x * x, x, 1.0, 0.0], 3) {
        out.push(Fit {
            kind: ShapeKind::GraphQuad,
            params: c,
        });
    }
    // Exponential: grid over the rate, least squares for scale and offset.
    let mut best: Option<(f64, Fit)> = None;
    let mut k = -2.5f64;
    while k <= 2.5 {
        if k.abs() >= 0.2
            && let Some(c) = lsq(xs, ys, &|x| [(k * x).exp(), 1.0, 0.0, 0.0], 2)
        {
            let f = Fit {
                kind: ShapeKind::GraphExp,
                params: vec![c[0], k.exp(), c[1]],
            };
            let r = rss(xs, ys, &f);
            if best.as_ref().is_none_or(|(b, _)| r < *b) {
                best = Some((r, f));
            }
        }
        k += 0.01;
    }
    out.extend(best.map(|(_, f)| f));
    // Sine: grid over the frequency (a third of a period to five periods
    // across the ink), least squares for the rest. Only for ink that turns
    // at least twice (a parabola turns once).
    if wavy {
        let span = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - xs.iter().copied().fold(f64::INFINITY, f64::min);
        let (lo, hi) = (TAU as f64 / (3.0 * span), TAU as f64 * 5.0 / span);
        let mut best: Option<(f64, Fit)> = None;
        for i in 0..400 {
            let b = lo * (hi / lo).powf(f64::from(i) / 399.0);
            if let Some(c) = lsq(xs, ys, &|x| [(b * x).sin(), (b * x).cos(), 1.0, 0.0], 3) {
                let f = Fit {
                    kind: ShapeKind::GraphSine,
                    params: vec![c[0].hypot(c[1]), b, c[1].atan2(c[0]), c[2]],
                };
                let r = rss(xs, ys, &f);
                if best.as_ref().is_none_or(|(bb, _)| r < *bb) {
                    best = Some((r, f));
                }
            }
        }
        out.extend(best.map(|(_, f)| f));
    }
    out
}

/// Turns of a sampled curve: sign changes of its slope, with hysteresis.
fn turns(ys: &[f64]) -> usize {
    let lo = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let hyst = 0.12 * (hi - lo);
    let (mut n, mut dir, mut ext) = (0usize, 0i8, ys[0]);
    for &y in &ys[1..] {
        match dir {
            0 => {
                if (y - ext).abs() >= hyst {
                    dir = if y > ext { 1 } else { -1 };
                    ext = y;
                }
            }
            1 => {
                if y > ext {
                    ext = y;
                } else if ext - y >= hyst {
                    n += 1;
                    dir = -1;
                    ext = y;
                }
            }
            _ => {
                if y < ext {
                    ext = y;
                } else if y - ext >= hyst {
                    n += 1;
                    dir = 1;
                    ext = y;
                }
            }
        }
    }
    n
}

/// Mean and worst distance (world) from the ink to the fitted curve over
/// the ink's x range.
fn distance(frame: &Frame, ink: &[P], f: &Fit, x0: f64, x1: f64) -> (f64, f64) {
    let n = 400;
    let curve: Vec<P> = (0..=n)
        .map(|i| {
            let x = x0 + (x1 - x0) * (f64::from(i) / f64::from(n) * 1.1 - 0.05);
            frame.to_world([x, f.eval(x).clamp(-1e6, 1e6)])
        })
        .collect();
    let (mut sum, mut worst) = (0.0f64, 0.0f64);
    for q in ink {
        let d = curve
            .windows(2)
            .map(|w| f64::from(super::seg_dist(*q, w[0], w[1])))
            .fold(f64::INFINITY, f64::min);
        sum += d;
        worst = worst.max(d);
    }
    (sum / ink.len() as f64, worst)
}

/// Rounding steps tried for each parameter, coarsest first.
fn steps(kind: ShapeKind, i: usize) -> &'static [f64] {
    const PI12: f64 = std::f64::consts::PI / 12.0;
    match (kind, i) {
        (ShapeKind::GraphLine, 0) => &[1.0, 0.5, 0.25, 0.1],
        (ShapeKind::GraphQuad, 0) => &[1.0, 0.5, 0.25, 0.1, 0.05],
        (ShapeKind::GraphSine, 1) => &[1.0, 0.5, 0.25, 0.05],
        (ShapeKind::GraphSine, 2) => &[3.0 * PI12, PI12],
        (ShapeKind::GraphExp, 1) => &[1.0, 0.5, 0.1],
        _ => &[1.0, 0.5, 0.1],
    }
}

/// Round each parameter to the coarsest step that keeps the fit about as
/// good, then to two decimals: the label and the drawn curve agree, and a
/// sketched `y = x²` reads `y = x²`, not `y = 1.04x² − 0.03x + 0.11`.
fn nicen(frame: &Frame, ink: &[P], f: &Fit, x0: f64, x1: f64) -> Fit {
    let (err0, _) = distance(frame, ink, f, x0, x1);
    let diag = {
        let b = bbox(ink);
        f64::from((b[2] - b[0]).hypot(b[3] - b[1]))
    };
    let allow = (err0 * 1.2).max(err0 + 0.006 * diag);
    let mut out = f.clone();
    for i in 0..out.params.len() {
        let orig = out.params[i];
        let mut chosen = (orig * 100.0).round() / 100.0;
        for &st in steps(f.kind, i) {
            let v = (orig / st).round() * st;
            if f.kind == ShapeKind::GraphExp && i == 1 && (v <= 0.05 || (v - 1.0).abs() < 0.05) {
                continue;
            }
            let mut t = out.clone();
            t.params[i] = v;
            if distance(frame, ink, &t, x0, x1).0 <= allow {
                chosen = v;
                break;
            }
        }
        out.params[i] = chosen;
    }
    // A negative sine amplitude reads better as a phase shift of π.
    if out.kind == ShapeKind::GraphSine && out.params[0] < 0.0 {
        out.params[0] = -out.params[0];
        out.params[2] += std::f64::consts::PI;
    }
    if out.kind == ShapeKind::GraphSine {
        let c = out.params[2].rem_euclid(TAU as f64);
        out.params[2] = if c > std::f64::consts::PI {
            c - TAU as f64
        } else {
            c
        };
    }
    out
}

/// The best fit of `ink` (world points) in `frame`, or `None` when no
/// family fits well enough.
pub fn fit(frame: &Frame, ink: &[P]) -> Option<(Fit, f64, f64)> {
    let m: Vec<[f64; 2]> = ink.iter().map(|q| frame.to_math(*q)).collect();
    let xs: Vec<f64> = m.iter().map(|q| q[0]).collect();
    let ys: Vec<f64> = m.iter().map(|q| q[1]).collect();
    let x0 = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let x1 = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let b = bbox(ink);
    let diag = f64::from((b[2] - b[0]).hypot(b[3] - b[1]));
    let cands: Vec<(Fit, f64, f64)> = fits(&xs, &ys, turns(&ys) >= 2)
        .into_iter()
        .map(|f| {
            let (mean, worst) = distance(frame, ink, &f, x0, x1);
            (f, mean, worst)
        })
        .collect();
    let best = cands.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    // Errors also count against the curve's own height (a wavy scribble
    // across a wide board is long but not tall).
    let scale = f64::from((b[3] - b[1]).max(0.3 * (b[2] - b[0])));
    let limit = (FIT_MEAN * diag).min(0.05 * scale);
    if best > limit {
        return None;
    }
    // Simplest first: line, parabola, exponential, sine.
    let order = [
        ShapeKind::GraphLine,
        ShapeKind::GraphQuad,
        ShapeKind::GraphExp,
        ShapeKind::GraphSine,
    ];
    let pick = order.iter().find_map(|k| {
        cands.iter().find(|c| {
            c.0.kind == *k && c.1 <= (best * 1.3 + 0.004 * diag).min(limit) && c.2 <= FIT_MAX * diag
        })
    })?;
    let nice = nicen(frame, ink, &pick.0, x0, x1);
    let (mean, worst) = distance(frame, ink, &nice, x0, x1);
    (mean <= 1.25 * limit && worst <= 1.2 * FIT_MAX * diag).then_some((nice, x0, x1))
}

/// Refit a clean curve already on the axes (for "Make it live"): the family
/// is known, the points lie on it exactly.
pub fn refit(frame: &Frame, curve: &[P], kind: ShapeKind) -> Option<Fit> {
    let m: Vec<[f64; 2]> = curve.iter().map(|q| frame.to_math(*q)).collect();
    let xs: Vec<f64> = m.iter().map(|q| q[0]).collect();
    let ys: Vec<f64> = m.iter().map(|q| q[1]).collect();
    let f = fits(&xs, &ys, kind == ShapeKind::GraphSine)
        .into_iter()
        .filter(|f| f.kind == kind)
        .min_by(|a, b| rss(&xs, &ys, a).total_cmp(&rss(&xs, &ys, b)))?;
    let r = |v: f64| (v * 1000.0).round() / 1000.0;
    let mut params: Vec<f64> = f.params.iter().map(|v| r(*v)).collect();
    if kind == ShapeKind::GraphSine && params[0] < 0.0 {
        params[0] = -params[0];
        params[2] += std::f64::consts::PI;
    }
    Some(Fit { kind, params })
}

/// A curve sketched over axes becomes the fitted curve, with its equation;
/// both join the axes' group.
pub struct CurveFit;

impl Recognizer for CurveFit {
    fn name(&self) -> &'static str {
        "curve-fit"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GRAPHS) || stroke.len() < 4 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let len = path_len(stroke);
        if len * px < 80.0 || dist(stroke[0], stroke[stroke.len() - 1]) < 0.3 * len {
            return None;
        }
        let (i, frame) = ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if n.shape.as_deref() != Some("axes") {
                return None;
            }
            Some((i, Frame::of(n.geometry.as_ref()?)?))
        })?;
        let ink = resample(stroke, 96);
        // Over the axes: nearly all of it inside their box.
        let b = frame.b;
        let (mx, my) = (0.06 * (b[2] - b[0]), 0.06 * (b[3] - b[1]));
        let inside = ink
            .iter()
            .filter(|q| {
                q[0] >= b[0] - mx && q[0] <= b[2] + mx && q[1] >= b[1] - my && q[1] <= b[3] + my
            })
            .count();
        if inside * 10 < ink.len() * 9 {
            return None;
        }
        // A function of x: it barely goes back.
        let (mut fwd, mut back) = (0.0f32, 0.0f32);
        for w in ink.windows(2) {
            let dx = w[1][0] - w[0][0];
            if dx >= 0.0 {
                fwd += dx;
            } else {
                back -= dx;
            }
        }
        if fwd.min(back) > 0.08 * (fwd + back) {
            return None;
        }
        let span = (fwd - back).abs() / frame.ux;
        if span < 1.5 {
            return None;
        }
        // Not a retrace of the x axis.
        let along = ink.iter().map(|q| (q[1] - frame.o[1]).abs()).sum::<f32>() / ink.len() as f32;
        if along < 0.25 * frame.uy {
            return None;
        }
        let (f, x0, x1) = fit(&frame, &ink)?;
        // The clean curve over the ink's x range, inside the axes' box.
        let n = 60;
        let pts: Vec<P> = (0..=n)
            .map(|k| {
                let x = x0 + (x1 - x0) * f64::from(k) / f64::from(n);
                frame.to_world([x, f.eval(x)])
            })
            .collect();
        let run = longest_inside(&pts, b);
        if run.len() < 8 || dist(run[0], run[run.len() - 1]) * px < 60.0 {
            return None;
        }
        let geometry = if f.kind == ShapeKind::GraphLine {
            Geometry::Segment {
                a: run[0],
                b: run[run.len() - 1],
            }
        } else {
            Geometry::open_path("path", &run)
        };
        let text = f.label();
        let size = (0.45 * frame.ux.min(frame.uy)).clamp(14.0 / px, 26.0 / px);
        let at = label_spot(&run, &text, size, b);
        let mut p = Proposal::new(
            Recognized {
                kind: f.kind,
                geometry,
                confidence: 1.0,
            },
            Vec::new(),
            self.name(),
        );
        p.join = Some(i);
        p.labels = vec![Label { text, at, size }];
        Some(p)
    }
}

/// The longest run of consecutive points inside the box.
fn longest_inside(pts: &[P], b: [f32; 4]) -> Vec<P> {
    let mut best: &[P] = &[];
    let mut start = None;
    for (i, q) in pts.iter().enumerate() {
        let inside = q[0] >= b[0] && q[0] <= b[2] && q[1] >= b[1] && q[1] <= b[3];
        match (inside, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if i - s > best.len() {
                    best = &pts[s..i];
                }
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start
        && pts.len() - s > best.len()
    {
        best = &pts[s..];
    }
    best.to_vec()
}

/// Where an equation label goes: just right of the curve's end when there
/// is room inside the axes, else beside the end on the side the curve isn't
/// (above it when it ends rising), kept inside the axes.
fn label_spot(run: &[P], text: &str, size: f32, b: [f32; 4]) -> P {
    let e = run[run.len() - 1];
    let prev = run[run.len().saturating_sub(4)];
    let w = 0.55 * size * text.chars().count() as f32;
    let rising = e[1] < prev[1];
    let dy = if rising { 0.6 * size } else { -0.6 * size };
    let mut at = if e[0] + 0.5 * size + w <= b[2] {
        [e[0] + 0.5 * size + w / 2.0, e[1] + dy]
    } else {
        [
            e[0] - w / 2.0 - 0.3 * size,
            if rising {
                e[1] - 0.9 * size
            } else {
                e[1] + 0.9 * size
            },
        ]
    };
    at[0] = at[0].clamp(b[0] + w / 2.0, (b[2] - w / 2.0).max(b[0] + w / 2.0));
    at[1] = at[1].clamp(
        b[1] + size / 2.0,
        (b[3] - size / 2.0).max(b[1] + size / 2.0),
    );
    at
}
