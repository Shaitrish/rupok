//! Logic gates: a gate body (AND's "D", OR's shield, NOT's triangle) plus
//! what makes it a gate: input/output pins, an inversion bubble, XOR's
//! second back curve. A body alone stays ink (a big "D" is a letter) unless
//! the teacher held the pen to ask for a shape.
//!
//! The clean gate faces right, left, up or down, with standard proportions
//! ([`gate_layout`]) and pins whose ends are connector anchors.

use super::registry::{Context, Neighbour, Proposal, Recognizer};
use super::{
    CORNER_WINDOW, DEG, Geometry, Options, P, PI, Recognized, ShapeKind, angle, circle_of,
    close_loop, corners, dist, family, max_dev, recognize_xy, resample, straight, total_turning,
};

/// Context strokes count when drawn within this long of the body (ms).
pub const GATE_WINDOW_MS: f64 = 6000.0;

/// Proportions of a clean gate along its facing direction, in body heights:
/// input pins, the gap before XOR's body, the body, the bubble's diameter,
/// the output pin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateLayout {
    pub pin_in: f32,
    pub gap: f32,
    pub body: f32,
    pub bubble: f32,
    pub pin_out: f32,
}

impl GateLayout {
    pub fn total(&self) -> f32 {
        self.pin_in + self.gap + self.body + self.bubble + self.pin_out
    }
}

/// The layout of a stored gate kind (`gate-and`, …).
pub fn gate_layout(kind: &str) -> Option<GateLayout> {
    let l = |gap, body, bubble| GateLayout {
        pin_in: 0.35,
        gap,
        body,
        bubble,
        pin_out: 0.3,
    };
    Some(match kind {
        "gate-and" => l(0.0, 1.0, 0.0),
        "gate-nand" => l(0.0, 1.0, 0.18),
        "gate-or" => l(0.0, 1.1, 0.0),
        "gate-nor" => l(0.0, 1.1, 0.18),
        "gate-xor" => l(0.16, 1.1, 0.0),
        "gate-xnor" => l(0.16, 1.1, 0.18),
        "gate-not" => l(0.0, 0.9, 0.18),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BodyKind {
    And,
    Or,
    Tri,
}

/// A gate body in its facing frame: `face` (radians, a multiple of 90°),
/// height `h` across it, the back's and the tip's positions along it from
/// the body's centre line point `back`.
#[derive(Clone, Copy, Debug)]
struct Body {
    kind: BodyKind,
    face: f32,
    /// Middle of the back.
    back: P,
    /// The front point (AND's arc apex, OR's and NOT's tip).
    tip: P,
    h: f32,
}

impl Body {
    fn dir(&self) -> P {
        [self.face.cos(), self.face.sin()]
    }
    /// (along, across) of `q` relative to the back's middle.
    fn uv(&self, q: P) -> P {
        let d = self.dir();
        let (dx, dy) = (q[0] - self.back[0], q[1] - self.back[1]);
        [dx * d[0] + dy * d[1], -dx * d[1] + dy * d[0]]
    }
    fn depth(&self) -> f32 {
        self.uv(self.tip)[0]
    }
}

/// Snap a direction to the nearest multiple of 90° within 20°.
fn snap_face(th: f32) -> Option<f32> {
    let q = PI / 2.0;
    let t = (th / q).round() * q;
    ((th - t).abs() <= 20.0 * DEG).then_some(t)
}

/// Signed distance of `q` from the line through `a`, `b` (positive to the
/// left of a → b on screen).
fn side(q: P, a: P, b: P) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy).max(1e-6);
    ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx) / l
}

/// The closed loop of a body stroke, if it is one.
fn body_loop(points: &[P]) -> Option<Vec<P>> {
    let lp = close_loop(
        &resample(points, 128),
        Options {
            deliberate: true,
            ..Options::default()
        },
    )?;
    let (total, abs) = total_turning(&lp);
    ((1.7 * PI..=2.35 * PI).contains(&total.abs()) && abs <= 1.5 * total.abs()).then_some(lp)
}

/// A cyclic run of the loop from corner `a` to corner `b`.
fn run(lp: &[P], a: usize, b: usize) -> Vec<P> {
    let n = lp.len();
    let len = (b + n - a) % n;
    (0..=len).map(|k| lp[(a + k) % n]).collect()
}

/// AND's "D": one straight back and one round front, meeting at two
/// corners.
fn and_body(lp: &[P]) -> Option<Body> {
    let c = corners(lp, CORNER_WINDOW, 50.0 * DEG, true);
    let [c0, c1] = c.as_slice() else {
        return None;
    };
    let (r1, r2) = (run(lp, *c0, *c1), run(lp, *c1, *c0));
    let dev = |r: &[P]| max_dev(r, r[0], r[r.len() - 1]) / dist(r[0], r[r.len() - 1]).max(1e-6);
    let (back, front) = if dev(&r1) < dev(&r2) {
        (r1, r2)
    } else {
        (r2, r1)
    };
    if dev(&back) > 0.08 || dev(&front) < 0.3 {
        return None;
    }
    let (a, b) = (back[0], back[back.len() - 1]);
    let h = dist(a, b);
    // The front bulges to one side only (convex), `d` deep.
    let s: Vec<f32> = front.iter().map(|q| side(*q, a, b)).collect();
    let sign = if s.iter().sum::<f32>() >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let d = s.iter().map(|v| v * sign).fold(f32::NEG_INFINITY, f32::max);
    if s.iter().any(|v| v * sign < -0.06 * h) || !(0.45..=1.5).contains(&(d / h)) {
        return None;
    }
    let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let nrm = [(b[1] - a[1]) / h * sign, -(b[0] - a[0]) / h * sign];
    let face = snap_face(angle(nrm))?;
    Some(Body {
        kind: BodyKind::And,
        face,
        back: m,
        tip: [m[0] + nrm[0] * d, m[1] + nrm[1] * d],
        h,
    })
}

/// OR's shield: a back that curves in, two sides that curve out to a tip.
fn or_body(lp: &[P]) -> Option<Body> {
    let c = corners(lp, CORNER_WINDOW, 40.0 * DEG, true);
    if c.len() != 3 {
        return None;
    }
    let g = {
        let n = lp.len() as f32;
        lp.iter()
            .fold([0.0, 0.0], |s, q| [s[0] + q[0] / n, s[1] + q[1] / n])
    };
    (0..3).find_map(|t| {
        let (ia, ib) = (c[(t + 1) % 3], c[(t + 2) % 3]);
        let tip = lp[c[t]];
        let back = run(lp, ia, ib);
        let (a, b) = (back[0], back[back.len() - 1]);
        let h = dist(a, b);
        let tip_side = side(tip, a, b);
        if h <= f32::EPSILON || tip_side.abs() < 0.5 * h {
            return None;
        }
        let sign = tip_side.signum();
        // Concave back: it bows towards the tip.
        let bow = back
            .iter()
            .map(|q| side(*q, a, b) * sign)
            .fold(f32::NEG_INFINITY, f32::max);
        if !(0.04 * h..=0.35 * h).contains(&bow) {
            return None;
        }
        // Convex or straight sides, bowing away from the middle.
        for r in [run(lp, ib, c[t]), run(lp, c[t], ia)] {
            let (p0, p1) = (r[0], r[r.len() - 1]);
            let l = dist(p0, p1).max(1e-6);
            let gs = side(g, p0, p1).signum();
            let out = r
                .iter()
                .map(|q| -side(*q, p0, p1) * gs)
                .fold(f32::NEG_INFINITY, f32::max);
            let inward = r
                .iter()
                .map(|q| side(*q, p0, p1) * gs)
                .fold(f32::NEG_INFINITY, f32::max);
            if out > 0.3 * l || inward > 0.06 * l {
                return None;
            }
        }
        let d = tip_side.abs();
        if !(0.6..=1.8).contains(&(d / h)) {
            return None;
        }
        let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let face = snap_face(angle([tip[0] - m[0], tip[1] - m[1]]))?;
        Some(Body {
            kind: BodyKind::Or,
            face,
            back: m,
            tip,
            h,
        })
    })
}

/// NOT's triangle: a stored triangle or triangular ink, one side facing
/// back and the opposite corner pointing left, right, up or down.
fn tri_body(n_geom: Option<&Geometry>, points: &[P]) -> Option<Body> {
    let pts = match n_geom {
        Some(Geometry::Polygon { pts }) if pts.len() == 3 => pts.clone(),
        Some(_) => return None,
        None => match recognize_xy(
            points,
            Options {
                deliberate: true,
                ..Options::default()
            },
        ) {
            Some(Recognized {
                kind: ShapeKind::Triangle,
                geometry: Geometry::Polygon { pts },
                ..
            }) => pts,
            _ => return None,
        },
    };
    (0..3).find_map(|i| {
        let (tip, a, b) = (pts[i], pts[(i + 1) % 3], pts[(i + 2) % 3]);
        let h = dist(a, b);
        let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let face = snap_face(angle([tip[0] - m[0], tip[1] - m[1]]))?;
        let d = dist(tip, m);
        ((0.5..=1.6).contains(&(d / h))).then_some(Body {
            kind: BodyKind::Tri,
            face,
            back: m,
            tip,
            h,
        })
    })
}

/// What a context stroke is to a body.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Part {
    Pin,
    Bubble,
    Arc,
}

fn part_of(b: &Body, points: &[P]) -> Option<Part> {
    let h = b.h;
    let depth = b.depth();
    // A bubble at the tip.
    if let Some((c, r)) = circle_of(points, 0.55) {
        let uv = b.uv(c);
        if (0.04 * h..=0.22 * h).contains(&r)
            && (uv[0] - depth - r).abs() <= 0.6 * r + 0.08 * h
            && uv[1].abs() <= 0.15 * h
        {
            return Some(Part::Bubble);
        }
        return None;
    }
    let p = resample(points, 32);
    if let Some((s, e)) = straight(&p, 0.1) {
        let (us, ue) = (b.uv(s), b.uv(e));
        let l = dist(s, e);
        let th = (ue[1] - us[1]).atan2(ue[0] - us[0]).abs();
        if (0.12 * h..=1.2 * h).contains(&l) && th.min(PI - th) <= 25.0 * DEG {
            let (near, far) = if us[0] > ue[0] { (us, ue) } else { (ue, us) };
            // An input pin ends at the back (OR's curves in); an output pin
            // starts at the front.
            let input = (-0.25 * h..=0.4 * h).contains(&near[0])
                && far[0] < near[0] - 0.08 * h
                && near[1].abs() <= 0.6 * h;
            let (n2, f2) = if us[0] < ue[0] { (us, ue) } else { (ue, us) };
            let output =
                (n2[0] - depth).abs() <= 0.3 * h && f2[0] > n2[0] && n2[1].abs() <= 0.2 * h;
            return (input || output).then_some(Part::Pin);
        }
    }
    // XOR's extra curve behind the back, across the facing direction.
    if b.kind == BodyKind::Or {
        let (s, e) = (p[0], p[p.len() - 1]);
        let (us, ue) = (b.uv(s), b.uv(e));
        let chord = dist(s, e);
        let th = (ue[1] - us[1]).atan2(ue[0] - us[0]).abs();
        let mid_u = p.iter().map(|q| b.uv(*q)[0]).sum::<f32>() / p.len() as f32;
        if chord >= 0.6 * h
            && chord <= 1.4 * h
            && (th - PI / 2.0).abs() <= 25.0 * DEG
            && max_dev(&p, s, e) >= 0.04 * chord
            && (-0.45 * h..=0.02 * h).contains(&mid_u)
            && us[0].max(ue[0]) < 0.1 * h
        {
            return Some(Part::Arc);
        }
    }
    None
}

struct Cand<'a> {
    idx: Option<usize>,
    points: &'a [P],
    n: Option<&'a Neighbour>,
}

/// Logic gates from a body and its context (see the module docs).
pub struct Gates;

impl Recognizer for Gates {
    fn name(&self) -> &'static str {
        "gates"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GATES) || stroke.len() < 3 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let mut cands = vec![Cand {
            idx: None,
            points: stroke,
            n: None,
        }];
        for (i, n) in ctx.neighbours.iter().enumerate() {
            let tri = n.shape.as_deref() == Some("triangle");
            if (n.shape.is_none() || tri) && (0.0..=GATE_WINDOW_MS).contains(&n.gap_ms) {
                cands.push(Cand {
                    idx: Some(i),
                    points: &n.points,
                    n: Some(n),
                });
            }
        }
        for (bi, bc) in cands.iter().enumerate() {
            let geom = bc.n.and_then(|n| n.geometry.as_ref());
            let body = match geom {
                Some(_) => tri_body(geom, bc.points),
                None => body_loop(bc.points).and_then(|lp| {
                    and_body(&lp)
                        .or_else(|| or_body(&lp))
                        .or_else(|| tri_body(None, bc.points))
                }),
            };
            let Some(body) = body else {
                continue;
            };
            if body.h * px < 40.0 || body.h * px > 600.0 {
                continue;
            }
            let mut parts: Vec<(Option<usize>, Part)> = Vec::new();
            for (ci, cc) in cands.iter().enumerate() {
                if ci != bi
                    && let Some(part) = part_of(&body, cc.points)
                {
                    parts.push((cc.idx, part));
                }
            }
            // The new stroke takes part: as the body or as a context stroke.
            if bc.idx.is_some() && !parts.iter().any(|p| p.0.is_none()) {
                continue;
            }
            let has = |k: Part| parts.iter().any(|p| p.1 == k);
            let deliberate = ctx.opts.deliberate && bc.idx.is_none();
            let kind = match body.kind {
                BodyKind::Tri if has(Part::Bubble) => ShapeKind::NotGate,
                BodyKind::Tri => continue,
                BodyKind::And if has(Part::Bubble) => ShapeKind::NandGate,
                BodyKind::And if has(Part::Pin) || deliberate => ShapeKind::AndGate,
                BodyKind::Or if has(Part::Arc) && has(Part::Bubble) => ShapeKind::XnorGate,
                BodyKind::Or if has(Part::Arc) => ShapeKind::XorGate,
                BodyKind::Or if has(Part::Bubble) => ShapeKind::NorGate,
                BodyKind::Or if has(Part::Pin) || deliberate => ShapeKind::OrGate,
                _ => continue,
            };
            return Some(gate(&body, kind, bc.idx, &parts));
        }
        None
    }
}

/// The stored kind of a gate shape.
pub fn gate_kind(k: ShapeKind) -> Option<&'static str> {
    Some(match k {
        ShapeKind::AndGate => "gate-and",
        ShapeKind::NandGate => "gate-nand",
        ShapeKind::OrGate => "gate-or",
        ShapeKind::NorGate => "gate-nor",
        ShapeKind::XorGate => "gate-xor",
        ShapeKind::XnorGate => "gate-xnor",
        ShapeKind::NotGate => "gate-not",
        _ => return None,
    })
}

fn gate(
    body: &Body,
    kind: ShapeKind,
    body_idx: Option<usize>,
    parts: &[(Option<usize>, Part)],
) -> Proposal {
    let tk = gate_kind(kind).unwrap_or("gate-and");
    let lay = gate_layout(tk).unwrap_or(GateLayout {
        pin_in: 0.35,
        gap: 0.0,
        body: 1.0,
        bubble: 0.0,
        pin_out: 0.3,
    });
    let h = body.h;
    let w = lay.total() * h;
    // The drawn body's middle (along the facing direction) stays put.
    let d = body.dir();
    let mid = [
        body.back[0] + d[0] * body.depth() / 2.0,
        body.back[1] + d[1] * body.depth() / 2.0,
    ];
    let local_mid = (lay.pin_in + lay.gap + lay.body / 2.0) * h;
    let shift = w / 2.0 - local_mid;
    let c = [mid[0] + d[0] * shift, mid[1] + d[1] * shift];
    let inputs = if kind == ShapeKind::NotGate { 1.0 } else { 2.0 };
    let two_part = matches!(kind, ShapeKind::XorGate | ShapeKind::XnorGate);
    let part = |n: f32| Recognized {
        kind,
        geometry: Geometry::template(
            tk,
            c,
            w,
            h,
            body.face,
            if two_part {
                vec![inputs, n]
            } else {
                vec![inputs, 0.0]
            },
        ),
        confidence: 1.0,
    };
    let mut absorbs: Vec<usize> = body_idx.into_iter().collect();
    absorbs.extend(parts.iter().filter_map(|p| p.0));
    let mut p = Proposal::new(part(0.0), absorbs, "gates");
    p.onto_stroke = body_idx.is_none();
    if two_part {
        p.extra = vec![part(1.0)];
    }
    p
}
