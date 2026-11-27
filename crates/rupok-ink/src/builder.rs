//! Live stroke construction ("wet ink").

use crate::curve::{FLATNESS_PX, smooth_spans};
use crate::filter::{OneEuro2, OneEuroParams};
use crate::pressure::PressureCurve;
use crate::simplify::simplify;
use crate::{Brush, InputPoint, StrokePoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Pen,
    Highlighter,
}

impl Tool {
    pub fn as_str(self) -> &'static str {
        match self {
            Tool::Pen => "pen",
            Tool::Highlighter => "highlighter",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pen" => Some(Tool::Pen),
            "highlighter" => Some(Tool::Highlighter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub tool: Tool,
    /// 0xRRGGBBAA
    pub color: u32,
    /// Nominal full width in world units.
    pub width: f32,
    pub opacity: f32,
    /// Pen style (`brush` key; [`Brush::Pen`] is not stored).
    pub brush: Brush,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            tool: Tool::Pen,
            color: 0x1d2433ff,
            width: 3.0,
            opacity: 1.0,
            brush: Brush::Pen,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuilderConfig {
    pub curve: PressureCurve,
    pub filter: OneEuroParams,
    /// Current camera zoom; tolerances are expressed in screen pixels.
    pub zoom: f32,
    /// Join the samples with a smooth curve ([`crate::curve`]). Off joins
    /// them with straight lines, as before v0.2.5 (diagnostics and tests).
    pub smooth: bool,
}

impl Default for BuilderConfig {
    fn default() -> Self {
        Self {
            curve: PressureCurve::default(),
            filter: OneEuroParams::default(),
            zoom: 1.0,
            smooth: true,
        }
    }
}

/// Accumulates filtered points for one stroke while the pen is down.
#[derive(Clone, Debug)]
pub struct StrokeBuilder {
    pub style: StrokeStyle,
    cfg: BuilderConfig,
    filter: OneEuro2,
    points: Vec<StrokePoint>,
    /// The curve through `points[..=settled]`: the part of the ink that is
    /// final (`crate::curve`). Ends at `points[settled]`.
    curve: Vec<StrokePoint>,
    settled: usize,
    t0: f64,
    has_pressure: bool,
    last_raw: Option<InputPoint>,
}

impl StrokeBuilder {
    pub fn begin(
        style: StrokeStyle,
        cfg: BuilderConfig,
        first: InputPoint,
        has_pressure: bool,
    ) -> Self {
        let mut b = Self {
            style,
            cfg,
            filter: OneEuro2::new(cfg.filter),
            points: Vec::with_capacity(256),
            curve: Vec::with_capacity(256),
            settled: 0,
            t0: first.t_ms,
            has_pressure: has_pressure && style.tool == Tool::Pen,
            last_raw: None,
        };
        b.push(first);
        b
    }

    /// Wall-clock start of the stroke in ms (same clock as the input samples).
    pub fn t0(&self) -> f64 {
        self.t0
    }

    fn pressure(&self, raw: f32) -> f32 {
        if self.style.tool == Tool::Highlighter {
            return 1.0;
        }
        self.cfg.curve.normalise(raw, self.has_pressure)
    }

    /// Feed one raw sample. Returns true if a point was appended.
    pub fn push(&mut self, s: InputPoint) -> bool {
        self.last_raw = Some(s);
        let (x, y) = if self.cfg.filter.is_raw() {
            (s.x, s.y)
        } else {
            self.filter.filter(s.x, s.y, s.t_ms)
        };
        let p = self.pressure(s.pressure);
        let dt = (s.t_ms - self.t0).max(0.0).min(f64::from(u32::MAX)) as u32;
        if let Some(last) = self.points.last_mut() {
            let min_d = 0.35 / self.cfg.zoom.max(1e-3);
            let d2 = (x - last.x).powi(2) + (y - last.y).powi(2);
            if d2 < min_d * min_d {
                // Too close to be worth a new point; keep the max pressure so
                // pressing harder in place still widens the line.
                last.p = last.p.max(p);
                // A stroke of one point: that point also starts the curve.
                if let ([only], [first]) = (&self.points[..], &mut self.curve[..]) {
                    first.p = only.p;
                }
                return false;
            }
        }
        self.points.push(StrokePoint { x, y, p, dt });
        self.settle();
        true
    }

    /// Flatness of the curve in world units.
    fn flatness(&self) -> f32 {
        FLATNESS_PX / self.cfg.zoom.max(1e-3)
    }

    /// Add to the final curve the spans the newest sample completed: the
    /// span from sample `i` is final once sample `i + 2` exists.
    fn settle(&mut self) {
        if self.curve.is_empty() {
            self.curve.push(self.points[0]);
        }
        let tol = self.flatness();
        while self.settled + 2 < self.points.len() {
            let i = self.settled;
            smooth_spans(&self.points, i..i + 1, tol, &mut self.curve);
            self.settled += 1;
        }
    }

    /// The curve from `points[settled]` to the end of `points` and on
    /// through `ahead`: the provisional end of the ink, appended to `out`
    /// (which holds the final curve).
    fn unsettled(&self, ahead: &[StrokePoint], out: &mut Vec<StrokePoint>) {
        // One sample of context before the first unsettled span.
        let from = self.settled.saturating_sub(1);
        if ahead.is_empty() {
            smooth_spans(
                &self.points[from..],
                self.settled - from..self.points.len() - from,
                self.flatness(),
                out,
            );
            return;
        }
        let mut tail = Vec::with_capacity(self.points.len() - from + ahead.len());
        tail.extend_from_slice(&self.points[from..]);
        tail.extend_from_slice(ahead);
        smooth_spans(&tail, self.settled - from..tail.len(), self.flatness(), out);
    }

    /// The samples so far (filtered, not yet simplified), without the
    /// [tip]. Smart Ink reads these; the ink on screen is [`wet`].
    ///
    /// [tip]: StrokeBuilder::tip
    /// [`wet`]: StrokeBuilder::wet
    pub fn points(&self) -> &[StrokePoint] {
        &self.points
    }

    /// The wet ink: the samples, the [tip] and `ahead` (the browser's
    /// predicted points), joined by a smooth curve, into `out`. Everything
    /// up to the second-newest sample is final and is the same in every
    /// later call and in [`finish`]; the stretch from there to the pen is
    /// redrawn as samples arrive (docs/performance.md, "The wet/dry
    /// contract").
    ///
    /// [tip]: StrokeBuilder::tip
    /// [`finish`]: StrokeBuilder::finish
    pub fn wet(&self, ahead: &[StrokePoint], out: &mut Vec<StrokePoint>) {
        out.clear();
        let tip = self.tip();
        if !self.cfg.smooth {
            out.extend_from_slice(&self.points);
            out.extend(tip);
            out.extend_from_slice(ahead);
            return;
        }
        // One allocation: the final curve and room for the provisional end.
        out.reserve(self.curve.len() + 2 * crate::curve::MAX_PIECES + 4 * ahead.len() + 8);
        out.extend_from_slice(&self.curve);
        match tip {
            None => self.unsettled(ahead, out),
            Some(t) => {
                let mut rest = Vec::with_capacity(1 + ahead.len());
                rest.push(t);
                rest.extend_from_slice(ahead);
                self.unsettled(&rest, out);
            }
        }
    }

    /// The pen's newest raw position, when the smoothed points trail it.
    ///
    /// Wet ink is `points()` + `tip()`: the smoothed body catches up behind
    /// the tip, but the ink always reaches the pen, at any smoothing.
    /// [`StrokeBuilder::finish`] ends the dry stroke at this same point, so
    /// pen-up changes nothing on screen (docs/performance.md).
    pub fn tip(&self) -> Option<StrokePoint> {
        let raw = self.last_raw?;
        let last = *self.points.last()?;
        if (raw.x - last.x).abs() <= 0.01 && (raw.y - last.y).abs() <= 0.01 {
            return None;
        }
        let dt = (raw.t_ms - self.t0).max(0.0).min(f64::from(u32::MAX)) as u32;
        Some(StrokePoint {
            x: raw.x,
            y: raw.y,
            p: last.p,
            dt: dt.max(last.dt),
        })
    }

    /// Finish the stroke: end it at the [tip] (the real pen-up position, which
    /// the last wet frame already showed) and simplify. The points are those
    /// of the curve the wet ink showed ([`wet`] with nothing ahead), so the
    /// stored stroke is the line that was on screen. Tolerances are ~0.2
    /// screen px, below what the eye can tell from the wet stroke. Brushes
    /// patterned along the stroke (dashes, the rainbow) keep every point, so
    /// the pattern does not shift at pen-up ([`Brush::keeps_every_point`]).
    ///
    /// [tip]: StrokeBuilder::tip
    /// [`wet`]: StrokeBuilder::wet
    pub fn finish(self) -> Vec<StrokePoint> {
        let mut pts = Vec::new();
        self.wet(&[], &mut pts);
        if self.style.brush.keeps_every_point() {
            return pts;
        }
        let eps = 0.2 / self.cfg.zoom.max(1e-3);
        simplify(&pts, eps, 0.06)
    }

    /// The stroke as sampled: [`points`] and the [tip] joined by straight
    /// lines, simplified like [`finish`]. What the Smart Ink recognisers
    /// read at pen-up: they look for corners and straight sides in what the
    /// hand did, not in the curve drawn through it.
    ///
    /// [`points`]: StrokeBuilder::points
    /// [tip]: StrokeBuilder::tip
    /// [`finish`]: StrokeBuilder::finish
    pub fn sampled(&self) -> Vec<StrokePoint> {
        let mut pts = self.points.clone();
        pts.extend(self.tip());
        if self.style.brush.keeps_every_point() {
            return pts;
        }
        let eps = 0.2 / self.cfg.zoom.max(1e-3);
        simplify(&pts, eps, 0.06)
    }

    /// Radius of a point under this builder's style and curve.
    pub fn radius(&self, p: f32) -> f32 {
        self.cfg.curve.radius(p, self.style.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_simplifies_a_line() {
        let first = InputPoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
            t_ms: 1000.0,
        };
        let mut b = StrokeBuilder::begin(
            StrokeStyle::default(),
            BuilderConfig::default(),
            first,
            true,
        );
        for i in 1..=200 {
            b.push(InputPoint {
                x: i as f32,
                y: 0.0,
                pressure: 0.5,
                t_ms: 1000.0 + i as f64 * 4.0,
            });
        }
        assert!(b.points().len() > 100);
        let pts = b.finish();
        assert!(pts.len() < 10, "{} points", pts.len());
        let last = pts.last().unwrap();
        assert_eq!(last.x, 200.0, "tail must snap to pen-up position");
        assert_eq!(last.dt, 800);
    }

    #[test]
    fn wet_tip_reaches_the_pen_and_dry_ends_there() {
        let cfg = BuilderConfig {
            filter: OneEuroParams::from_smoothing(1.0),
            ..Default::default()
        };
        let at = |i: i32| InputPoint {
            x: i as f32 * 3.0,
            y: 0.0,
            pressure: 0.5,
            t_ms: i as f64 * 4.0,
        };
        let mut b = StrokeBuilder::begin(StrokeStyle::default(), cfg, at(0), true);
        for i in 1..=60 {
            b.push(at(i));
        }
        // Heavy smoothing trails the pen; the tip does not.
        assert!(b.points().last().unwrap().x < 175.0);
        let tip = b.tip().expect("tip");
        assert_eq!(tip.x, 180.0);
        let dry = b.clone().finish();
        assert_eq!(dry.last().unwrap().x, tip.x);
        assert_eq!(dry.last().unwrap().p, tip.p);
    }

    #[test]
    fn smoothing_zero_keeps_every_sample() {
        let cfg = BuilderConfig {
            filter: OneEuroParams::from_smoothing(0.0),
            ..Default::default()
        };
        let at = |i: i32| InputPoint {
            x: (i * 5) as f32,
            y: ((i * 37) % 11) as f32,
            pressure: 0.5,
            t_ms: i as f64 * 4.0,
        };
        let mut b = StrokeBuilder::begin(StrokeStyle::default(), cfg, at(0), true);
        for i in 1..20 {
            b.push(at(i));
        }
        for (i, p) in b.points().iter().enumerate() {
            let s = at(i as i32);
            assert_eq!((p.x, p.y), (s.x, s.y));
        }
        assert!(b.tip().is_none(), "raw points already reach the pen");
    }

    #[test]
    fn patterned_brushes_keep_every_wet_point() {
        let at = |i: i32| InputPoint {
            x: i as f32,
            y: 0.0,
            pressure: 0.5,
            t_ms: i as f64 * 4.0,
        };
        for (brush, simplified) in [
            (Brush::Pen, true),
            (Brush::Chalk, true),
            (Brush::Dashed, false),
            (Brush::Rainbow, false),
        ] {
            let style = StrokeStyle {
                brush,
                ..Default::default()
            };
            let mut b = StrokeBuilder::begin(style, BuilderConfig::default(), at(0), true);
            for i in 1..=100 {
                b.push(at(i));
            }
            let mut wet = b.points().to_vec();
            wet.extend(b.tip());
            let dry = b.finish();
            assert_eq!(dry.len() < wet.len(), simplified, "{brush:?}");
            if !simplified {
                assert_eq!(dry, wet, "{brush:?}: dry is exactly the last wet frame");
            }
        }
    }

    /// A fast hand on a mouse: a sample every 30 ms, 40 px apart, on a
    /// circle of radius 115 about (300, 300).
    fn sparse_arc(i: i32) -> InputPoint {
        let a = i as f32 * 2.0 * (20.0f32 / 115.0).asin();
        InputPoint {
            x: 300.0 + 115.0 * a.cos(),
            y: 300.0 + 115.0 * a.sin(),
            pressure: 0.5,
            t_ms: f64::from(i) * 30.0,
        }
    }

    fn off_circle(pts: &[StrokePoint]) -> f32 {
        let at = |x: f32, y: f32| ((x - 300.0).hypot(y - 300.0) - 115.0).abs();
        pts.windows(2)
            .map(|w| at(w[1].x, w[1].y).max(at((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0)))
            .fold(0.0, f32::max)
    }

    #[test]
    fn sparse_samples_come_out_as_a_curve_wet_and_dry() {
        for smoothing in [0.0, 0.25, 0.5] {
            let cfg = BuilderConfig {
                filter: OneEuroParams::from_smoothing(smoothing),
                ..Default::default()
            };
            let mut b = StrokeBuilder::begin(StrokeStyle::default(), cfg, sparse_arc(0), false);
            let mut wet = Vec::new();
            let mut last: Vec<StrokePoint> = Vec::new();
            for i in 1..=14 {
                b.push(sparse_arc(i));
                b.wet(&[], &mut wet);
                // The ink reaches the pen at every smoothing: no lag.
                let end = wet.last().unwrap();
                let pen = sparse_arc(i);
                assert!(
                    (end.x - pen.x).hypot(end.y - pen.y) < 0.02,
                    "smoothing {smoothing}: the ink ends at the pen"
                );
                // Ink behind the second-newest sample never moves again.
                let settled = b.points().len().saturating_sub(2);
                let upto = wet
                    .iter()
                    .position(|p| *p == b.points()[settled])
                    .expect("settled sample");
                let same = last.iter().zip(&wet).take_while(|(a, b)| a == b).count();
                if i > 3 {
                    let was = last
                        .iter()
                        .position(|p| *p == b.points()[settled - 1])
                        .expect("sample");
                    assert!(
                        same > was,
                        "smoothing {smoothing}, sample {i}: drawn ink moved"
                    );
                }
                assert!(upto < wet.len());
                last.clone_from(&wet);
            }
            if smoothing == 0.0 {
                // Raw samples on the circle: the wet ink is round.
                assert!(off_circle(&wet) < 0.12, "{}", off_circle(&wet));
                assert!(wet.len() > 5 * b.points().len());
            }
            // Dry: the same curve, simplified by 0.2 px.
            let dry = b.clone().finish();
            assert_eq!(dry.first(), wet.first());
            assert_eq!(dry.last(), wet.last());
            let far = wet
                .iter()
                .map(|p| {
                    dry.windows(2)
                        .map(|w| seg_dist(p, &w[0], &w[1]))
                        .fold(f32::MAX, f32::min)
                })
                .fold(0.0, f32::max);
            assert!(
                far <= 0.2 + 1e-3,
                "smoothing {smoothing}: dry is {far} px off wet"
            );
            if smoothing == 0.0 {
                assert!(off_circle(&dry) < 0.3, "{}", off_circle(&dry));
                // Stored no more densely than a 240 Hz pen stores the same arc.
                assert!(dry.len() < 4 * b.points().len(), "{} points", dry.len());
            }
            // Smart Ink still reads the samples.
            assert!(b.sampled().len() <= b.points().len() + 1);
        }
    }

    fn seg_dist(p: &StrokePoint, a: &StrokePoint, b: &StrokePoint) -> f32 {
        let (vx, vy) = (b.x - a.x, b.y - a.y);
        let l2 = vx * vx + vy * vy;
        let t = if l2 > 0.0 {
            (((p.x - a.x) * vx + (p.y - a.y) * vy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (a.x + t * vx - p.x).hypot(a.y + t * vy - p.y)
    }

    #[test]
    fn patterned_brushes_store_exactly_the_last_wet_curve() {
        for brush in [Brush::Dashed, Brush::Dotted, Brush::Rainbow, Brush::Magic] {
            let style = StrokeStyle {
                brush,
                ..Default::default()
            };
            let mut b = StrokeBuilder::begin(style, BuilderConfig::default(), sparse_arc(0), false);
            for i in 1..=12 {
                b.push(sparse_arc(i));
            }
            let mut wet = Vec::new();
            b.wet(&[], &mut wet);
            assert_eq!(
                b.finish(),
                wet,
                "{brush:?}: the pattern would shift at pen-up"
            );
        }
    }

    #[test]
    fn dense_pen_input_is_stored_as_before() {
        // 240 Hz handwriting, half a pixel between samples: the curve adds
        // nothing, so the stored stroke is what v0.2.4 stored.
        let at = |i: i32| {
            let x = 20.0 + i as f32 * 0.5;
            InputPoint {
                x,
                y: 80.0 + (x * 0.06).sin() * 40.0,
                pressure: 0.5 + 0.3 * (i as f32 * 0.01).sin(),
                t_ms: f64::from(i) * 4.0,
            }
        };
        for smoothing in [0.0, 0.25] {
            let cfg = BuilderConfig {
                filter: OneEuroParams::from_smoothing(smoothing),
                ..Default::default()
            };
            let mut b = StrokeBuilder::begin(StrokeStyle::default(), cfg, at(0), true);
            let mut straight = StrokeBuilder::begin(
                StrokeStyle::default(),
                BuilderConfig {
                    smooth: false,
                    ..cfg
                },
                at(0),
                true,
            );
            for i in 1..520 {
                b.push(at(i));
                straight.push(at(i));
            }
            let mut wet = Vec::new();
            b.wet(&[], &mut wet);
            let mut raw = b.points().to_vec();
            raw.extend(b.tip());
            let first = wet.iter().zip(&raw).position(|(a, b)| a != b);
            assert_eq!(
                (wet.len(), first),
                (raw.len(), None),
                "smoothing {smoothing}: nothing added"
            );
            assert_eq!(b.sampled(), straight.clone().finish());
            assert_eq!(b.finish(), straight.finish(), "smoothing {smoothing}");
        }
    }

    #[test]
    fn predicted_points_extend_the_curve_and_are_not_stored() {
        let mut b = StrokeBuilder::begin(
            StrokeStyle::default(),
            BuilderConfig::default(),
            sparse_arc(0),
            false,
        );
        for i in 1..=6 {
            b.push(sparse_arc(i));
        }
        let ahead = sparse_arc(7);
        let ahead = [StrokePoint {
            x: ahead.x,
            y: ahead.y,
            p: 0.5,
            dt: 0,
        }];
        let (mut plain, mut wet) = (Vec::new(), Vec::new());
        b.wet(&[], &mut plain);
        b.wet(&ahead, &mut wet);
        assert_eq!(wet.last(), ahead.last());
        assert!(wet.len() > plain.len());
        // Up to the second-newest sample both are the same ink.
        let settled = b.points()[b.points().len() - 2];
        let k = plain.iter().position(|p| *p == settled).unwrap();
        assert_eq!(plain[..=k], wet[..=k]);
        assert!(b.finish().iter().all(|p| *p != ahead[0]));
    }

    #[test]
    fn highlighter_ignores_pressure() {
        let style = StrokeStyle {
            tool: Tool::Highlighter,
            ..Default::default()
        };
        let first = InputPoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.1,
            t_ms: 0.0,
        };
        let b = StrokeBuilder::begin(style, BuilderConfig::default(), first, true);
        assert_eq!(b.points()[0].p, 1.0);
    }

    #[test]
    fn jitter_in_place_does_not_add_points() {
        let first = InputPoint {
            x: 10.0,
            y: 10.0,
            pressure: 0.3,
            t_ms: 0.0,
        };
        let mut b = StrokeBuilder::begin(
            StrokeStyle::default(),
            BuilderConfig::default(),
            first,
            true,
        );
        for i in 0..50 {
            b.push(InputPoint {
                x: 10.05,
                y: 10.0,
                pressure: 0.9,
                t_ms: i as f64,
            });
        }
        assert_eq!(b.points().len(), 1);
        assert!(b.points()[0].p > 0.5);
    }
}
