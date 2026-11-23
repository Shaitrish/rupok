//! Shape tools produce ordinary strokes (tagged with `shape`), so shapes get
//! the same renderer, eraser, lasso and replay behaviour as ink for free.
//!
//! A shape stroke also stores its editable geometry ([`ShapeGeom`], the
//! `geom` key): resizing transforms the geometry and regenerates the points
//! from it, so the line width stays constant. Readers that don't know `geom`
//! just draw the points.

use rupok_core::ShapeGeom;
use rupok_ink::StrokePoint;
use rupok_ink::recognize::{Geometry, Recognized};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rect,
    Ellipse,
    Line,
    Arrow,
}

impl ShapeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ShapeKind::Rect => "rect",
            ShapeKind::Ellipse => "ellipse",
            ShapeKind::Line => "line",
            ShapeKind::Arrow => "arrow",
        }
    }
}

/// Pressure of generated shape points (the shape tools' look).
pub const SHAPE_PRESSURE: f32 = 0.6;

fn ptp(x: f32, y: f32, p: f32) -> StrokePoint {
    StrokePoint { x, y, p, dt: 0 }
}

/// Points for a shape dragged from `a` to `b` (world units). `width` sizes the
/// arrow head.
pub fn shape_points(kind: ShapeKind, a: [f32; 2], b: [f32; 2], width: f32) -> Vec<StrokePoint> {
    geom_points(&shape_geom(kind, a, b), width, SHAPE_PRESSURE).unwrap_or_default()
}

/// Geometry of a shape dragged from `a` to `b`.
pub fn shape_geom(kind: ShapeKind, a: [f32; 2], b: [f32; 2]) -> ShapeGeom {
    let (a, b) = (
        [f64::from(a[0]), f64::from(a[1])],
        [f64::from(b[0]), f64::from(b[1])],
    );
    match kind {
        ShapeKind::Line | ShapeKind::Arrow => ShapeGeom {
            kind: kind.as_str().into(),
            x: a[0],
            y: a[1],
            w: b[0] - a[0],
            h: b[1] - a[1],
            rot: 0.0,
            v: Vec::new(),
        },
        ShapeKind::Rect | ShapeKind::Ellipse => ShapeGeom {
            kind: kind.as_str().into(),
            x: a[0].min(b[0]),
            y: a[1].min(b[1]),
            w: (b[0] - a[0]).abs(),
            h: (b[1] - a[1]).abs(),
            rot: 0.0,
            v: Vec::new(),
        },
    }
}

/// Stored `shape` name and geometry for a Smart Ink result.
pub fn from_recognized(r: &Recognized) -> (&'static str, ShapeGeom) {
    let f = f64::from;
    let g = match &r.geometry {
        Geometry::Segment { a, b } => ShapeGeom {
            kind: if r.kind == rupok_ink::recognize::ShapeKind::Arrow {
                "arrow".into()
            } else {
                "line".into()
            },
            x: f(a[0]),
            y: f(a[1]),
            w: f(b[0] - a[0]),
            h: f(b[1] - a[1]),
            rot: 0.0,
            v: Vec::new(),
        },
        Geometry::Ellipse { c, rx, ry, rot } => ShapeGeom {
            kind: "ellipse".into(),
            x: f(c[0] - rx),
            y: f(c[1] - ry),
            w: f(2.0 * rx),
            h: f(2.0 * ry),
            rot: f(*rot),
            v: Vec::new(),
        },
        Geometry::Rect { c, w, h, rot } => ShapeGeom {
            kind: "rect".into(),
            x: f(c[0] - w / 2.0),
            y: f(c[1] - h / 2.0),
            w: f(*w),
            h: f(*h),
            rot: f(*rot),
            v: Vec::new(),
        },
        Geometry::Polygon { pts } => {
            poly(&pts.iter().map(|p| [f(p[0]), f(p[1])]).collect::<Vec<_>>())
        }
    };
    (r.kind.as_str(), g)
}

/// Clean geometry (world units) of a stored shape, for recognisers that look
/// at shapes already on the board.
pub fn to_geometry(g: &ShapeGeom) -> Option<Geometry> {
    if !g.is_valid() {
        return None;
    }
    let f = |v: f64| v as f32;
    let c = center(g);
    Some(match g.kind.as_str() {
        "line" | "arrow" => Geometry::Segment {
            a: [f(g.x), f(g.y)],
            b: [f(g.x + g.w), f(g.y + g.h)],
        },
        "ellipse" => Geometry::Ellipse {
            c: [f(c[0]), f(c[1])],
            rx: f(g.w / 2.0),
            ry: f(g.h / 2.0),
            rot: f(g.rot),
        },
        "rect" => Geometry::Rect {
            c: [f(c[0]), f(c[1])],
            w: f(g.w),
            h: f(g.h),
            rot: f(g.rot),
        },
        "poly" => Geometry::Polygon {
            pts: vertices(g).iter().map(|p| [f(p[0]), f(p[1])]).collect(),
        },
        _ => return None,
    })
}

/// A named point on a shape that connectors and endpoint snapping can
/// attach to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// `start`, `end`, `mid`, `center`, `n`/`e`/`s`/`w` (box side midpoints),
    /// `v` (a vertex) or `edge` (an edge midpoint).
    pub name: &'static str,
    pub p: [f64; 2],
}

/// Anchors of a shape, derived from its geometry (nothing extra is stored).
pub fn anchors(g: &ShapeGeom) -> Vec<Anchor> {
    if !g.is_valid() {
        return Vec::new();
    }
    let a = |name, p| Anchor { name, p };
    let c = center(g);
    match g.kind.as_str() {
        "line" | "arrow" => vec![
            a("start", [g.x, g.y]),
            a("end", [g.x + g.w, g.y + g.h]),
            a("mid", [g.x + g.w / 2.0, g.y + g.h / 2.0]),
        ],
        "ellipse" | "rect" => {
            let mut v = vec![a("center", c)];
            for (name, p) in [
                ("n", [c[0], g.y]),
                ("e", [g.x + g.w, c[1]]),
                ("s", [c[0], g.y + g.h]),
                ("w", [g.x, c[1]]),
            ] {
                v.push(a(name, rot_about(p, c, g.rot)));
            }
            if g.kind == "rect" {
                v.extend(vertices(g).into_iter().map(|p| a("v", p)));
            }
            v
        }
        "poly" => {
            let vs = vertices(g);
            let mut v = vec![a("center", c)];
            for (i, p) in vs.iter().enumerate() {
                let q = vs[(i + 1) % vs.len()];
                v.push(a("v", *p));
                v.push(a("edge", [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0]));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// A `poly` geometry through absolute vertices.
fn poly(pts: &[[f64; 2]]) -> ShapeGeom {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pts {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
    ShapeGeom {
        kind: "poly".into(),
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
        rot: 0.0,
        v: pts
            .iter()
            .flat_map(|p| [(p[0] - x0) / w, (p[1] - y0) / h])
            .collect(),
    }
}

fn center(g: &ShapeGeom) -> [f64; 2] {
    [g.x + g.w / 2.0, g.y + g.h / 2.0]
}

fn rot_about(p: [f64; 2], c: [f64; 2], rot: f64) -> [f64; 2] {
    if rot == 0.0 {
        return p;
    }
    let (s, co) = rot.sin_cos();
    let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
    [c[0] + dx * co - dy * s, c[1] + dx * s + dy * co]
}

/// Absolute outline vertices of a `rect` or `poly` (rotation applied).
fn vertices(g: &ShapeGeom) -> Vec<[f64; 2]> {
    let c = center(g);
    let local: Vec<[f64; 2]> = match g.kind.as_str() {
        "rect" => vec![
            [g.x, g.y],
            [g.x + g.w, g.y],
            [g.x + g.w, g.y + g.h],
            [g.x, g.y + g.h],
        ],
        "poly" => {
            g.v.as_chunks::<2>()
                .0
                .iter()
                .map(|v| [g.x + v[0] * g.w, g.y + v[1] * g.h])
                .collect()
        }
        _ => Vec::new(),
    };
    local.into_iter().map(|p| rot_about(p, c, g.rot)).collect()
}

/// Stroke points for a geometry. `p` is the pressure of every point (it sets
/// the drawn thickness); `width` sizes arrow heads. `None` for an unknown or
/// invalid geometry.
pub fn geom_points(g: &ShapeGeom, width: f32, p: f32) -> Option<Vec<StrokePoint>> {
    if !g.is_valid() {
        return None;
    }
    let pt = |q: [f64; 2]| ptp(q[0] as f32, q[1] as f32, p);
    match g.kind.as_str() {
        "line" => Some(vec![pt([g.x, g.y]), pt([g.x + g.w, g.y + g.h])]),
        "arrow" => {
            let (a, b) = (
                [g.x as f32, g.y as f32],
                [(g.x + g.w) as f32, (g.y + g.h) as f32],
            );
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (ux, uy) = (dx / len, dy / len);
            let head = (width * 4.0 + 10.0).min(len * 0.45);
            let wing = |s: f32| {
                let (c, sn) = (0.5f32.cos(), 0.5f32.sin() * s);
                // rotate (-u) by ±~28°
                let (rx, ry) = (-ux * c + uy * sn, -uy * c - ux * sn);
                ptp(b[0] + rx * head, b[1] + ry * head, p)
            };
            Some(vec![
                ptp(a[0], a[1], p),
                ptp(b[0], b[1], p),
                wing(1.0),
                ptp(b[0], b[1], p),
                wing(-1.0),
            ])
        }
        "ellipse" => {
            let c = center(g);
            let (rx, ry) = (g.w.abs() / 2.0, g.h.abs() / 2.0);
            let n = ((rx + ry).sqrt() * 6.0).clamp(24.0, 160.0) as usize;
            Some(
                (0..=n)
                    .map(|i| {
                        let t = i as f64 / n as f64 * std::f64::consts::TAU;
                        pt(rot_about(
                            [c[0] + rx * t.cos(), c[1] + ry * t.sin()],
                            c,
                            g.rot,
                        ))
                    })
                    .collect(),
            )
        }
        "rect" | "poly" => {
            let v = vertices(g);
            if v.len() < 3 && g.kind == "poly" {
                return None;
            }
            let mut out: Vec<StrokePoint> = v.iter().map(|q| pt(*q)).collect();
            out.push(out[0]);
            Some(out)
        }
        _ => None,
    }
}

/// Geometric bounds `[x0, y0, x1, y1]` (without the stroke's thickness).
pub fn geom_aabb(g: &ShapeGeom) -> Option<[f64; 4]> {
    if !g.is_valid() {
        return None;
    }
    let pts: Vec<[f64; 2]> = match g.kind.as_str() {
        "line" | "arrow" => vec![[g.x, g.y], [g.x + g.w, g.y + g.h]],
        "ellipse" => {
            let c = center(g);
            let (a, b) = (g.w.abs() / 2.0, g.h.abs() / 2.0);
            let (s, co) = g.rot.sin_cos();
            let ex = ((a * co).powi(2) + (b * s).powi(2)).sqrt();
            let ey = ((a * s).powi(2) + (b * co).powi(2)).sqrt();
            vec![[c[0] - ex, c[1] - ey], [c[0] + ex, c[1] + ey]]
        }
        "rect" | "poly" => vertices(g),
        _ => return None,
    };
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in pts {
        b = [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ];
    }
    Some(b)
}

/// The geometry moved by `(dx, dy)`.
pub fn translate(g: &ShapeGeom, dx: f64, dy: f64) -> ShapeGeom {
    ShapeGeom {
        x: g.x + dx,
        y: g.y + dy,
        ..g.clone()
    }
}

/// Scale by `s` and rotate by `rot` radians about `pivot` (hold-to-snap:
/// the pen drags the snapped shape bigger, smaller or round).
pub fn scale_rotate(g: &ShapeGeom, pivot: [f64; 2], s: f64, rot: f64) -> ShapeGeom {
    let tf = |p: [f64; 2]| {
        let q = rot_about(p, pivot, rot);
        [
            pivot[0] + (q[0] - pivot[0]) * s,
            pivot[1] + (q[1] - pivot[1]) * s,
        ]
    };
    match g.kind.as_str() {
        "line" | "arrow" => {
            let (a, b) = (tf([g.x, g.y]), tf([g.x + g.w, g.y + g.h]));
            ShapeGeom {
                x: a[0],
                y: a[1],
                w: b[0] - a[0],
                h: b[1] - a[1],
                ..g.clone()
            }
        }
        "poly" => poly(&vertices(g).into_iter().map(tf).collect::<Vec<_>>()),
        _ => {
            let c = tf(center(g));
            let (w, h) = (g.w * s, g.h * s);
            ShapeGeom {
                x: c[0] - w / 2.0,
                y: c[1] - h / 2.0,
                w,
                h,
                rot: g.rot + rot,
                ..g.clone()
            }
        }
    }
}

/// Resize: map the geometry's bounds `from` onto `to` (both `[x0, y0, x1,
/// y1]`). An axis with no extent (a horizontal line's height) keeps its
/// scale. Rotated rectangles and polygons become unrotated polygons, and a
/// rotated ellipse stays an ellipse (an affine image of one).
pub fn map_box(g: &ShapeGeom, from: [f64; 4], to: [f64; 4]) -> Option<ShapeGeom> {
    if !g.is_valid() || ![from, to].iter().flatten().all(|v| v.is_finite()) {
        return None;
    }
    let scale = |a0: f64, a1: f64, b0: f64, b1: f64| {
        if (a1 - a0).abs() < 1e-6 {
            1.0
        } else {
            (b1 - b0) / (a1 - a0)
        }
    };
    let sx = scale(from[0], from[2], to[0], to[2]);
    let sy = scale(from[1], from[3], to[1], to[3]);
    let f = |p: [f64; 2]| [to[0] + (p[0] - from[0]) * sx, to[1] + (p[1] - from[1]) * sy];
    let rotated = g.rot.abs() > 1e-9;
    let out = match g.kind.as_str() {
        "line" | "arrow" => {
            let (a, b) = (f([g.x, g.y]), f([g.x + g.w, g.y + g.h]));
            ShapeGeom {
                x: a[0],
                y: a[1],
                w: b[0] - a[0],
                h: b[1] - a[1],
                ..g.clone()
            }
        }
        "rect" | "poly" if rotated => poly(&vertices(g).into_iter().map(f).collect::<Vec<_>>()),
        "ellipse" if rotated => {
            // Semi-axes of M = diag(sx, sy) · R(rot) · diag(a, b) from M Mᵀ.
            let (a, b) = (g.w / 2.0, g.h / 2.0);
            let (s, c) = g.rot.sin_cos();
            let m = [[sx * c * a, -sx * s * b], [sy * s * a, sy * c * b]];
            let p = m[0][0] * m[0][0] + m[0][1] * m[0][1];
            let q = m[0][0] * m[1][0] + m[0][1] * m[1][1];
            let r = m[1][0] * m[1][0] + m[1][1] * m[1][1];
            let mid = (p + r) / 2.0;
            let d = (((p - r) / 2.0).powi(2) + q * q).sqrt();
            let (l1, l2) = ((mid + d).max(0.0), (mid - d).max(0.0));
            let rot = 0.5 * (2.0 * q).atan2(p - r);
            let cc = f(center(g));
            let (w, h) = (2.0 * l1.sqrt(), 2.0 * l2.sqrt());
            ShapeGeom {
                x: cc[0] - w / 2.0,
                y: cc[1] - h / 2.0,
                w,
                h,
                rot,
                ..g.clone()
            }
        }
        _ => {
            let (a, b) = (f([g.x, g.y]), f([g.x + g.w, g.y + g.h]));
            ShapeGeom {
                x: a[0].min(b[0]),
                y: a[1].min(b[1]),
                w: (b[0] - a[0]).abs(),
                h: (b[1] - a[1]).abs(),
                ..g.clone()
            }
        }
    };
    out.is_valid().then_some(out)
}

/// Spread `duration` ms over the points by arc length, so replay draws a
/// snapped shape at the pace the teacher drew it.
pub fn with_timing(mut pts: Vec<StrokePoint>, duration: u32) -> Vec<StrokePoint> {
    let total: f32 = pts
        .windows(2)
        .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
        .sum();
    if total <= f32::EPSILON || duration == 0 {
        return pts;
    }
    let mut acc = 0.0f32;
    for i in 0..pts.len() {
        if i > 0 {
            acc += (pts[i].x - pts[i - 1].x).hypot(pts[i].y - pts[i - 1].y);
        }
        pts[i].dt = (acc / total * duration as f32).round() as u32;
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_is_closed() {
        let p = shape_points(ShapeKind::Rect, [0.0, 0.0], [10.0, 5.0], 2.0);
        assert_eq!(p.len(), 5);
        assert_eq!((p[0].x, p[0].y), (p[4].x, p[4].y));
    }

    #[test]
    fn arrow_head_points_back() {
        let p = shape_points(ShapeKind::Arrow, [0.0, 0.0], [100.0, 0.0], 2.0);
        assert!(p[2].x < 100.0 && p[4].x < 100.0);
        assert!(p[2].y * p[4].y < 0.0, "wings on both sides");
    }

    fn tri() -> ShapeGeom {
        poly(&[[100.0, 0.0], [200.0, 100.0], [0.0, 100.0]])
    }

    #[test]
    fn polygons_close_and_round_trip_their_vertices() {
        let g = tri();
        assert_eq!((g.x, g.y, g.w, g.h), (0.0, 0.0, 200.0, 100.0));
        let p = geom_points(&g, 4.0, 0.6).unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!((p[0].x, p[0].y), (100.0, 0.0));
        assert_eq!((p[3].x, p[3].y), (p[0].x, p[0].y));
        assert_eq!(geom_aabb(&g), Some([0.0, 0.0, 200.0, 100.0]));
        // An unknown kind draws nothing new (the stored points remain).
        let odd = ShapeGeom {
            kind: "spline".into(),
            ..g
        };
        assert_eq!(geom_points(&odd, 4.0, 0.6), None);
    }

    #[test]
    fn resizing_maps_bounds_and_keeps_pressure() {
        let g = tri();
        let r = map_box(&g, [0.0, 0.0, 200.0, 100.0], [10.0, 20.0, 410.0, 70.0]).unwrap();
        assert_eq!(geom_aabb(&r), Some([10.0, 20.0, 410.0, 70.0]));
        assert_eq!(r.v, g.v, "normalised vertices are unchanged");
        let p = geom_points(&r, 4.0, 0.42).unwrap();
        assert!(p.iter().all(|q| q.p == 0.42));
        // A horizontal line has no height: its vertical scale stays 1.
        let line = shape_geom(ShapeKind::Line, [0.0, 50.0], [100.0, 50.0]);
        let l = map_box(&line, [0.0, 50.0, 100.0, 50.0], [0.0, 40.0, 300.0, 60.0]).unwrap();
        assert_eq!((l.x, l.y, l.w, l.h), (0.0, 40.0, 300.0, 0.0));
    }

    #[test]
    fn rotated_ellipse_resizes_to_an_ellipse_with_the_new_bounds() {
        let g = ShapeGeom {
            kind: "ellipse".into(),
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 80.0,
            rot: 0.5,
            v: Vec::new(),
        };
        let from = geom_aabb(&g).unwrap();
        let to = [
            from[0],
            from[1],
            from[0] + (from[2] - from[0]) * 2.0,
            from[3],
        ];
        let r = map_box(&g, from, to).unwrap();
        let got = geom_aabb(&r).unwrap();
        for i in 0..4 {
            assert!((got[i] - to[i]).abs() < 1e-6, "{got:?} vs {to:?}");
        }
        // Uniform scale keeps the angle and the axis ratio.
        let to = [
            from[0],
            from[1],
            from[0] + (from[2] - from[0]) * 3.0,
            from[1] + (from[3] - from[1]) * 3.0,
        ];
        let r = map_box(&g, from, to).unwrap();
        assert!((r.rot - 0.5).abs() < 1e-9 && (r.w / r.h - 2.5).abs() < 1e-9);
    }

    #[test]
    fn rotated_rect_resizes_as_a_polygon() {
        let g = ShapeGeom {
            kind: "rect".into(),
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
            rot: 0.3,
            v: Vec::new(),
        };
        let from = geom_aabb(&g).unwrap();
        let r = map_box(&g, from, [0.0, 0.0, 300.0, 300.0]).unwrap();
        assert_eq!(r.kind, "poly");
        assert_eq!(r.v.len(), 8);
        let b = geom_aabb(&r).unwrap();
        assert!((b[2] - 300.0).abs() < 1e-6 && (b[3] - 300.0).abs() < 1e-6);
    }

    #[test]
    fn scale_rotate_about_a_pivot() {
        let g = shape_geom(ShapeKind::Rect, [-50.0, -25.0], [50.0, 25.0]);
        let r = scale_rotate(&g, [0.0, 0.0], 2.0, 0.25);
        assert_eq!((r.w, r.h, r.rot), (200.0, 100.0, 0.25));
        assert!((center(&r)[0]).abs() < 1e-9);
        let line = shape_geom(ShapeKind::Line, [0.0, 0.0], [10.0, 0.0]);
        let l = scale_rotate(&line, [0.0, 0.0], 3.0, std::f64::consts::FRAC_PI_2);
        assert!((l.w).abs() < 1e-9 && (l.h - 30.0).abs() < 1e-9);
    }

    #[test]
    fn anchors_and_geometry_come_from_the_stored_shape() {
        let line = shape_geom(ShapeKind::Arrow, [0.0, 0.0], [100.0, 0.0]);
        let names: Vec<&str> = anchors(&line).iter().map(|a| a.name).collect();
        assert_eq!(names, ["start", "end", "mid"]);
        let r = shape_geom(ShapeKind::Rect, [0.0, 0.0], [100.0, 50.0]);
        let an = anchors(&r);
        assert_eq!(an.len(), 1 + 4 + 4);
        assert!(an.contains(&Anchor {
            name: "e",
            p: [100.0, 25.0]
        }));
        assert_eq!(anchors(&tri()).len(), 1 + 3 * 2);
        assert_eq!(
            to_geometry(&line),
            Some(Geometry::Segment {
                a: [0.0, 0.0],
                b: [100.0, 0.0]
            })
        );
        let Some(Geometry::Polygon { pts }) = to_geometry(&tri()) else {
            panic!()
        };
        assert_eq!(pts[0], [100.0, 0.0]);
    }

    #[test]
    fn timing_spreads_the_stroke_duration_by_arc_length() {
        let p = geom_points(&tri(), 4.0, 0.6).unwrap();
        let t = with_timing(p, 900);
        assert_eq!(t[0].dt, 0);
        assert_eq!(t[t.len() - 1].dt, 900);
        assert!(t.windows(2).all(|w| w[0].dt <= w[1].dt));
    }
}
