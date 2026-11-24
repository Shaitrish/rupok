//! Pen timing: turn positioned strokes into timed [`StrokePoint`]s that
//! replay like real writing. The pen slows down in curves and corners,
//! accelerates out of each pen-down and eases into each lift; it pauses
//! between strokes, longer between glyphs, words and lines, and travels in
//! the air at a pace set by the distance.

use super::layout::Pause;
use crate::StrokePoint;
use crate::math::geom::Pt;

/// Writing speed at `speed = 1`, in x-heights per second of pen travel.
const XH_PER_S: f32 = 30.0;
/// Time between emitted samples, ms (a pen digitiser reports ~120–240 Hz).
const SAMPLE_MS: f32 = 7.0;
/// Most points one stroke may have.
const MAX_POINTS: usize = 600;

/// A stroke ready to store: points in block coordinates, `dt` from the
/// stroke's first point, and when the stroke starts in the writing.
#[derive(Clone, Debug, PartialEq)]
pub struct PenStroke {
    pub points: Vec<StrokePoint>,
    /// Milliseconds from the start of the writing to this stroke's first point.
    pub start_ms: f64,
    /// Index of the input line (solution step) the stroke belongs to.
    pub line: usize,
}

impl PenStroke {
    /// Milliseconds the stroke takes to draw.
    pub fn duration_ms(&self) -> f64 {
        f64::from(self.points.last().map_or(0, |p| p.dt))
    }
    pub fn end_ms(&self) -> f64 {
        self.start_ms + self.duration_ms()
    }
}

fn pause_ms(p: Pause) -> f32 {
    match p {
        Pause::Stroke => 32.0,
        Pause::Glyph => 46.0,
        Pause::Word => 140.0,
        Pause::Line => 320.0,
    }
}

/// Resample a polyline every `step` along its length (ends kept).
fn resample(pts: &[Pt], step: f32) -> Vec<Pt> {
    let mut out = Vec::with_capacity(pts.len() * 2);
    let Some(&first) = pts.first() else {
        return out;
    };
    out.push(first);
    let mut carry = 0.0f32;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        if len <= 0.0 {
            continue;
        }
        let mut t = step - carry;
        while t < len {
            out.push(a.lerp(b, t / len));
            t += step;
        }
        carry = len - (t - step);
    }
    if let Some(&last) = pts.last()
        && out.last().is_some_and(|l| l.dist(last) > step * 0.25)
    {
        out.push(last);
    }
    out
}

/// Time one stroke. Returns points with `dt` from the stroke start.
fn time_stroke(pts: &[Pt], xh_px: f32, speed: f32) -> Vec<StrokePoint> {
    let vmax = XH_PER_S * xh_px * speed / 1000.0; // px per ms
    let len: f32 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    // A dot (or a stroke with one point): a short tap.
    if len < xh_px * 0.08 || pts.len() < 2 {
        let p = pts.first().copied().unwrap_or_default();
        let q = pts.last().copied().unwrap_or(p);
        let d = (45.0 / speed).max(8.0) as u32;
        return vec![
            StrokePoint {
                x: p.x,
                y: p.y,
                p: 0.45,
                dt: 0,
            },
            StrokePoint {
                x: (p.x + q.x) * 0.5 + 0.2,
                y: (p.y + q.y) * 0.5 + 0.2,
                p: 0.6,
                dt: d / 2,
            },
            StrokePoint {
                x: q.x + 0.4,
                y: q.y + 0.4,
                p: 0.5,
                dt: d,
            },
        ];
    }
    let step = (len / (MAX_POINTS as f32 * 3.0)).max((xh_px / 28.0).max(0.6));
    let r = resample(pts, step);
    let n = r.len();
    let ease = (xh_px * 0.55).max(1.0);
    let mut s_acc = 0.0f32;
    let mut t = 0.0f32;
    let mut last_emit = f32::NEG_INFINITY;
    let mut out: Vec<StrokePoint> = Vec::with_capacity(n / 2 + 2);
    let mut p_smooth = f32::NAN;
    for i in 0..n {
        let mut down = 0.0f32;
        let mut rel_v = 0.6f32;
        if i > 0 {
            let ds = r[i - 1].dist(r[i]);
            s_acc += ds;
            // Turning over a short window: slower in curves and corners.
            let a = r[i.saturating_sub(2)];
            let b = r[i];
            let c = r[(i + 2).min(n - 1)];
            let (ux, uy) = (b.x - a.x, b.y - a.y);
            let (vx, vy) = (c.x - b.x, c.y - b.y);
            let turn = if ux.hypot(uy) > 0.0 && vx.hypot(vy) > 0.0 {
                (ux * vy - uy * vx).atan2(ux * vx + uy * vy).abs()
            } else {
                0.0
            };
            let curve = 0.35 + 0.65 / (1.0 + 2.2 * turn);
            let start = (0.3 + 0.7 * (s_acc / ease)).min(1.0);
            let end = (0.35 + 0.65 * ((len - s_acc) / ease)).min(1.0);
            let k = curve * start.min(end).max(0.2);
            t += ds / (vmax * k).max(1e-4);
            rel_v = k;
            if ds > 0.0 {
                down = (r[i].y - r[i - 1].y) / ds; // y grows downwards
            }
        }
        // Pressure: heavier on downstrokes and where the pen slows, lighter
        // on fast upstrokes, tapering at both ends; smoothed so the width
        // swells and thins gradually.
        let edge = (s_acc / ease).min((len - s_acc) / ease).clamp(0.0, 1.0);
        let raw = 0.34 + 0.24 * edge.sqrt() + 0.12 * down - 0.12 * (rel_v - 0.6);
        p_smooth = if p_smooth.is_nan() {
            raw
        } else {
            p_smooth + 0.3 * (raw - p_smooth)
        };
        let last = i + 1 == n;
        if i == 0 || last || t - last_emit >= SAMPLE_MS {
            let p = if i == 0 || last {
                p_smooth.min(0.4)
            } else {
                p_smooth
            };
            out.push(StrokePoint {
                x: r[i].x,
                y: r[i].y,
                p: p.clamp(0.15, 0.95),
                dt: t.round() as u32,
            });
            last_emit = t;
        }
    }
    // Keep dt strictly increasing.
    for i in 1..out.len() {
        if out[i].dt <= out[i - 1].dt {
            out[i].dt = out[i - 1].dt + 1;
        }
    }
    out
}

/// Time positioned strokes (block pixels) in order.
pub fn time_all(strokes: &[(Vec<Pt>, Pause, usize)], xh_px: f32, speed: f32) -> Vec<PenStroke> {
    let speed = speed.clamp(0.1, 10.0);
    let xh_px = xh_px.max(1.0);
    let air = XH_PER_S * xh_px * speed / 1000.0 * 3.0;
    let mut out = Vec::with_capacity(strokes.len());
    let mut clock = 0.0f64;
    let mut prev_end: Option<Pt> = None;
    for (pts, pause, line) in strokes {
        if pts.is_empty() {
            continue;
        }
        if let Some(pe) = prev_end {
            let travel = pe.dist(pts[0]) / air;
            clock += f64::from(pause_ms(*pause) / speed + travel.min(900.0));
        }
        let points = time_stroke(pts, xh_px, speed);
        let st = PenStroke {
            points,
            start_ms: clock.round(),
            line: *line,
        };
        clock = st.end_ms();
        prev_end = pts.last().copied();
        out.push(st);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_are_slower_than_lines() {
        let line: Vec<Pt> = (0..=20).map(|i| Pt::new(i as f32 * 3.0, 0.0)).collect();
        let len = 60.0;
        let circle: Vec<Pt> = (0..=40)
            .map(|i| {
                let a = i as f32 / 40.0 * std::f32::consts::TAU;
                Pt::new(
                    a.cos() * len / std::f32::consts::TAU,
                    a.sin() * len / std::f32::consts::TAU,
                )
            })
            .collect();
        let tl = time_stroke(&line, 30.0, 1.0).last().unwrap().dt;
        let tc = time_stroke(&circle, 30.0, 1.0).last().unwrap().dt;
        assert!(tc > tl, "circle {tc} ms vs line {tl} ms");
        let fast = time_stroke(&line, 30.0, 2.0).last().unwrap().dt;
        assert!(fast < tl);
    }

    #[test]
    fn pressure_varies_downstrokes_heavier() {
        // A zigzag: down, up, down at the same pace.
        let mut pts = Vec::new();
        for k in 0..4 {
            for i in 0..=20 {
                let y = if k % 2 == 0 {
                    i as f32
                } else {
                    20.0 - i as f32
                };
                pts.push(Pt::new(k as f32 * 6.0 + i as f32 * 0.3, y * 2.0));
            }
        }
        let s = time_stroke(&pts, 30.0, 1.0);
        let ps: Vec<f32> = s.iter().map(|p| p.p).collect();
        let (lo, hi) = ps
            .iter()
            .fold((1.0f32, 0.0f32), |(a, b), p| (a.min(*p), b.max(*p)));
        assert!(hi - lo > 0.15, "pressure should vary: {lo}..{hi}");
        assert!(ps.iter().all(|p| (0.0..=1.0).contains(p)));
        // Ends are lighter than the middle of a downstroke.
        assert!(ps[0] < hi && ps[ps.len() - 1] < hi);
    }

    #[test]
    fn dots_are_taps() {
        let d = time_stroke(&[Pt::new(5.0, 5.0)], 30.0, 1.0);
        assert_eq!(d.len(), 3);
        assert!(d[2].dt > 0);
    }
}
