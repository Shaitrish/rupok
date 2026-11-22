//! One-Euro filter (Casiez et al. 2012): low lag at speed, low jitter at rest.

#[derive(Clone, Copy, Debug)]
pub struct OneEuroParams {
    /// Minimum cutoff frequency (Hz). Lower = smoother at rest.
    pub min_cutoff: f32,
    /// Speed coefficient. Higher = less lag during fast motion.
    pub beta: f32,
    /// Cutoff for the derivative (Hz).
    pub d_cutoff: f32,
}

impl Default for OneEuroParams {
    fn default() -> Self {
        // Tuned for CSS-pixel coordinates from pens/touch at 60–240 Hz.
        Self {
            min_cutoff: 1.2,
            beta: 0.02,
            d_cutoff: 1.0,
        }
    }
}

impl OneEuroParams {
    /// Map a user-facing "smoothing" setting (0 = raw, 1 = very smooth).
    pub fn from_smoothing(s: f32) -> Self {
        let s = s.clamp(0.0, 1.0);
        Self {
            min_cutoff: 6.0 - 5.5 * s,
            beta: 0.05 - 0.045 * s,
            d_cutoff: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
struct LowPass {
    y: Option<f32>,
}

impl LowPass {
    fn filter(&mut self, x: f32, alpha: f32) -> f32 {
        let y = match self.y {
            Some(prev) => alpha * x + (1.0 - alpha) * prev,
            None => x,
        };
        self.y = Some(y);
        y
    }
}

#[inline]
fn alpha(cutoff: f32, dt_s: f32) -> f32 {
    let tau = 1.0 / (2.0 * std::f32::consts::PI * cutoff);
    1.0 / (1.0 + tau / dt_s)
}

/// Scalar One-Euro filter.
#[derive(Clone, Debug)]
pub struct OneEuro {
    params: OneEuroParams,
    x: LowPass,
    dx: LowPass,
    last_raw: Option<f32>,
    last_t: Option<f64>,
}

impl OneEuro {
    pub fn new(params: OneEuroParams) -> Self {
        Self {
            params,
            x: LowPass { y: None },
            dx: LowPass { y: None },
            last_raw: None,
            last_t: None,
        }
    }

    /// Filter sample `v` taken at `t_ms`.
    pub fn filter(&mut self, v: f32, t_ms: f64) -> f32 {
        let dt_s = match self.last_t {
            Some(lt) => (((t_ms - lt) / 1000.0) as f32).max(1.0 / 1000.0),
            None => 1.0 / 120.0,
        };
        self.last_t = Some(t_ms);
        let dv = match self.last_raw {
            Some(prev) => (v - prev) / dt_s,
            None => 0.0,
        };
        self.last_raw = Some(v);
        let edv = self.dx.filter(dv, alpha(self.params.d_cutoff, dt_s));
        let cutoff = self.params.min_cutoff + self.params.beta * edv.abs();
        self.x.filter(v, alpha(cutoff, dt_s))
    }
}

/// 2-D convenience wrapper.
#[derive(Clone, Debug)]
pub struct OneEuro2 {
    x: OneEuro,
    y: OneEuro,
}

impl OneEuro2 {
    pub fn new(params: OneEuroParams) -> Self {
        Self {
            x: OneEuro::new(params),
            y: OneEuro::new(params),
        }
    }
    pub fn filter(&mut self, x: f32, y: f32, t_ms: f64) -> (f32, f32) {
        (self.x.filter(x, t_ms), self.y.filter(y, t_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_signal_is_unchanged() {
        let mut f = OneEuro::new(OneEuroParams::default());
        for i in 0..100 {
            let v = f.filter(42.0, i as f64 * 8.0);
            assert!((v - 42.0).abs() < 1e-4);
        }
    }

    #[test]
    fn reduces_jitter() {
        let mut f = OneEuro::new(OneEuroParams::default());
        let mut max_dev: f32 = 0.0;
        for i in 0..400 {
            let noise = if i % 2 == 0 { 0.8 } else { -0.8 };
            let v = f.filter(100.0 + noise, i as f64 * 4.0);
            if i > 50 {
                max_dev = max_dev.max((v - 100.0).abs());
            }
        }
        assert!(max_dev < 0.5, "jitter not reduced: {max_dev}");
    }

    #[test]
    fn follows_fast_motion() {
        let mut f = OneEuro::new(OneEuroParams::default());
        let mut last = 0.0;
        for i in 0..120 {
            last = f.filter(i as f32 * 20.0, i as f64 * 8.0);
        }
        // Moving 20px per 8ms: the filter should trail by only a few px.
        assert!((119.0 * 20.0 - last).abs() < 60.0, "lag too large: {last}");
    }
}
