//! Pen styles (brushes), disappearing magic ink and classroom stamps in
//! the engine: what is stored, what survives editing, and what is never
//! saved.

use super::tests::{MockSink, draw, engine};
use super::*;

fn strokes(e: &Engine<MockSink>) -> Vec<StrokeData> {
    e.lesson
        .items(&e.page.clone())
        .unwrap()
        .into_iter()
        .filter_map(|i| match i.data {
            ItemData::Stroke(s) => Some(s),
            _ => None,
        })
        .collect()
}

#[test]
fn every_stored_brush_is_saved_on_the_stroke_and_reloads() {
    let mut e = engine();
    let stored: Vec<Brush> = Brush::ALL
        .into_iter()
        .filter(|b| *b != Brush::Magic)
        .collect();
    for (i, b) in stored.iter().enumerate() {
        e.set_brush(*b);
        let y = 60.0 + i as f32 * 40.0;
        draw(&mut e, 1, PointerKind::Pen, [100.0, y], [500.0, y + 10.0]);
    }
    let s = strokes(&e);
    assert_eq!(s.len(), stored.len());
    for (st, b) in s.iter().zip(&stored) {
        assert_eq!(st.brush(), *b);
        assert_eq!(st.brush.as_deref(), b.key(), "plain pen stores no key");
        // The renderer draws it with the same brush.
        assert!(e.sink.strokes.values().any(|r| r.brush == st.brush));
    }
    // Patterned brushes keep every wet point; the others are simplified.
    let dashed = &s[stored.iter().position(|b| *b == Brush::Dashed).unwrap()];
    let ball = &s[stored.iter().position(|b| *b == Brush::Ballpoint).unwrap()];
    assert!(dashed.points.len() > ball.points.len());
    let bytes = e.save(10.0).unwrap();
    let mut f = engine();
    f.load(&bytes).unwrap();
    assert_eq!(strokes(&f), s, "brushes survive save and open");
}

#[test]
fn each_tool_remembers_its_own_brush() {
    let mut e = engine();
    e.set_brush(Brush::Fountain);
    e.set_tool(ToolKind::Highlighter);
    assert_eq!(e.brush(), Brush::Pen);
    e.set_brush(Brush::Marker);
    // Magic ink is for the pen only.
    e.set_brush(Brush::Magic);
    assert_eq!(e.brush(), Brush::Pen);
    e.set_tool(ToolKind::Shape(crate::shapes::ShapeKind::Rect));
    e.set_brush(Brush::Dashed);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [300.0, 200.0]);
    e.set_tool(ToolKind::Pen);
    assert_eq!(e.brush(), Brush::Fountain);
    let s = strokes(&e);
    assert_eq!(s[0].shape.as_deref(), Some("rect"));
    assert_eq!(s[0].brush.as_deref(), Some("dashed"));
}

#[test]
fn magic_ink_shows_like_its_wet_frame_then_fades_and_is_never_saved() {
    let mut e = engine();
    e.set_brush(Brush::Magic);
    e.pointer_down(1, 100.0, 100.0, 0.5, 0.0, PointerKind::Pen, 0, 0.0);
    let s: Vec<f64> = (1..=20)
        .flat_map(|i| [100.0 + f64::from(i) * 10.0, 100.0, 0.5, f64::from(i) * 8.0])
        .collect();
    e.pointer_move(1, &s, &[], 160.0);
    let wet = e.sink.wet.clone();
    assert_eq!(wet.len(), 1);
    assert_eq!(wet[0].brush, Brush::Magic);
    e.pointer_up(1, 200.0);
    assert!(strokes(&e).is_empty(), "not in the document");
    assert!(e.sink.strokes.is_empty(), "not dry ink");
    assert!(e.has_animation());
    // Pen-up changes nothing on screen.
    assert_eq!(e.sink.wet, wet);
    e.tick(200.0 + MAGIC_HOLD_MS);
    assert_eq!(e.sink.wet[0].opacity, wet[0].opacity, "held in full");
    e.tick(200.0 + MAGIC_HOLD_MS + MAGIC_FADE_MS / 2.0);
    let k = e.sink.wet[0].opacity / wet[0].opacity;
    assert!(k > 0.1 && k < 0.5, "fading: {k}");
    e.tick(200.0 + MAGIC_HOLD_MS + MAGIC_FADE_MS + 1.0);
    assert!(e.sink.wet.is_empty(), "gone");
    assert!(!e.has_animation());
    assert!(!e.can_undo_for_tests(), "nothing to undo");
}

#[test]
fn restyle_changes_the_brush_of_selected_ink_in_one_step() {
    let mut e = engine();
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    e.set_brush(Brush::Chalk);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 200.0], [400.0, 200.0]);
    e.set_tool(ToolKind::Lasso);
    e.select_all();
    let meta: serde_json::Value = serde_json::from_str(&e.selection_meta_json()).unwrap();
    assert!(meta["style"]["brush"].is_null(), "mixed");
    assert!(e.restyle_selection(None, None, None, Some(Brush::Neon)));
    assert!(strokes(&e).iter().all(|s| s.brush() == Brush::Neon));
    let meta: serde_json::Value = serde_json::from_str(&e.selection_meta_json()).unwrap();
    assert_eq!(meta["style"]["brush"], "neon");
    // Back to the plain pen removes the key; magic is not a stored style.
    assert!(e.restyle_selection(None, None, None, Some(Brush::Pen)));
    assert!(strokes(&e).iter().all(|s| s.brush.is_none()));
    assert!(!e.restyle_selection(None, None, None, Some(Brush::Magic)));
    assert!(e.undo());
    assert!(strokes(&e).iter().all(|s| s.brush() == Brush::Neon));
}

#[test]
fn erasing_part_of_a_brush_stroke_keeps_the_brush_on_the_pieces() {
    let mut e = engine();
    e.set_brush(Brush::Dotted);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [500.0, 100.0]);
    e.set_tool(ToolKind::Eraser);
    e.set_width(40.0);
    draw(&mut e, 2, PointerKind::Pen, [300.0, 60.0], [300.0, 140.0]);
    let s = strokes(&e);
    assert_eq!(s.len(), 2, "cut in two");
    assert!(s.iter().all(|p| p.brush() == Brush::Dotted));
}

#[test]
fn copy_paste_keeps_the_brush() {
    let mut e = engine();
    e.set_brush(Brush::Rainbow);
    draw(&mut e, 1, PointerKind::Pen, [100.0, 100.0], [400.0, 100.0]);
    let ids: Vec<ItemId> = e.scene.ordered().iter().map(|i| i.id.clone()).collect();
    let json = e.export_items_json(&ids);
    let mut f = engine();
    f.import_items_json(&json, 0.0, 0.0, 1.0).unwrap();
    assert_eq!(strokes(&f)[0].brush.as_deref(), Some("rainbow"));
}

#[test]
fn stamps_are_text_items_that_resize_rotate_recolour_and_reload() {
    let mut e = engine();
    let id = e
        .add_stamp("star", "⭐", 300.0, 200.0, 120.0, 120.0, 0xf5b301ff, 5.0)
        .unwrap();
    assert!(e.selection().is_empty(), "tap, tap, tap");
    let items: Vec<serde_json::Value> = serde_json::from_str(&e.dom_items_json()).unwrap();
    assert_eq!(items[0]["stamp"], "star");
    assert_eq!(items[0]["text"], "⭐");
    let g = BoxGeom {
        x: 10.0,
        y: 20.0,
        w: 200.0,
        h: 200.0,
        rot: 0.5,
    };
    e.set_stamp_geom(&id, g);
    e.set_text_color(&id, 0xe5484dff);
    let bytes = e.save(10.0).unwrap();
    let mut f = engine();
    f.load(&bytes).unwrap();
    let it = f.scene.get(&id).unwrap();
    let ItemData::Text(t) = &it.data else {
        panic!("a text item")
    };
    assert_eq!(t.stamp.as_deref(), Some("star"));
    assert_eq!(t.geom, g);
    assert_eq!(t.color, 0xe5484dff);
    assert!(
        (t.size - 140.0).abs() < 1e-6,
        "fallback text follows the size"
    );
    // Replay shows it at the time it was placed.
    assert_eq!(it.created, 5.0);
    // Bad input is refused.
    assert!(e.add_stamp("", "x", 0.0, 0.0, 10.0, 10.0, 0, 0.0).is_none());
    assert!(
        e.add_stamp("star", "x", 0.0, 0.0, f64::NAN, 10.0, 0, 0.0)
            .is_none()
    );
}

#[test]
fn stamps_stay_on_a_slide_page() {
    let mut e = engine();
    e.set_camera(-500.0, -500.0, 1.0);
    // Screen (10, 10) is world (-490, -490): far off the page.
    let id = e
        .add_stamp("tick", "✅", 10.0, 10.0, 100.0, 100.0, 0x30a46cff, 0.0)
        .unwrap();
    let ItemData::Text(t) = &e.scene.get(&id).unwrap().data else {
        panic!()
    };
    assert!(t.geom.x >= 0.0 && t.geom.y >= 0.0, "{:?}", t.geom);
}
