//! The board side of the live agent bridge ([`rupok_live`]): run agent ops on
//! the [`Engine`] as local changes (so the teacher's undo covers them), and
//! exchange Loro updates with the agent server's mirror.
//!
//! Errors are returned as the JSON body the agent should see
//! (`{"error": …, "diagnostics"?: […]}`), ready to send back as is.

use rupok_live::{BoardCtx, Op};

use crate::engine::{Engine, InkSink};

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
    let op: Op = serde_json::from_str(op_json).map_err(|err| error_json(format!("bad op: {err}")))?;
    let page = e.page_id().to_string();
    let selection = e.selection().to_vec();
    let read_only = e.read_only();
    let out = rupok_live::apply(
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
    )
    .map_err(|err| err.to_json().to_string())?;
    if out.changed {
        e.resync();
    }
    if let Some(i) = out.show_page {
        e.goto_page(i);
    }
    Ok(out.result.to_string())
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
    serde_json::to_string(&rupok_live::agent_items(&e.lesson, &page)).unwrap_or_else(|_| "{}".into())
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
        assert_eq!(attribution_json(&e, 0), format!(r#"{{"{id}":"Claude Code"}}"#));
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
        let err = apply(&mut e, r#"{"op":"add_text","text":"","x":0,"y":0}"#, "A", 4.0).unwrap_err();
        assert!(err.contains("\"error\""));
        assert!(apply(&mut e, "{", "A", 4.0).is_err());
        e.set_read_only(true);
        assert!(apply(&mut e, r#"{"op":"add_text","text":"x","x":0,"y":0}"#, "A", 4.0).is_err());
        assert!(apply(&mut e, r#"{"op":"get_page"}"#, "A", 4.0).is_ok());
    }

    #[test]
    fn updates_flow_both_ways_and_merge() {
        // the board and the server mirror start from the same lesson
        let mut board = engine();
        let mut mirror = rupok_core::Lesson::load(&board.lesson.save()).unwrap();
        let base = rupok_live::sync::version(&mirror);

        // the agent edits on the board; the teacher keeps writing too
        apply(&mut board, r#"{"op":"add_text","text":"A","x":1,"y":1}"#, "Codex", 2.0).unwrap();
        mirror.import(&export_updates(&board, &base).unwrap()).unwrap();
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
