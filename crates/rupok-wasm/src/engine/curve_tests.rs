//! Sparse pointer samples (a mouse, a touch screen, a smartboard that
//! reports as a mouse: one sample a frame) come out as a curve, the same
//! wet and dry, and everything that reads the stored stroke sees that curve
//! (`rupok_ink::curve`, docs/performance.md "The wet/dry contract").

use super::smart::{SmartInk, SmartMode};
use super::tests::{MockSink, engine};
use super::*;

const C: [f32; 2] = [400.0, 300.0];
const R: f32 = 62.0;

/// Sample `i` of a fast hand going round a circle: 40 px from the last one.
fn on_circle(i: i32) -> [f32; 2] {
    let a = i as f32 * 2.0 * (20.0 / R).asin();
    [C[0] + R * a.cos(), C[1] + R * a.sin()]
}

/// Pen down, then one sample per pointer event every 29 ms (34 Hz), as
/// WebKitGTK delivers a mouse. The pen stays down.
fn sparse_down(e: &mut Engine<MockSink>, kind: PointerKind, n: i32) {
    let p = on_circle(0);
    e.pointer_down(1, p[0], p[1], 0.5, 0.0, kind, 0, 0.0);
    for i in 1..n {
        let p = on_circle(i);
        let t = f64::from(i) * 29.0;
        e.pointer_move(1, &[f64::from(p[0]), f64::from(p[1]), 0.5, t], &[], t);
        // The ink always reaches the pointer.
        let tip = e.sink.wet[0].points.last().unwrap();
        assert!((tip.x - p[0]).hypot(tip.y - p[1]) < 0.02, "sample {i}");
    }
}

/// How far the polyline strays from the circle (points and piece middles).
fn off_circle(pts: &[StrokePoint]) -> f32 {
    let at = |x: f32, y: f32| ((x - C[0]).hypot(y - C[1]) - R).abs();
    pts.windows(2)
        .map(|w| at(w[1].x, w[1].y).max(at((w[0].x + w[1].x) / 2.0, (w[0].y + w[1].y) / 2.0)))
        .fold(0.0, f32::max)
}

fn stored(e: &Engine<MockSink>) -> Vec<StrokeData> {
    e.scene
        .ordered()
        .iter()
        .filter_map(|i| match &i.data {
            ItemData::Stroke(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_mouse_at_34_hz_draws_a_curve_wet_and_stores_that_curve() {
    let mut e = engine();
    e.set_smoothing(0.0);
    sparse_down(&mut e, PointerKind::Mouse, 9);
    let wet = e.sink.wet[0].points.clone();
    // Nine samples, 37° apart: joined straight they cut 3.3 px inside the
    // circle. The wet ink follows it.
    assert!(wet.len() > 40, "{} wet points from 9 samples", wet.len());
    assert!(
        off_circle(&wet) < 0.3,
        "wet ink {} px off",
        off_circle(&wet)
    );
    e.pointer_up(1, 300.0);
    let s = stored(&e);
    assert_eq!(s.len(), 1);
    let dry = &s[0].points;
    assert!(off_circle(dry) < 0.5, "dry ink {} px off", off_circle(dry));
    assert_eq!(dry.first(), wet.first());
    assert_eq!(dry.last(), wet.last(), "pen-up moves nothing");
    // Stored about as densely as a pen would have left it, not as the
    // 60-odd wet points.
    assert!(
        dry.len() < wet.len() && dry.len() >= 18,
        "{} stored",
        dry.len()
    );
    // Times still run forward, for replay.
    assert!(dry.windows(2).all(|w| w[1].dt >= w[0].dt));
    assert_eq!(dry.last().unwrap().dt, 232);
}

#[test]
fn straight_joins_are_still_there_for_diagnostics() {
    let mut e = engine();
    e.set_smoothing(0.0);
    e.set_smooth_ink(false);
    assert!(!e.smooth_ink());
    sparse_down(&mut e, PointerKind::Mouse, 9);
    assert_eq!(
        e.sink.wet[0].points.len(),
        9,
        "the samples and nothing else"
    );
    assert!(off_circle(&e.sink.wet[0].points) > 3.0);
    e.pointer_up(1, 300.0);
    assert_eq!(stored(&e)[0].points.len(), 9);
}

#[test]
fn the_eraser_and_the_picker_follow_the_curve_not_the_chords() {
    let mut e = engine();
    e.set_smoothing(0.0);
    e.set_width(3.0);
    sparse_down(&mut e, PointerKind::Mouse, 9);
    e.pointer_up(1, 300.0);
    // Half way between samples 3 and 4: on the circle, and on the chord.
    let (p, q) = (on_circle(3), on_circle(4));
    let mid = [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    let out = {
        let (dx, dy) = (mid[0] - C[0], mid[1] - C[1]);
        let d = dx.hypot(dy);
        [C[0] + dx / d * R, C[1] + dy / d * R]
    };
    assert!((out[0] - mid[0]).hypot(out[1] - mid[1]) > 3.0);
    assert!(
        e.scene.hit_test(out[0], out[1], 0.5).is_some(),
        "on the ink"
    );
    assert!(
        e.scene.hit_test(mid[0], mid[1], 0.5).is_none(),
        "where the straight line used to be there is no ink"
    );
    // Rub a small eraser across the ink there: the stroke is cut in two.
    e.set_tool(ToolKind::Eraser);
    e.set_width(4.0);
    e.pointer_down(2, out[0], out[1], 0.5, 400.0, PointerKind::Mouse, 0, 400.0);
    e.pointer_move(
        2,
        &[f64::from(out[0] + 0.5), f64::from(out[1] + 0.5), 0.5, 410.0],
        &[],
        410.0,
    );
    e.pointer_up(2, 420.0);
    let pieces = stored(&e);
    assert_eq!(pieces.len(), 2, "cut where the curve is");
    // What is left is still the same curve.
    for s in &pieces {
        assert!(off_circle(&s.points) < 0.5, "{}", off_circle(&s.points));
    }
}

#[test]
fn smart_ink_reads_the_samples_and_the_shape_replaces_the_curve() {
    // A square from a mouse: a sample at each corner and one mid-side.
    let mut e = Engine::new(MockSink::default(), "Smart", 0.0);
    e.set_view(1600.0, 900.0);
    e.set_smart_ink(SmartInk {
        mode: SmartMode::Auto,
        ..Default::default()
    });
    let pts: [[f32; 2]; 9] = [
        [300.0, 300.0],
        [400.0, 301.0],
        [500.0, 300.0],
        [501.0, 400.0],
        [500.0, 500.0],
        [400.0, 499.0],
        [300.0, 500.0],
        [299.0, 400.0],
        [300.0, 303.0],
    ];
    e.pointer_down(
        1,
        pts[0][0],
        pts[0][1],
        0.5,
        0.0,
        PointerKind::Mouse,
        0,
        0.0,
    );
    let mut t = 0.0;
    for p in &pts[1..] {
        t += 29.0;
        e.pointer_move(1, &[f64::from(p[0]), f64::from(p[1]), 0.5, t], &[], t);
    }
    e.pointer_up(1, t);
    let s = stored(&e);
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].shape.as_deref(), Some("square"), "{:?}", s[0].shape);
}

#[test]
fn the_laser_trail_is_a_curve_too() {
    let mut e = engine();
    e.set_tool(ToolKind::Laser);
    sparse_down(&mut e, PointerKind::Mouse, 9);
    let trail = &e.sink.wet[0].points;
    assert!(trail.len() > 40, "{}", trail.len());
    assert!(off_circle(trail) < 0.3, "{}", off_circle(trail));
}

#[test]
fn a_pen_at_240_hz_stores_what_it_stored_before() {
    // Dense samples: the curve adds nothing, so the stroke is byte for byte
    // the one straight joins give.
    let run = |smooth: bool| {
        let mut e = engine();
        e.set_smooth_ink(smooth);
        e.pointer_down(1, 100.0, 300.0, 0.4, 0.0, PointerKind::Pen, 0, 0.0);
        for i in 1..400 {
            let x = 100.0 + f64::from(i) * 0.6;
            let y = 300.0 + (x * 0.05).sin() * 45.0;
            let t = f64::from(i) * 4.17;
            e.pointer_move(1, &[x, y, 0.4 + 0.2 * (x * 0.01).sin(), t], &[], t);
        }
        e.pointer_up(1, 1700.0);
        stored(&e)[0].points.clone()
    };
    assert_eq!(run(true), run(false));
}

/// Largest distance of the points from the outline of the rect
/// (100, 100)-(400, 300).
fn off_rect(pts: &[StrokePoint]) -> f32 {
    pts.iter()
        .map(|p| {
            let dx = (p.x - 100.0).abs().min((p.x - 400.0).abs());
            let dy = (p.y - 100.0).abs().min((p.y - 300.0).abs());
            let inside_x = (100.0..=400.0).contains(&p.x);
            let inside_y = (100.0..=300.0).contains(&p.y);
            match (inside_x, inside_y) {
                (true, true) => dx.min(dy),
                (true, false) => dy,
                (false, true) => dx,
                (false, false) => dx.hypot(dy),
            }
        })
        .fold(0.0, f32::max)
}

#[test]
fn only_a_pointer_writing_is_curved() {
    // Ink an agent draws: the points it gave, corners and all.
    let mut e = engine();
    let given = [
        [100.0, 100.0],
        [220.0, 100.0],
        [220.0, 190.0],
        [330.0, 250.0],
    ];
    crate::live::apply(
        &mut e,
        r#"{"op":"draw_stroke","points":[[100,100],[220,100],[220,190],[330,250]]}"#,
        "Claude",
        1_000.0,
    )
    .unwrap();
    let s = stored(&e);
    assert_eq!(s.len(), 1);
    let xy: Vec<[f32; 2]> = s[0]
        .points
        .iter()
        .map(|p| [p.x + s[0].offset[0], p.y + s[0].offset[1]])
        .collect();
    assert_eq!(xy, given, "an agent's polyline is stored as given");

    // A shape: its outline, with square corners, and the same with the
    // curve switched off.
    let rect = |smooth: bool| {
        let mut e = engine();
        e.set_smooth_ink(smooth);
        e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
        e.pointer_down(1, 100.0, 100.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
        for i in 1..=8 {
            let t = f64::from(i) * 29.0;
            let k = f64::from(i) / 8.0;
            e.pointer_move(1, &[100.0 + 300.0 * k, 100.0 + 200.0 * k, 0.5, t], &[], t);
        }
        // The preview is the shape too, not a curve through its corners.
        assert!(off_rect(&e.sink.wet[0].points) < 0.01);
        e.pointer_up(1, 300.0);
        e
    };
    let mut e = rect(true);
    let shape = stored(&e);
    assert_eq!(shape[0].shape.as_deref(), Some("rect"));
    assert!(
        off_rect(&shape[0].points) < 0.01,
        "{}",
        off_rect(&shape[0].points)
    );
    assert_eq!(shape[0].points, stored(&rect(false))[0].points);

    // Cut by the partial eraser, its pieces are still on the outline: the
    // corners they keep stay square.
    e.set_tool(ToolKind::Eraser);
    e.set_width(10.0);
    e.pointer_down(2, 250.0, 90.0, 0.5, 400.0, PointerKind::Mouse, 0, 400.0);
    e.pointer_move(2, &[250.0, 110.0, 0.5, 410.0], &[], 410.0);
    e.pointer_up(2, 420.0);
    let pieces = stored(&e);
    assert!(!pieces.is_empty() && pieces.iter().all(|p| p.shape.is_none()));
    let corners = pieces
        .iter()
        .flat_map(|p| &p.points)
        .filter(|q| (q.x == 100.0 || q.x == 400.0) && (q.y == 100.0 || q.y == 300.0))
        .count();
    assert!(corners >= 4, "{corners} corner points left");
    for p in &pieces {
        assert!(off_rect(&p.points) < 0.01, "{}", off_rect(&p.points));
    }

    // And a lesson opened again draws exactly what was saved.
    let bytes = e.save(500.0).unwrap();
    let mut f = engine();
    f.load(&bytes).unwrap();
    assert_eq!(stored(&f), pieces);
}
