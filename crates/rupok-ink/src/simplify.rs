//! Ramer–Douglas–Peucker simplification that also respects pressure, so tapered
//! strokes keep their shape while straight runs shed redundant samples.

use crate::StrokePoint;

/// Simplify `points` with spatial tolerance `epsilon` (world units) and
/// pressure tolerance `p_epsilon` (0..1). Endpoints are always kept.
pub fn simplify(points: &[StrokePoint], epsilon: f32, p_epsilon: f32) -> Vec<StrokePoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    // Iterative stack instead of recursion: long strokes must not blow the
    // (small) WASM stack.
    let mut stack = vec![(0usize, points.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (points[a], points[b]);
        let mut worst = 0.0f32;
        let mut worst_i = 0usize;
        for (i, p) in points.iter().enumerate().take(b).skip(a + 1) {
            let d = seg_dist(p.x, p.y, pa.x, pa.y, pb.x, pb.y);
            // Pressure deviation from linear interpolation along the chord.
            let t = (i - a) as f32 / (b - a) as f32;
            let p_lin = pa.p + (pb.p - pa.p) * t;
            let dp = (p.p - p_lin).abs();
            let score = (d / epsilon.max(1e-6)).max(dp / p_epsilon.max(1e-6));
            if score > worst {
                worst = score;
                worst_i = i;
            }
        }
        if worst > 1.0 {
            keep[worst_i] = true;
            stack.push((a, worst_i));
            stack.push((worst_i, b));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

#[inline]
fn seg_dist(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let (vx, vy) = (bx - ax, by - ay);
    let len2 = vx * vx + vy * vy;
    if len2 <= f32::EPSILON {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = (((px - ax) * vx + (py - ay) * vy) / len2).clamp(0.0, 1.0);
    let (cx, cy) = (ax + t * vx, ay + t * vy);
    ((px - cx).powi(2) + (py - cy).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32, p: f32) -> StrokePoint {
        StrokePoint { x, y, p, dt: 0 }
    }

    #[test]
    fn straight_line_collapses_to_endpoints() {
        let pts: Vec<_> = (0..100).map(|i| pt(i as f32, 0.0, 0.5)).collect();
        let s = simplify(&pts, 0.25, 0.05);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn corner_is_kept() {
        let mut pts: Vec<_> = (0..50).map(|i| pt(i as f32, 0.0, 0.5)).collect();
        pts.extend((1..50).map(|i| pt(49.0, i as f32, 0.5)));
        let s = simplify(&pts, 0.25, 0.05);
        assert_eq!(s.len(), 3);
        assert_eq!((s[1].x, s[1].y), (49.0, 0.0));
    }

    #[test]
    fn pressure_ramp_detail_is_kept() {
        // Straight line, but pressure spikes in the middle.
        let pts: Vec<_> = (0..100)
            .map(|i| pt(i as f32, 0.0, if (45..55).contains(&i) { 1.0 } else { 0.2 }))
            .collect();
        let s = simplify(&pts, 0.25, 0.05);
        assert!(s.len() > 2);
    }
}
