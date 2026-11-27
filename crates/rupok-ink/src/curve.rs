//! A smooth curve through pointer samples.
//!
//! A mouse, a touch screen or a smartboard that reports itself as a mouse
//! gives a sample per frame: 30 to 60 a second, 30 px or more apart under a
//! fast hand. Joined by straight lines that reads as a polygon. Here the
//! samples are joined by a curve instead: it passes through every sample,
//! leaves each along the centripetal Catmull-Rom tangent (which never loops
//! or overshoots between samples) and is exact for samples on a circle. It
//! is cut into straight pieces just fine enough to look round.
//!
//! The curve is made where the samples arrive, in the
//! [`crate::StrokeBuilder`], and the stroke is stored as the points of the
//! curve: the file format and every reader keep drawing straight pieces
//! between stored points (spec/rupok-format.md, "Stroke width and
//! rendering"), so the board, thumbnails, replay, the PDF export and the
//! eraser all see the same line with no code of their own.
//!
//! Rules that keep wet ink and dry ink the same (docs/performance.md, "The
//! wet/dry contract"):
//!
//! - **A span is final once the sample after its end has arrived.** The
//!   pieces between samples `i` and `i + 1` depend on samples `i - 1` to
//!   `i + 2` and on nothing else, so ink already drawn never moves again.
//!   Only the newest stretch, from the second-newest sample to the pen, is
//!   provisional.
//! - **Dense input passes through untouched.** The number of pieces follows
//!   how far the curve bows from the straight line, in screen pixels. A pen
//!   at 240 Hz leaves samples half a pixel apart; no span bows by
//!   [`FLATNESS_PX`], so none is cut and the output is the input.
//! - **Sharp corners stay sharp.** Where the path turns back on itself the
//!   curve does not round it ([`sharpness`]).

use std::ops::Range;

use crate::StrokePoint;

/// The straight pieces stay within this of the curve, in screen pixels.
pub const FLATNESS_PX: f32 = 0.05;
/// Most pieces one span between two samples is cut into.
pub const MAX_PIECES: usize = 32;
/// Samples closer than this (world units) are the same place.
const EPS: f32 = 1e-4;

/// The curve through `points`, as the points of its straight pieces. Every
/// input point is in the output, in order. `tol` is the flatness in the
/// units of the points ([`FLATNESS_PX`] over the zoom).
pub fn smooth(points: &[StrokePoint], tol: f32) -> Vec<StrokePoint> {
    let mut out = Vec::with_capacity(points.len() + 16);
    if let Some(p) = points.first() {
        out.push(*p);
        smooth_spans(points, 0..points.len() - 1, tol, &mut out);
    }
    out
}

/// Append the pieces of spans `spans` of the curve through `points`: for
/// span `i`, the points after `points[i]` up to and including
/// `points[i + 1]`. The caller has `points[spans.start]` already.
///
/// The curve is that of the whole of `points`: a span next to either end of
/// the slice is drawn as an end of the stroke.
pub fn smooth_spans(
    points: &[StrokePoint],
    spans: Range<usize>,
    tol: f32,
    out: &mut Vec<StrokePoint>,
) {
    let n = points.len();
    for i in spans {
        if i + 1 >= n {
            break;
        }
        span(points, i, tol, out);
    }
}

/// How much of the smooth tangent a sample keeps, 0..=1, from the cosine of
/// the turn there: all of it up to a 60° turn, none from 120° (a reversal
/// such as the bottom of a "v" stays a point), eased in between.
pub fn sharpness(cos_turn: f32) -> f32 {
    let t = (cos_turn + 0.5).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The curve's direction at the sample `at`, scaled by [`sharpness`] (so
/// its length is 0..=1): the direction of the centripetal Catmull-Rom
/// tangent, `(at - prev) · t1/t0 + (next - at) · t0/t1` with `t` the square
/// roots of the two distances. Evenly spaced samples on a circle get the
/// circle's own tangent.
fn tangent(prev: StrokePoint, at: StrokePoint, next: StrokePoint) -> (f32, f32) {
    let (ax, ay) = (at.x - prev.x, at.y - prev.y);
    let (bx, by) = (next.x - at.x, next.y - at.y);
    let (d0, d1) = (ax.hypot(ay), bx.hypot(by));
    if d0 < EPS || d1 < EPS {
        // A doubled sample has no direction of its own: the span leaves
        // along the other end's.
        return (0.0, 0.0);
    }
    let (k0, k1) = ((d1 / d0).sqrt(), (d0 / d1).sqrt());
    let (vx, vy) = (ax * k0 + bx * k1, ay * k0 + by * k1);
    let len = vx.hypot(vy);
    if len < EPS {
        return (0.0, 0.0);
    }
    let s = sharpness((ax * bx + ay * by) / (d0 * d1)) / len;
    (vx * s, vy * s)
}

fn span(points: &[StrokePoint], i: usize, tol: f32, out: &mut Vec<StrokePoint>) {
    let (a, b) = (points[i], points[i + 1]);
    let (cx, cy) = (b.x - a.x, b.y - a.y);
    let len = cx.hypot(cy);
    if len < EPS {
        out.push(b);
        return;
    }
    let (ux, uy) = (cx / len, cy / len);
    let t1 = (i > 0).then(|| tangent(points[i - 1], a, b));
    let t2 = (i + 2 < points.len()).then(|| tangent(a, b, points[i + 2]));
    // An end of the stroke has one neighbour: its direction mirrors the
    // other end's about the chord, which makes the span an even arc. When
    // the next sample continues that arc, the span barely changes as it
    // becomes final.
    let mirror = |t: (f32, f32)| {
        let along = 2.0 * (t.0 * ux + t.1 * uy);
        (along * ux - t.0, along * uy - t.1)
    };
    let (t1, t2) = match (t1, t2) {
        (Some(t1), Some(t2)) => (t1, t2),
        (Some(t1), None) => (t1, mirror(t1)),
        (None, Some(t2)) => (mirror(t2), t2),
        (None, None) => {
            out.push(b);
            return;
        }
    };
    // Bézier handles along those directions. Their length is the one that
    // makes a circular arc exact: chord × 2/3 ÷ (1 + cos of the angle
    // between direction and chord), a third of the chord when they agree.
    // A sharp sample (a short direction vector) gets a short handle.
    let handle = |t: (f32, f32)| {
        let s = t.0.hypot(t.1);
        if s < 1e-6 {
            return (0.0, 0.0);
        }
        let cos = ((t.0 * ux + t.1 * uy) / s).max(0.0);
        let k = len * (2.0 / 3.0) / (1.0 + cos);
        (t.0 * k, t.1 * k)
    };
    let (h1, h2) = (handle(t1), handle(t2));
    let (b1x, b1y) = (a.x + h1.0, a.y + h1.1);
    let (b2x, b2y) = (b.x - h2.0, b.y - h2.1);
    // How far the control points stand off the chord: a cubic bows at most
    // 3/4 of that, and n even pieces each bow 1/n² of it.
    let off = (h1.0 * uy - h1.1 * ux)
        .abs()
        .max((h2.0 * uy - h2.1 * ux).abs());
    let n = if off * 0.75 <= tol {
        1
    } else {
        ((off * 0.75 / tol.max(1e-6)).sqrt().ceil() as usize).clamp(1, MAX_PIECES)
    };
    let ddt = b.dt as f32 - a.dt as f32;
    for k in 1..n {
        let t = k as f32 / n as f32;
        let m = 1.0 - t;
        let (w0, w1, w2, w3) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
        out.push(StrokePoint {
            x: w0 * a.x + w1 * b1x + w2 * b2x + w3 * b.x,
            y: w0 * a.y + w1 * b1y + w2 * b2y + w3 * b.y,
            p: a.p + (b.p - a.p) * t,
            dt: (a.dt as f32 + ddt * t).round().max(0.0) as u32,
        });
    }
    out.push(b);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32) -> StrokePoint {
        StrokePoint {
            x,
            y,
            p: 0.5,
            dt: 0,
        }
    }

    /// Samples `chord` apart on a circle of radius `r` about (0, 0).
    fn arc(r: f32, chord: f32, turns: f32) -> Vec<StrokePoint> {
        let step = 2.0 * (chord / (2.0 * r)).asin();
        let n = (turns * std::f32::consts::TAU / step) as usize;
        (0..=n)
            .map(|i| {
                let a = i as f32 * step;
                StrokePoint {
                    x: r * a.cos(),
                    y: r * a.sin(),
                    p: 0.3 + 0.4 * (i as f32 / n as f32),
                    dt: i as u32 * 30,
                }
            })
            .collect()
    }

    /// Largest distance of the polyline (its points and the middles of its
    /// pieces) from the circle of radius `r`.
    fn off_circle(pts: &[StrokePoint], r: f32) -> f32 {
        let at = |x: f32, y: f32| (x.hypot(y) - r).abs();
        let ends = pts.iter().map(|p| at(p.x, p.y)).fold(0.0, f32::max);
        let mids = pts
            .windows(2)
            .map(|w| at((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0))
            .fold(0.0, f32::max);
        ends.max(mids)
    }

    /// Largest turn between neighbouring pieces, in degrees.
    fn max_turn(pts: &[StrokePoint]) -> f32 {
        pts.windows(3)
            .map(|w| {
                let (ax, ay) = (w[1].x - w[0].x, w[1].y - w[0].y);
                let (bx, by) = (w[2].x - w[1].x, w[2].y - w[1].y);
                (ax * by - ay * bx)
                    .atan2(ax * bx + ay * by)
                    .abs()
                    .to_degrees()
            })
            .fold(0.0, f32::max)
    }

    #[test]
    fn a_sparse_arc_comes_out_round() {
        // A fast hand at 34 Hz: samples 40 px apart on a 115 px circle.
        let (r, tol) = (115.0, FLATNESS_PX);
        let samples = arc(r, 40.0, 0.75);
        assert!(samples.len() >= 12);
        // Joined straight, as before: a polygon 1.75 px off the circle with
        // a 20° corner at every sample.
        assert!(off_circle(&samples, r) > 1.5);
        assert!(max_turn(&samples) > 19.0);

        let curve = smooth(&samples, tol);
        let off = off_circle(&curve, r);
        let turn = max_turn(&curve);
        assert!(off < 0.12, "{off} px off the true arc");
        assert!(turn < 5.0, "{turn}° between neighbouring pieces");
        // Every sample is still there, in order.
        let mut it = curve.iter();
        for s in &samples {
            assert!(it.any(|c| c == s), "sample dropped");
        }
        // The budget: no more than 8 pieces to a 40 px span.
        let per_span = (curve.len() - 1) as f32 / (samples.len() - 1) as f32;
        assert!(per_span <= 8.0, "{per_span} pieces per span");
    }

    #[test]
    fn dense_input_is_left_alone() {
        // 240 Hz handwriting: a point every 0.5 px on a wave, with jitter.
        let pts: Vec<StrokePoint> = (0..600)
            .map(|i| {
                let x = i as f32 * 0.5;
                let j = ((i * 7919) % 13) as f32 / 13.0 - 0.5;
                pt(x, (x * 0.06).sin() * 40.0 + j * 0.15)
            })
            .collect();
        assert_eq!(smooth(&pts, FLATNESS_PX), pts);
        // Up to two pixels apart nothing is added either.
        let pts: Vec<StrokePoint> = (0..200)
            .map(|i| {
                let x = i as f32 * 2.0;
                pt(x, (x * 0.03).sin() * 60.0)
            })
            .collect();
        let out = smooth(&pts, FLATNESS_PX);
        assert!(out.len() <= pts.len() + pts.len() / 4, "{}", out.len());
    }

    #[test]
    fn straight_runs_and_short_strokes_add_nothing() {
        let line: Vec<StrokePoint> = (0..10).map(|i| pt(i as f32 * 50.0, 7.0)).collect();
        assert_eq!(smooth(&line, FLATNESS_PX), line);
        assert_eq!(smooth(&[], FLATNESS_PX), vec![]);
        assert_eq!(smooth(&line[..1], FLATNESS_PX), line[..1].to_vec());
        assert_eq!(smooth(&line[..2], FLATNESS_PX), line[..2].to_vec());
        // Doubled samples do not divide by zero.
        let doubled = [
            pt(0.0, 0.0),
            pt(0.0, 0.0),
            pt(30.0, 10.0),
            pt(30.0, 10.0),
            pt(60.0, 0.0),
        ];
        let out = smooth(&doubled, FLATNESS_PX);
        assert!(out.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }

    #[test]
    fn ink_already_drawn_does_not_move_as_the_stroke_grows() {
        // A wobbly sparse path. Up to sample k - 1 the curve through the
        // first k + 1 samples is the curve through all of them.
        let samples: Vec<StrokePoint> = (0..40)
            .map(|i| {
                let a = i as f32 * 0.45;
                pt(i as f32 * 28.0 + a.sin() * 20.0, (a * 1.7).cos() * 45.0)
            })
            .collect();
        let whole = smooth(&samples, FLATNESS_PX);
        for k in 2..samples.len() - 1 {
            let part = smooth(&samples[..=k], FLATNESS_PX);
            let settled = part
                .iter()
                .position(|p| *p == samples[k - 1])
                .expect("sample in the curve");
            assert_eq!(part[..=settled], whole[..=settled], "after sample {k}");
        }
    }

    #[test]
    fn the_newest_span_settles_gently_on_an_even_arc() {
        // While the pen keeps to its arc, the span that was newest moves by
        // a fraction of a pixel when the next sample makes it final.
        let r = 115.0;
        let samples = arc(r, 40.0, 0.5);
        for k in 3..samples.len() {
            let before = smooth(&samples[..k], FLATNESS_PX);
            assert!(off_circle(&before, r) < 0.25, "provisional span, {k}");
        }
    }

    #[test]
    fn a_reversal_stays_a_point() {
        // The bottom of a fast "v": three samples, a 140° turn.
        let v = [pt(0.0, 0.0), pt(20.0, 60.0), pt(40.0, 0.0)];
        assert_eq!(smooth(&v, FLATNESS_PX), v.to_vec());
        assert_eq!(sharpness(1.0), 1.0);
        assert_eq!(sharpness(0.5), 1.0, "a 60° turn is still smooth");
        assert_eq!(sharpness(-0.5), 0.0, "a 120° turn is a corner");
        // A square corner at sparse samples is eased, not swung wide: the
        // curve stays within 4 px of the two straight sides.
        let l = [
            pt(0.0, 0.0),
            pt(40.0, 0.0),
            pt(80.0, 0.0),
            pt(80.0, 40.0),
            pt(80.0, 80.0),
        ];
        let out = smooth(&l, FLATNESS_PX);
        let off = out
            .iter()
            .map(|p| p.y.max(0.0).min((80.0 - p.x).max(0.0)))
            .fold(0.0, f32::max);
        assert!(off < 4.0, "{off}");
    }

    #[test]
    fn pressure_and_time_run_evenly_along_a_span() {
        let samples = arc(115.0, 40.0, 0.5);
        let out = smooth(&samples, FLATNESS_PX);
        assert!(out.windows(2).all(|w| w[1].dt >= w[0].dt));
        assert!(out.windows(2).all(|w| w[1].p >= w[0].p - 1e-6));
        assert_eq!(out.last(), samples.last());
    }

    #[test]
    fn the_direction_is_the_centripetal_catmull_rom_tangent() {
        // Textbook: m1 = (P1-P0)/(t1-t0) - (P2-P0)/(t2-t0) + (P2-P1)/(t2-t1),
        // with knots a square root of the distance apart.
        let (p0, p1, p2) = (pt(0.0, 0.0), pt(9.0, 3.0), pt(40.0, 10.0));
        let (t01, t12) = ((9.0f32.hypot(3.0)).sqrt(), (31.0f32.hypot(7.0)).sqrt());
        let m = |a: f32, b: f32, c: f32| (b - a) / t01 - (c - a) / (t01 + t12) + (c - b) / t12;
        let (mx, my) = (m(p0.x, p1.x, p2.x), m(p0.y, p1.y, p2.y));
        let t = tangent(p0, p1, p2);
        // A gentle turn: full length, same direction.
        assert!((t.0.hypot(t.1) - 1.0).abs() < 1e-5, "{t:?}");
        assert!((t.0 * my - t.1 * mx).abs() < 1e-4 * mx.hypot(my), "{t:?}");
        assert!(t.0 * mx + t.1 * my > 0.0);
    }

    #[test]
    fn a_circle_from_very_few_samples_is_still_a_circle() {
        // Six samples to the turn (60° each), 40 px apart: plain
        // Catmull-Rom handles would flatten the sides by a pixel.
        let r = 40.0;
        let samples = arc(r, 40.0, 1.0);
        let curve = smooth(&samples, FLATNESS_PX);
        // The first and last spans only guess where the stroke came from.
        let from = curve.iter().position(|p| *p == samples[1]).unwrap();
        let to = curve
            .iter()
            .position(|p| *p == samples[samples.len() - 2])
            .unwrap();
        let off = off_circle(&curve[from..=to], r);
        assert!(off < 0.1, "{off} px off a 40 px circle");
    }
}
