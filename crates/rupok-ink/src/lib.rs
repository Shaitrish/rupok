//! Ink input pipeline for rupok.
//!
//! Raw pointer samples flow through: [`filter::OneEuro`] smoothing →
//! [`pressure::PressureCurve`] → [`builder::StrokeBuilder`] (accumulation,
//! wet-stroke state) → [`simplify`] (on finish) → [`codec`] (compact storage).
//!
//! Everything here is allocation-light and `no_std`-friendly in spirit, so the
//! same code runs in the webview (WASM) on the pen hot path and natively.

pub mod brush;
pub mod builder;
pub mod codec;
pub mod filter;
pub mod math;
pub mod pressure;
pub mod recognize;
pub mod simplify;
pub mod write;

pub use brush::Brush;
pub use builder::{BuilderConfig, StrokeBuilder, StrokeStyle, Tool};
pub use codec::{CodecError, decode_points, encode_points};

/// A raw pointer sample as delivered by the platform (CSS px in *world* units,
/// pressure 0..=1, timestamp in milliseconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputPoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub t_ms: f64,
}

/// A processed, stored stroke point. `dt` is milliseconds since stroke start,
/// which is what makes faithful step-by-step replay possible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    /// Normalised pressure after the pressure curve, 0..=1.
    pub p: f32,
    pub dt: u32,
}

/// Width at zero pressure as a fraction of the nominal width. Part of the file
/// spec: stored strokes must render identically everywhere, so this is not a
/// user setting (users tune the input-side pressure *curve* instead).
pub const MIN_WIDTH_RATIO: f32 = 0.35;

/// Rendered radius of a stored point with normalised pressure `p`.
#[inline]
pub fn point_radius(p: f32, width: f32) -> f32 {
    0.5 * width * (MIN_WIDTH_RATIO + (1.0 - MIN_WIDTH_RATIO) * p.clamp(0.0, 1.0))
}

/// Axis-aligned bounding box `[min_x, min_y, max_x, max_y]`.
pub type Aabb = [f32; 4];

/// Bounding box of points inflated by `pad` (normally the max stroke radius).
pub fn bounds(points: &[StrokePoint], pad: f32) -> Aabb {
    let mut b = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for p in points {
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.y);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.y);
    }
    if points.is_empty() {
        return [0.0; 4];
    }
    [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad]
}
