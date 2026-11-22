//! Pressure normalisation and width mapping.

#[derive(Clone, Copy, Debug)]
pub struct PressureCurve {
    /// Exponent applied to raw pressure (1 = linear, <1 = softer, >1 = firmer).
    pub gamma: f32,
    /// Pressure assumed for devices that report none (mouse, many touch screens).
    pub default_pressure: f32,
}

impl Default for PressureCurve {
    fn default() -> Self {
        Self {
            gamma: 0.8,
            default_pressure: 0.5,
        }
    }
}

impl PressureCurve {
    /// Normalise a raw platform pressure. Browsers report 0 for hovering pens and
    /// 0.5 for buttons-down mice; treat both "unknown" cases as the default.
    pub fn normalise(&self, raw: f32, has_pressure: bool) -> f32 {
        if !has_pressure || !raw.is_finite() || raw <= 0.0 {
            return self.default_pressure;
        }
        raw.clamp(0.0, 1.0).powf(self.gamma)
    }

    /// Radius for a normalised pressure and a nominal full width.
    pub fn radius(&self, p: f32, width: f32) -> f32 {
        crate::point_radius(p, width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_pressure_uses_default() {
        let c = PressureCurve::default();
        assert_eq!(c.normalise(0.0, true), 0.5);
        assert_eq!(c.normalise(0.9, false), 0.5);
    }

    #[test]
    fn radius_is_monotonic_and_bounded() {
        let c = PressureCurve::default();
        let a = c.radius(0.0, 4.0);
        let b = c.radius(1.0, 4.0);
        assert!(a < b);
        assert!((b - 2.0).abs() < 1e-6);
        assert!((a - 0.7).abs() < 1e-6);
    }
}
