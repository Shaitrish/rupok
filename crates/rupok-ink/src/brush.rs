//! Pen styles ("brushes"): how a stroke's points turn into ink.
//!
//! A brush is stored on a stroke as the optional `brush` key
//! (spec/rupok-format.md, Brushes). A reader that does not know it draws the
//! stroke as a plain pen stroke, so every brush degrades to readable ink.
//!
//! Everything a brush changes is a function of the stored points and the
//! position on the page, never of how many points there are or when a frame
//! was drawn. That is what keeps wet ink (dense points) and dry ink
//! (simplified points) pixel-identical (docs/performance.md, "The wet/dry
//! contract"):
//!
//! - widths come from pressure and, for [`Brush::Brush`], the distance along
//!   the stroke from its ends ([`Brush::radii`]);
//! - textures (pencil grain, chalk) are sampled in page space;
//! - dashes, dots and the rainbow follow the distance along the stroke from
//!   its start, so those brushes are stored unsimplified
//!   ([`Brush::keeps_every_point`]);
//! - the fountain pen and marker are a flat nib: the width follows the
//!   direction of travel, measured over the last [`NIB_WINDOW`] widths of
//!   the path (not between neighbouring points, which a dense wet stroke and
//!   its simplified dry copy do not share). The direction only looks back,
//!   so ink already drawn never changes while the pen moves on.

use crate::{MIN_WIDTH_RATIO, StrokePoint};

/// A pen style. [`Brush::Pen`] is the plain round, pressure-sensitive pen
/// and is not stored (no `brush` key).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Brush {
    #[default]
    Pen,
    /// Constant width, crisp.
    Ballpoint,
    /// A flat nib at [`NIB_ANGLE`]: width follows direction and pressure.
    Fountain,
    /// A wide chisel nib at [`CHISEL_ANGLE`], no pressure; usually
    /// translucent.
    Marker,
    /// Thin, with a paper grain.
    Pencil,
    /// Broken, dusty edge.
    Chalk,
    /// Pressure-sensitive with tapered ends and a soft edge.
    Brush,
    Dashed,
    Dotted,
    /// A glowing line with a light core.
    Neon,
    /// Colour cycles through the rainbow along the stroke.
    Rainbow,
    /// Disappearing ink: shown like [`Brush::Neon`], never saved.
    Magic,
}

/// Nib angle of the fountain pen, radians from the x axis (y points down):
/// a nib held like "/" at about 40°.
pub const NIB_ANGLE: f32 = -0.698;
/// Nib angle of the marker's chisel tip.
pub const CHISEL_ANGLE: f32 = -1.05;
/// Width of a nib drawn along its own angle, as a fraction of the full
/// width (drawn across it).
pub const NIB_THICKNESS: f32 = 0.16;
pub const CHISEL_THICKNESS: f32 = 0.45;
/// The direction of travel is measured over this many stroke widths back
/// along the path.
pub const NIB_WINDOW: f32 = 1.0;
/// Brush: tapered ends span this many widths of the stroke.
pub const TAPER_WIDTHS: f32 = 3.0;
/// Brush: width at the very tip, as a fraction of the full width.
pub const TAPER_MIN: f32 = 0.12;
/// Neon and magic ink: how far the glow reaches, in radii past the line.
pub const GLOW_RADII: f32 = 1.6;
/// Dashes: a dash's straight part and the period, in stroke widths.
pub const DASH_ON: f32 = 2.0;
pub const DASH_PERIOD: f32 = 4.0;
/// Dots: the period in stroke widths.
pub const DOT_PERIOD: f32 = 2.0;
/// Rainbow: one full turn of the colour wheel, in stroke widths (at least
/// [`RAINBOW_MIN`] world units).
pub const RAINBOW_WIDTHS: f32 = 40.0;
pub const RAINBOW_MIN: f32 = 160.0;

impl Brush {
    /// Every brush, in the order the pen popover shows them.
    pub const ALL: [Brush; 12] = [
        Brush::Pen,
        Brush::Ballpoint,
        Brush::Fountain,
        Brush::Marker,
        Brush::Pencil,
        Brush::Chalk,
        Brush::Brush,
        Brush::Dashed,
        Brush::Dotted,
        Brush::Neon,
        Brush::Rainbow,
        Brush::Magic,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Brush::Pen => "pen",
            Brush::Ballpoint => "ballpoint",
            Brush::Fountain => "fountain",
            Brush::Marker => "marker",
            Brush::Pencil => "pencil",
            Brush::Chalk => "chalk",
            Brush::Brush => "brush",
            Brush::Dashed => "dashed",
            Brush::Dotted => "dotted",
            Brush::Neon => "neon",
            Brush::Rainbow => "rainbow",
            Brush::Magic => "magic",
        }
    }

    /// Unknown names are a plain pen (readers MUST accept other values).
    pub fn parse(s: &str) -> Brush {
        Brush::ALL
            .into_iter()
            .find(|b| b.as_str() == s)
            .unwrap_or(Brush::Pen)
    }

    /// The brush of a stored stroke's optional `brush` key.
    pub fn of(key: Option<&str>) -> Brush {
        key.map_or(Brush::Pen, Brush::parse)
    }

    /// The value to store in the `brush` key: `None` for the plain pen, and
    /// for magic ink, which is never saved.
    pub fn key(self) -> Option<&'static str> {
        match self {
            Brush::Pen | Brush::Magic => None,
            b => Some(b.as_str()),
        }
    }

    /// Id of the brush in the ink shader (`ink.wgsl`).
    pub fn shader_id(self) -> u8 {
        self as u8
    }

    /// Whether pen pressure changes the width.
    pub fn uses_pressure(self) -> bool {
        !matches!(self, Brush::Ballpoint | Brush::Marker)
    }

    /// Whether the look depends on the distance along the stroke, so the dry
    /// stroke keeps every wet point (simplifying would shorten the path by a
    /// fraction of a pixel per point and shift the pattern).
    pub fn keeps_every_point(self) -> bool {
        matches!(
            self,
            Brush::Dashed | Brush::Dotted | Brush::Rainbow | Brush::Magic
        )
    }

    /// Radius of a point with pressure `p`, before the brush's end taper.
    /// For the nib brushes this is the nib's half length.
    pub fn radius(self, p: f32, width: f32) -> f32 {
        let p = p.clamp(0.0, 1.0);
        match self {
            Brush::Ballpoint | Brush::Marker => 0.5 * width,
            Brush::Brush => 0.5 * width * (0.15 + 0.85 * p),
            Brush::Pencil => 0.5 * width * (0.6 + 0.4 * p),
            _ => 0.5 * width * (MIN_WIDTH_RATIO + (1.0 - MIN_WIDTH_RATIO) * p),
        }
    }

    /// How far ink reaches past [`Brush::radius`], as a factor of it (the
    /// glow). Bounds, culling and hit-testing use it.
    pub fn reach(self) -> f32 {
        match self {
            Brush::Neon | Brush::Magic => 1.0 + GLOW_RADII,
            _ => 1.0,
        }
    }

    /// Whether a point's radius depends on the rest of the path (the
    /// brush's taper, the nib's direction), not only on its pressure.
    pub fn shapes_by_path(self) -> bool {
        matches!(self, Brush::Brush | Brush::Fountain | Brush::Marker)
    }

    /// The radius of every point, in order, including the end taper of
    /// [`Brush::Brush`] and the nib of [`Brush::Fountain`] and
    /// [`Brush::Marker`]. The renderer, the PDF export and the tests all use
    /// this one function.
    pub fn radii(self, points: &[StrokePoint], width: f32, out: &mut Vec<f32>) {
        out.clear();
        out.extend(points.iter().map(|q| self.radius(q.p, width)));
        if !self.shapes_by_path() || points.len() < 2 {
            return;
        }
        let mut arc = Vec::with_capacity(points.len());
        arc_lengths(points, &mut arc);
        if self == Brush::Brush {
            let total = *arc.last().unwrap_or(&0.0);
            let span = (TAPER_WIDTHS * width).max(1e-3);
            for (r, s) in out.iter_mut().zip(&arc) {
                let from_ends = s.min(total - s).max(0.0);
                *r *= taper(from_ends / span);
            }
            return;
        }
        let (ang, thin) = if self == Brush::Marker {
            (CHISEL_ANGLE, CHISEL_THICKNESS)
        } else {
            (NIB_ANGLE, NIB_THICKNESS)
        };
        let n = (ang.cos(), ang.sin());
        let window = (NIB_WINDOW * width).max(1e-3);
        // Travel direction at point i: from the point `window` back along the
        // path (or the start) to point i; before the stroke has gone that
        // far, the first stretch of it.
        let first_dir = first_direction(points);
        let mut j = 0usize;
        for i in 0..points.len() {
            let back = arc[i] - window;
            while j + 1 < i && arc[j + 1] <= back {
                j += 1;
            }
            let from = if back <= 0.0 {
                (points[0].x, points[0].y)
            } else {
                lerp_at(points, &arc, j, back)
            };
            let (dx, dy) = (points[i].x - from.0, points[i].y - from.1);
            let len = dx.hypot(dy);
            let d = if len > 0.25 * window {
                (dx / len, dy / len)
            } else {
                first_dir
            };
            let across = (d.0 * n.1 - d.1 * n.0).abs();
            out[i] *= thin + (1.0 - thin) * across;
        }
    }

    /// Dash pattern along the stroke for [`Brush::Dashed`] and
    /// [`Brush::Dotted`]: `(period, straight length of a dash)`, world units.
    pub fn dash(self, width: f32) -> Option<(f32, f32)> {
        match self {
            Brush::Dashed => Some((DASH_PERIOD * width, DASH_ON * width)),
            Brush::Dotted => Some((DOT_PERIOD * width, 0.0)),
            _ => None,
        }
    }

    /// Rainbow: distance along the stroke for one turn of the colour wheel.
    pub fn rainbow_period(width: f32) -> f32 {
        (RAINBOW_WIDTHS * width).max(RAINBOW_MIN)
    }
}

/// Direction of the first stretch of the stroke that is long enough to
/// have one (along x for a dot).
fn first_direction(points: &[StrokePoint]) -> (f32, f32) {
    let p0 = points[0];
    let far = points
        .iter()
        .find(|q| (q.x - p0.x).hypot(q.y - p0.y) > 0.5)
        .or(points.last());
    match far {
        Some(q) if (q.x - p0.x).hypot(q.y - p0.y) > 1e-4 => {
            let l = (q.x - p0.x).hypot(q.y - p0.y);
            ((q.x - p0.x) / l, (q.y - p0.y) / l)
        }
        _ => (1.0, 0.0),
    }
}

/// The point at distance `s` along the path, in the segment starting at
/// point `j` (`arc[j] <= s <= arc[j + 1]`).
fn lerp_at(points: &[StrokePoint], arc: &[f32], j: usize, s: f32) -> (f32, f32) {
    let k = (j + 1).min(points.len() - 1);
    let (a, b) = (arc[j], arc[k]);
    let t = if b > a {
        ((s - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (
        points[j].x + (points[k].x - points[j].x) * t,
        points[j].y + (points[k].y - points[j].y) * t,
    )
}

/// Width factor `TAPER_MIN..=1` at `t` taper spans from the nearer end
/// (ease-out, so the tip is pointed and the body full).
pub fn taper(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let e = 1.0 - (1.0 - t) * (1.0 - t);
    TAPER_MIN + (1.0 - TAPER_MIN) * e
}

/// Cumulative distance along the polyline at every point (world units).
pub fn arc_lengths(points: &[StrokePoint], out: &mut Vec<f32>) {
    out.clear();
    let mut s = 0.0f32;
    let mut prev: Option<&StrokePoint> = None;
    for p in points {
        if let Some(q) = prev {
            s += (p.x - q.x).hypot(p.y - q.y);
        }
        out.push(s);
        prev = Some(p);
    }
}

/// The rainbow colour (0xRRGGBB) at distance `s` along a stroke of `width`.
/// Matches `rainbow()` in `ink.wgsl`.
pub fn rainbow_rgb(s: f32, width: f32) -> u32 {
    let h = (s / Brush::rainbow_period(width)).rem_euclid(1.0) * 6.0;
    // Saturated but not neon: reads on white and on a chalkboard.
    let (sat, val) = (0.78f32, 0.92f32);
    let c = val * sat;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = val - c;
    let to = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u32;
    (to(r) << 16) | (to(g) << 8) | to(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point_radius;
    use std::f32::consts::FRAC_PI_2;

    fn line(n: usize, step: f32, p: f32) -> Vec<StrokePoint> {
        (0..n)
            .map(|i| StrokePoint {
                x: i as f32 * step,
                y: 0.0,
                p,
                dt: i as u32 * 4,
            })
            .collect()
    }

    #[test]
    fn names_round_trip_and_unknown_is_a_pen() {
        for b in Brush::ALL {
            assert_eq!(Brush::parse(b.as_str()), b);
        }
        assert_eq!(Brush::parse("watercolour"), Brush::Pen);
        assert_eq!(Brush::of(None), Brush::Pen);
        assert_eq!(Brush::Pen.key(), None);
        assert_eq!(Brush::Magic.key(), None, "magic ink is never stored");
        assert_eq!(Brush::Chalk.key(), Some("chalk"));
    }

    #[test]
    fn plain_pen_keeps_the_format_width_mapping() {
        for p in [0.0, 0.3, 1.0] {
            assert_eq!(Brush::Pen.radius(p, 4.0), point_radius(p, 4.0));
        }
    }

    #[test]
    fn ballpoint_and_marker_ignore_pressure() {
        for b in [Brush::Ballpoint, Brush::Marker] {
            assert_eq!(b.radius(0.0, 6.0), 3.0);
            assert_eq!(b.radius(1.0, 6.0), 3.0);
            assert!(!b.uses_pressure());
        }
        assert!(Brush::Fountain.radius(1.0, 6.0) > Brush::Fountain.radius(0.0, 6.0));
    }

    #[test]
    fn brush_tapers_both_ends_and_is_full_in_the_middle() {
        let pts = line(200, 1.0, 1.0);
        let mut r = Vec::new();
        Brush::Brush.radii(&pts, 4.0, &mut r);
        let full = Brush::Brush.radius(1.0, 4.0);
        assert!((r[0] - full * TAPER_MIN).abs() < 1e-4);
        assert!((r[199] - full * TAPER_MIN).abs() < 1e-4);
        assert!((r[100] - full).abs() < 1e-4);
        assert!(r[3] < r[8] && r[8] < r[30]);
        // Other brushes have no taper.
        Brush::Ballpoint.radii(&pts, 4.0, &mut r);
        assert!(r.iter().all(|v| *v == 2.0));
    }

    #[test]
    fn taper_depends_on_distance_not_on_point_count() {
        // The same line sampled densely and sparsely tapers the same at the
        // shared points: wet (dense) and dry ink agree.
        let dense = line(201, 0.5, 1.0);
        let sparse = line(26, 4.0, 1.0);
        let (mut a, mut b) = (Vec::new(), Vec::new());
        Brush::Brush.radii(&dense, 5.0, &mut a);
        Brush::Brush.radii(&sparse, 5.0, &mut b);
        for (i, rb) in b.iter().enumerate() {
            assert!((a[i * 8] - rb).abs() < 1e-4, "point {i}");
        }
    }

    #[test]
    fn arc_lengths_accumulate() {
        let mut s = Vec::new();
        arc_lengths(&line(5, 2.5, 0.5), &mut s);
        assert_eq!(s, vec![0.0, 2.5, 5.0, 7.5, 10.0]);
    }

    #[test]
    fn reach_covers_the_glow() {
        assert_eq!(Brush::Pen.reach(), 1.0);
        assert!(Brush::Neon.reach() > 2.0);
    }

    fn along(n: usize, step: f32, ang: f32) -> Vec<StrokePoint> {
        (0..n)
            .map(|i| StrokePoint {
                x: i as f32 * step * ang.cos(),
                y: i as f32 * step * ang.sin(),
                p: 1.0,
                dt: 0,
            })
            .collect()
    }

    #[test]
    fn the_nib_is_wide_across_its_angle_and_thin_along_it() {
        let mut r = Vec::new();
        let full = Brush::Fountain.radius(1.0, 8.0);
        Brush::Fountain.radii(&along(40, 1.0, NIB_ANGLE), 8.0, &mut r);
        assert!(
            r.iter().all(|v| (v - full * NIB_THICKNESS).abs() < 1e-3),
            "{r:?}"
        );
        Brush::Fountain.radii(&along(40, 1.0, NIB_ANGLE + FRAC_PI_2), 8.0, &mut r);
        assert!(r.iter().all(|v| (v - full).abs() < 1e-3));
        // The marker's chisel is thicker along its angle.
        Brush::Marker.radii(&along(40, 1.0, CHISEL_ANGLE), 8.0, &mut r);
        assert!((r[20] - 4.0 * CHISEL_THICKNESS).abs() < 1e-3);
    }

    #[test]
    fn the_nib_reads_the_same_from_dense_and_sparse_points() {
        // A curve sampled every 0.5 units and every 4: the width at the
        // shared points matches (the wet/dry contract).
        let curve = |step: f32| -> Vec<StrokePoint> {
            (0..((200.0 / step) as usize + 1))
                .map(|i| {
                    let x = i as f32 * step;
                    StrokePoint {
                        x,
                        y: (x * 0.05).sin() * 30.0,
                        p: 0.8,
                        dt: 0,
                    }
                })
                .collect()
        };
        let (mut a, mut b) = (Vec::new(), Vec::new());
        Brush::Fountain.radii(&curve(0.5), 8.0, &mut a);
        Brush::Fountain.radii(&curve(4.0), 8.0, &mut b);
        for (i, rb) in b.iter().enumerate().skip(3) {
            assert!(
                (a[i * 8] - rb).abs() < 0.15,
                "point {i}: {} vs {rb}",
                a[i * 8]
            );
        }
    }

    #[test]
    fn ink_already_drawn_keeps_its_width_as_the_stroke_grows() {
        let pts = along(60, 1.0, 0.3);
        let mut whole = Vec::new();
        Brush::Fountain.radii(&pts, 6.0, &mut whole);
        let mut part = Vec::new();
        Brush::Fountain.radii(&pts[..30], 6.0, &mut part);
        assert_eq!(&whole[..30], &part[..]);
    }

    #[test]
    fn dashes_and_rainbow_keep_every_point() {
        for b in [Brush::Dashed, Brush::Dotted, Brush::Rainbow] {
            assert!(b.keeps_every_point());
        }
        // The taper follows the distance from the ends, which simplifying
        // barely changes.
        assert!(!Brush::Pen.keeps_every_point() && !Brush::Brush.keeps_every_point());
        assert_eq!(Brush::Dashed.dash(2.0), Some((8.0, 4.0)));
        assert_eq!(Brush::Dotted.dash(2.0), Some((4.0, 0.0)));
        assert_eq!(Brush::Pen.dash(2.0), None);
    }

    #[test]
    fn rainbow_starts_red_and_turns() {
        assert_eq!(rainbow_rgb(0.0, 4.0) >> 16, 0xeb);
        let p = Brush::rainbow_period(4.0);
        assert_eq!(rainbow_rgb(p, 4.0), rainbow_rgb(0.0, 4.0));
        assert_ne!(rainbow_rgb(p / 3.0, 4.0), rainbow_rgb(0.0, 4.0));
    }
}
