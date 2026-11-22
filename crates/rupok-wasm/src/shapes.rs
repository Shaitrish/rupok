//! Shape tools produce ordinary strokes (tagged with `shape`), so shapes get
//! the same renderer, eraser, lasso and replay behaviour as ink for free.

use rupok_ink::StrokePoint;

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

fn pt(x: f32, y: f32) -> StrokePoint {
    StrokePoint {
        x,
        y,
        p: 0.6,
        dt: 0,
    }
}

/// Points for a shape dragged from `a` to `b` (world units). `width` sizes the
/// arrow head.
pub fn shape_points(kind: ShapeKind, a: [f32; 2], b: [f32; 2], width: f32) -> Vec<StrokePoint> {
    match kind {
        ShapeKind::Line => vec![pt(a[0], a[1]), pt(b[0], b[1])],
        ShapeKind::Rect => vec![
            pt(a[0], a[1]),
            pt(b[0], a[1]),
            pt(b[0], b[1]),
            pt(a[0], b[1]),
            pt(a[0], a[1]),
        ],
        ShapeKind::Ellipse => {
            let (cx, cy) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
            let (rx, ry) = (((b[0] - a[0]) / 2.0).abs(), ((b[1] - a[1]) / 2.0).abs());
            let n = ((rx + ry).sqrt() * 6.0).clamp(24.0, 160.0) as usize;
            (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    pt(cx + rx * t.cos(), cy + ry * t.sin())
                })
                .collect()
        }
        ShapeKind::Arrow => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len = (dx * dx + dy * dy).sqrt().max(1e-3);
            let (ux, uy) = (dx / len, dy / len);
            let head = (width * 4.0 + 10.0).min(len * 0.45);
            let wing = |s: f32| {
                let (c, sn) = (0.5f32.cos(), 0.5f32.sin() * s);
                // rotate (-u) by ±~28°
                let (rx, ry) = (-ux * c + uy * sn, -uy * c - ux * sn);
                pt(b[0] + rx * head, b[1] + ry * head)
            };
            vec![
                pt(a[0], a[1]),
                pt(b[0], b[1]),
                wing(1.0),
                pt(b[0], b[1]),
                wing(-1.0),
            ]
        }
    }
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
}
