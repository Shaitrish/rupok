//! Vector page data for export (PDF): every item of a page in stacking order,
//! with each stroke as one filled outline polygon.
//!
//! The renderer draws a stroke as a chain of capsules: a disc of radius
//! [`rupok_ink::point_radius`] at every point, joined by the outer tangents
//! of neighbouring discs. [`stroke_outline`] traces the boundary of that
//! shape: the left tangents forward, a round cap, the right tangents back and
//! a round cap. At a joint the outer side follows the disc with an arc and the
//! inner side passes through the disc centre, so every piece of the polygon
//! (segment trapezoids, joint wedges, caps) winds the same way. Filled with
//! the non-zero rule it covers exactly the capsule chain, with no holes where
//! the inner side folds over itself, and a translucent highlighter is painted
//! once, as on the board.

use rupok_core::{BoxGeom, Frame, ItemData, Lesson};
use rupok_format::Package;
use rupok_ink::{StrokePoint, point_radius};
use serde::Serialize;
use std::f32::consts::{PI, TAU};

/// Largest distance (world units) between the round outline and its polygon.
pub const OUTLINE_TOLERANCE: f32 = 0.12;

/// Vertices closer than this (world units) to the simplified outline are
/// dropped (Douglas–Peucker). 0.1 units is 0.05 pt on a slide page.
pub const SIMPLIFY_TOLERANCE: f32 = 0.1;

#[derive(Clone, Copy, Debug)]
struct Disc {
    x: f32,
    y: f32,
    r: f32,
}

type P = [f32; 2];

fn add(c: Disc, m: P) -> P {
    [c.x + c.r * m[0], c.y + c.r * m[1]]
}

fn dist(a: P, b: P) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Discs of a stroke with the offset applied, split into chains. Where one
/// disc lies inside its neighbour the outer tangents do not exist, so the
/// chain ends there and a new one starts: the union of the chains is still
/// exactly the stroke. Inside every chain `|r0 - r1| < distance`.
fn chains(points: &[StrokePoint], width: f32, offset: [f32; 2]) -> Vec<Vec<Disc>> {
    const EPS: f32 = 1e-3;
    let mut out: Vec<Vec<Disc>> = Vec::new();
    let mut cur: Vec<Disc> = Vec::with_capacity(points.len());
    for p in points {
        let d = Disc {
            x: p.x + offset[0],
            y: p.y + offset[1],
            r: point_radius(p.p, width).max(0.05),
        };
        if !(d.x.is_finite() && d.y.is_finite() && d.r.is_finite()) {
            continue;
        }
        if let Some(&last) = cur.last() {
            let gap = (d.x - last.x).hypot(d.y - last.y);
            if gap <= EPS && (d.r - last.r).abs() <= EPS {
                continue; // a repeated point
            }
            if gap + d.r.min(last.r) <= d.r.max(last.r) + EPS {
                // One disc holds the other. A lone disc inside this one adds
                // nothing; otherwise keep the chain so far.
                let lone_inside = cur.len() == 1 && last.r <= d.r;
                let done = std::mem::replace(&mut cur, Vec::with_capacity(8));
                if !lone_inside {
                    out.push(done);
                }
            }
        }
        cur.push(d);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn angle(m: P) -> f32 {
    m[1].atan2(m[0])
}

/// The angle from `from` to `to` going the positive way, in `[0, 2π)`.
fn positive_span(from: f32, to: f32) -> f32 {
    (to - from).rem_euclid(TAU)
}

/// Angle step that keeps an arc of radius `r` within `tol` of the circle.
fn arc_step(r: f32, tol: f32) -> f32 {
    if r <= tol {
        return PI / 2.0;
    }
    (2.0 * (1.0 - tol / r).acos()).clamp(0.05, PI / 2.0)
}

/// Push the points strictly between `from` and `to` on the circle around
/// `c`, sweeping in the positive (`positive`) or negative angle direction.
fn arc(out: &mut Vec<P>, c: Disc, from: P, to: P, positive: bool, tol: f32) {
    let a0 = (from[1] - c.y).atan2(from[0] - c.x);
    let a1 = (to[1] - c.y).atan2(to[0] - c.x);
    let sweep = if positive {
        positive_span(a0, a1)
    } else {
        -positive_span(a1, a0)
    };
    let k = (sweep.abs() / arc_step(c.r, tol)).ceil() as usize;
    for i in 1..k {
        let a = a0 + sweep * i as f32 / k as f32;
        out.push([c.x + c.r * a.cos(), c.y + c.r * a.sin()]);
    }
}

fn circle(c: Disc, tol: f32) -> Vec<P> {
    let k = ((TAU / arc_step(c.r, tol)).ceil() as usize).max(8);
    (0..k)
        .map(|i| {
            // Negative angle direction: the same winding as the chains.
            let a = -TAU * i as f32 / k as f32;
            [c.x + c.r * a.cos(), c.y + c.r * a.sin()]
        })
        .collect()
}

/// One segment between two discs: the unit normals of its left and right
/// outer tangent lines.
struct Seg {
    ml: P,
    mr: P,
}

fn seg(a: Disc, b: Disc) -> Seg {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy).max(1e-6);
    let u = [dx / len, dy / len];
    let n = [-u[1], u[0]];
    // The tangent normal m satisfies m·u = (r0 - r1) / len.
    let s = ((a.r - b.r) / len).clamp(-0.999, 0.999);
    let c = (1.0 - s * s).sqrt();
    Seg {
        ml: [u[0] * s + n[0] * c, u[1] * s + n[1] * c],
        mr: [u[0] * s - n[0] * c, u[1] * s - n[1] * c],
    }
}

/// The outline of a stroke as closed polygons (usually one), each flat
/// `[x0, y0, x1, y1, …]` in world units and all winding the same way. Fill
/// them together with the non-zero rule. Empty when the stroke has no usable
/// points.
pub fn stroke_outline(
    points: &[StrokePoint],
    width: f32,
    offset: [f32; 2],
    tol: f32,
) -> Vec<Vec<f32>> {
    let tol = tol.max(0.01);
    chains(points, width, offset)
        .iter()
        .map(|d| flatten(simplify_closed(&chain_outline(d, tol), SIMPLIFY_TOLERANCE)))
        .filter(|p| p.len() >= 6)
        .collect()
}

fn chain_outline(d: &[Disc], tol: f32) -> Vec<P> {
    let n = d.len();
    match n {
        0 => return Vec::new(),
        1 => return circle(d[0], tol),
        _ => {}
    }
    let segs: Vec<Seg> = d.windows(2).map(|w| seg(w[0], w[1])).collect();
    let mut left: Vec<P> = Vec::with_capacity(n * 2);
    let mut right: Vec<P> = Vec::with_capacity(n * 2);
    left.push(add(d[0], segs[0].ml));
    right.push(add(d[0], segs[0].mr));
    for j in 1..n - 1 {
        let (c, p, q) = (d[j], &segs[j - 1], &segs[j]);
        let (al, bl) = (add(c, p.ml), add(c, q.ml));
        let (ar, br) = (add(c, p.mr), add(c, q.mr));
        if dist(al, bl) <= tol && dist(ar, br) <= tol {
            left.push([(al[0] + bl[0]) / 2.0, (al[1] + bl[1]) / 2.0]);
            right.push([(ar[0] + br[0]) / 2.0, (ar[1] + br[1]) / 2.0]);
            continue;
        }
        // Around the disc, going the positive way: the end of the previous
        // trapezoid covers aLp → aRp, the right joint aRp → aRn, the next
        // trapezoid aRn → aLn and the left joint aLn → aLp. Together they
        // wrap the circle k times. With k = 1 every joint span is a real gap
        // and both sides go round; with k ≥ 2 three of the four already
        // cover the disc, so the side with the larger span (where the
        // trapezoids overlap) passes through the centre instead.
        let (alp, arp) = (angle(p.ml), angle(p.mr));
        let (aln, arn) = (angle(q.ml), angle(q.mr));
        let span_l = positive_span(aln, alp);
        let span_r = positive_span(arp, arn);
        let total = positive_span(alp, arp) + span_r + positive_span(arn, aln) + span_l;
        let both = (total / TAU).round() <= 1.0;
        let centre = [c.x, c.y];
        if both || span_l <= span_r {
            left.push(al);
            arc(&mut left, c, al, bl, false, tol);
            left.push(bl);
        } else {
            left.extend([al, centre, bl]);
        }
        if both || span_r < span_l {
            right.push(ar);
            arc(&mut right, c, ar, br, true, tol);
            right.push(br);
        } else {
            right.extend([ar, centre, br]);
        }
    }
    let (last, ls) = (d[n - 1], &segs[n - 2]);
    left.push(add(last, ls.ml));
    right.push(add(last, ls.mr));

    let mut poly = Vec::with_capacity(left.len() + right.len() + 32);
    poly.push(right[0]);
    arc(&mut poly, d[0], right[0], left[0], false, tol);
    poly.extend_from_slice(&left);
    let (l_end, r_end) = (left[left.len() - 1], right[right.len() - 1]);
    arc(&mut poly, last, l_end, r_end, false, tol);
    poly.extend(right[1..].iter().rev());
    poly
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: P, a: P, b: P) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return dist(p, a);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0);
    dist(p, [a[0] + dx * t, a[1] + dy * t])
}

/// Douglas–Peucker on a closed polygon (iterative, so long strokes cannot
/// overflow the stack). The first vertex is always kept.
fn simplify_closed(poly: &[P], tol: f32) -> Vec<P> {
    let n = poly.len();
    if n < 4 {
        return poly.to_vec();
    }
    // Treat the ring as the open path poly[0] … poly[n-1], poly[0].
    let at = |i: usize| poly[i % n];
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    keep[n] = true;
    let mut stack = vec![(0usize, n)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (at(a), at(b));
        let (mut far, mut best) = (a, -1.0f32);
        for i in a + 1..b {
            let d = segment_distance(at(i), pa, pb);
            if d > best {
                best = d;
                far = i;
            }
        }
        if best > tol {
            keep[far] = true;
            stack.push((a, far));
            stack.push((far, b));
        }
    }
    (0..n).filter(|&i| keep[i]).map(|i| poly[i]).collect()
}

/// Round to 0.1 world units and drop repeated vertices.
fn flatten(poly: Vec<P>) -> Vec<f32> {
    let mut out = Vec::with_capacity(poly.len() * 2);
    let mut prev: Option<P> = None;
    for p in poly {
        let q = [round1(p[0]), round1(p[1])];
        if prev != Some(q) {
            out.extend_from_slice(&q);
            prev = Some(q);
        }
    }
    out
}

fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

#[derive(Serialize)]
struct ExportPage<'a> {
    title: &'a str,
    frame: Option<Frame>,
    /// 0xRRGGBBAA
    bg: u32,
    pattern: &'static str,
    /// World bounds of all items, `[minX, minY, maxX, maxY]`.
    bounds: Option<[f32; 4]>,
    items: Vec<ExportItem<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum ExportItem<'a> {
    Stroke {
        /// 0xRRGGBBAA
        color: u32,
        opacity: f32,
        tool: &'a str,
        shape: Option<&'a str>,
        /// Outline polygons, see [`stroke_outline`].
        outline: Vec<Vec<f32>>,
    },
    Text {
        rect: [f64; 4],
        text: &'a str,
        size: f64,
        color: u32,
        /// `text` is LaTeX maths.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        latex: bool,
    },
    Image {
        rect: [f64; 4],
        asset: &'a str,
    },
    Object {
        id: &'a str,
        rect: [f64; 4],
        title: &'a str,
        bundle: &'a str,
        step: u32,
        steps: u32,
        state: Option<&'a str>,
    },
}

fn rect(g: &BoxGeom) -> [f64; 4] {
    [g.x, g.y, g.w, g.h]
}

/// Everything needed to draw page `index` as vectors, as JSON:
/// `{title, frame, bg, pattern, bounds, items}` with items in stacking order
/// (`stroke` items carry an `outline`); `null` for a bad index. With
/// `strokes` false, strokes are left out (a quick look for Smart Objects).
/// Reads the document only, so exporting never changes the lesson or its
/// history.
pub fn page_export_json(lesson: &Lesson, pkg: &Package, index: usize, strokes: bool) -> String {
    let Ok(id) = lesson.page_id_at(index) else {
        return "null".into();
    };
    let Ok(info) = lesson.page(&id) else {
        return "null".into();
    };
    let items = lesson.items(&id).unwrap_or_default();
    let mut bounds: Option<[f32; 4]> = None;
    let mut out = Vec::with_capacity(items.len());
    for it in &items {
        let b = it.data.aabb();
        bounds = Some(match bounds {
            None => b,
            Some(a) => [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ],
        });
        out.push(match &it.data {
            ItemData::Stroke(_) if !strokes => continue,
            ItemData::Stroke(s) => ExportItem::Stroke {
                color: s.color,
                opacity: s.opacity,
                tool: &s.tool,
                shape: s.shape.as_deref(),
                outline: stroke_outline(&s.points, s.width, s.offset, OUTLINE_TOLERANCE),
            },
            ItemData::Text(t) => ExportItem::Text {
                rect: rect(&t.geom),
                text: &t.text,
                size: t.size,
                color: t.color,
                latex: t.latex,
            },
            ItemData::Image(i) => ExportItem::Image {
                rect: rect(&i.geom),
                asset: &i.asset,
            },
            ItemData::Object(o) => ExportItem::Object {
                id: &it.id,
                rect: rect(&o.geom),
                title: &o.title,
                bundle: &o.bundle,
                step: o.step,
                steps: pkg.objects.get(&o.bundle).map_or(0, |b| b.manifest.steps),
                state: o.state.as_deref(),
            },
        });
    }
    serde_json::to_string(&ExportPage {
        title: &info.title,
        frame: info.frame,
        bg: info.bg_color,
        pattern: info.pattern.as_str(),
        bounds,
        items: out,
    })
    .unwrap_or_else(|_| "null".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32, p: f32) -> StrokePoint {
        StrokePoint { x, y, p, dt: 0 }
    }

    fn pairs(flat: &[f32]) -> Vec<P> {
        flat.chunks(2).map(|c| [c[0], c[1]]).collect()
    }

    /// Winding number of a closed polygon around `q`.
    fn winding(poly: &[P], q: P) -> i32 {
        let mut w = 0;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let side = (b[0] - a[0]) * (q[1] - a[1]) - (q[0] - a[0]) * (b[1] - a[1]);
            if a[1] <= q[1] {
                if b[1] > q[1] && side > 0.0 {
                    w += 1;
                }
            } else if b[1] <= q[1] && side < 0.0 {
                w -= 1;
            }
        }
        w
    }

    /// Distance from `q` to the capsule chain the renderer draws (the
    /// shader interpolates the radius at the projected point).
    fn capsule_distance(points: &[StrokePoint], width: f32, q: P) -> f32 {
        let mut best = f32::INFINITY;
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (ra, rb) = (point_radius(a.p, width), point_radius(b.p, width));
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = (dx * dx + dy * dy).max(1e-9);
            let t = (((q[0] - a.x) * dx + (q[1] - a.y) * dy) / len2).clamp(0.0, 1.0);
            let (cx, cy) = (a.x + dx * t, a.y + dy * t);
            let r = ra + (rb - ra) * t;
            best = best.min((q[0] - cx).hypot(q[1] - cy) - r);
        }
        best
    }

    /// Distance from `q` to the union of the convex hulls of neighbouring
    /// discs (what the outline traces; it contains the shader's shape and
    /// differs from it only where the pressure jumps between two points).
    fn hull_distance(points: &[StrokePoint], width: f32, q: P) -> f32 {
        let mut best = f32::INFINITY;
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (ra, rb) = (point_radius(a.p, width), point_radius(b.p, width));
            for k in 0..=400 {
                let t = k as f32 / 400.0;
                let (cx, cy) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                best = best.min((q[0] - cx).hypot(q[1] - cy) - (ra + (rb - ra) * t));
            }
        }
        best
    }

    /// Sample a grid over the stroke: everything the renderer covers (with a
    /// margin) must be inside the outline, nothing beyond the disc hulls.
    fn check_cover(points: &[StrokePoint], width: f32) {
        let polys: Vec<Vec<P>> = stroke_outline(points, width, [0.0, 0.0], OUTLINE_TOLERANCE)
            .iter()
            .map(|p| pairs(p))
            .collect();
        assert!(!polys.is_empty());
        let pad = width * 1.5;
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            x0 = x0.min(p.x - pad);
            y0 = y0.min(p.y - pad);
            x1 = x1.max(p.x + pad);
            y1 = y1.max(p.y + pad);
        }
        let n = 90;
        for i in 0..=n {
            for j in 0..=n {
                let q = [
                    x0 + (x1 - x0) * i as f32 / n as f32,
                    y0 + (y1 - y0) * j as f32 / n as f32,
                ];
                let w: i32 = polys.iter().map(|p| winding(p, q)).sum();
                if w == 0 {
                    let d = capsule_distance(points, width, q);
                    assert!(d > -0.25, "hole at {q:?} (inside by {})", -d);
                } else {
                    let d = hull_distance(points, width, q);
                    assert!(d < 0.25, "spill at {q:?} (outside by {d})");
                }
            }
        }
    }

    #[test]
    fn straight_line_is_a_capsule() {
        let pts = [pt(0.0, 0.0, 1.0), pt(100.0, 0.0, 1.0)];
        check_cover(&pts, 10.0);
        let out = stroke_outline(&pts, 10.0, [0.0, 0.0], 0.1);
        assert_eq!(out.len(), 1, "one polygon");
        let poly = pairs(&out[0]);
        let max_x = poly.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let min_x = poly.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        assert!(
            (max_x - 105.0).abs() < 0.2 && (min_x + 5.0).abs() < 0.2,
            "round caps"
        );
    }

    #[test]
    fn sharp_turns_have_no_holes() {
        // A zigzag with hairpin turns and a pressure ramp.
        let pts: Vec<_> = (0..12)
            .map(|i| {
                let x = i as f32 * 6.0;
                let y = if i % 2 == 0 { 0.0 } else { 40.0 };
                pt(x, y, i as f32 / 11.0)
            })
            .collect();
        check_cover(&pts, 14.0);
    }

    #[test]
    fn dense_wobbly_ink_with_pressure() {
        let pts: Vec<_> = (0..200)
            .map(|i| {
                let t = i as f32 * 0.07;
                pt(
                    t * 30.0,
                    (t * 3.1).sin() * 25.0 + (t * 11.0).sin() * 3.0,
                    0.5 + 0.5 * (t * 2.0).sin(),
                )
            })
            .collect();
        check_cover(&pts, 8.0);
    }

    #[test]
    fn full_reversal_and_closed_shapes() {
        check_cover(
            &[pt(0.0, 0.0, 0.6), pt(80.0, 0.0, 0.6), pt(0.0, 0.0, 0.6)],
            6.0,
        );
        let rect = crate::shapes::shape_points(
            crate::shapes::ShapeKind::Rect,
            [0.0, 0.0],
            [120.0, 60.0],
            4.0,
        );
        check_cover(&rect, 4.0);
        let arrow = crate::shapes::shape_points(
            crate::shapes::ShapeKind::Arrow,
            [0.0, 0.0],
            [150.0, 40.0],
            4.0,
        );
        check_cover(&arrow, 4.0);
    }

    #[test]
    fn random_strokes_cover_exactly() {
        // Deterministic random walks: short and long steps, pressure jumps.
        let mut seed: u32 = 0x9e37_79b9;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32) / (u32::MAX as f32)
        };
        for _ in 0..24 {
            let width = 2.0 + rnd() * 28.0;
            let (mut x, mut y, mut a) = (0.0f32, 0.0f32, 0.0f32);
            let pts: Vec<_> = (0..30)
                .map(|_| {
                    a += (rnd() - 0.5) * 3.0;
                    let step = 0.3 + rnd() * 9.0;
                    x += a.cos() * step;
                    y += a.sin() * step;
                    pt(x, y, rnd())
                })
                .collect();
            check_cover(&pts, width);
        }
    }

    #[test]
    fn simplification_drops_collinear_vertices_only() {
        let square: Vec<P> = vec![
            [0.0, 0.0],
            [5.0, 0.0],
            [10.0, 0.0],
            [10.0, 5.0],
            [10.0, 10.0],
            [5.0, 10.02],
            [0.0, 10.0],
        ];
        let s = simplify_closed(&square, 0.1);
        assert_eq!(s, vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]]);
        // Dense smooth ink loses most of its outline vertices.
        let pts: Vec<_> = (0..300)
            .map(|i| pt(i as f32 * 0.5, (i as f32 * 0.01).sin() * 4.0, 0.5))
            .collect();
        let raw = chain_outline(&chains(&pts, 4.0, [0.0, 0.0])[0], OUTLINE_TOLERANCE).len();
        let kept = stroke_outline(&pts, 4.0, [0.0, 0.0], OUTLINE_TOLERANCE)[0].len() / 2;
        assert!(kept * 3 < raw, "{kept} of {raw} vertices kept");
    }

    #[test]
    fn dot_and_contained_points() {
        let dot = stroke_outline(&[pt(5.0, 5.0, 1.0)], 10.0, [1.0, 2.0], 0.1);
        let poly = pairs(&dot[0]);
        assert!(poly.len() >= 8);
        assert_ne!(winding(&poly, [6.0, 7.0]), 0);
        // A heavy point swallowing its light neighbours, and duplicates.
        check_cover(
            &[
                pt(0.0, 0.0, 0.0),
                pt(0.5, 0.0, 1.0),
                pt(0.5, 0.0, 1.0),
                pt(1.0, 0.0, 0.0),
                pt(40.0, 0.0, 0.2),
            ],
            20.0,
        );
        assert!(stroke_outline(&[], 4.0, [0.0, 0.0], 0.1).is_empty());
        let nan = stroke_outline(&[pt(f32::NAN, 0.0, 1.0)], 4.0, [0.0, 0.0], 0.1);
        assert!(nan.is_empty());
    }

    #[test]
    fn page_json_lists_items_in_order() {
        let mut lesson = Lesson::new("Export", 1.0);
        let page = lesson.page_id_at(0).unwrap();
        let stroke = rupok_core::StrokeData {
            tool: "highlighter".into(),
            color: 0xffd60aff,
            width: 22.0,
            opacity: 0.38,
            points: vec![pt(10.0, 10.0, 0.5), pt(200.0, 30.0, 0.5)],
            offset: [5.0, 0.0],
            shape: None,
            geom: None,
        };
        lesson
            .add_items(
                &page,
                &[
                    rupok_core::NewItem {
                        data: ItemData::Text(rupok_core::TextData {
                            latex: false,
                            geom: BoxGeom {
                                x: 1.0,
                                y: 2.0,
                                w: 300.0,
                                h: 50.0,
                                rot: 0.0,
                            },
                            text: "বাংলা লেখা".into(),
                            size: 40.0,
                            color: 0x1c2024ff,
                        }),
                        created: 2.0,
                        group: None,
                        z: None,
                    },
                    rupok_core::NewItem {
                        data: ItemData::Stroke(stroke),
                        created: 3.0,
                        group: None,
                        z: None,
                    },
                ],
            )
            .unwrap();
        let quick = page_export_json(&lesson, &Package::default(), 0, false);
        let v: serde_json::Value = serde_json::from_str(&quick).unwrap();
        assert_eq!(v["items"].as_array().unwrap().len(), 1, "strokes left out");
        let json = page_export_json(&lesson, &Package::default(), 0, true);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["frame"]["w"], 1920.0);
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["kind"], "text");
        assert_eq!(items[0]["text"], "বাংলা লেখা");
        assert_eq!(items[1]["kind"], "stroke");
        assert_eq!(items[1]["tool"], "highlighter");
        let outline = items[1]["outline"][0].as_array().unwrap();
        assert!(outline.len() > 20);
        // The offset is applied: the stroke starts at x = 10 + 5 minus its radius.
        let min_x = outline
            .iter()
            .step_by(2)
            .map(|x| x.as_f64().unwrap())
            .fold(f64::MAX, f64::min);
        assert!((min_x - (15.0 - 0.5 * 22.0 * (0.35 + 0.65 * 0.5))).abs() < 0.3);
        assert_eq!(
            page_export_json(&lesson, &Package::default(), 7, true),
            "null"
        );
    }
}
