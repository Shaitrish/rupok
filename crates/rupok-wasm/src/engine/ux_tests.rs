//! Board usability on a smartboard: the partial eraser (`duster.rs`), ink
//! kept on the page (`clip.rs`), selection editing (`restyle.rs`), and
//! finger-friendly selection.

use super::tests::{MockSink, draw, engine, select_rect};
use super::*;

fn items(e: &Engine<MockSink>) -> Vec<Item> {
    e.lesson.items(&e.page.clone()).unwrap()
}

fn strokes(e: &Engine<MockSink>) -> Vec<StrokeData> {
    items(e)
        .into_iter()
        .filter_map(|i| match i.data {
            ItemData::Stroke(s) => Some(s),
            _ => None,
        })
        .collect()
}

fn xs(s: &StrokeData) -> (f32, f32) {
    let lo = s
        .points
        .iter()
        .map(|p| p.x + s.offset[0])
        .fold(f32::MAX, f32::min);
    let hi = s
        .points
        .iter()
        .map(|p| p.x + s.offset[0])
        .fold(f32::MIN, f32::max);
    (lo, hi)
}

/// Look at the page from (-200, -200), so screen (100, 100) is off the page.
fn off_page_view(e: &mut Engine<MockSink>) {
    e.set_camera(-200.0, -200.0, 1.0);
}

// ------------------------------------------------------------- eraser

#[test]
fn partial_eraser_splits_a_stroke_in_one_undo_step() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    let before = items(&e);
    assert_eq!(before.len(), 1);
    e.set_tool(ToolKind::Eraser);
    e.set_width(40.0);
    draw(&mut e, 2, PointerKind::Pen, [250.0, 50.0], [250.0, 150.0]);
    let after = items(&e);
    assert_eq!(after.len(), 2, "the stroke is cut in two");
    assert_eq!(e.sink.strokes.len(), 2);
    assert!(
        e.sink.strokes.keys().all(|k| !k.starts_with("~cut")),
        "temporary ids are gone"
    );
    // The pieces keep the stroke's place in the stacking order.
    assert!(after.iter().all(|i| i.z == before[0].z));
    let (l, r) = (xs(&strokes(&e)[0]), xs(&strokes(&e)[1]));
    let (left, right) = if l.0 < r.0 { (l, r) } else { (r, l) };
    assert!(left.1 < 232.0 && right.0 > 268.0, "{left:?} {right:?}");
    assert!(e.undo(), "one undo");
    assert_eq!(items(&e), before, "brings the whole stroke back");
    assert_eq!(e.sink.strokes.len(), 1);
}

#[test]
fn a_rub_that_cuts_a_piece_again_is_still_one_undo_step() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [500.0, 100.0]);
    e.set_tool(ToolKind::Eraser);
    e.set_width(30.0);
    // One gesture crossing the stroke three times.
    e.pointer_down(2, 200.0, 50.0, 0.5, 0.0, PointerKind::Pen, 0, 0.0);
    e.pointer_move(
        2,
        &[200.0, 150.0, 0.5, 1.0, 300.0, 150.0, 0.5, 2.0],
        &[],
        1.0,
    );
    e.pointer_move(2, &[300.0, 50.0, 0.5, 3.0, 400.0, 50.0, 0.5, 4.0], &[], 2.0);
    e.pointer_move(2, &[400.0, 150.0, 0.5, 5.0], &[], 3.0);
    e.pointer_up(2, 4.0);
    assert_eq!(items(&e).len(), 4);
    assert!(e.undo());
    assert_eq!(items(&e).len(), 1, "the whole rub was one step");
}

#[test]
fn a_finger_erases_with_the_eraser_tool_even_after_a_pen() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    let cam = e.camera;
    e.set_tool(ToolKind::Eraser);
    e.set_width(40.0);
    draw(&mut e, 2, PointerKind::Touch, [250.0, 50.0], [250.0, 150.0]);
    assert_eq!(items(&e).len(), 2, "the finger rubbed the stroke");
    assert_eq!(e.camera, cam, "and did not pan");
    // With the pen tool, fingers still pan once a pen was seen.
    e.set_tool(ToolKind::Pen);
    draw(
        &mut e,
        3,
        PointerKind::Touch,
        [100.0, 400.0],
        [300.0, 400.0],
    );
    assert_eq!(items(&e).len(), 2);
}

#[test]
fn palm_duster_rubs_out_part_of_a_stroke() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [600.0, 100.0]);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 400.0], [600.0, 400.0]);
    assert_eq!(e.area_erase([300.0, 60.0], [300.0, 140.0], 40.0), 1);
    assert_eq!(
        e.area_erase([300.0, 140.0], [320.0, 60.0], 40.0),
        0,
        "the same stroke"
    );
    assert_eq!(e.area_erase_end(), 1);
    assert_eq!(items(&e).len(), 3);
    assert!(e.undo());
    assert_eq!(items(&e).len(), 2);
}

#[test]
fn stroke_eraser_still_erases_whole_strokes() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    e.set_tool(ToolKind::Eraser);
    e.set_eraser_mode(EraserMode::parse("stroke"));
    assert_eq!(e.eraser_mode().as_str(), "stroke");
    draw(&mut e, 2, PointerKind::Pen, [250.0, 50.0], [250.0, 150.0]);
    assert!(items(&e).is_empty());
}

// ------------------------------------------------------------- page clip

#[test]
fn a_stroke_that_starts_off_the_page_draws_nothing() {
    let mut e = engine();
    off_page_view(&mut e);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 300.0], [400.0, 300.0]);
    assert!(items(&e).is_empty());
    assert!(e.sink.wet.is_empty());
    // Shapes too.
    e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
    draw(&mut e, 1, PointerKind::Pen, [100.0, 300.0], [400.0, 400.0]);
    assert!(items(&e).is_empty());
}

#[test]
fn a_stroke_crossing_the_edge_is_cut_there_wet_and_dry() {
    let mut e = engine();
    off_page_view(&mut e);
    // From world (200, 100) to world (-100, 100).
    e.pointer_down(1, 400.0, 300.0, 0.5, 0.0, PointerKind::Pen, 0, 0.0);
    let mut s = Vec::new();
    for i in 1..=20 {
        s.extend_from_slice(&[400.0 - 15.0 * f64::from(i), 300.0, 0.5, f64::from(i) * 8.0]);
    }
    e.pointer_move(1, &s, &[], 1.0);
    let wet: Vec<&StrokePoint> = e.sink.wet.iter().flat_map(|w| &w.points).collect();
    assert!(!wet.is_empty());
    assert!(wet.iter().all(|p| p.x >= 0.0), "wet ink stops at the edge");
    e.pointer_up(1, 2.0);
    let st = strokes(&e);
    assert_eq!(st.len(), 1);
    let (lo, hi) = xs(&st[0]);
    assert!((0.0..=3.0).contains(&lo), "ends at the edge: {lo}");
    assert!(hi > 190.0);
}

#[test]
fn a_stroke_that_leaves_and_comes_back_is_two_strokes_in_one_undo_step() {
    let mut e = engine();
    off_page_view(&mut e);
    // world (100, 50) → (100, -50) → (300, -50) → (300, 50)
    let path: [[f64; 2]; 4] = [
        [300.0, 250.0],
        [300.0, 150.0],
        [500.0, 150.0],
        [500.0, 250.0],
    ];
    e.pointer_down(1, 300.0, 250.0, 0.5, 0.0, PointerKind::Pen, 0, 0.0);
    let mut s = Vec::new();
    for (i, w) in path.windows(2).enumerate() {
        for k in 1..=10 {
            let t = k as f64 / 10.0;
            s.extend_from_slice(&[
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
                0.5,
                (i * 10 + k) as f64 * 8.0,
            ]);
        }
    }
    e.pointer_move(1, &s, &[], 1.0);
    e.pointer_up(1, 2.0);
    assert_eq!(strokes(&e).len(), 2);
    assert!(e.undo());
    assert!(items(&e).is_empty(), "both pieces were one undo step");
}

#[test]
fn clipping_can_be_turned_off_and_infinite_pages_are_never_clipped() {
    let mut e = engine();
    e.set_clip_ink(false);
    assert!(!e.clip_ink());
    off_page_view(&mut e);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 300.0], [400.0, 300.0]);
    assert_eq!(items(&e).len(), 1);
    e.set_clip_ink(true);
    let id = e
        .lesson
        .add_page(
            None,
            PageSpec {
                frame: None,
                ..Default::default()
            },
        )
        .unwrap();
    let i = e.lesson.pages().iter().position(|p| p.id == id).unwrap();
    e.goto_page(i);
    off_page_view(&mut e);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 300.0], [400.0, 300.0]);
    assert_eq!(items(&e).len(), 1, "an infinite board takes ink anywhere");
}

#[test]
fn shapes_moves_and_text_stay_on_the_page() {
    let mut e = engine();
    off_page_view(&mut e);
    e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Line));
    // From world (100, 100) to world (-150, 100): ends at the edge.
    draw(&mut e, 1, PointerKind::Pen, [300.0, 300.0], [50.0, 300.0]);
    let st = strokes(&e);
    assert_eq!(st.len(), 1);
    assert!(xs(&st[0]).0 >= 1.0, "{:?}", xs(&st[0]));
    // Dragging the line far left stops at the edge.
    e.set_tool(ToolKind::Lasso);
    e.select(vec![items(&e)[0].id.clone()]);
    e.pointer_down(2, 250.0, 300.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
    e.pointer_move(2, &[0.0, 300.0, 0.5, 1.0], &[], 1.0);
    e.pointer_up(2, 2.0);
    let b = e.selection_bounds().unwrap();
    assert!(b[0] >= -0.01 && b[0] < 1.0, "{b:?}");
    // Text placed off the page lands on it.
    let t = e.add_text(10.0, 10.0, "Hi", 40.0, 0.0).unwrap();
    let ItemData::Text(td) = e.lesson.item(&e.page.clone(), &t).unwrap().data else {
        panic!()
    };
    assert!(td.geom.x >= 0.0 && td.geom.y >= 0.0);
}

// ------------------------------------------------------------- selection

#[test]
fn a_finger_tap_near_handwriting_selects_it() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    e.set_tool(ToolKind::Lasso);
    // 14 px away: too far for a mouse, close enough for a finger.
    e.pointer_down(2, 250.0, 114.0, 0.5, 0.0, PointerKind::Mouse, 0, 0.0);
    e.pointer_up(2, 1.0);
    assert!(e.selection().is_empty());
    e.pointer_down(3, 250.0, 114.0, 0.5, 0.0, PointerKind::Touch, 0, 0.0);
    e.pointer_up(3, 1.0);
    assert_eq!(e.selection().len(), 1);
}

#[test]
fn recolour_and_rethicken_ink_and_text_in_one_undo_step() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    e.set_tool(ToolKind::Highlighter);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 200.0], [400.0, 200.0]);
    let t = e.add_text(100.0, 300.0, "Text", 40.0, 0.0).unwrap();
    e.set_tool(ToolKind::Lasso);
    e.select_all();
    let before = items(&e);
    assert!(e.restyle_selection(Some(0xe5484dff), None, Some(2.0)));
    for it in items(&e) {
        match it.data {
            ItemData::Stroke(s) => {
                assert_eq!(s.color, 0xe5484dff);
                let old = before.iter().find(|b| b.id == it.id).unwrap();
                let ItemData::Stroke(o) = &old.data else {
                    panic!()
                };
                assert!((s.width - o.width * 2.0).abs() < 1e-3);
                // A highlighter keeps its own see-through opacity.
                assert_eq!(s.opacity, o.opacity);
            }
            ItemData::Text(td) => assert_eq!(td.color, 0xe5484dff, "{t}"),
            _ => {}
        }
    }
    let meta: serde_json::Value = serde_json::from_str(&e.selection_meta_json()).unwrap();
    assert_eq!(meta["style"]["color"], "#e5484dff");
    assert!(
        !e.restyle_selection(Some(0xe5484dff), None, None),
        "no change, no step"
    );
    assert!(e.undo());
    assert_eq!(items(&e), before, "one undo restores colour and width");
    // An absolute width.
    e.select_all();
    assert!(e.restyle_selection(None, Some(9.0), None));
    assert!(strokes(&e).iter().all(|s| s.width == 9.0));
}

#[test]
fn shapes_keep_their_geometry_when_rethickened_and_scaled() {
    let mut e = engine();
    e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [300.0, 200.0]);
    let id = items(&e)[0].id.clone();
    e.set_tool(ToolKind::Lasso);
    e.select(vec![id.clone()]);
    let g0 = strokes(&e)[0].geom.clone().unwrap();
    assert!(e.restyle_selection(None, None, Some(3.0)));
    let s = &strokes(&e)[0];
    assert_eq!(s.shape.as_deref(), Some("rect"));
    let g1 = s.geom.clone().unwrap();
    assert!((g1.w - g0.w).abs() < 1e-6 && (g1.h - g0.h).abs() < 1e-6);
    assert!(e.scale_selection_by(1.5));
    let s = &strokes(&e)[0];
    let g2 = s.geom.clone().unwrap();
    assert_eq!(s.shape.as_deref(), Some("rect"));
    assert!((g2.w - g0.w * 1.5).abs() < 1.0, "{} {}", g2.w, g0.w);
}

#[test]
fn scaling_handwriting_and_text_is_one_undo_step_and_previews_without_editing() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [300.0, 140.0]);
    let t = e.add_text(400.0, 100.0, "abc", 40.0, 0.0).unwrap();
    e.set_tool(ToolKind::Lasso);
    select_rect(&mut e, [50.0, 50.0], [1100.0, 400.0]);
    assert_eq!(e.selection().len(), 2);
    let before = items(&e);
    let b = e.selection_bounds().unwrap();
    let to = [
        f64::from(b[0]),
        f64::from(b[1]),
        f64::from(b[0] + (b[2] - b[0]) * 2.0),
        f64::from(b[1] + (b[3] - b[1]) * 2.0),
    ];
    // A preview draws the scaled ink but leaves the document alone.
    assert!(e.scale_selection(to, false));
    assert_eq!(items(&e), before);
    e.scale_cancel();
    assert!(e.scale_selection(to, true));
    let ItemData::Text(td) = e.lesson.item(&e.page.clone(), &t).unwrap().data else {
        panic!()
    };
    assert!((td.size - 80.0).abs() < 0.5, "{}", td.size);
    let nb = e.selection_bounds().unwrap();
    assert!(((nb[2] - nb[0]) - (b[2] - b[0]) * 2.0).abs() < 4.0);
    assert!(e.undo());
    assert_eq!(items(&e), before);
    // Shrinking and the size buttons.
    e.select_all();
    assert!(e.scale_selection_by(0.8));
    assert!(!e.scale_selection_by(f64::NAN));
}
