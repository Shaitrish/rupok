//! Geometry marks: an arc drawn in the corner between two lines becomes an
//! angle mark with the measured degrees, a small square corner becomes a
//! right-angle mark, and a line drawn near another snaps exactly parallel
//! or perpendicular to it when within a few degrees.

use super::registry::{Context, Label, Neighbour, Proposal, Recognizer};
use super::{
    DEG, Geometry, P, PI, Recognized, ShapeKind, angle, as_segment, corners, dist, family, max_dev,
    path_len, resample, seg_dist, wrap_half_pi, wrap_pi,
};

/// A line within this many degrees of parallel / perpendicular to a nearby
/// line snaps to it.
pub const ALIGN_DEG: f32 = 4.0;

/// Straight edges near the stroke: lines and arrows, polygon and rectangle
/// sides, straight ink.
fn segments(ctx: &Context) -> Vec<(P, P)> {
    let mut out = Vec::new();
    for n in &ctx.neighbours {
        match &n.geometry {
            Some(Geometry::Segment { a, b }) => out.push((*a, *b)),
            Some(Geometry::Polygon { pts }) => {
                for i in 0..pts.len() {
                    out.push((pts[i], pts[(i + 1) % pts.len()]));
                }
            }
            Some(Geometry::Rect { c, w, h, rot }) => {
                let (s, co) = rot.sin_cos();
                let v: Vec<P> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
                    .iter()
                    .map(|k: &[f32; 2]| {
                        let (dx, dy) = (k[0] * w / 2.0, k[1] * h / 2.0);
                        [c[0] + dx * co - dy * s, c[1] + dx * s + dy * co]
                    })
                    .collect();
                for i in 0..4 {
                    out.push((v[i], v[(i + 1) % 4]));
                }
            }
            Some(_) => {}
            None => out.extend(raw_segment(n)),
        }
    }
    out
}

fn raw_segment(n: &Neighbour) -> Option<(P, P)> {
    if n.shape.is_some() {
        return None;
    }
    as_segment(n)
}

/// A corner: where two edges meet, and the two directions (unit) along
/// them with their lengths.
#[derive(Clone, Copy, Debug)]
struct Corner {
    v: P,
    u1: P,
    l1: f32,
    u2: P,
    l2: f32,
}

/// Corners formed by pairs of edges that share an end, or cross.
fn corners_of(segs: &[(P, P)], px: f32) -> Vec<Corner> {
    let mut out = Vec::new();
    let unit = |a: P, b: P| {
        let l = dist(a, b).max(1e-6);
        ([(b[0] - a[0]) / l, (b[1] - a[1]) / l], l)
    };
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let (s1, s2) = (segs[i], segs[j]);
            let tol = (0.06 * dist(s1.0, s1.1).min(dist(s2.0, s2.1))).max(6.0 / px);
            let mut shared = false;
            for (p, q) in [(s1.0, s1.1), (s1.1, s1.0)] {
                for (r, t) in [(s2.0, s2.1), (s2.1, s2.0)] {
                    if dist(p, r) <= tol {
                        let v = [(p[0] + r[0]) / 2.0, (p[1] + r[1]) / 2.0];
                        let ((u1, l1), (u2, l2)) = (unit(v, q), unit(v, t));
                        out.push(Corner { v, u1, l1, u2, l2 });
                        shared = true;
                    }
                }
            }
            if shared {
                continue;
            }
            // Crossing edges: four corners around the crossing.
            let (d1, d2) = (unit(s1.0, s1.1).0, unit(s2.0, s2.1).0);
            let den = d1[0] * d2[1] - d1[1] * d2[0];
            if den.abs() < 0.2 {
                continue;
            }
            let t = ((s2.0[0] - s1.0[0]) * d2[1] - (s2.0[1] - s1.0[1]) * d2[0]) / den;
            let x = [s1.0[0] + t * d1[0], s1.0[1] + t * d1[1]];
            if seg_dist(x, s1.0, s1.1) > tol || seg_dist(x, s2.0, s2.1) > tol {
                continue;
            }
            for a in [s1.0, s1.1] {
                for b in [s2.0, s2.1] {
                    let ((u1, l1), (u2, l2)) = (unit(x, a), unit(x, b));
                    if l1 > tol && l2 > tol {
                        out.push(Corner {
                            v: x,
                            u1,
                            l1,
                            u2,
                            l2,
                        });
                    }
                }
            }
        }
    }
    out
}

/// Is direction `d` within `tol` of unit ray `u`?
fn along(d: P, u: P, tol: f32) -> bool {
    let l = d[0].hypot(d[1]);
    l > 1e-6 && ((d[0] * u[0] + d[1] * u[1]) / l).clamp(-1.0, 1.0).acos() <= tol
}

/// An arc in the corner between two edges becomes an angle mark labelled
/// with the angle in degrees.
pub struct AngleMark;

impl Recognizer for AngleMark {
    fn name(&self) -> &'static str {
        "angle"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GEOMETRY) || stroke.len() < 4 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let p = resample(stroke, 40);
        let (e1, e2) = (p[0], p[p.len() - 1]);
        let chord = dist(e1, e2);
        if chord * px < 6.0
            || max_dev(&p, e1, e2) < 0.05 * chord
            || path_len(&p) > 2.2 * chord
            || !corners(&p, 3, 50.0 * DEG, false).is_empty()
        {
            return None;
        }
        let segs = segments(ctx);
        corners_of(&segs, px).into_iter().find_map(|k| {
            let d: Vec<f32> = p.iter().map(|q| dist(*q, k.v)).collect();
            let r = d.iter().sum::<f32>() / d.len() as f32;
            let sd = (d.iter().map(|x| (x - r).powi(2)).sum::<f32>() / d.len() as f32).sqrt();
            if r * px < 8.0 || r > 0.7 * k.l1.min(k.l2) || sd > 0.18 * r {
                return None;
            }
            let tol = 14.0 * DEG;
            let to = |q: P| [q[0] - k.v[0], q[1] - k.v[1]];
            let (u1, u2) = if along(to(e1), k.u1, tol) && along(to(e2), k.u2, tol) {
                (k.u1, k.u2)
            } else if along(to(e1), k.u2, tol) && along(to(e2), k.u1, tol) {
                (k.u2, k.u1)
            } else {
                return None;
            };
            let a0 = angle(u1);
            let sweep = wrap_pi(angle(u2) - a0);
            // The ink sweeps the same way, about as far.
            let drawn = p
                .windows(2)
                .map(|w| wrap_pi(angle(to(w[1])) - angle(to(w[0]))))
                .sum::<f32>();
            if drawn.signum() != sweep.signum()
                || (drawn - sweep).abs() > 0.3 * sweep.abs() + 6.0 * DEG
            {
                return None;
            }
            let deg = (sweep.abs() / DEG).round();
            let size = (0.4 * r).clamp(14.0 / px, 26.0 / px);
            let bis = a0 + sweep / 2.0;
            let lr = r + 0.9 * size;
            let mut prop = Proposal::new(
                Recognized {
                    kind: ShapeKind::Angle,
                    geometry: Geometry::template(
                        "arc",
                        k.v,
                        2.0 * r,
                        2.0 * r,
                        0.0,
                        vec![a0, sweep],
                    ),
                    confidence: (1.0 - sd / (0.18 * r)).clamp(0.0, 1.0),
                },
                Vec::new(),
                self.name(),
            );
            prop.labels = vec![Label {
                text: format!("{deg}°"),
                at: [k.v[0] + lr * bis.cos(), k.v[1] + lr * bis.sin()],
                size,
            }];
            Some(prop)
        })
    }
}

/// A small square corner drawn between two perpendicular edges becomes a
/// right-angle mark.
pub struct RightAngleMark;

impl Recognizer for RightAngleMark {
    fn name(&self) -> &'static str {
        "right-angle"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::GEOMETRY) || stroke.len() < 3 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let p = resample(stroke, 48);
        let c = corners(&p, 3, 50.0 * DEG, false);
        let [kc] = c.as_slice() else {
            return None;
        };
        let (e1, m, e2) = (p[0], p[*kc], p[p.len() - 1]);
        let (l1, l2) = (dist(e1, m), dist(m, e2));
        let turn =
            PI - super::angle_between([e1[0] - m[0], e1[1] - m[1]], [e2[0] - m[0], e2[1] - m[1]]);
        if l1.min(l2) * px < 6.0
            || l1.min(l2) / l1.max(l2) < 0.5
            || (turn - PI / 2.0).abs() > 30.0 * DEG
            || max_dev(&p[..=*kc], e1, m) > 0.15 * l1
            || max_dev(&p[*kc..], m, e2) > 0.15 * l2
        {
            return None;
        }
        let segs = segments(ctx);
        corners_of(&segs, px).into_iter().find_map(|k| {
            let cross = (k.u1[0] * k.u2[0] + k.u1[1] * k.u2[1]).abs();
            if cross > (78.0 * DEG).cos() {
                return None;
            }
            // Each end on one edge, the corner inside the angle.
            let proj = |q: P, u: P| {
                let d = [q[0] - k.v[0], q[1] - k.v[1]];
                let t = d[0] * u[0] + d[1] * u[1];
                (t, (d[0] - t * u[0]).hypot(d[1] - t * u[1]))
            };
            let leg = (l1 + l2) / 2.0;
            let tol = 0.3 * leg + 4.0 / px;
            let try_pair = |a: P, b: P| {
                let (t1, o1) = proj(a, k.u1);
                let (t2, o2) = (proj(b, k.u2).0, proj(b, k.u2).1);
                (o1 <= tol && o2 <= tol && t1 > 0.0 && t2 > 0.0).then_some((t1 + t2) / 2.0)
            };
            let side = try_pair(e1, e2).or_else(|| try_pair(e2, e1))?;
            let (tm1, om1) = proj(m, k.u1);
            let (tm2, om2) = proj(m, k.u2);
            if tm1 <= 0.0 || tm2 <= 0.0 || om1 < 0.5 * side || om2 < 0.5 * side {
                return None;
            }
            if side > 0.5 * k.l1.min(k.l2) || side * px < 6.0 {
                return None;
            }
            let a = [k.v[0] + side * k.u1[0], k.v[1] + side * k.u1[1]];
            let b = [k.v[0] + side * k.u2[0], k.v[1] + side * k.u2[1]];
            let corner = [a[0] + side * k.u2[0], a[1] + side * k.u2[1]];
            Some(Proposal::new(
                Recognized {
                    kind: ShapeKind::RightAngle,
                    geometry: Geometry::open_path("polyline", &[a, corner, b]),
                    confidence: 1.0,
                },
                Vec::new(),
                self.name(),
            ))
        })
    }
}

/// Turn a freshly recognised line about its middle to be exactly parallel
/// or perpendicular to a nearby line within [`ALIGN_DEG`]. Lines already
/// snapped to 0°/45°/90° are left alone.
pub fn align_line(r: &mut Recognized, ctx: &Context) {
    if r.kind != ShapeKind::Line {
        return;
    }
    let Geometry::Segment { a, b } = r.geometry else {
        return;
    };
    let th = angle([b[0] - a[0], b[1] - a[1]]);
    let q = PI / 4.0;
    if (th - (th / q).round() * q).abs() < 1e-4 {
        return;
    }
    let len = dist(a, b);
    let mut best: Option<(f32, f32)> = None;
    for n in &ctx.neighbours {
        let Some(Geometry::Segment { a: na, b: nb }) = n.geometry else {
            continue;
        };
        let near = [
            seg_dist(a, na, nb),
            seg_dist(b, na, nb),
            seg_dist(na, a, b),
            seg_dist(nb, a, b),
        ]
        .into_iter()
        .fold(f32::INFINITY, f32::min);
        if near > 1.5 * len {
            continue;
        }
        let phi = angle([nb[0] - na[0], nb[1] - na[1]]);
        for target in [phi, phi + PI / 2.0] {
            let d = wrap_half_pi(target - th);
            if d.abs() <= ALIGN_DEG * DEG && best.is_none_or(|(bd, _)| d.abs() < bd.abs()) {
                best = Some((d, near));
            }
        }
    }
    if let Some((d, _)) = best {
        let m = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let (s, c) = (th + d).sin_cos();
        let h = len / 2.0;
        r.geometry = Geometry::Segment {
            a: [m[0] - c * h, m[1] - s * h],
            b: [m[0] + c * h, m[1] + s * h],
        };
    }
}
