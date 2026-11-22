//! Live stroke construction ("wet ink").

use crate::filter::{OneEuro2, OneEuroParams};
use crate::pressure::PressureCurve;
use crate::simplify::simplify;
use crate::{InputPoint, StrokePoint};

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
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            tool: Tool::Pen,
            color: 0x1d2433ff,
            width: 3.0,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BuilderConfig {
    pub curve: PressureCurve,
    pub filter: OneEuroParams,
    /// Current camera zoom; tolerances are expressed in screen pixels.
    pub zoom: f32,
}

impl Default for BuilderConfig {
    fn default() -> Self {
        Self {
            curve: PressureCurve::default(),
            filter: OneEuroParams::default(),
            zoom: 1.0,
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
        let (x, y) = self.filter.filter(s.x, s.y, s.t_ms);
        let p = self.pressure(s.pressure);
        let dt = (s.t_ms - self.t0).max(0.0).min(f64::from(u32::MAX)) as u32;
        if let Some(last) = self.points.last_mut() {
            let min_d = 0.35 / self.cfg.zoom.max(1e-3);
            let d2 = (x - last.x).powi(2) + (y - last.y).powi(2);
            if d2 < min_d * min_d {
                // Too close to be worth a new point; keep the max pressure so
                // pressing harder in place still widens the line.
                last.p = last.p.max(p);
                return false;
            }
        }
        self.points.push(StrokePoint { x, y, p, dt });
        true
    }

    /// Current wet points (filtered, not yet simplified).
    pub fn points(&self) -> &[StrokePoint] {
        &self.points
    }

    /// Finish the stroke: snap the tail to the real pen-up position (the filter
    /// trails slightly) and simplify. Tolerances are ~0.2 screen px.
    pub fn finish(mut self) -> Vec<StrokePoint> {
        if let (Some(raw), Some(last)) = (self.last_raw, self.points.last().copied())
            && self.points.len() > 1
        {
            let dt = (raw.t_ms - self.t0).max(0.0) as u32;
            let p = last.p;
            if (raw.x - last.x).abs() > 0.01 || (raw.y - last.y).abs() > 0.01 {
                self.points.push(StrokePoint {
                    x: raw.x,
                    y: raw.y,
                    p,
                    dt: dt.max(last.dt),
                });
            }
        }
        let eps = 0.2 / self.cfg.zoom.max(1e-3);
        simplify(&self.points, eps, 0.06)
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
