//! The board side of the live agent bridge ([`rupok_live`]): run agent ops on
//! the [`Engine`] as local changes (so the teacher's undo covers them), and
//! exchange Loro updates with the agent server's mirror.
//!
//! Errors are returned as the JSON body the agent should see
//! (`{"error": …, "diagnostics"?: […]}`), ready to send back as is.

use std::cell::RefCell;

use rupok_ink::math::geom::Pt;
use rupok_ink::write::GlyphSet;
use rupok_live::{BoardCtx, Op};

use crate::engine::{Engine, InkSink};

thread_local! {
    /// The teacher's handwriting for `write_handwriting` (calibration, set by
    /// the board app with [`set_glyphs_json`]) over the built-in neat hand.
    static GLYPHS: RefCell<GlyphSet> = RefCell::new(GlyphSet::new());
}

/// Replace the teacher's glyphs. JSON: `{"x": [sample, …], …}`, a sample is a
/// list of strokes, a stroke a flat `[x, y, x, y, …]` list in writing units
/// (baseline y = 0, x-height line y = −14; see `rupok_ink::write`). Returns
/// how many characters were accepted.
pub fn set_glyphs_json(json: &str) -> Result<usize, String> {
    let v: std::collections::BTreeMap<String, Vec<Vec<Vec<f32>>>> =
        serde_json::from_str(json).map_err(|e| format!("bad glyphs: {e}"))?;
    let mut set = GlyphSet::new();
    for (key, samples) in &v {
        let mut chars = key.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            continue;
        };
        for sample in samples
            .iter()
            .take(rupok_ink::write::glyphs::MAX_USER_SAMPLES)
        {
            let strokes: Vec<Vec<Pt>> = sample
                .iter()
                .map(|xy| {
                    xy.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|p| Pt::new(p[0], p[1]))
                        .collect()
                })
                .collect();
            set.add_user(c, &strokes);
        }
    }
    let n = set.user_chars();
    GLYPHS.with(|g| *g.borrow_mut() = set);
    Ok(n)
}

/// Synthesise `lines` without touching the board (the teacher's private
/// preview). Returns `{width, height, duration_ms, strokes: [[x, y, dt, …]], lines: [..]}`.
pub fn preview_json(lines_json: &str, x_height: f32, teacher: bool, speed: f32) -> String {
    let lines: Vec<String> = serde_json::from_str(lines_json).unwrap_or_default();
    let w = GLYPHS.with(|g| {
        rupok_ink::write::write(
            &lines,
            &g.borrow(),
            &rupok_ink::write::WriteOptions {
                x_height,
                prefer_user: teacher,
                speed,
                seed: lines.join("\n").len() as u64,
                max_width: Some(x_height * 40.0),
                ..Default::default()
            },
        )
    });
    let strokes: Vec<Vec<f32>> = w
        .strokes
        .iter()
        .map(|s| {
            let mut v = Vec::with_capacity(s.points.len() * 3 + 1);
            v.push(s.start_ms as f32);
            for p in &s.points {
                v.extend([
                    (p.x * 10.0).round() / 10.0,
                    (p.y * 10.0).round() / 10.0,
                    p.dt as f32,
                ]);
            }
            v
        })
        .collect();
    serde_json::json!({
        "width": w.width,
        "height": w.height,
        "duration_ms": w.duration_ms,
        "strokes": strokes,
        "line_of": w.strokes.iter().map(|s| s.line).collect::<Vec<_>>(),
    })
    .to_string()
}

fn error_json(msg: impl std::fmt::Display) -> String {
    serde_json::json!({ "error": msg.to_string() }).to_string()
}

/// Run one agent op (JSON, see [`rupok_live::Op`]) as `agent`. Returns the
/// result JSON. Document changes are one undo step each, tagged with the
/// agent's name; page changes show the page.
pub fn apply<S: InkSink>(
    e: &mut Engine<S>,
    op_json: &str,
    agent: &str,
    now_ms: f64,
) -> Result<String, String> {
    let op: Op =
        serde_json::from_str(op_json).map_err(|err| error_json(format!("bad op: {err}")))?;
    // Shapes and tables: geometry lives in the engine (`engine/insert.rs`).
    if matches!(op, Op::AddShape { .. } | Op::AddTable { .. }) {
        return e.ai_insert(&op, agent, now_ms);
    }
    let page = e.page_id().to_string();
    let selection = e.selection().to_vec();
    let read_only = e.read_only();
    let mut out = GLYPHS
        .with(|g| {
            rupok_live::apply_with(
                BoardCtx {
                    pkg: &mut e.pkg,
                    lesson: &mut e.lesson,
                    page_id: &page,
                    selection: &selection,
                    read_only,
                },
                &op,
                agent,
                now_ms,
                Some(&g.borrow()),
            )
        })
        .map_err(|err| err.to_json().to_string())?;
    if out.changed {
        e.resync();
    }
    if let Some(i) = out.show_page {
        e.goto_page(i);
    }
    if let Op::WriteHandwriting { .. } = &op {
        start_reveal(e, &mut out.result, now_ms);
    }
    Ok(out.result.to_string())
}

/// Animate the strokes `write_handwriting` just added, and keep the agent's
/// answer short (a stroke count instead of hundreds of ids).
fn start_reveal<S: InkSink>(e: &mut Engine<S>, result: &mut serde_json::Value, now_ms: f64) {
    let ids: Vec<String> = result["item_ids"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let per_line: Vec<usize> = result["strokes_by_line"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as usize).collect())
        .unwrap_or_default();
    let mut lines = Vec::with_capacity(ids.len());
    for (li, n) in per_line.iter().enumerate() {
        lines.extend(std::iter::repeat_n(li, *n));
    }
    let step = result["step_mode"] == "one-line-per-click";
    e.reveal_start(&ids, &lines, step, now_ms);
    if let Some(m) = result.as_object_mut() {
        m.remove("item_ids");
        m.insert("strokes".into(), serde_json::json!(ids.len()));
        m.insert("first_item_id".into(), serde_json::json!(ids.first()));
    }
}

/// The lesson's encoded version vector.
pub fn version<S: InkSink>(e: &Engine<S>) -> Vec<u8> {
    rupok_live::sync::version(&e.lesson)
}

/// Loro updates a peer at encoded version `from` lacks (empty = everything).
pub fn export_updates<S: InkSink>(e: &Engine<S>, from: &[u8]) -> Result<Vec<u8>, String> {
    rupok_live::sync::export_since(&e.lesson, from)
}

/// Merge updates from the agent server's mirror and refresh the scene.
pub fn import_updates<S: InkSink>(e: &mut Engine<S>, bytes: &[u8]) -> Result<(), String> {
    e.lesson.import(bytes).map_err(|err| err.to_string())?;
    e.resync();
    Ok(())
}

/// `{key, title}` of the open lesson (see [`rupok_live::doc_key`]).
pub fn doc_info_json<S: InkSink>(e: &Engine<S>) -> String {
    serde_json::json!({ "key": rupok_live::doc_key(&e.lesson), "title": e.lesson.title() })
        .to_string()
}

/// `{item_id: agent}` for the items an agent made on page `index`.
pub fn attribution_json<S: InkSink>(e: &Engine<S>, index: usize) -> String {
    let Ok(page) = e.lesson.page_id_at(index) else {
        return "{}".into();
    };
    serde_json::to_string(&rupok_live::agent_items(&e.lesson, &page))
        .unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tests::MockSink;
    use rupok_core::ItemData;

    fn engine() -> Engine<MockSink> {
        Engine::new(MockSink::default(), "Live", 1_000.0)
    }

    #[test]
    fn agent_op_is_one_undo_step_on_the_board() {
        let mut e = engine();
        let r = apply(
            &mut e,
            r#"{"op":"add_text","text":"Quiz time","x":100,"y":100,"size":48}"#,
            "Claude Code",
            2_000.0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        let id = v["item_id"].as_str().unwrap().to_string();
        assert!(e.dom_items_json().contains(&id), "scene refreshed");
        assert_eq!(
            attribution_json(&e, 0),
            format!(r#"{{"{id}":"Claude Code"}}"#)
        );
        assert!(e.undo());
        assert!(!e.dom_items_json().contains(&id));
        assert!(e.redo());

        // a new page shown right away
        let r = apply(
            &mut e,
            r#"{"op":"add_page_after_current","title":"Practice","show":true}"#,
            "Claude Code",
            3_000.0,
        )
        .unwrap();
        assert!(r.contains(r#""page":2"#));
        assert_eq!(e.page_index(), 1);

        // bad input is a JSON error, the board is unchanged
        let err = apply(
            &mut e,
            r#"{"op":"add_text","text":"","x":0,"y":0}"#,
            "A",
            4.0,
        )
        .unwrap_err();
        assert!(err.contains("\"error\""));
        assert!(apply(&mut e, "{", "A", 4.0).is_err());
        e.set_read_only(true);
        assert!(
            apply(
                &mut e,
                r#"{"op":"add_text","text":"x","x":0,"y":0}"#,
                "A",
                4.0
            )
            .is_err()
        );
        assert!(apply(&mut e, r#"{"op":"get_page"}"#, "A", 4.0).is_ok());
    }

    #[test]
    fn handwriting_is_revealed_stroke_by_stroke_and_undone_at_once() {
        let mut e = engine();
        let r = apply(
            &mut e,
            r#"{"op":"write_handwriting","lines":["$2x = 4$","$x = 2$"],"anchor":{"x":100,"y":100}}"#,
            "Claude",
            10_000.0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        let n = v["strokes"].as_u64().unwrap() as usize;
        assert!(n > 5, "{v}");
        assert!(
            v.get("item_ids").is_none(),
            "the agent gets a count, not every id"
        );
        let dur = v["duration_ms"].as_f64().unwrap();
        // Nothing is drawn yet; the document already has everything.
        assert_eq!(e.sink.strokes.len(), 0);
        assert_eq!(e.stroke_count(), n);
        assert!(e.has_animation());
        // The stroke count grows with time.
        let mut seen = Vec::new();
        let mut t = 10_000.0;
        while e.reveal_state().is_some() && t < 10_000.0 + dur + 5_000.0 {
            t += 50.0;
            e.tick(t);
            seen.push(e.sink.strokes.len());
        }
        assert!(seen.windows(2).all(|w| w[1] >= w[0]));
        assert!(
            seen.iter().any(|&k| k > 0 && k < n),
            "partially written at some point"
        );
        assert_eq!(e.sink.strokes.len(), n, "all written in the end");
        assert!(!e.has_animation());
        // One undo removes the whole solution.
        assert!(e.undo());
        assert_eq!(e.stroke_count(), 0);
        assert!(e.sink.strokes.is_empty());
    }

    #[test]
    fn step_mode_waits_for_next_before_each_line() {
        let mut e = engine();
        let r = apply(
            &mut e,
            r#"{"op":"write_handwriting","lines":["$a$","$b$","$c$"],"anchor":{"x":100,"y":100},"step_mode":"one-line-per-click"}"#,
            "Claude",
            1_000.0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&r).unwrap();
        let per: Vec<usize> = v["strokes_by_line"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as usize)
            .collect();
        let mut t = 1_000.0;
        let run = |e: &mut Engine<MockSink>, t: &mut f64| {
            for _ in 0..400 {
                *t += 50.0;
                e.tick(*t);
            }
        };
        run(&mut e, &mut t);
        assert_eq!(e.sink.strokes.len(), per[0], "only the first line");
        let st = e.reveal_state().unwrap();
        assert!(st.waiting && st.gate == 1 && st.lines == 3);
        assert!(
            !e.has_animation(),
            "no frames while waiting for the teacher"
        );
        assert!(e.reveal_next(t));
        run(&mut e, &mut t);
        assert_eq!(e.sink.strokes.len(), per[0] + per[1]);
        assert!(e.reveal_next(t));
        run(&mut e, &mut t);
        assert_eq!(e.sink.strokes.len(), per.iter().sum::<usize>());
        assert!(e.reveal_state().is_none());
        assert!(!e.reveal_next(t));
    }

    #[test]
    fn teacher_glyphs_and_preview() {
        assert!(set_glyphs_json("not json").is_err());
        let n = set_glyphs_json(r#"{"x":[[[0,-14,10,0,10,-14,0,0]]],"ab":[[[0,0,1,1]]]}"#).unwrap();
        assert_eq!(n, 1, "only single characters count");
        let p: serde_json::Value =
            serde_json::from_str(&preview_json(r#"["x"]"#, 20.0, true, 1.0)).unwrap();
        assert_eq!(
            p["strokes"].as_array().unwrap().len(),
            1,
            "the teacher's one-stroke x"
        );
        let p: serde_json::Value =
            serde_json::from_str(&preview_json(r#"["x"]"#, 20.0, false, 1.0)).unwrap();
        assert_eq!(p["strokes"].as_array().unwrap().len(), 2);
        set_glyphs_json("{}").unwrap();
    }

    #[test]
    fn updates_flow_both_ways_and_merge() {
        // the board and the server mirror start from the same lesson
        let mut board = engine();
        let mut mirror = rupok_core::Lesson::load(&board.lesson.save()).unwrap();
        let base = rupok_live::sync::version(&mirror);

        // the agent edits on the board; the teacher keeps writing too
        apply(
            &mut board,
            r#"{"op":"add_text","text":"A","x":1,"y":1}"#,
            "Codex",
            2.0,
        )
        .unwrap();
        mirror
            .import(&export_updates(&board, &base).unwrap())
            .unwrap();
        assert_eq!(mirror.version(), board.lesson.version());

        // a concurrent change on the mirror side merges into the board
        let page = mirror.page_id_at(0).unwrap();
        let before = rupok_live::sync::version(&board.lesson);
        mirror.set_title("Renamed");
        let delta = rupok_live::sync::export_since(&mirror, &before).unwrap();
        import_updates(&mut board, &delta).unwrap();
        assert_eq!(board.lesson.title(), "Renamed");
        assert_eq!(mirror.items(&page).unwrap().len(), 1);
        assert!(matches!(
            board.lesson.items(&page).unwrap()[0].data,
            ItemData::Text(_)
        ));
        assert!(doc_info_json(&board).contains(&rupok_live::doc_key(&mirror)));
    }
}
