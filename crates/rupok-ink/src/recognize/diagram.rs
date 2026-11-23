//! Diagrams: number lines (a long level line crossed by evenly spaced
//! ticks), tables (a rectangle crossed by straight lines) and Venn diagrams
//! (two or three overlapping circles).

use super::registry::{Context, Label, Neighbour, Proposal, Recognizer};
use super::{
    DEG, Geometry, Options, P, PI, Recognized, ShapeKind, as_circle, as_segment, circle_of, dist,
    family, recognize_xy, resample, straight,
};

/// Shortest number line, screen px.
pub const MIN_NUMBER_LINE_PX: f32 = 240.0;
/// Fewest ticks that make a number line.
pub const MIN_TICKS: usize = 4;
/// Ticks are read from strokes drawn within this long (ms).
const TICK_WINDOW_MS: f64 = 20000.0;
/// Most ticks (and so labels, −16 … 16 at most) a number line gets.
const MAX_TICKS: usize = 25;

/// A level segment, left end first.
fn level_segment(a: P, b: P, tol_deg: f32) -> Option<(P, P)> {
    let th = (b[1] - a[1]).atan2(b[0] - a[0]).abs();
    if th.min(PI - th) > tol_deg * DEG {
        return None;
    }
    Some(if a[0] <= b[0] { (a, b) } else { (b, a) })
}

/// A tick across the level line at height `y` spanning `x0..x1`: short,
/// nearly upright and touching or crossing the line. Returns its x.
fn tick_x(t: (P, P), y: f32, x0: f32, x1: f32, line_len: f32, px: f32) -> Option<f32> {
    let (a, b) = t;
    let l = dist(a, b);
    if l * px < 12.0 || l * px > 140.0 || l > 0.3 * line_len {
        return None;
    }
    let th = (b[1] - a[1]).atan2(b[0] - a[0]).abs();
    if (th - PI / 2.0).abs() > 25.0 * DEG {
        return None;
    }
    let (top, bot) = (a[1].min(b[1]), a[1].max(b[1]));
    let tol = 0.2 * l;
    if y < top - tol || y > bot + tol {
        return None;
    }
    // Where it meets the line.
    let k = if (b[1] - a[1]).abs() > 1e-3 {
        ((y - a[1]) / (b[1] - a[1])).clamp(0.0, 1.0)
    } else {
        0.5
    };
    let x = a[0] + k * (b[0] - a[0]);
    let m = 0.02 * (x1 - x0);
    (x >= x0 - m && x <= x1 + m).then_some(x)
}

/// A number line, read once the pen rests after its last tick: a long level
/// line with four or more evenly spaced ticks. Ticks fill the whole line,
/// zero is the middle tick drawn, and each tick gets its integer.
pub struct NumberLine;

impl Recognizer for NumberLine {
    fn name(&self) -> &'static str {
        "number-line"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn deferred(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::NUMBER_LINE) || stroke.len() < 2 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        // The line: the new stroke or a neighbour, the longest level one.
        let seg_of_stroke = straight(&resample(stroke, 48), 0.07);
        let mut line: Option<(Option<usize>, P, P)> = None;
        let mut consider = |idx: Option<usize>, a: P, b: P| {
            if let Some((a, b)) = level_segment(a, b, 6.0)
                && dist(a, b) * px >= MIN_NUMBER_LINE_PX
                && line.is_none_or(|(_, la, lb)| dist(a, b) > dist(la, lb))
            {
                line = Some((idx, a, b));
            }
        };
        if let Some((a, b)) = seg_of_stroke {
            consider(None, a, b);
        }
        for (i, n) in ctx.neighbours.iter().enumerate() {
            if n.shape.as_deref().is_none_or(|s| s == "line")
                && let Some((a, b)) = as_segment(n)
            {
                consider(Some(i), a, b);
            }
        }
        let (line_idx, la, lb) = line?;
        let y = (la[1] + lb[1]) / 2.0;
        let (x0, x1) = (la[0], lb[0]);
        let len = x1 - x0;
        // Ticks: the new stroke (unless it is the line) and recent raw ink.
        let mut ticks: Vec<(Option<usize>, f32)> = Vec::new();
        if line_idx.is_some() {
            let (a, b) = seg_of_stroke?;
            ticks.push((None, tick_x((a, b), y, x0, x1, len, px)?));
        }
        for (i, n) in ctx.neighbours.iter().enumerate() {
            if Some(i) == line_idx || n.shape.is_some() {
                continue;
            }
            let crosses = n.bounds[1] <= y + 4.0 / px
                && n.bounds[3] >= y - 4.0 / px
                && n.bounds[2] >= x0
                && n.bounds[0] <= x1;
            if !crosses {
                continue;
            }
            match straight(&n.points, 0.12).and_then(|t| tick_x(t, y, x0, x1, len, px)) {
                Some(x) if (0.0..=TICK_WINDOW_MS).contains(&n.gap_ms) => ticks.push((Some(i), x)),
                // Writing or a drawing on the line: not a number line.
                _ => return None,
            }
        }
        if ticks.len() < MIN_TICKS {
            return None;
        }
        ticks.sort_by(|a, b| a.1.total_cmp(&b.1));
        let gaps: Vec<f32> = ticks.windows(2).map(|w| w[1].1 - w[0].1).collect();
        let g = gaps.iter().sum::<f32>() / gaps.len() as f32;
        let sd = (gaps.iter().map(|d| (d - g).powi(2)).sum::<f32>() / gaps.len() as f32).sqrt();
        if g * px < 20.0 || sd > 0.2 * g || gaps.iter().any(|d| (d - g).abs() > 0.35 * g) {
            return None;
        }
        // Regular positions: least squares x_i = x_z + (i − z)·g.
        let z = (ticks.len() - 1) / 2;
        let xz = ticks
            .iter()
            .enumerate()
            .map(|(i, t)| t.1 - (i as f32 - z as f32) * g)
            .sum::<f32>()
            / ticks.len() as f32;
        // Fill the line (clear of the arrowheads).
        let margin = (0.6 * g).max(28.0 / px);
        let (lo, hi) = (x0 + margin, x1 - margin);
        let k0 = ((lo - xz) / g).ceil() as i32;
        let k1 = ((hi - xz) / g).floor() as i32;
        let mut ks: Vec<i32> = (k0..=k1).filter(|k| k.abs() <= 16).collect();
        while ks.len() > MAX_TICKS {
            // Drop the end further from zero.
            if ks[0].abs() >= ks[ks.len() - 1].abs() {
                ks.remove(0);
            } else {
                ks.pop();
            }
        }
        if ks.len() < MIN_TICKS {
            return None;
        }
        let v: Vec<f32> = ks
            .iter()
            .flat_map(|&k| [(xz + k as f32 * g - x0) / len, k as f32])
            .collect();
        let size = (0.36 * g).clamp(14.0 / px, 28.0 / px);
        let half_tick = (0.18 * g).clamp(6.0 / px, 16.0 / px);
        let labels = ks
            .iter()
            .map(|&k| Label {
                text: if k < 0 {
                    format!("−{}", -k)
                } else {
                    k.to_string()
                },
                at: [xz + k as f32 * g, y + half_tick + 0.85 * size],
                size,
            })
            .collect();
        let mut absorbs: Vec<usize> = line_idx.into_iter().collect();
        absorbs.extend(ticks.iter().filter_map(|t| t.0));
        let mut p = Proposal::new(
            Recognized {
                kind: ShapeKind::NumberLine,
                geometry: Geometry::Template {
                    kind: "numberline",
                    x: x0,
                    y,
                    w: len,
                    h: 0.0,
                    rot: 0.0,
                    v,
                },
                confidence: (1.0 - sd / (0.2 * g)).clamp(0.0, 1.0),
            },
            absorbs,
            self.name(),
        );
        p.onto_stroke = line_idx.is_none();
        p.labels = labels;
        Some(p)
    }
}

// ----------------------------------------------------------------- table

/// A table's box and dividers from a stored `rect` or `table`.
fn table_of(n: &Neighbour) -> Option<([f32; 4], Vec<f32>)> {
    match (&n.geometry, n.shape.as_deref()) {
        (Some(Geometry::Rect { c, w, h, rot }), _) if rot.abs() < 1.0 * DEG => Some((
            [
                c[0] - w / 2.0,
                c[1] - h / 2.0,
                c[0] + w / 2.0,
                c[1] + h / 2.0,
            ],
            Vec::new(),
        )),
        (
            Some(Geometry::Template {
                kind: "table",
                x,
                y,
                w,
                h,
                rot,
                v,
            }),
            _,
        ) if rot.abs() < 1.0 * DEG => Some(([*x, *y, x + w, y + h], v.clone())),
        _ => None,
    }
}

/// Snap dividers along one axis to equal spacing when they are close to it.
fn even_out(fr: &mut [f32]) {
    fr.sort_by(f32::total_cmp);
    let k = fr.len() as f32 + 1.0;
    if fr
        .iter()
        .enumerate()
        .all(|(i, f)| (f - (i as f32 + 1.0) / k).abs() <= 0.06)
    {
        for (i, f) in fr.iter_mut().enumerate() {
            *f = (i as f32 + 1.0) / k;
        }
    }
}

/// A straight line across a rectangle (edge to edge, level or upright)
/// makes it a table; every further line adds a row or column.
pub struct Table;

impl Recognizer for Table {
    fn name(&self) -> &'static str {
        "table"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::TABLES) || stroke.len() < 2 {
            return None;
        }
        let (a, b) = straight(&resample(stroke, 48), 0.06)?;
        let th = (b[1] - a[1]).atan2(b[0] - a[0]).abs();
        let off = th.min(PI - th);
        let horizontal = off <= 8.0 * DEG;
        if !horizontal && (off - PI / 2.0).abs() > 8.0 * DEG {
            return None;
        }
        ctx.neighbours.iter().enumerate().find_map(|(i, n)| {
            let (bx, mut divs) = table_of(n)?;
            let (w, h) = (bx[2] - bx[0], bx[3] - bx[1]);
            // Ends on the two opposite edges (a little over or short is fine).
            let (lo, hi, at, from, to, size) = if horizontal {
                (
                    a[0].min(b[0]),
                    a[0].max(b[0]),
                    (a[1] + b[1]) / 2.0,
                    bx[0],
                    bx[2],
                    w,
                )
            } else {
                (
                    a[1].min(b[1]),
                    a[1].max(b[1]),
                    (a[0] + b[0]) / 2.0,
                    bx[1],
                    bx[3],
                    h,
                )
            };
            let tol = 0.1 * size;
            if (lo - from).abs() > tol || (hi - to).abs() > tol {
                return None;
            }
            let (o0, o1) = if horizontal { (bx[1], h) } else { (bx[0], w) };
            let f = (at - o0) / o1;
            if !(0.08..=0.92).contains(&f) {
                return None;
            }
            let axis = if horizontal { 1.0 } else { 0.0 };
            let mut same: Vec<f32> = divs
                .chunks(2)
                .filter(|d| d.len() == 2 && d[0] == axis)
                .map(|d| d[1])
                .collect();
            if same.iter().any(|g| (g - f).abs() < 0.06) {
                return None;
            }
            same.push(f);
            even_out(&mut same);
            divs = divs
                .chunks(2)
                .filter(|d| d.len() == 2 && d[0] != axis)
                .flatten()
                .copied()
                .collect();
            divs.extend(same.iter().flat_map(|g| [axis, *g]));
            Some(Proposal::new(
                Recognized {
                    kind: ShapeKind::Table,
                    geometry: Geometry::Template {
                        kind: "table",
                        x: bx[0],
                        y: bx[1],
                        w,
                        h,
                        rot: 0.0,
                        v: divs,
                    },
                    confidence: 1.0,
                },
                vec![i],
                self.name(),
            ))
        })
    }
}

// ------------------------------------------------------------------ venn

/// Two circles overlap as a Venn diagram does: neither inside the other,
/// a real lens between them, similar sizes.
fn venn_pair(a: (P, f32), b: (P, f32)) -> bool {
    let d = dist(a.0, b.0);
    let r = (a.1 + b.1) / 2.0;
    a.1.min(b.1) / a.1.max(b.1) >= 0.75
        && d >= 0.35 * r
        && d <= 1.65 * r
        && d < a.1 + b.1 - 0.15 * r
        && d > (a.1 - b.1).abs() + 0.15 * r
}

/// Snap an angle to the nearest multiple of `step` when within `tol`.
fn snap_angle(th: f32, step: f32, tol: f32) -> f32 {
    let t = (th / step).round() * step;
    if (th - t).abs() <= tol { t } else { th }
}

/// A circle drawn overlapping one or two circles makes a Venn diagram:
/// equal circles, level (or an even triangle for three), grouped.
pub struct Venn;

impl Recognizer for Venn {
    fn name(&self) -> &'static str {
        "venn"
    }
    fn contextual(&self) -> bool {
        true
    }
    fn recognize(&self, stroke: &[P], ctx: &Context) -> Option<Proposal> {
        if !ctx.opts.has(family::TABLES) || stroke.len() < 6 {
            return None;
        }
        let px = ctx.opts.px_per_unit.max(1e-4);
        let opts = Options {
            deliberate: true,
            ..ctx.opts
        };
        let new = match recognize_xy(stroke, opts) {
            Some(Recognized {
                geometry: Geometry::Ellipse { c, rx, ry, .. },
                ..
            }) if rx.min(ry) / rx.max(ry) >= 0.75 => (c, (rx + ry) / 2.0),
            Some(_) => return None,
            None => circle_of(stroke, 0.75)?,
        };
        if 2.0 * new.1 * px < 60.0 {
            return None;
        }
        let near: Vec<(usize, (P, f32))> = ctx
            .neighbours
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n.shape.as_deref(), Some("circle" | "ellipse" | "venn")))
            .filter_map(|(i, n)| Some((i, as_circle(n, 0.85)?)))
            .filter(|(_, c)| venn_pair(new, *c))
            .collect();
        let circles: Vec<(usize, (P, f32))> = match near.as_slice() {
            [one] => vec![*one],
            [a, b] if venn_pair(a.1, b.1) => vec![*a, *b],
            _ => return None,
        };
        let mut all: Vec<(P, f32)> = circles.iter().map(|c| c.1).collect();
        all.push(new);
        let r = all.iter().map(|c| c.1).sum::<f32>() / all.len() as f32;
        let g = [
            all.iter().map(|c| c.0[0]).sum::<f32>() / all.len() as f32,
            all.iter().map(|c| c.0[1]).sum::<f32>() / all.len() as f32,
        ];
        let centres: Vec<P> = if all.len() == 2 {
            let d = dist(all[0].0, all[1].0);
            let th = snap_angle(
                (all[1].0[1] - all[0].0[1]).atan2(all[1].0[0] - all[0].0[0]),
                PI / 2.0,
                15.0 * DEG,
            );
            let (s, c) = th.sin_cos();
            vec![
                [g[0] - c * d / 2.0, g[1] - s * d / 2.0],
                [g[0] + c * d / 2.0, g[1] + s * d / 2.0],
            ]
        } else {
            let side =
                (dist(all[0].0, all[1].0) + dist(all[1].0, all[2].0) + dist(all[0].0, all[2].0))
                    / 3.0;
            let rad = side / 3f32.sqrt();
            // One circle on top (−90°) or at the bottom (90°), whichever is
            // nearer to how they were drawn: sin 3θ is +1 for "one on top".
            let s3 = all
                .iter()
                .map(|c| (3.0 * (c.0[1] - g[1]).atan2(c.0[0] - g[0])).sin())
                .sum::<f32>();
            let th0 = if s3 >= 0.0 { -PI / 2.0 } else { PI / 2.0 };
            (0..3)
                .map(|k| {
                    let a = th0 + k as f32 * 2.0 * PI / 3.0;
                    [g[0] + rad * a.cos(), g[1] + rad * a.sin()]
                })
                .collect()
        };
        // Each drawn circle goes to the nearest clean position.
        let mut free: Vec<P> = centres.clone();
        let mut placed: Vec<P> = Vec::new();
        for c in &all {
            let (j, _) = free
                .iter()
                .enumerate()
                .min_by(|a, b| dist(*a.1, c.0).total_cmp(&dist(*b.1, c.0)))?;
            placed.push(free.remove(j));
        }
        let circle = |c: P| Recognized {
            kind: ShapeKind::Venn,
            geometry: Geometry::Ellipse {
                c,
                rx: r,
                ry: r,
                rot: 0.0,
            },
            confidence: 1.0,
        };
        let mut p = Proposal::new(
            circle(placed[0]),
            circles.iter().map(|c| c.0).collect(),
            self.name(),
        );
        p.extra = placed[1..].iter().map(|c| circle(*c)).collect();
        Some(p)
    }
}
