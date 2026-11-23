//! Synthetic hand-drawn shapes (wobble, jitter, rounded corners, overshoot,
//! gaps, rotations, sizes) that must snap, and letters, words and scribbles
//! that must not.

use super::*;

/// Deterministic LCG so the "hand" is the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

fn densify(path: &[P], step: f32) -> Vec<P> {
    let mut out = vec![path[0]];
    for w in path.windows(2) {
        let d = dist(w[0], w[1]);
        let n = (d / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            out.push([
                w[0][0] + t * (w[1][0] - w[0][0]),
                w[0][1] + t * (w[1][1] - w[0][1]),
            ]);
        }
    }
    out
}

/// A hand-drawn version of `path`: rounded corners (moving average, like
/// the pen filter), slow wobble across the path (`wobble` × size) and pen
/// jitter (`jitter` px).
fn hand(path: &[P], rng: &mut Rng, wobble: f32, jitter: f32) -> Vec<P> {
    let d = densify(path, 2.0);
    let b = bbox(&d);
    let size = (b[2] - b[0]).hypot(b[3] - b[1]);
    let k = 3usize;
    let sm: Vec<P> = (0..d.len())
        .map(|i| {
            let lo = i.saturating_sub(k);
            let hi = (i + k).min(d.len() - 1);
            let n = (hi - lo + 1) as f32;
            let s = d[lo..=hi]
                .iter()
                .fold([0.0, 0.0], |s, q| [s[0] + q[0], s[1] + q[1]]);
            [s[0] / n, s[1] / n]
        })
        .collect();
    let (f1, f2) = (rng.range(1.5, 3.0), rng.range(4.0, 7.0));
    let (p1, p2) = (rng.range(0.0, TAU), rng.range(0.0, TAU));
    let n = sm.len();
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let (a, c) = (sm[i.saturating_sub(1)], sm[(i + 1).min(n - 1)]);
            let tn = norm([c[0] - a[0], c[1] - a[1]]);
            let off =
                size * wobble * (0.6 * (TAU * f1 * t + p1).sin() + 0.4 * (TAU * f2 * t + p2).sin());
            let j = [rng.range(-jitter, jitter), rng.range(-jitter, jitter)];
            [sm[i][0] - tn[1] * off + j[0], sm[i][1] + tn[0] * off + j[1]]
        })
        .collect()
}

fn rotate(path: &[P], c: P, a: f32) -> Vec<P> {
    let (s, co) = a.sin_cos();
    path.iter()
        .map(|q| {
            let (dx, dy) = (q[0] - c[0], q[1] - c[1]);
            [c[0] + dx * co - dy * s, c[1] + dx * s + dy * co]
        })
        .collect()
}

fn ellipse_path(c: P, rx: f32, ry: f32, rot: f32, start: f32, turns: f32) -> Vec<P> {
    let n = 200;
    let pts: Vec<P> = (0..=n)
        .map(|i| {
            let a = start + TAU * turns * i as f32 / n as f32;
            [c[0] + rx * a.cos(), c[1] + ry * a.sin()]
        })
        .collect();
    rotate(&pts, c, rot)
}

/// A closed polygon drawn from its first vertex, overshooting a little.
fn poly_path(v: &[P], overshoot: f32) -> Vec<P> {
    let mut p = v.to_vec();
    p.push(v[0]);
    p.push([
        v[0][0] + overshoot * (v[1][0] - v[0][0]),
        v[0][1] + overshoot * (v[1][1] - v[0][1]),
    ]);
    p
}

fn regular_verts(c: P, r: f32, k: usize, phase: f32) -> Vec<P> {
    (0..k)
        .map(|i| {
            let a = phase + TAU * i as f32 / k as f32;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

fn rec(p: &[P]) -> Option<Recognized> {
    recognize_xy(p, Options::default())
}

fn kind(p: &[P]) -> Option<ShapeKind> {
    rec(p).map(|r| r.kind)
}

fn poly_pts(r: &Recognized) -> Vec<P> {
    match &r.geometry {
        Geometry::Polygon { pts } => pts.clone(),
        g => panic!("not a polygon: {g:?}"),
    }
}

fn sp(p: &[P]) -> Vec<StrokePoint> {
    p.iter()
        .enumerate()
        .map(|(i, q)| StrokePoint {
            x: q[0],
            y: q[1],
            p: 0.5,
            dt: i as u32 * 8,
        })
        .collect()
}

// ------------------------------------------------------------- positives

#[test]
fn circles_snap_at_several_sizes() {
    let mut rng = Rng(1);
    for d in [110.0f32, 250.0, 600.0, 1200.0] {
        for seed in 0..4 {
            let start = seed as f32 * 1.3;
            let p = hand(
                &ellipse_path([700.0, 500.0], d / 2.0, d / 2.0 * 0.96, 0.0, start, 1.04),
                &mut rng,
                0.012,
                1.0,
            );
            let r = rec(&p).unwrap_or_else(|| panic!("circle d={d} seed={seed}"));
            assert_eq!(r.kind, ShapeKind::Circle, "d={d}");
            let Geometry::Ellipse { c, rx, ry, .. } = r.geometry else {
                panic!()
            };
            assert_eq!(rx, ry);
            assert!(dist(c, [700.0, 500.0]) < 0.06 * d, "centre {c:?}");
            assert!((rx - d * 0.49).abs() < 0.08 * d, "radius {rx} for d={d}");
        }
    }
}

#[test]
fn circles_with_overshoot_or_a_small_gap_snap() {
    let mut rng = Rng(2);
    for turns in [0.9, 0.95, 1.1, 1.2] {
        let p = hand(
            &ellipse_path([400.0, 400.0], 150.0, 145.0, 0.0, 0.7, turns),
            &mut rng,
            0.01,
            1.0,
        );
        assert_eq!(kind(&p), Some(ShapeKind::Circle), "turns={turns}");
    }
}

#[test]
fn size_gate_is_in_screen_pixels() {
    let mut rng = Rng(3);
    // 70 world units across: too small at 100 %, a shape at 200 %.
    let p = hand(
        &ellipse_path([100.0, 100.0], 35.0, 34.0, 0.0, 0.0, 1.03),
        &mut rng,
        0.01,
        0.4,
    );
    assert_eq!(kind(&p), None);
    let zoomed = Options {
        px_per_unit: 2.0,
        deliberate: false,
    };
    assert_eq!(
        recognize_xy(&p, zoomed).map(|r| r.kind),
        Some(ShapeKind::Circle)
    );
    // Hold-to-snap accepts it at 100 %.
    let held = Options {
        px_per_unit: 1.0,
        deliberate: true,
    };
    assert_eq!(
        recognize_xy(&p, held).map(|r| r.kind),
        Some(ShapeKind::Circle)
    );
}

#[test]
fn ellipses_axis_aligned_upright_and_rotated() {
    let mut rng = Rng(4);
    let p = hand(
        &ellipse_path([500.0, 300.0], 170.0, 80.0, 0.06, 0.3, 1.05),
        &mut rng,
        0.01,
        1.0,
    );
    let r = rec(&p).expect("wide ellipse");
    assert_eq!(r.kind, ShapeKind::Ellipse);
    let Geometry::Ellipse { rx, ry, rot, .. } = r.geometry else {
        panic!()
    };
    assert_eq!(rot, 0.0, "3.4° snaps to axis-aligned");
    assert!(rx > 150.0 && ry < 95.0, "{rx} {ry}");

    let p = hand(
        &ellipse_path([500.0, 300.0], 70.0, 160.0, 0.0, 2.0, 1.04),
        &mut rng,
        0.01,
        1.0,
    );
    let r = rec(&p).expect("upright ellipse");
    let Geometry::Ellipse { rx, ry, rot, .. } = r.geometry else {
        panic!()
    };
    assert_eq!(rot, 0.0);
    assert!(ry > rx, "upright: {rx} {ry}");

    let p = hand(
        &ellipse_path([500.0, 300.0], 170.0, 70.0, 30.0 * DEG, 1.0, 1.04),
        &mut rng,
        0.01,
        1.0,
    );
    let r = rec(&p).expect("tilted ellipse");
    assert_eq!(r.kind, ShapeKind::Ellipse);
    let Geometry::Ellipse { rot, .. } = r.geometry else {
        panic!()
    };
    assert!((rot - 30.0 * DEG).abs() < 5.0 * DEG, "rot {}", rot / DEG);
}

#[test]
fn rectangles_snap_axis_aligned_when_close() {
    let mut rng = Rng(5);
    for (tilt, seed) in [(0.0f32, 0), (4.0, 1), (-6.5, 2)] {
        let v = [
            [300.0, 200.0],
            [620.0, 200.0],
            [620.0, 390.0],
            [300.0, 390.0],
        ];
        let p = rotate(&poly_path(&v, 0.08), [460.0, 295.0], tilt * DEG);
        let p = hand(&p, &mut rng, 0.006, 1.0);
        let r = rec(&p).unwrap_or_else(|| panic!("rect tilt={tilt} seed={seed}"));
        assert_eq!(r.kind, ShapeKind::Rectangle);
        let Geometry::Rect { c, w, h, rot } = r.geometry else {
            panic!()
        };
        assert_eq!(rot, 0.0, "tilt {tilt}° snaps");
        assert!(
            (w - 320.0).abs() < 25.0 && (h - 190.0).abs() < 25.0,
            "{w}×{h}"
        );
        assert!(dist(c, [460.0, 295.0]) < 12.0);
    }
}

#[test]
fn rotated_rectangles_keep_their_angle() {
    let mut rng = Rng(6);
    for tilt in [20.0f32, 30.0, -25.0] {
        let v = [
            [300.0, 200.0],
            [640.0, 200.0],
            [640.0, 380.0],
            [300.0, 380.0],
        ];
        let p = hand(
            &rotate(&poly_path(&v, 0.06), [470.0, 290.0], tilt * DEG),
            &mut rng,
            0.006,
            1.0,
        );
        let r = rec(&p).unwrap_or_else(|| panic!("tilted rect {tilt}"));
        assert_eq!(r.kind, ShapeKind::Rectangle, "tilt {tilt}");
        let Geometry::Rect { rot, .. } = r.geometry else {
            panic!()
        };
        // The rotation is defined modulo 90°.
        assert!(
            axis_offset(rot - tilt * DEG) < 4.0 * DEG,
            "rot {}",
            rot / DEG
        );
    }
}

#[test]
fn squares_become_squares() {
    let mut rng = Rng(7);
    for size in [120.0f32, 240.0, 500.0] {
        let v = [
            [100.0, 100.0],
            [100.0 + size * 1.04, 100.0],
            [100.0 + size * 1.04, 100.0 + size],
            [100.0, 100.0 + size],
        ];
        let p = hand(&poly_path(&v, 0.1), &mut rng, 0.006, 1.0);
        let r = rec(&p).unwrap_or_else(|| panic!("square {size}"));
        assert_eq!(r.kind, ShapeKind::Square, "size {size}");
        let Geometry::Rect { w, h, .. } = r.geometry else {
            panic!()
        };
        assert_eq!(w, h);
    }
}

#[test]
fn equilateral_triangles_get_a_flat_base() {
    let mut rng = Rng(8);
    for (size, tilt) in [(180.0f32, 4.0f32), (400.0, -7.0)] {
        let v = regular_verts([500.0, 400.0], size / 3f32.sqrt(), 3, -PI / 2.0);
        let p = hand(
            &rotate(&poly_path(&v, 0.07), [500.0, 400.0], tilt * DEG),
            &mut rng,
            0.006,
            1.0,
        );
        let r = rec(&p).unwrap_or_else(|| panic!("triangle {size}"));
        assert_eq!(r.kind, ShapeKind::Triangle);
        let t = poly_pts(&r);
        let s: Vec<f32> = (0..3).map(|i| dist(t[i], t[(i + 1) % 3])).collect();
        assert!((s[0] - s[1]).abs() < 0.01 * size && (s[1] - s[2]).abs() < 0.01 * size);
        // One side exactly horizontal, apex up.
        let flat = (0..3).any(|i| (t[i][1] - t[(i + 1) % 3][1]).abs() < 0.01);
        assert!(flat, "{t:?}");
        let top = t.iter().map(|q| q[1]).fold(f32::INFINITY, f32::min);
        assert!(top < 400.0 - size * 0.4);
    }
}

#[test]
fn right_triangles_get_a_true_right_angle() {
    let mut rng = Rng(9);
    let v = [[200.0, 200.0], [200.0, 500.0], [560.0, 500.0]];
    let p = hand(
        &rotate(&poly_path(&v, 0.05), [300.0, 400.0], 5.0 * DEG),
        &mut rng,
        0.005,
        1.0,
    );
    let r = rec(&p).expect("right triangle");
    assert_eq!(r.kind, ShapeKind::Triangle);
    let t = poly_pts(&r);
    let right = (0..3).find(|&i| {
        let (a, b, c) = (t[i], t[(i + 1) % 3], t[(i + 2) % 3]);
        (angle_between([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]) - PI / 2.0).abs()
            < 0.2 * DEG
    });
    let i = right.expect("a 90° corner");
    // Legs snapped to the axes.
    let (a, b) = (t[i], t[(i + 1) % 3]);
    assert!(
        (a[0] - b[0]).abs() < 0.5 || (a[1] - b[1]).abs() < 0.5,
        "{t:?}"
    );
}

#[test]
fn isosceles_and_scalene_triangles() {
    let mut rng = Rng(10);
    // Tall isosceles (apex 45°): equal sides become exactly equal.
    let v = [[400.0, 100.0], [550.0, 460.0], [250.0, 460.0]];
    let p = hand(&poly_path(&v, 0.06), &mut rng, 0.005, 1.0);
    let r = rec(&p).expect("isosceles");
    assert_eq!(r.kind, ShapeKind::Triangle);
    let t = poly_pts(&r);
    let s: Vec<f32> = (0..3).map(|i| dist(t[i], t[(i + 1) % 3])).collect();
    let mut sorted = s.clone();
    sorted.sort_by(f32::total_cmp);
    assert!(
        (sorted[1] - sorted[2]).abs() < 0.5 || (sorted[0] - sorted[1]).abs() < 0.5,
        "{s:?}"
    );
    // Scalene: straightened, vertices where they were drawn.
    let v = [[200.0, 150.0], [620.0, 260.0], [330.0, 470.0]];
    let p = hand(&poly_path(&v, 0.05), &mut rng, 0.004, 1.0);
    let r = rec(&p).expect("scalene");
    assert_eq!(r.kind, ShapeKind::Triangle);
    for q in poly_pts(&r) {
        let near = v.iter().map(|w| dist(*w, q)).fold(f32::INFINITY, f32::min);
        assert!(near < 25.0, "{q:?}");
    }
}

#[test]
fn diamonds() {
    let mut rng = Rng(11);
    let v = [
        [400.0, 100.0],
        [560.0, 300.0],
        [400.0, 500.0],
        [240.0, 300.0],
    ];
    let p = hand(&poly_path(&v, 0.06), &mut rng, 0.005, 1.0);
    let r = rec(&p).expect("diamond");
    assert_eq!(r.kind, ShapeKind::Diamond);
    let d = poly_pts(&r);
    assert_eq!(d.len(), 4);
    assert!((d[0][0] - d[2][0]).abs() < 0.01, "top and bottom aligned");
    assert!((d[1][1] - d[3][1]).abs() < 0.01, "left and right aligned");
}

#[test]
fn pentagons_and_hexagons_become_regular() {
    let mut rng = Rng(12);
    for (k, want) in [(5, ShapeKind::Pentagon), (6, ShapeKind::Hexagon)] {
        for (size, phase) in [(160.0f32, 0.1f32), (320.0, 0.5)] {
            let v = regular_verts([500.0, 400.0], size, k, phase);
            // Uneven hand: radii vary by a few percent.
            let v: Vec<P> = v
                .iter()
                .map(|q| {
                    let s = rng.range(0.94, 1.06);
                    [500.0 + (q[0] - 500.0) * s, 400.0 + (q[1] - 400.0) * s]
                })
                .collect();
            let p = hand(&poly_path(&v, 0.06), &mut rng, 0.004, 1.0);
            let r = rec(&p).unwrap_or_else(|| panic!("{want:?} {size}"));
            assert_eq!(r.kind, want, "size {size}");
            let pts = poly_pts(&r);
            assert_eq!(pts.len(), k);
            let s: Vec<f32> = (0..k).map(|i| dist(pts[i], pts[(i + 1) % k])).collect();
            assert!(s.iter().all(|x| (x - s[0]).abs() < 0.01 * size), "{s:?}");
        }
    }
}

#[test]
fn hexagon_snaps_to_flat_top_when_close() {
    let mut rng = Rng(13);
    let v = regular_verts([500.0, 400.0], 200.0, 6, 4.0 * DEG);
    let p = hand(&poly_path(&v, 0.05), &mut rng, 0.004, 1.0);
    let r = rec(&p).expect("hexagon");
    let pts = poly_pts(&r);
    // Two vertices share the top y (a flat top edge).
    let top = pts.iter().map(|q| q[1]).fold(f32::INFINITY, f32::min);
    assert_eq!(
        pts.iter().filter(|q| (q[1] - top).abs() < 0.05).count(),
        2,
        "{pts:?}"
    );
}

#[test]
fn stars_as_pentagram_or_outline() {
    let mut rng = Rng(14);
    let tips = regular_verts([500.0, 400.0], 220.0, 5, -PI / 2.0);
    let order = [0, 2, 4, 1, 3];
    let gram: Vec<P> = order.iter().map(|&i| tips[i]).collect();
    let p = hand(&poly_path(&gram, 0.04), &mut rng, 0.003, 1.0);
    let r = rec(&p).expect("pentagram");
    assert_eq!(r.kind, ShapeKind::Star);
    assert_eq!(poly_pts(&r).len(), 5);

    let mut outline = Vec::new();
    for i in 0..5 {
        let a = -PI / 2.0 + TAU * i as f32 / 5.0;
        outline.push([500.0 + 230.0 * a.cos(), 400.0 + 230.0 * a.sin()]);
        let b = a + TAU / 10.0;
        outline.push([500.0 + 95.0 * b.cos(), 400.0 + 95.0 * b.sin()]);
    }
    let p = hand(&poly_path(&outline, 0.05), &mut rng, 0.002, 0.8);
    let r = rec(&p).expect("star outline");
    assert_eq!(r.kind, ShapeKind::Star);
    let pts = poly_pts(&r);
    assert_eq!(pts.len(), 10);
    // A tip points straight up.
    let top = pts.iter().map(|q| q[1]).fold(f32::INFINITY, f32::min);
    let tip = pts.iter().find(|q| q[1] == top).unwrap();
    assert!((tip[0] - 500.0).abs() < 8.0, "{tip:?}");
}

#[test]
fn shapes_survive_any_rotation() {
    let mut rng = Rng(15);
    for deg in [0.0f32, 17.0, 33.0, 51.0, 72.0] {
        let tri = regular_verts([500.0, 400.0], 200.0, 3, deg * DEG);
        let p = hand(&poly_path(&tri, 0.05), &mut rng, 0.005, 1.0);
        assert_eq!(kind(&p), Some(ShapeKind::Triangle), "triangle at {deg}°");
        let hex = regular_verts([500.0, 400.0], 200.0, 6, deg * DEG);
        let p = hand(&poly_path(&hex, 0.05), &mut rng, 0.004, 1.0);
        assert_eq!(kind(&p), Some(ShapeKind::Hexagon), "hexagon at {deg}°");
        let rect = [
            [300.0, 250.0],
            [700.0, 250.0],
            [700.0, 470.0],
            [300.0, 470.0],
        ];
        let p = hand(
            &rotate(&poly_path(&rect, 0.05), [500.0, 360.0], deg * DEG),
            &mut rng,
            0.005,
            1.0,
        );
        let k = kind(&p);
        assert!(
            matches!(k, Some(ShapeKind::Rectangle) | Some(ShapeKind::Diamond)),
            "rect at {deg}°: {k:?}"
        );
    }
}

#[test]
fn lines_snap_to_0_45_90_degrees() {
    let mut rng = Rng(16);
    for (deg, want) in [
        (1.7f32, 0.0f32),
        (-5.0, 0.0),
        (41.0, 45.0),
        (93.0, 90.0),
        (-133.0, -135.0),
    ] {
        let a = [400.0f32, 400.0];
        let l = 350.0;
        let b = [a[0] + l * (deg * DEG).cos(), a[1] + l * (deg * DEG).sin()];
        let p = hand(&[a, b], &mut rng, 0.004, 0.8);
        let r = rec(&p).unwrap_or_else(|| panic!("line {deg}"));
        assert_eq!(r.kind, ShapeKind::Line);
        let Geometry::Segment { a, b } = r.geometry else {
            panic!()
        };
        let got = angle([b[0] - a[0], b[1] - a[1]]) / DEG;
        assert!((got - want).abs() < 0.01, "{deg}° → {got}°");
        assert!((dist(a, b) - l).abs() < 0.06 * l);
    }
    // 25° is no snap angle: kept.
    let p = hand(
        &[
            [100.0, 100.0],
            [
                100.0 + 300.0 * (25.0 * DEG).cos(),
                100.0 + 300.0 * (25.0 * DEG).sin(),
            ],
        ],
        &mut rng,
        0.003,
        0.8,
    );
    let r = rec(&p).expect("free line");
    let Geometry::Segment { a, b } = r.geometry else {
        panic!()
    };
    assert!((angle([b[0] - a[0], b[1] - a[1]]) / DEG - 25.0).abs() < 1.5);
}

#[test]
fn one_stroke_arrows_shaft_first_or_head_first() {
    let mut rng = Rng(17);
    let (tail, tip) = ([200.0f32, 300.0], [560.0f32, 300.0]);
    let w1 = [500.0, 260.0];
    let w2 = [500.0, 340.0];
    // Shaft, wing, back to the tip, other wing.
    let p = hand(&[tail, tip, w1, tip, w2], &mut rng, 0.003, 0.8);
    let r = rec(&p).expect("arrow");
    assert_eq!(r.kind, ShapeKind::Arrow);
    let Geometry::Segment { a, b } = r.geometry else {
        panic!()
    };
    assert!(dist(b, tip) < 15.0 && dist(a, tail) < 15.0, "{a:?} {b:?}");
    // Closed triangular head.
    let p = hand(&[tail, tip, w1, w2, tip], &mut rng, 0.003, 0.8);
    assert_eq!(kind(&p), Some(ShapeKind::Arrow));
    // Head first, then the shaft away from the tip.
    let p = hand(&[w1, tip, w2, tip, tail], &mut rng, 0.003, 0.8);
    let r = rec(&p).expect("head-first arrow");
    assert_eq!(r.kind, ShapeKind::Arrow);
    let Geometry::Segment { b, .. } = r.geometry else {
        panic!()
    };
    assert!(dist(b, tip) < 15.0, "tip {b:?}");
}

#[test]
fn a_v_after_a_line_is_an_arrow_head() {
    let mut rng = Rng(18);
    let (a, b) = ([100.0f32, 300.0], [500.0f32, 300.0]);
    let opts = Options::default();
    let v = hand(
        &[[460.0, 265.0], [502.0, 301.0], [458.0, 338.0]],
        &mut rng,
        0.01,
        0.6,
    );
    assert_eq!(arrow_head(a, b, &sp(&v), opts), Some(true));
    let v = hand(
        &[[140.0, 262.0], [99.0, 300.0], [142.0, 337.0]],
        &mut rng,
        0.01,
        0.6,
    );
    assert_eq!(arrow_head(a, b, &sp(&v), opts), Some(false));
    // Far from both ends.
    let v = hand(
        &[[290.0, 265.0], [330.0, 300.0], [290.0, 335.0]],
        &mut rng,
        0.01,
        0.6,
    );
    assert_eq!(arrow_head(a, b, &sp(&v), opts), None);
    // At the end but opening forwards (a "<" past the tip, not a head).
    let v = hand(
        &[[540.0, 265.0], [500.0, 300.0], [540.0, 335.0]],
        &mut rng,
        0.01,
        0.6,
    );
    assert_eq!(arrow_head(a, b, &sp(&v), opts), None);
    // Huge "V": a second line, not a head.
    let v = hand(
        &[[200.0, 0.0], [500.0, 300.0], [200.0, 600.0]],
        &mut rng,
        0.01,
        0.6,
    );
    assert_eq!(arrow_head(a, b, &sp(&v), opts), None);
}

// ------------------------------------------------------------- negatives

/// Letters as single strokes, in units of their height (origin top-left).
fn letter(name: &str, h: f32, at: P) -> Vec<P> {
    let s = |p: &[P]| -> Vec<P> {
        p.iter()
            .map(|q| [at[0] + q[0] * h, at[1] + q[1] * h])
            .collect()
    };
    let arc = |c: P, rx: f32, ry: f32, a0: f32, a1: f32| -> Vec<P> {
        (0..=60)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / 60.0;
                [c[0] + rx * a.cos(), c[1] + ry * a.sin()]
            })
            .collect()
    };
    match name {
        "o" => s(&arc([0.5, 0.5], 0.42, 0.45, -1.0, -1.0 - TAU * 1.08)),
        "0" => s(&arc([0.3, 0.5], 0.28, 0.5, -1.6, -1.6 - TAU * 1.04)),
        "D" => {
            let mut p = vec![[0.0, 0.0], [0.0, 1.0]];
            p.extend(arc([0.0, 0.5], 0.62, 0.5, PI / 2.0, -PI / 2.0));
            s(&p)
        }
        "a" => {
            let mut p = arc([0.38, 0.62], 0.3, 0.34, -0.5, -0.5 - TAU);
            p.extend([[0.68, 0.3], [0.68, 0.95], [0.78, 1.0]]);
            s(&p)
        }
        "৪" => s(&(0..=80)
            .map(|i| {
                let t = TAU * i as f32 / 80.0;
                [0.4 + 0.35 * (2.0 * t).sin(), 0.5 - 0.5 * t.cos()]
            })
            .collect::<Vec<_>>()),
        "ব" => {
            let mut p = vec![[0.15, 0.0]];
            p.extend(arc([0.85, 0.0], 0.7, 0.95, PI, PI / 2.0));
            p.extend([[0.85, 0.95], [0.85, 0.0], [-0.05, 0.0]]);
            s(&p)
        }
        "8" => s(&(0..=80)
            .map(|i| {
                let t = TAU * i as f32 / 80.0 + 0.3;
                [0.35 + 0.3 * (2.0 * t).sin(), 0.5 - 0.5 * t.cos()]
            })
            .collect::<Vec<_>>()),
        _ => unreachable!(),
    }
}

#[test]
fn letters_at_writing_size_never_snap() {
    let mut rng = Rng(19);
    for name in ["o", "0", "D", "a", "৪", "ব", "8"] {
        for h in [36.0f32, 55.0, 75.0, 90.0] {
            for _ in 0..3 {
                let p = hand(&letter(name, h, [300.0, 300.0]), &mut rng, 0.012, 0.8);
                let r = rec(&p);
                assert!(
                    r.is_none(),
                    "{name} at {h}px snapped to {:?}",
                    r.map(|r| r.kind)
                );
            }
        }
    }
}

#[test]
fn big_letters_are_still_not_shapes() {
    // Beyond the size gate, geometry alone must reject them: a `D` has two
    // sharp corners, `a` has a stem, `8` / `৪` turn both ways.
    let mut rng = Rng(20);
    for name in ["D", "a", "৪", "8"] {
        for h in [140.0f32, 220.0] {
            let p = hand(&letter(name, h, [300.0, 300.0]), &mut rng, 0.01, 0.8);
            let r = rec(&p);
            assert!(
                r.is_none(),
                "{name} at {h}px snapped to {:?}",
                r.map(|r| r.kind)
            );
        }
    }
}

#[test]
fn long_cursive_words_never_snap() {
    let mut rng = Rng(21);
    // Looped cursive ("lell…"): a prolate trochoid.
    for (w, loops) in [(300.0f32, 4), (600.0, 7), (900.0, 9)] {
        let n = 400;
        let p: Vec<P> = (0..=n)
            .map(|i| {
                let t = TAU * loops as f32 * i as f32 / n as f32;
                [
                    200.0 + w * i as f32 / n as f32 - 28.0 * t.sin(),
                    400.0 - 40.0 * t.cos(),
                ]
            })
            .collect();
        let p = hand(&p, &mut rng, 0.003, 0.8);
        assert_eq!(kind(&p), None, "cursive {w}");
    }
    // Humps ("mmmm") and a wave ("~~~~").
    let humps: Vec<P> = (0..=300)
        .map(|i| {
            let t = i as f32 / 300.0;
            [100.0 + 500.0 * t, 300.0 - 70.0 * (t * 6.0 * PI).sin().abs()]
        })
        .collect();
    assert_eq!(kind(&hand(&humps, &mut rng, 0.003, 0.8)), None);
    let wave: Vec<P> = (0..=300)
        .map(|i| {
            let t = i as f32 / 300.0;
            [100.0 + 500.0 * t, 300.0 - 25.0 * (t * 5.0 * TAU).sin()]
        })
        .collect();
    assert_eq!(kind(&hand(&wave, &mut rng, 0.003, 0.8)), None);
}

#[test]
fn scribbles_spirals_and_open_curves_never_snap() {
    let mut rng = Rng(22);
    for seed in 0..12 {
        let mut r2 = Rng(100 + seed);
        let mut p = vec![[400.0f32, 400.0f32]];
        let mut dir = r2.range(0.0, TAU);
        for _ in 0..40 {
            dir += r2.range(-1.6, 1.6);
            let last = p[p.len() - 1];
            p.push([last[0] + 18.0 * dir.cos(), last[1] + 18.0 * dir.sin()]);
        }
        let k = kind(&hand(&p, &mut rng, 0.0, 0.8));
        assert_eq!(k, None, "scribble {seed}");
    }
    let spiral: Vec<P> = (0..=300)
        .map(|i| {
            let t = i as f32 / 300.0 * 3.0 * TAU;
            let r = 20.0 + 15.0 * t;
            [500.0 + r * t.cos(), 400.0 + r * t.sin()]
        })
        .collect();
    assert_eq!(kind(&hand(&spiral, &mut rng, 0.002, 0.8)), None);
    // "C" and "U": open arcs are neither closed shapes nor lines.
    let c = ellipse_path([400.0, 400.0], 150.0, 150.0, 0.0, 0.8, 0.75);
    assert_eq!(kind(&hand(&c, &mut rng, 0.006, 0.8)), None);
    let u = ellipse_path([400.0, 300.0], 140.0, 200.0, 0.0, 0.0, 0.5);
    assert_eq!(kind(&hand(&u, &mut rng, 0.006, 0.8)), None);
}

#[test]
fn short_dashes_and_strokes_of_letters_never_snap() {
    let mut rng = Rng(23);
    for l in [20.0f32, 45.0, 70.0, 85.0] {
        let p = hand(&[[300.0, 300.0], [300.0 + l, 302.0]], &mut rng, 0.004, 0.6);
        assert_eq!(kind(&p), None, "dash {l}");
        let p = hand(&[[300.0, 300.0], [301.0, 300.0 + l]], &mut rng, 0.004, 0.6);
        assert_eq!(kind(&p), None, "stem {l}");
    }
    // A wobbly long stroke is not a line.
    let p: Vec<P> = (0..=100)
        .map(|i| {
            [
                100.0 + 4.0 * i as f32,
                300.0 + 30.0 * (i as f32 / 100.0 * TAU).sin(),
            ]
        })
        .collect();
    assert_eq!(kind(&p), None);
    // "L" and "V" shapes are not arrows.
    let p = hand(
        &[[200.0, 100.0], [200.0, 400.0], [400.0, 400.0]],
        &mut rng,
        0.003,
        0.6,
    );
    assert_eq!(kind(&p), None, "L");
    let p = hand(
        &[[200.0, 100.0], [300.0, 400.0], [400.0, 100.0]],
        &mut rng,
        0.003,
        0.6,
    );
    assert_eq!(kind(&p), None, "V");
}

#[test]
fn degenerate_input_is_none() {
    assert_eq!(rec(&[]), None);
    assert_eq!(rec(&[[1.0, 1.0]; 50]), None);
    let mut p: Vec<P> = (0..50).map(|i| [i as f32 * 10.0, 0.0]).collect();
    p[10] = [f32::NAN, 0.0];
    assert_eq!(rec(&p), None);
    assert_eq!(
        arrow_head([0.0, 0.0], [0.0, 0.0], &sp(&p), Options::default()),
        None
    );
    assert!(!is_scratch_out(&sp(&p), Options::default()));
}

// -------------------------------------------------------------- scratch

fn zigzag(c: P, along: f32, across: f32, passes: usize, vertical: bool) -> Vec<P> {
    (0..=passes)
        .map(|i| {
            let a = if i % 2 == 0 {
                -along / 2.0
            } else {
                along / 2.0
            };
            let b = -across / 2.0 + across * i as f32 / passes as f32;
            if vertical {
                [c[0] + b, c[1] + a]
            } else {
                [c[0] + a, c[1] + b]
            }
        })
        .collect()
}

#[test]
fn dense_zigzags_are_scratch_outs() {
    let mut rng = Rng(24);
    let opts = Options::default();
    let p = hand(
        &zigzag([400.0, 300.0], 220.0, 50.0, 8, false),
        &mut rng,
        0.004,
        0.8,
    );
    assert!(is_scratch_out(&sp(&p), opts), "horizontal passes");
    let p = hand(
        &zigzag([400.0, 300.0], 70.0, 160.0, 12, true),
        &mut rng,
        0.004,
        0.8,
    );
    assert!(
        is_scratch_out(&sp(&p), opts),
        "vertical passes over a wide word"
    );
    let p = rotate(
        &zigzag([400.0, 300.0], 150.0, 40.0, 9, false),
        [400.0, 300.0],
        0.4,
    );
    assert!(
        is_scratch_out(&sp(&hand(&p, &mut rng, 0.004, 0.8)), opts),
        "slanted"
    );
}

#[test]
fn writing_and_shapes_are_not_scratch_outs() {
    let mut rng = Rng(25);
    let opts = Options::default();
    // Few passes.
    let p = hand(
        &zigzag([400.0, 300.0], 200.0, 40.0, 4, false),
        &mut rng,
        0.004,
        0.8,
    );
    assert!(!is_scratch_out(&sp(&p), opts), "four passes");
    // "www": widely spaced passes.
    let p = hand(
        &zigzag([400.0, 300.0], 60.0, 240.0, 8, true),
        &mut rng,
        0.004,
        0.8,
    );
    assert!(!is_scratch_out(&sp(&p), opts), "www");
    // Rounded humps ("mmmm").
    let humps: Vec<P> = (0..=300)
        .map(|i| {
            let t = i as f32 / 300.0;
            [100.0 + 200.0 * t, 300.0 - 60.0 * (t * 8.0 * PI).sin().abs()]
        })
        .collect();
    assert!(
        !is_scratch_out(&sp(&hand(&humps, &mut rng, 0.002, 0.6)), opts),
        "mmmm"
    );
    // Circles, lines, a coil.
    let c = hand(
        &ellipse_path([300.0, 300.0], 100.0, 100.0, 0.0, 0.0, 1.0),
        &mut rng,
        0.01,
        0.8,
    );
    assert!(!is_scratch_out(&sp(&c), opts));
    let l = hand(&[[0.0, 0.0], [400.0, 10.0]], &mut rng, 0.004, 0.8);
    assert!(!is_scratch_out(&sp(&l), opts));
    let coil: Vec<P> = (0..=400)
        .map(|i| {
            let t = i as f32 / 400.0 * 8.0 * TAU;
            [
                100.0 + 2.0 * i as f32 / 4.0 + 30.0 * t.cos(),
                300.0 + 30.0 * t.sin(),
            ]
        })
        .collect();
    assert!(!is_scratch_out(&sp(&coil), opts), "coil");
}

/// Tuning harness: hit rates over many random hands. Run with
/// `cargo test -p rupok-ink stress_rates -- --ignored --nocapture`.
#[test]
#[ignore]
fn stress_rates() {
    let mut rng = Rng(77);
    let n = 200;
    let mut report =
        |name: &str, want: Option<ShapeKind>, make: &mut dyn FnMut(&mut Rng) -> Vec<P>| {
            let mut ok = 0;
            let mut got = std::collections::BTreeMap::new();
            for _ in 0..n {
                let p = make(&mut rng);
                let k = kind(&p);
                if k == want {
                    ok += 1;
                }
                *got.entry(format!("{k:?}")).or_insert(0) += 1;
            }
            eprintln!("{name:>14}: {:5.1}% {got:?}", 100.0 * ok as f32 / n as f32);
        };
    let wob = 0.014;
    report("circle", Some(ShapeKind::Circle), &mut |r| {
        let d = r.range(110.0, 700.0);
        let p = ellipse_path(
            [500.0, 400.0],
            d / 2.0,
            d / 2.0 * r.range(0.9, 1.0),
            r.range(0.0, PI),
            r.range(0.0, TAU),
            r.range(0.92, 1.15),
        );
        hand(&p, r, wob, 1.0)
    });
    report("ellipse", Some(ShapeKind::Ellipse), &mut |r| {
        let d = r.range(200.0, 700.0);
        let p = ellipse_path(
            [500.0, 400.0],
            d / 2.0,
            d / 2.0 * r.range(0.35, 0.75),
            r.range(0.0, PI),
            r.range(0.0, TAU),
            r.range(0.95, 1.12),
        );
        hand(&p, r, wob, 1.0)
    });
    report("rect", Some(ShapeKind::Rectangle), &mut |r| {
        let (w, h) = (r.range(200.0, 600.0), r.range(100.0, 180.0));
        let v = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
        let v = rotate(&v, [w / 2.0, h / 2.0], r.range(-6.0, 6.0) * DEG);
        hand(&poly_path(&v, r.range(-0.05, 0.12)), r, wob * 0.6, 1.0)
    });
    report("triangle", Some(ShapeKind::Triangle), &mut |r| {
        let v = regular_verts([400.0, 400.0], r.range(90.0, 300.0), 3, r.range(0.0, TAU));
        let v: Vec<P> = v
            .iter()
            .map(|q| [q[0] + r.range(-20.0, 20.0), q[1] + r.range(-20.0, 20.0)])
            .collect();
        hand(&poly_path(&v, r.range(-0.04, 0.1)), r, wob * 0.6, 1.0)
    });
    report("diamond", Some(ShapeKind::Diamond), &mut |r| {
        let (w, h) = (r.range(150.0, 400.0), r.range(150.0, 400.0));
        let v = [[w / 2.0, 0.0], [w, h / 2.0], [w / 2.0, h], [0.0, h / 2.0]];
        hand(&poly_path(&v, r.range(-0.04, 0.1)), r, wob * 0.6, 1.0)
    });
    report("pentagon", Some(ShapeKind::Pentagon), &mut |r| {
        let v = regular_verts([400.0, 400.0], r.range(90.0, 300.0), 5, r.range(0.0, TAU));
        hand(&poly_path(&v, r.range(-0.04, 0.1)), r, wob * 0.5, 1.0)
    });
    report("hexagon", Some(ShapeKind::Hexagon), &mut |r| {
        let v = regular_verts([400.0, 400.0], r.range(90.0, 300.0), 6, r.range(0.0, TAU));
        hand(&poly_path(&v, r.range(-0.04, 0.1)), r, wob * 0.5, 1.0)
    });
    report("pentagram", Some(ShapeKind::Star), &mut |r| {
        let t = regular_verts([400.0, 400.0], r.range(120.0, 300.0), 5, r.range(0.0, TAU));
        let v: Vec<P> = [0, 2, 4, 1, 3].iter().map(|&i| t[i]).collect();
        hand(&poly_path(&v, r.range(-0.03, 0.06)), r, wob * 0.4, 1.0)
    });
    report("line", Some(ShapeKind::Line), &mut |r| {
        let a = r.range(0.0, TAU);
        let l = r.range(100.0, 700.0);
        hand(
            &[[400.0, 400.0], [400.0 + l * a.cos(), 400.0 + l * a.sin()]],
            r,
            wob * 0.4,
            1.0,
        )
    });
    for name in ["o", "0", "D", "a", "৪", "ব", "8"] {
        report(name, None, &mut |r| {
            let h = r.range(30.0, 95.0);
            let p = letter(name, h, [300.0, 300.0]);
            let p = rotate(&p, [300.0, 300.0], r.range(-0.3, 0.3));
            let w = r.range(0.005, 0.03);
            hand(&p, r, w, 1.0)
        });
    }
    for name in ["D", "a", "৪", "8"] {
        report(&format!("big {name}"), None, &mut |r| {
            let h = r.range(100.0, 300.0);
            let w = r.range(0.005, 0.03);
            hand(&letter(name, h, [300.0, 300.0]), r, w, 1.0)
        });
    }
    report("scribble", None, &mut |r| {
        let mut p = vec![[400.0f32, 400.0f32]];
        let mut dir = r.range(0.0, TAU);
        let step = r.range(8.0, 30.0);
        for _ in 0..(r.range(15.0, 60.0) as usize) {
            dir += r.range(-1.6, 1.6);
            let last = p[p.len() - 1];
            p.push([last[0] + step * dir.cos(), last[1] + step * dir.sin()]);
        }
        hand(&p, r, 0.0, 1.0)
    });
}
