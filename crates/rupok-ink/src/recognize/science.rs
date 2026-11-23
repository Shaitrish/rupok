//! Physics and chemistry: circuit symbols (a zig-zag between two leads is a
//! resistor, a long and a short plate a cell, a circle with an X a bulb, a
//! lead with a raised blade a switch), benzene (a hexagon with a circle or
//! a double bond inside) and the equilibrium arrow ⇌.
//!
//! Off by default (see [`family::DEFAULT`]): these marks are common in
//! other subjects too.

use super::registry::{Context, Neighbour, Proposal, Recognizer};
use super::{
    DEG, Geometry, Options, P, PI, Recognized, ShapeKind, angle, as_circle, as_segment, circle_of,
    corners, dist, family, fit_line, max_dev, path_len, recognize_xy, resample, straight, turning,
    wrap_pi,
};

/// Context strokes count when drawn within this long (ms).
pub const SCIENCE_WINDOW_MS: f64 = 5000.0;

/// Snap a direction to the nearest multiple of 90° within `tol`.
fn snap90(th: f32, tol: f32) -> f32 {
    let q = PI / 2.0;
    let t = (th / q).round() * q;
    if (th - t).abs() <= tol { t } else { th }
}

fn recent(n: &Neighbour) -> bool {
    (0.0..=SCIENCE_WINDOW_MS).contains(&n.gap_ms)
}

// -------------------------------------------------------------- resistor

/// A zig-zag between two straight, collinear leads.
pub struct Resistor;

impl Recognizer for Resistor {
    fn name(&self) -> &'static str {
        "resistor"
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 8 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let p = resample(stroke, 160);
        let (a, z) = (p[0], p[p.len() - 1]);
        let l = dist(a, z);
        if l * px < 100.0 {
            return None;
        }
        let u = [(z[0] - a[0]) / l, (z[1] - a[1]) / l];
        let along: Vec<f32> = p
            .iter()
            .map(|q| (q[0] - a[0]) * u[0] + (q[1] - a[1]) * u[1])
            .collect();
        let across: Vec<f32> = p
            .iter()
            .map(|q| -(q[0] - a[0]) * u[1] + (q[1] - a[1]) * u[0])
            .collect();
        // Forward all the way (no going back along the wire).
        if along.windows(2).any(|w| w[1] < w[0] - 0.03 * l) {
            return None;
        }
        // Leads: the first and last stretches stay on the wire.
        let amp = across.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        if amp < 0.04 * l {
            return None;
        }
        let flat = 0.18 * amp;
        let i0 = across.iter().position(|v| v.abs() > flat)?;
        let i1 = across.iter().rposition(|v| v.abs() > flat)?;
        let (lead0, lead1) = (along[i0], l - along[i1]);
        if lead0 < 0.08 * l || lead1 < 0.08 * l || lead0.min(lead1) / lead0.max(lead1) < 0.3 {
            return None;
        }
        let zig_len = along[i1] - along[i0];
        if !(0.06..=0.5).contains(&(amp / zig_len)) {
            return None;
        }
        // Teeth: sharp reversals across the wire, evenly spaced.
        let zig = &p[i0..=i1];
        let acr = &across[i0..=i1];
        let mut peaks: Vec<usize> = Vec::new();
        let (mut dir, mut ext) = (0i8, 0usize);
        for k in 1..acr.len() {
            match dir {
                0 => {
                    if (acr[k] - acr[0]).abs() > 0.5 * amp {
                        dir = if acr[k] > acr[0] { 1 } else { -1 };
                        ext = k;
                    }
                }
                1 => {
                    if acr[k] > acr[ext] {
                        ext = k;
                    } else if acr[ext] - acr[k] > 0.8 * amp {
                        peaks.push(ext);
                        dir = -1;
                        ext = k;
                    }
                }
                _ => {
                    if acr[k] < acr[ext] {
                        ext = k;
                    } else if acr[k] - acr[ext] > 0.8 * amp {
                        peaks.push(ext);
                        dir = 1;
                        ext = k;
                    }
                }
            }
        }
        if dir != 0 {
            peaks.push(ext);
        }
        if !(4..=10).contains(&peaks.len()) {
            return None;
        }
        let pos: Vec<f32> = peaks.iter().map(|&k| along[i0 + k]).collect();
        let gaps: Vec<f32> = pos.windows(2).map(|w| w[1] - w[0]).collect();
        let g = gaps.iter().sum::<f32>() / gaps.len() as f32;
        // Sharp teeth: the turn at each peak happens within a short window
        // (a wave turns gradually over its whole crest).
        let per = (zig.len() as f32 / (zig_len / g).max(1.0)).max(4.0);
        let w = ((0.12 * per).round() as usize).max(2);
        let big = ((0.45 * per).round() as usize).max(w + 2);
        // Windows clipped at the ends of the zig-zag, so end teeth count too.
        let eligible: Vec<(usize, usize, usize)> = peaks
            .iter()
            .filter_map(|&k| {
                let room = k.min(zig.len() - 1 - k);
                (room > w).then_some((k, w, big.min(room)))
            })
            .collect();
        // Mean share of the crest's turn that happens near its tip: about
        // 0.65 for a drawn zig-zag, under 0.5 for a wave.
        let conc = eligible
            .iter()
            .map(|&(k, w, b)| {
                let t = turning(zig, k, w, false).abs();
                (t / turning(zig, k, b, false).abs().max(1e-3)).min(1.0)
            })
            .sum::<f32>()
            / eligible.len().max(1) as f32;
        let sharp = eligible
            .iter()
            .filter(|&&(k, w, _)| turning(zig, k, w, false).abs() >= 45.0 * DEG)
            .count();
        if eligible.len() < 3
            || conc < 0.55
            || sharp * 10 < eligible.len() * 7
            || gaps.iter().any(|d| (d - g).abs() > 0.45 * g)
        {
            return None;
        }
        // Straight flanks between the teeth (a wave's are curved).
        let bent = peaks
            .windows(2)
            .filter(|w| {
                let f = &zig[w[0]..=w[1]];
                let (s, e) = (f[0], f[f.len() - 1]);
                max_dev(f, s, e) > 0.09 * dist(s, e)
            })
            .count();
        if bent * 4 > peaks.len() - 1 {
            return None;
        }
        let rot = snap90(angle(u), 10.0 * DEG);
        let c = [(a[0] + z[0]) / 2.0, (a[1] + z[1]) / 2.0];
        let lead = ((lead0 + lead1) / 2.0 / l).clamp(0.12, 0.35);
        Some(Proposal::new(
            Recognized {
                kind: ShapeKind::Resistor,
                geometry: Geometry::template(
                    "resistor",
                    c,
                    l,
                    2.0 * amp,
                    rot,
                    vec![lead, (peaks.len() as f32).clamp(4.0, 8.0)],
                ),
                confidence: 1.0,
            },
            Vec::new(),
            self.name(),
        ))
    }
}

// ------------------------------------------------------------------ cell

/// A long and a short plate side by side, drawn one after the other.
pub struct Cell;

impl Recognizer for Cell {
    fn name(&self) -> &'static str {
        "cell"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 2 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let s = straight(&resample(stroke, 32), 0.08)?;
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if !recent(n) || n.shape.as_deref().is_some_and(|k| k != "line") {
                return None;
            }
            let t = as_segment(n)?;
            let (ls, lt) = (dist(s.0, s.1), dist(t.0, t.1));
            let (long, short) = if ls >= lt { (s, t) } else { (t, s) };
            let (ll, lsh) = (ls.max(lt), ls.min(lt));
            if ll * px < 36.0 || !(1.4..=3.5).contains(&(ll / lsh)) {
                return None;
            }
            let dl = [long.1[0] - long.0[0], long.1[1] - long.0[1]];
            let ds = [short.1[0] - short.0[0], short.1[1] - short.0[1]];
            let ang = super::angle_between(dl, ds);
            if ang.min(PI - ang) > 12.0 * DEG {
                return None;
            }
            let ul = [dl[0] / ll, dl[1] / ll];
            let (ml, ms) = (
                [(long.0[0] + long.1[0]) / 2.0, (long.0[1] + long.1[1]) / 2.0],
                [
                    (short.0[0] + short.1[0]) / 2.0,
                    (short.0[1] + short.1[1]) / 2.0,
                ],
            );
            let d = [ms[0] - ml[0], ms[1] - ml[1]];
            let off = d[0] * ul[0] + d[1] * ul[1];
            let gap = (-d[0] * ul[1] + d[1] * ul[0]).abs();
            if off.abs() > 0.2 * ll || !(0.12 * ll..=0.7 * ll).contains(&gap) {
                return None;
            }
            // The wire runs across the plates, long plate first (+).
            let wire = angle([-ul[1], ul[0]]);
            let wire = if (-d[0] * ul[1] + d[1] * ul[0]) >= 0.0 {
                wire
            } else {
                wire + PI
            };
            let rot = snap90(wire, 12.0 * DEG);
            let lead = 0.7 * ll;
            let w = 2.0 * lead + gap;
            let c = [(ml[0] + ms[0]) / 2.0, (ml[1] + ms[1]) / 2.0];
            // v: part, lead length / width, short / long plate.
            let part = |k: f32| Recognized {
                kind: ShapeKind::Cell,
                geometry: Geometry::template("cell", c, w, ll, rot, vec![k, lead / w]),
                confidence: 1.0,
            };
            let mut p = Proposal::new(part(0.0), vec![i], self.name());
            p.extra = vec![part(1.0)];
            Some(p)
        })
    }
}

// ------------------------------------------------------------------ bulb

/// A circle with an X across it (the X as two strokes), in any order.
pub struct Bulb;

impl Recognizer for Bulb {
    fn name(&self) -> &'static str {
        "bulb"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 2 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        // The circle: the new stroke or a neighbour.
        let new_circle = circle_of(stroke, 0.8);
        let new_seg = straight(&resample(stroke, 32), 0.08);
        let mut circle: Option<(Option<usize>, P, f32)> = new_circle.map(|(c, r)| (None, c, r));
        if circle.is_none() {
            circle = ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
                if !matches!(n.shape.as_deref(), None | Some("circle")) {
                    return None;
                }
                let (c, r) = as_circle(n, 0.8)?;
                Some((Some(i), c, r))
            });
        }
        let (ci, c, r) = circle?;
        if 2.0 * r * px < 30.0 {
            return None;
        }
        let diag = |s: (P, P)| {
            let th = angle([s.1[0] - s.0[0], s.1[1] - s.0[1]]);
            let off = ((th.abs() % (PI / 2.0)) - PI / 4.0).abs();
            let mid = [(s.0[0] + s.1[0]) / 2.0, (s.0[1] + s.1[1]) / 2.0];
            let ends = [s.0, s.1]
                .iter()
                .all(|e| (dist(*e, c) - r).abs() <= 0.3 * r);
            (off <= 12.0 * DEG && dist(mid, c) <= 0.25 * r && ends).then_some(th)
        };
        let mut lines: Vec<(Option<usize>, f32)> = Vec::new();
        if ci.is_some() {
            lines.push((None, diag(new_seg?)?));
        }
        for (i, n) in ctx.neighbours.iter().enumerate() {
            if Some(i) == ci || !recent(n) || n.shape.as_deref().is_some_and(|k| k != "line") {
                continue;
            }
            if let Some(th) = as_segment(n).and_then(diag) {
                lines.push((Some(i), th));
            }
        }
        let (a, b) = (lines.first()?, lines.get(1)?);
        let cross = super::angle_between([a.1.cos(), a.1.sin()], [b.1.cos(), b.1.sin()]);
        if (cross - PI / 2.0).abs() > 20.0 * DEG {
            return None;
        }
        let mut absorbs: Vec<usize> = ci.into_iter().collect();
        absorbs.extend([a.0, b.0].into_iter().flatten());
        let mut p = Proposal::new(
            Recognized {
                kind: ShapeKind::Bulb,
                geometry: Geometry::template("bulb", c, 3.2 * r, 2.0 * r, 0.0, Vec::new()),
                confidence: 1.0,
            },
            absorbs,
            self.name(),
        );
        p.onto_stroke = ci.is_none();
        Some(p)
    }
}

// ---------------------------------------------------------------- switch

/// A lead with a blade raised from its end (one stroke, "_/") and a second
/// lead starting under the blade's tip ("_"), in either order.
pub struct Switch;

/// A lead-and-blade stroke: (lead start, pivot, blade tip).
fn blade(points: &[P]) -> Option<(P, P, P)> {
    let p = resample(points, 64);
    let c = corners(&p, 3, 15.0 * DEG, false);
    let [k] = c.as_slice() else {
        return None;
    };
    let (a, m, t) = (p[0], p[*k], p[p.len() - 1]);
    let (l1, l2) = (dist(a, m), dist(m, t));
    if l1 < 0.4 * l2 || max_dev(&p[..=*k], a, m) > 0.08 * l1 || max_dev(&p[*k..], m, t) > 0.08 * l2
    {
        return None;
    }
    // The wire is level or upright.
    let lead = angle([m[0] - a[0], m[1] - a[1]]);
    let q = PI / 2.0;
    if (lead - (lead / q).round() * q).abs() > 10.0 * DEG {
        return None;
    }
    let raise = wrap_pi(angle([t[0] - m[0], t[1] - m[1]]) - lead);
    (15.0 * DEG..=50.0 * DEG)
        .contains(&raise.abs())
        .then_some((a, m, t))
}

impl Recognizer for Switch {
    fn name(&self) -> &'static str {
        "switch"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 3 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let new_blade = blade(stroke);
        let new_lead = straight(&resample(stroke, 32), 0.06);
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if !recent(n) || n.shape.as_deref().is_some_and(|k| k != "line") {
                return None;
            }
            let (bl, lead) = match (new_blade, new_lead) {
                (Some(b), _) => (b, as_segment(n)?),
                (None, Some(l)) if n.shape.is_none() => (blade(&n.points)?, l),
                _ => return None,
            };
            let (a, m, t) = bl;
            let lw = dist(a, m);
            let u = [(m[0] - a[0]) / lw, (m[1] - a[1]) / lw];
            let blen = dist(m, t);
            if blen * px < 30.0 {
                return None;
            }
            // The second lead: along the wire, starting under the tip.
            let (l0, l1) = if dist(lead.0, t) <= dist(lead.1, t) {
                lead
            } else {
                (lead.1, lead.0)
            };
            let uv = |q: P| {
                let d = [q[0] - m[0], q[1] - m[1]];
                [d[0] * u[0] + d[1] * u[1], -d[0] * u[1] + d[1] * u[0]]
            };
            let (s0, s1, tt) = (uv(l0), uv(l1), uv(t));
            if s0[1].abs() > 0.2 * blen
                || s1[1].abs() > 0.2 * blen
                || s1[0] <= s0[0]
                || (s0[0] - tt[0]).abs() > 0.35 * blen
                || dist(l0, l1) < 0.3 * lw
            {
                return None;
            }
            let rot = snap90(angle(u), 10.0 * DEG);
            let (x0, x1) = (uv(a)[0], s1[0]);
            let w = x1 - x0;
            let h = tt[1].abs();
            let wire_mid = [m[0] + u[0] * (x0 + x1) / 2.0, m[1] + u[1] * (x0 + x1) / 2.0];
            let up = if tt[1] < 0.0 { -1.0 } else { 1.0 };
            // The box sits on the wire with the blade above (or below) it.
            let nrm = [-u[1] * up, u[0] * up];
            let c = [
                wire_mid[0] + nrm[0] * h / 2.0,
                wire_mid[1] + nrm[1] * h / 2.0,
            ];
            let flip = if up < 0.0 { 0.0 } else { 1.0 };
            // v: part, pivot, contact (fractions of the width), flipped.
            let v = |k: f32| vec![k, (0.0 - x0) / w, (s0[0] - x0) / w, flip];
            let part = |k: f32| Recognized {
                kind: ShapeKind::Switch,
                geometry: Geometry::template("switch", c, w, h, rot, v(k)),
                confidence: 1.0,
            };
            let mut p = Proposal::new(part(0.0), vec![i], self.name());
            p.extra = vec![part(1.0)];
            Some(p)
        })
    }
}

// --------------------------------------------------------------- benzene

/// A circle or a short line inside a hexagon makes it a benzene ring.
pub struct Benzene;

impl Recognizer for Benzene {
    fn name(&self) -> &'static str {
        "benzene"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 2 {
            return None;
        }
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if n.shape.as_deref() != Some("hexagon") {
                return None;
            }
            let Some(Geometry::Polygon { pts }) = &n.geometry else {
                return None;
            };
            let g = [
                pts.iter().map(|q| q[0]).sum::<f32>() / 6.0,
                pts.iter().map(|q| q[1]).sum::<f32>() / 6.0,
            ];
            let big = pts.iter().map(|q| dist(*q, g)).sum::<f32>() / 6.0;
            let hex = Recognized {
                kind: ShapeKind::Benzene,
                geometry: Geometry::Polygon { pts: pts.clone() },
                confidence: 1.0,
            };
            // Aromatic circle.
            if let Some((c, r)) = circle_of(stroke, 0.7)
                && dist(c, g) <= 0.2 * big
                && (0.35 * big..=0.8 * big).contains(&r)
            {
                let mut p = Proposal::new(hex, vec![i], self.name());
                p.extra = vec![Recognized {
                    kind: ShapeKind::Benzene,
                    geometry: Geometry::Ellipse {
                        c: g,
                        rx: 0.58 * big,
                        ry: 0.58 * big,
                        rot: 0.0,
                    },
                    confidence: 1.0,
                }];
                return Some(p);
            }
            // Kekulé: a short line inside, parallel to a side: three double
            // bonds on alternate sides.
            let (a, b) = straight(&resample(stroke, 32), 0.08)?;
            let l = dist(a, b);
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let k = (0..6).find(|&k| {
                let (p0, p1) = (pts[k], pts[(k + 1) % 6]);
                let el = dist(p0, p1);
                let ang = super::angle_between(
                    [b[0] - a[0], b[1] - a[1]],
                    [p1[0] - p0[0], p1[1] - p0[1]],
                );
                let em = [(p0[0] + p1[0]) / 2.0, (p0[1] + p1[1]) / 2.0];
                let inset = dist(mid, em);
                ang.min(PI - ang) <= 12.0 * DEG
                    && (0.4 * el..=0.95 * el).contains(&l)
                    && (0.08 * big..=0.35 * big).contains(&inset)
                    && dist(mid, g) < dist(em, g)
            })?;
            let mut p = Proposal::new(hex, vec![i], self.name());
            p.extra = (0..3)
                .map(|j| {
                    let e = (k + 2 * j) % 6;
                    let (p0, p1) = (pts[e], pts[(e + 1) % 6]);
                    let pull = |q: P| [q[0] + (g[0] - q[0]) * 0.2, q[1] + (g[1] - q[1]) * 0.2];
                    let (q0, q1) = (pull(p0), pull(p1));
                    let shrink =
                        |x: P, y: P| [x[0] + (y[0] - x[0]) * 0.15, x[1] + (y[1] - x[1]) * 0.15];
                    Recognized {
                        kind: ShapeKind::Benzene,
                        geometry: Geometry::Segment {
                            a: shrink(q0, q1),
                            b: shrink(q1, q0),
                        },
                        confidence: 1.0,
                    }
                })
                .collect();
            Some(p)
        })
    }
}

// ----------------------------------------------------------- equilibrium

/// A one-stroke harpoon or arrow: (tail, tip). Harpoons have one barb.
fn half_arrow(points: &[P], opts: Options) -> Option<(P, P)> {
    if let Some(Recognized {
        kind: ShapeKind::Arrow,
        geometry: Geometry::Segment { a, b },
        ..
    }) = recognize_xy(
        points,
        Options {
            deliberate: true,
            ..opts
        },
    ) {
        return Some((a, b));
    }
    let p = resample(points, 64);
    let c = corners(&p, 3, 60.0 * DEG, false);
    let k = *c.last()?;
    // Shaft, then one barb back from the tip.
    let (tail, tip) = (p[0], p[k]);
    let l = dist(tail, tip);
    let barb = &p[k..];
    if l <= f32::EPSILON
        || max_dev(&p[..=k], tail, tip) > 0.06 * l
        || path_len(barb) > 0.45 * l
        || path_len(barb) < 0.08 * l
    {
        return None;
    }
    let u = [(tip[0] - tail[0]) / l, (tip[1] - tail[1]) / l];
    let e = barb[barb.len() - 1];
    let d = [e[0] - tip[0], e[1] - tip[1]];
    let back = d[0] * u[0] + d[1] * u[1];
    (back < -0.05 * l).then_some((tail, tip))
}

/// Two half-arrows (or arrows) side by side pointing opposite ways: ⇌.
pub struct Equilibrium;

impl Recognizer for Equilibrium {
    fn name(&self) -> &'static str {
        "equilibrium"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::SCIENCE) || stroke.len() < 3 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let s = half_arrow(stroke, ctx.opts)?;
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            if !recent(n) {
                return None;
            }
            let t = match (&n.geometry, n.shape.as_deref()) {
                (Some(Geometry::Segment { a, b }), Some("arrow")) => (*a, *b),
                (None, None) => half_arrow(&n.points, ctx.opts)?,
                _ => return None,
            };
            let (ls, lt) = (dist(s.0, s.1), dist(t.0, t.1));
            if ls.min(lt) * px < 40.0 || ls.min(lt) / ls.max(lt) < 0.6 {
                return None;
            }
            let us = [(s.1[0] - s.0[0]) / ls, (s.1[1] - s.0[1]) / ls];
            let ut = [(t.1[0] - t.0[0]) / lt, (t.1[1] - t.0[1]) / lt];
            if us[0] * ut[0] + us[1] * ut[1] > -(170.0 * DEG).cos().abs() {
                return None;
            }
            let l = (ls + lt) / 2.0;
            let ms = [(s.0[0] + s.1[0]) / 2.0, (s.0[1] + s.1[1]) / 2.0];
            let mt = [(t.0[0] + t.1[0]) / 2.0, (t.0[1] + t.1[1]) / 2.0];
            let d = [mt[0] - ms[0], mt[1] - ms[1]];
            let gap = (-d[0] * us[1] + d[1] * us[0]).abs();
            let slide = (d[0] * us[0] + d[1] * us[1]).abs();
            if !(0.04 * l..=0.4 * l).contains(&gap) || slide > 0.4 * l {
                return None;
            }
            // Level when nearly so; the top arrow points right.
            let (_, dir) = fit_line(&[s.0, s.1, [t.1[0], t.1[1]], [t.0[0], t.0[1]]]);
            let mut th = angle(dir);
            if dir[0] < 0.0 {
                th += PI;
            }
            let th = snap90(wrap_pi(th), 12.0 * DEG);
            let (sn, cs) = th.sin_cos();
            let g = [(ms[0] + mt[0]) / 2.0, (ms[1] + mt[1]) / 2.0];
            let gap = gap.clamp(0.08 * l, 0.3 * l);
            // Upper (−normal) points along `th`, lower the other way.
            let nrm = [sn, -cs];
            let harpoon = |off: f32, fwd: bool| {
                let c = [g[0] + nrm[0] * off, g[1] + nrm[1] * off];
                let (a, b) = (
                    [c[0] - cs * l / 2.0, c[1] - sn * l / 2.0],
                    [c[0] + cs * l / 2.0, c[1] + sn * l / 2.0],
                );
                let (tail, tip) = if fwd { (a, b) } else { (b, a) };
                Recognized {
                    kind: ShapeKind::Equilibrium,
                    geometry: Geometry::Template {
                        kind: "harpoon",
                        x: tail[0],
                        y: tail[1],
                        w: tip[0] - tail[0],
                        h: tip[1] - tail[1],
                        rot: 0.0,
                        v: Vec::new(),
                    },
                    confidence: 1.0,
                }
            };
            let mut p = Proposal::new(harpoon(gap / 2.0, true), vec![i], self.name());
            p.extra = vec![harpoon(-gap / 2.0, false)];
            Some(p)
        })
    }
}
