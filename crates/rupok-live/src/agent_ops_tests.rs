//! Tests of the agent operations on pages, items and stamps
//! (`items.rs`, `stamps.rs`): each write is one attributed undo step,
//! input is validated, Student view refuses writes, board-app ops are
//! refused by the document.

use rupok_core::{Item, ItemData, Lesson};
use rupok_format::Package;
use serde_json::{Value, json};

use crate::apply::{BoardCtx, LiveError, Outcome, apply};
use crate::op::{Access, Op};

struct Board {
    pkg: Package,
    lesson: Lesson,
    page: String,
    selection: Vec<String>,
    read_only: bool,
}

impl Board {
    fn new() -> Self {
        let lesson = Lesson::new("Agent", 1_000.0);
        let pkg = Package::from_lesson(&lesson, "test", 1_000.0);
        let page = lesson.page_id_at(0).unwrap();
        Self {
            pkg,
            lesson,
            page,
            selection: Vec::new(),
            read_only: false,
        }
    }
    fn run(&mut self, v: Value) -> Result<Outcome, LiveError> {
        let op: Op = serde_json::from_value(v).expect("op parses");
        let out = apply(
            BoardCtx {
                pkg: &mut self.pkg,
                lesson: &mut self.lesson,
                page_id: &self.page,
                selection: &self.selection,
                read_only: self.read_only,
            },
            &op,
            "Claude Code",
            9_000.0,
        )?;
        if let Some(i) = out.show_page {
            self.page = self.lesson.page_id_at(i).unwrap();
        }
        Ok(out)
    }
    fn ok(&mut self, v: Value) -> Value {
        match self.run(v.clone()) {
            Ok(o) => o.result,
            Err(e) => panic!("{v} failed: {e}"),
        }
    }
    fn err(&mut self, v: Value) -> String {
        match self.run(v.clone()) {
            Ok(o) => panic!("{v} should fail, got {}", o.result),
            Err(e) => e.to_string(),
        }
    }
    fn items(&self) -> Vec<Item> {
        self.lesson.items(&self.page).unwrap()
    }
    fn item(&self, id: &str) -> Item {
        self.lesson.item(&self.page, id).unwrap()
    }
    fn last_message(&self) -> Option<(String, String)> {
        let f = self.lesson.loro().oplog_frontiers();
        let id = f.iter().next()?;
        let m = self.lesson.loro().get_change(id)?.message?.to_string();
        crate::parse_commit_message(&m)
    }
    /// Run a write; it must be one undo step tagged with the agent and op.
    fn one_step(&mut self, v: Value) -> Value {
        let before = self.lesson.save();
        let name = v["op"].as_str().unwrap().to_string();
        let out = self.ok(v);
        assert_eq!(
            self.last_message(),
            Some(("Claude Code".into(), name.clone())),
            "{name} is attributed"
        );
        assert!(self.lesson.undo().unwrap(), "{name} can be undone");
        let undone = Lesson::load(&self.lesson.save()).unwrap();
        let orig = Lesson::load(&before).unwrap();
        assert_eq!(
            outline(&undone),
            outline(&orig),
            "one undo takes back all of {name}"
        );
        assert!(self.lesson.redo().unwrap());
        out
    }
    fn stroke(&mut self, pts: &[(f64, f64)]) -> String {
        let r = self.ok(json!({"op": "draw_stroke", "points": pts.iter().map(|(x, y)| vec![*x, *y]).collect::<Vec<_>>()}));
        r["item_ids"][0].as_str().unwrap().to_string()
    }
    fn text(&mut self, t: &str, x: f64, y: f64) -> String {
        let r = self.ok(json!({"op": "add_text", "text": t, "x": x, "y": y, "size": 40}));
        r["item_id"].as_str().unwrap().to_string()
    }
}

/// Everything visible about a lesson, to compare before/after an undo.
fn outline(l: &Lesson) -> Value {
    let pages: Vec<Value> = l
        .pages()
        .iter()
        .map(|p| {
            let items: Vec<Value> = l
                .items(&p.id)
                .unwrap()
                .iter()
                .map(|i| json!([i.id, format!("{:?}", i.data), i.group, i.z]))
                .collect();
            json!([p.id, p.title, items])
        })
        .collect();
    json!({ "pages": pages, "stamps": l.stamp_defs() })
}

#[test]
fn pages_list_move_delete_duplicate_rename() {
    let mut b = Board::new();
    for t in ["Two", "Three", "Four"] {
        b.ok(json!({"op": "add_page", "title": t}));
    }
    let l = b.ok(json!({"op": "list_pages"}));
    assert_eq!(l["page_count"], 4);
    assert_eq!(l["pages"][1]["title"], "Two");
    let ids: Vec<String> = b.lesson.pages().iter().map(|p| p.id.clone()).collect();

    // move pages 3 and 4 to the front
    let r = b.one_step(json!({"op": "move_pages", "pages": [3, 4], "to": 1}));
    assert_eq!(r["pages_now"], json!([1, 2]));
    let now: Vec<String> = b.lesson.pages().iter().map(|p| p.id.clone()).collect();
    assert_eq!(
        now,
        vec![
            ids[2].clone(),
            ids[3].clone(),
            ids[0].clone(),
            ids[1].clone()
        ]
    );
    // move one page to the end
    b.one_step(json!({"op": "move_pages", "pages": [1], "to": 4}));
    assert_eq!(b.lesson.pages()[3].id, ids[2]);
    assert!(
        b.err(json!({"op": "move_pages", "pages": [1], "to": 9}))
            .contains("between 1 and 4")
    );
    assert!(
        b.err(json!({"op": "move_pages", "pages": [], "to": 1}))
            .contains("at least one")
    );
    assert!(
        b.err(json!({"op": "move_pages", "pages": [7], "to": 1}))
            .contains("does not exist")
    );

    let r = b.one_step(json!({"op": "duplicate_pages", "pages": [2]}));
    assert_eq!(r["copies"], json!([3]));
    assert_eq!(b.lesson.page_count(), 5);

    b.one_step(json!({"op": "set_page_title", "page": 1, "title": "Intro"}));
    assert_eq!(b.lesson.pages()[0].title, "Intro");

    let r = b.one_step(json!({"op": "delete_pages", "pages": [2, 3]}));
    assert_eq!(r["deleted"], 2);
    assert!(
        b.err(json!({"op": "delete_pages", "pages": [1, 2, 3]}))
            .contains("at least one page")
    );
}

#[test]
fn deleting_the_page_on_screen_shows_a_neighbour() {
    let mut b = Board::new();
    b.ok(json!({"op": "add_page"}));
    b.ok(json!({"op": "go_to_page", "page": 2}));
    let out = b.run(json!({"op": "delete_pages", "pages": [2]})).unwrap();
    assert_eq!(out.show_page, Some(0));
}

#[test]
fn select_move_align_distribute() {
    let mut b = Board::new();
    let a = b.text("A", 100.0, 100.0);
    let c = b.text("B", 400.0, 160.0);
    let d = b.text("C", 900.0, 130.0);

    let out = b
        .run(json!({"op": "select_items", "item_ids": [a, c]}))
        .unwrap();
    assert_eq!(out.select, Some(vec![a.clone(), c.clone()]));
    assert!(!out.changed);
    assert_eq!(
        serde_json::from_value::<Op>(json!({"op": "select_items", "item_ids": []}))
            .unwrap()
            .access(),
        Access::Control
    );
    assert!(
        b.err(json!({"op": "select_items", "item_ids": ["nope"]}))
            .contains("not on page 1")
    );

    b.one_step(json!({"op": "move_items", "item_ids": [a], "dx": 10, "dy": 5}));
    let g = |b: &Board, id: &str| match b.item(id).data {
        ItemData::Text(t) => t.geom,
        _ => panic!(),
    };
    assert_eq!((g(&b, &a).x, g(&b, &a).y), (110.0, 105.0));
    b.one_step(json!({"op": "move_items", "item_ids": [a], "x": 50, "y": 60}));
    assert_eq!((g(&b, &a).x, g(&b, &a).y), (50.0, 60.0));
    assert!(
        b.err(json!({"op": "move_items", "item_ids": [a]}))
            .contains("dx/dy")
    );

    b.one_step(json!({"op": "align_items", "item_ids": [a, c, d], "align": "top"}));
    assert_eq!(g(&b, &a).y, g(&b, &c).y);
    assert_eq!(g(&b, &c).y, g(&b, &d).y);
    b.one_step(json!({"op": "align_items", "item_ids": [a, c, d], "distribute": "horizontal"}));
    let (ga, gc, gd) = (g(&b, &a), g(&b, &c), g(&b, &d));
    let gap1 = gc.x - (ga.x + ga.w);
    let gap2 = gd.x - (gc.x + gc.w);
    assert!((gap1 - gap2).abs() < 0.01, "{gap1} vs {gap2}");
    b.one_step(
        json!({"op": "align_items", "item_ids": [a], "align": "center", "relative_to": "page"}),
    );
    let ga = g(&b, &a);
    assert!((ga.x + ga.w / 2.0 - 960.0).abs() < 0.01);
    assert!(
        b.err(json!({"op": "align_items", "item_ids": [a], "align": "left"}))
            .contains("two or more")
    );
    assert!(
        b.err(json!({"op": "align_items", "item_ids": [a, c], "align": "sideways"}))
            .contains("align must be")
    );
    assert!(
        b.err(json!({"op": "align_items", "item_ids": [a, c], "distribute": "vertical"}))
            .contains("three or more")
    );
}

#[test]
fn grouped_ink_aligns_as_one_unit() {
    let mut b = Board::new();
    let s1 = b.stroke(&[(100.0, 100.0), (150.0, 140.0)]);
    let s2 = b.stroke(&[(160.0, 300.0), (200.0, 340.0)]);
    let t = b.text("T", 600.0, 500.0);
    b.one_step(json!({"op": "group_items", "item_ids": [s1, s2]}));
    assert!(b.item(&s1).group.is_some());
    assert_eq!(b.item(&s1).group, b.item(&s2).group);
    let before1 = b.item(&s1).data.aabb();
    let before2 = b.item(&s2).data.aabb();
    b.ok(json!({"op": "align_items", "item_ids": [s1, s2, t], "align": "left"}));
    let after1 = b.item(&s1).data.aabb();
    let after2 = b.item(&s2).data.aabb();
    // both strokes moved by the same amount: the group kept its shape
    assert!(((after1[0] - before1[0]) - (after2[0] - before2[0])).abs() < 0.01);
    b.one_step(json!({"op": "ungroup_items", "item_ids": [s1]}));
    assert!(b.item(&s1).group.is_none() && b.item(&s2).group.is_none());
    let e = b.err(json!({"op": "ungroup_items", "item_ids": [t]}));
    assert!(e.contains("is in a group"), "{e}");
    assert!(
        b.err(json!({"op": "group_items", "item_ids": [t]}))
            .contains("two items")
    );
}

#[test]
fn restyle_scale_reorder_delete() {
    let mut b = Board::new();
    let s = b.stroke(&[(100.0, 100.0), (300.0, 200.0)]);
    let h = b.ok(
        json!({"op": "draw_stroke", "tool": "highlighter", "points": [[100, 300], [400, 300]]}),
    )["item_ids"][0]
        .as_str()
        .unwrap()
        .to_string();
    let t = b.text("Hello", 500.0, 500.0);

    let r = b.one_step(json!({"op": "restyle_items", "item_ids": [s, h, t], "color": "#e5484d", "width": 8, "brush": "chalk"}));
    assert_eq!(r["restyled"], 3);
    match b.item(&s).data {
        ItemData::Stroke(st) => {
            assert_eq!(st.color, 0xe5484dff);
            assert_eq!(st.width, 8.0);
            assert_eq!(st.brush.as_deref(), Some("chalk"));
        }
        _ => panic!(),
    }
    match b.item(&h).data {
        ItemData::Stroke(st) => assert!(
            (st.opacity - 0.38).abs() < 1e-6,
            "highlighter keeps its translucency"
        ),
        _ => panic!(),
    }
    match b.item(&t).data {
        ItemData::Text(tx) => assert_eq!(tx.color, 0xe5484dff),
        _ => panic!(),
    }
    b.one_step(json!({"op": "restyle_items", "item_ids": [s], "brush": "pen", "opacity": 0.5}));
    match b.item(&s).data {
        ItemData::Stroke(st) => {
            assert_eq!(st.brush, None);
            assert_eq!(st.opacity, 0.5);
        }
        _ => panic!(),
    }
    b.one_step(json!({"op": "restyle_items", "item_ids": [t], "text_size": 80}));
    match b.item(&t).data {
        ItemData::Text(tx) => assert_eq!(tx.size, 80.0),
        _ => panic!(),
    }
    assert!(
        b.err(json!({"op": "restyle_items", "item_ids": [s]}))
            .contains("nothing to change")
    );
    assert!(
        b.err(json!({"op": "restyle_items", "item_ids": [s], "brush": "magic"}))
            .contains("brush must be")
    );
    assert!(
        b.err(json!({"op": "restyle_items", "item_ids": [s], "color": "red"}))
            .contains("#rrggbb")
    );
    assert!(
        b.err(json!({"op": "restyle_items", "item_ids": [s], "opacity": 3}))
            .contains("opacity")
    );

    // a uniform scale about the centre
    let before = b.item(&s).data.aabb();
    b.one_step(json!({"op": "scale_items", "item_ids": [s], "factor": 2}));
    let after = b.item(&s).data.aabb();
    let (bw, aw) = (before[2] - before[0], after[2] - after[0]);
    let (bc, ac) = ((before[0] + before[2]) / 2.0, (after[0] + after[2]) / 2.0);
    assert!((aw / bw - 2.0).abs() < 0.05, "{bw} → {aw}");
    assert!((ac - bc).abs() < 0.5);
    match b.item(&s).data {
        ItemData::Stroke(st) => assert_eq!(st.width, 16.0),
        _ => panic!(),
    }
    assert!(
        b.err(json!({"op": "scale_items", "item_ids": [s], "factor": 0}))
            .contains("factor")
    );

    // reorder
    b.one_step(json!({"op": "reorder_items", "item_ids": [s], "to": "front"}));
    assert_eq!(b.items().last().unwrap().id, s);
    b.one_step(json!({"op": "reorder_items", "item_ids": [s], "to": "back"}));
    assert_eq!(b.items().first().unwrap().id, s);
    assert!(
        b.err(json!({"op": "reorder_items", "item_ids": [s], "to": "up"}))
            .contains("front or back")
    );

    let r = b.one_step(json!({"op": "delete_items", "item_ids": [s, h]}));
    assert_eq!(r["deleted"], 2);
    assert_eq!(b.items().len(), 1);
    assert!(
        b.err(json!({"op": "delete_items", "item_ids": []}))
            .contains("item_ids")
    );
}

#[test]
fn shapes_keep_fill_and_scale_with_their_geometry() {
    let mut b = Board::new();
    let pts: Vec<rupok_ink::StrokePoint> = [
        (100.0, 100.0),
        (300.0, 100.0),
        (300.0, 200.0),
        (100.0, 200.0),
        (100.0, 100.0),
    ]
    .iter()
    .enumerate()
    .map(|(i, (x, y))| rupok_ink::StrokePoint {
        x: *x,
        y: *y,
        p: 0.6,
        dt: i as u32 * 10,
    })
    .collect();
    let id = b
        .lesson
        .add_item(
            &b.page.clone(),
            &ItemData::Stroke(rupok_core::StrokeData {
                tool: "pen".into(),
                color: 0x000000ff,
                width: 4.0,
                opacity: 1.0,
                points: pts,
                offset: [10.0, 0.0],
                shape: Some("rect".into()),
                geom: Some(rupok_core::ShapeGeom {
                    kind: "rect".into(),
                    x: 100.0,
                    y: 100.0,
                    w: 200.0,
                    h: 100.0,
                    ..Default::default()
                }),
                brush: None,
            }),
            1.0,
        )
        .unwrap();
    b.one_step(
        json!({"op": "restyle_items", "item_ids": [id], "fill": "#20c99733", "dashed": true}),
    );
    let geom = |b: &Board| match b.item(&id).data {
        ItemData::Stroke(s) => (s.geom.unwrap(), s.offset),
        _ => panic!(),
    };
    let (g, _) = geom(&b);
    assert_eq!(g.fill, Some(0x20c99733));
    assert!(g.dash);
    b.one_step(json!({"op": "scale_items", "item_ids": [id], "factor": 0.5}));
    let (g, off) = geom(&b);
    assert_eq!(off, [0.0, 0.0], "the offset is folded in");
    assert!((g.w - 100.0).abs() < 1e-6 && (g.h - 50.0).abs() < 1e-6);
    b.ok(json!({"op": "restyle_items", "item_ids": [id], "fill": "none"}));
    assert_eq!(geom(&b).0.fill, None);
}

#[test]
fn draw_stroke_and_update_text() {
    let mut b = Board::new();
    let r = b.one_step(json!({
        "op": "draw_stroke",
        "strokes": [
            {"points": [[100, 100], [200, 150, 0.8], [300, 100]], "color": "#3e63dd", "brush": "fountain"},
            {"points": [[100, 300], [300, 300]], "width": 10}
        ]
    }));
    assert_eq!(r["item_ids"].as_array().unwrap().len(), 2);
    let first = r["item_ids"][0].as_str().unwrap().to_string();
    match b.item(&first).data {
        ItemData::Stroke(s) => {
            assert_eq!(s.color, 0x3e63ddff);
            assert_eq!(s.brush.as_deref(), Some("fountain"));
            assert!(
                s.points.windows(2).all(|w| w[1].dt > w[0].dt),
                "timed for replay"
            );
        }
        _ => panic!(),
    }
    assert!(b.err(json!({"op": "draw_stroke"})).contains("points"));
    assert!(
        b.err(json!({"op": "draw_stroke", "points": [[1]]}))
            .contains("[x, y]")
    );
    assert!(
        b.err(json!({"op": "draw_stroke", "points": [[1, 2, 5]]}))
            .contains("pressure")
    );
    assert!(
        b.err(json!({"op": "draw_stroke", "points": [[1, 2]], "tool": "spray"}))
            .contains("pen or highlighter")
    );

    let t = b.text("Old", 100.0, 500.0);
    b.one_step(json!({"op": "update_text", "item_id": t, "text": "New text", "size": 60, "color": "#30a46c", "x": 120}));
    match b.item(&t).data {
        ItemData::Text(tx) => {
            assert_eq!(tx.text, "New text");
            assert_eq!(tx.size, 60.0);
            assert_eq!(tx.color, 0x30a46cff);
            assert_eq!(tx.geom.x, 120.0);
        }
        _ => panic!(),
    }
    // A long edit at title size stays inside the page (fit.rs).
    let long =
        "Ohm's law: the current through a conductor is proportional to the voltage across it. "
            .repeat(4);
    let r = b
        .one_step(json!({"op": "update_text", "item_id": t, "text": long, "size": 120, "x": 1700}));
    match b.item(&t).data {
        ItemData::Text(tx) => {
            assert!(tx.size < 120.0, "shrunk to fit: {}", tx.size);
            assert!(
                tx.geom.x + tx.geom.w <= 1920.0 && tx.geom.y + tx.geom.h <= 1080.0,
                "{:?}",
                tx.geom
            );
        }
        _ => panic!(),
    }
    assert!(!r["warnings"].as_array().unwrap().is_empty());
    assert!(
        b.err(json!({"op": "update_text", "item_id": t}))
            .contains("nothing to change")
    );
    assert!(
        b.err(json!({"op": "update_text", "item_id": first, "text": "x"}))
            .contains("not a text box")
    );
}

#[test]
fn builtin_stamps_are_placed_like_the_board_does() {
    let mut b = Board::new();
    let r = b.one_step(json!({"op": "add_stamp", "stamp": "star", "x": 100, "y": 100}));
    let id = r["item_id"].as_str().unwrap().to_string();
    match b.item(&id).data {
        ItemData::Text(t) => {
            assert_eq!(t.stamp.as_deref(), Some("star"));
            assert_eq!(t.text, "⭐");
            assert_eq!((t.geom.w, t.geom.h), (96.0, 96.0));
        }
        _ => panic!(),
    }
    let r = b.ok(json!({"op": "add_stamp", "stamp": "badge"}));
    let r2 = b.ok(json!({"op": "add_stamp", "stamp": "badge"}));
    let txt = |b: &Board, v: &Value| match b.item(v["item_id"].as_str().unwrap()).data {
        ItemData::Text(t) => t.text,
        _ => panic!(),
    };
    assert_eq!(txt(&b, &r), "1");
    assert_eq!(txt(&b, &r2), "2");
    let r = b.ok(json!({"op": "add_stamp", "stamp": "excellent", "text": "Brilliant!"}));
    assert_eq!(txt(&b, &r), "Brilliant!");
    assert!(
        b.err(json!({"op": "add_stamp", "stamp": "emoji"}))
            .contains("needs `text`")
    );
    assert!(
        b.err(json!({"op": "add_stamp", "stamp": "nope"}))
            .contains("no stamp")
    );
    assert!(
        b.err(json!({"op": "add_stamp", "stamp": "Bad!"}))
            .contains("no stamp")
    );
    assert!(
        b.err(json!({"op": "add_stamp", "stamp": "star", "size": 5}))
            .contains("size")
    );
    let l = b.ok(json!({"op": "list_stamps"}));
    assert_eq!(
        l["builtin"].as_array().unwrap().len(),
        crate::stamps::BUILTINS.len()
    );
}

const SPARKLE_SVG: &str = r##"<svg viewBox="0 0 100 100"><path d="M50 5 L61 39 L95 39 L67 60 L78 94 L50 73 L22 94 L33 60 L5 39 L39 39 Z" fill="currentColor"/></svg>"##;

const DICE: &str = r#"<!doctype html><html><head><title>Dice</title>
<style>body{margin:0;display:grid;place-items:center;height:100vh;font:96px system-ui}</style></head>
<body><div id="d">⚀</div><script>
const f = ["⚀","⚁","⚂","⚃","⚄","⚅"];
document.body.onclick = () => { document.getElementById("d").textContent = f[Math.floor(Math.random()*6)]; };
</script></body></html>"#;

#[test]
fn custom_svg_stamp_with_behaviour_lifecycle() {
    let mut b = Board::new();
    let r = b.one_step(json!({
        "op": "create_stamp", "name": "Super star", "keywords": ["Star", "praise"],
        "art": "svg", "svg": SPARKLE_SVG, "color": "#f5b301",
        "behavior": {"animation": "sparkle", "burst": "stars", "burst_colors": ["#ffd43b"], "counter": true, "sound": "chime"}
    }));
    let id = r["id"].as_str().unwrap().to_string();
    assert!(id.starts_with("c-super-star-"), "{id}");
    assert!(crate::stamps::valid_id(&id));
    let def = b.lesson.stamp_def(&id).unwrap();
    assert!(def.contains("xmlns"), "sanitised SVG stored: {def}");
    assert!(def.contains("\"keywords\":[\"star\",\"praise\"]"));

    // place it twice: a text item naming the definition, counted on the page
    let p1 = b.one_step(json!({"op": "add_stamp", "stamp": id, "x": 200, "y": 200}));
    let p2 = b.ok(json!({"op": "add_stamp", "stamp": id, "x": 400, "y": 200, "size": 140}));
    assert_eq!(p1["behavior"]["animation"], "sparkle");
    assert_eq!(p2["count_on_page"], 2);
    match b.item(p2["item_id"].as_str().unwrap()).data {
        ItemData::Text(t) => {
            assert_eq!(t.stamp.as_deref(), Some(id.as_str()));
            assert_eq!(
                t.text, "Super star",
                "fallback text for readers without stamps"
            );
            assert_eq!(t.color, 0xf5b301ff);
            assert_eq!(t.geom.h, 140.0);
        }
        _ => panic!(),
    }
    let l = b.ok(json!({"op": "list_stamps"}));
    assert_eq!(l["custom"][0]["placements"], 2);

    // update: only the given fields change
    b.one_step(json!({"op": "update_stamp", "id": id, "name": "Gold star", "behavior": {"animation": "bounce"}}));
    let d = crate::stamps::lesson_def(&b.lesson, &id).unwrap();
    assert_eq!(d.name.as_deref(), Some("Gold star"));
    assert_eq!(d.art.as_deref(), Some("svg"));
    assert!(d.svg.is_some());
    assert_eq!(d.behavior.unwrap().animation.as_deref(), Some("bounce"));

    // delete: placements keep it unless asked
    let r = b.ok(json!({"op": "delete_stamp", "id": id}));
    assert_eq!(r["kept_in_lesson"], true);
    assert!(b.lesson.stamp_def(&id).is_some());
    let r = b.one_step(json!({"op": "delete_stamp", "id": id, "remove_placements": true}));
    assert_eq!(r["placements_removed"], 2);
    assert!(b.lesson.stamp_def(&id).is_none());
    assert!(b.items().is_empty());
    assert!(
        b.err(json!({"op": "delete_stamp", "id": id}))
            .contains("no custom stamp")
    );
    assert!(
        b.run(json!({"op": "delete_stamp", "id": id, "in_library": true}))
            .is_ok()
    );
}

#[test]
fn stamp_validation_is_strict() {
    let mut b = Board::new();
    let bad = [
        (
            json!({"op": "create_stamp", "art": "emoji", "emoji": "🦉"}),
            "name",
        ),
        (
            json!({"op": "create_stamp", "name": "X"}),
            "art is required",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "gif"}),
            "art must be",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "svg", "svg": "<svg><script>alert(1)</script></svg>"}),
            "svg refused",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "svg", "svg": "<svg><image href=\"https://x/y.png\"/></svg>"}),
            "svg refused",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji"}),
            "needs `emoji`",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "text", "text": "a very long label that goes on and on"}),
            "1 to 24",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "behavior": {"animation": "explode"}}),
            "animation must be",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "behavior": {"sound": "siren"}}),
            "sound must be",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "behavior": {"fade_after": 9999}}),
            "fade_after",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "id": "star"}),
            "built-in",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "id": "Bad Id"}),
            "id must be",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "smart"}),
            "needs `html`",
        ),
        (
            json!({"op": "create_stamp", "name": "X", "art": "emoji", "emoji": "⭐", "size": 5000}),
            "size",
        ),
        (
            json!({"op": "update_stamp", "id": "star", "name": "Y"}),
            "built-in",
        ),
        (
            json!({"op": "update_stamp", "id": "c-none", "name": "Y"}),
            "no custom stamp",
        ),
    ];
    for (op, want) in bad {
        let e = b.err(op.clone());
        assert!(e.contains(want), "{op}: {e}");
    }
    assert!(b.lesson.stamp_defs().is_empty(), "nothing was stored");
    // the same id twice
    b.ok(json!({"op": "create_stamp", "id": "owl", "name": "Owl", "art": "emoji", "emoji": "🦉"}));
    assert!(
        b.err(
            json!({"op": "create_stamp", "id": "owl", "name": "Owl", "art": "emoji", "emoji": "🦉"})
        )
        .contains("already exists")
    );
    // no picture/smart switch
    assert!(
        b.err(json!({"op": "update_stamp", "id": "owl", "art": "smart", "html": DICE}))
            .contains("cannot become")
    );
}

#[test]
fn smart_stamps_run_as_sandboxed_objects() {
    let mut b = Board::new();
    let r = b.one_step(json!({
        "op": "create_stamp", "id": "dice", "name": "Dice", "art": "smart", "html": DICE,
        "width": 160, "height": 160, "behavior": {"animation": "pop"}
    }));
    let bundle = r["stamp"]["bundle"].as_str().unwrap().to_string();
    assert!(b.pkg.objects.contains_key(&bundle));
    assert!(
        !b.lesson.stamp_def("dice").unwrap().contains("Math.random"),
        "the HTML is in the bundle, not the definition"
    );
    let p = b.one_step(json!({"op": "add_stamp", "stamp": "dice", "x": 100, "y": 100}));
    let item = b.item(p["item_id"].as_str().unwrap());
    match &item.data {
        ItemData::Object(o) => {
            assert_eq!(o.bundle, bundle);
            assert_eq!((o.geom.w, o.geom.h), (160.0, 160.0));
        }
        _ => panic!("a Smart stamp is an object"),
    }
    // a second stamp with the same HTML shares the bundle
    let r2 = b.ok(
        json!({"op": "create_stamp", "id": "dice-2", "name": "Dice", "art": "smart", "html": DICE}),
    );
    assert_eq!(r2["stamp"]["bundle"], json!(bundle));
    // network is refused: the sandbox rules apply unchanged
    let net = r#"<!doctype html><script src="https://cdn.example.com/x.js"></script>"#;
    let e = b
        .run(json!({"op": "create_stamp", "name": "Net", "art": "smart", "html": net}))
        .unwrap_err();
    assert!(matches!(e, LiveError::Diagnostics(_)), "{e}");
    // new HTML re-points the placement
    let newer = DICE.replace("⚀</div>", "⚅</div>");
    let u = b.ok(json!({"op": "update_stamp", "id": "dice", "html": newer}));
    assert_eq!(u["placements_updated"], 1);
    let nb = u["stamp"]["bundle"].as_str().unwrap();
    assert_ne!(nb, bundle);
    assert!(
        b.items()
            .iter()
            .any(|i| matches!(&i.data, ItemData::Object(o) if o.bundle == nb))
    );
}

#[test]
fn a_stamp_from_the_device_library_comes_with_its_definition() {
    let mut b = Board::new();
    let def =
        json!({"name": "Owl", "art": "emoji", "emoji": "🦉", "behavior": {"burst": "hearts"}});
    let r = b.one_step(json!({"op": "add_stamp", "stamp": "c-owl", "def": def}));
    assert!(
        b.lesson.stamp_def("c-owl").is_some(),
        "stored with the placement, one step"
    );
    match b.item(r["item_id"].as_str().unwrap()).data {
        ItemData::Text(t) => assert_eq!(t.text, "🦉"),
        _ => panic!(),
    }
    // update of a library-only stamp: checked, not stored
    let u = b
        .ok(json!({"op": "update_stamp", "id": "c-lib", "name": "New", "base": {"name": "Lib", "art": "text", "text": "Yes!"}}));
    assert_eq!(u["in_lesson"], false);
    assert_eq!(u["stamp"]["text"], "Yes!");
    assert!(b.lesson.stamp_def("c-lib").is_none());
}

#[test]
fn student_view_refuses_every_new_write_and_board_ops_are_not_document_ops() {
    let mut b = Board::new();
    let t = b.text("x", 10.0, 10.0);
    b.read_only = true;
    for op in [
        json!({"op": "move_pages", "pages": [1], "to": 1}),
        json!({"op": "delete_items", "item_ids": [t]}),
        json!({"op": "restyle_items", "item_ids": [t], "color": "#000000"}),
        json!({"op": "draw_stroke", "points": [[1, 1], [2, 2]]}),
        json!({"op": "add_stamp", "stamp": "star"}),
        json!({"op": "create_stamp", "name": "S", "art": "emoji", "emoji": "⭐"}),
    ] {
        assert!(
            matches!(b.run(op.clone()), Err(LiveError::ReadOnly)),
            "{op}"
        );
    }
    assert!(b.run(json!({"op": "list_pages"})).is_ok());
    assert!(b.run(json!({"op": "list_stamps"})).is_ok());
    b.read_only = false;
    for op in [
        json!({"op": "set_mode", "mode": "teach"}),
        json!({"op": "fit_page"}),
        json!({"op": "export_pdf"}),
        json!({"op": "library_list"}),
        json!({"op": "get_settings"}),
        json!({"op": "set_dock_layout", "layout": {}}),
        json!({"op": "celebrate"}),
    ] {
        let o: Op = serde_json::from_value(op.clone()).unwrap();
        assert!(o.board_only(), "{op}");
        assert!(
            matches!(b.run(op.clone()), Err(LiveError::Unsupported(_))),
            "{op}"
        );
    }
    let access = |v: Value| serde_json::from_value::<Op>(v).unwrap().access();
    assert_eq!(
        access(json!({"op": "set_setting", "key": "theme", "value": "dark"})),
        Access::Write
    );
    assert_eq!(
        access(json!({"op": "set_mode", "mode": "teach"})),
        Access::Control
    );
    assert_eq!(access(json!({"op": "get_dock_layout"})), Access::Read);
    assert_eq!(access(json!({"op": "undo"})), Access::Write);
}
