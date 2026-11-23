//! Who made what: the agent attribution kept in the lesson history.
//!
//! Agent changes carry a commit message ([`crate::commit_message`]). Box items
//! (text, images, Smart Objects) are created by inserting a child map under
//! their id in the page's `items` map, so "the agent made item X" means "an
//! agent-tagged change inserted a container under key X". Edits to an item
//! happen inside its own map and never re-insert the key, so a teacher moving
//! an agent's text keeps the attribution, and an agent stepping a teacher's
//! object does not take it over.

use std::collections::BTreeMap;

use loro::{JsonMapOp, JsonOpContent, LoroValue, VersionVector};
use rupok_core::Lesson;

use crate::COMMIT_PREFIX;

/// `(agent, op)` from an agent commit message, or `None` for any other message.
pub fn parse_commit_message(msg: &str) -> Option<(String, String)> {
    let json = msg.strip_prefix(COMMIT_PREFIX)?;
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    Some((
        v.get("agent")?.as_str()?.to_string(),
        v.get("op")?.as_str()?.to_string(),
    ))
}

/// Items on page `page_id` that an agent made, with the agent's name.
///
/// Reads the whole history, so call it on demand (e.g. when the teacher
/// turns on "Show AI edits"), not per frame.
pub fn agent_items(lesson: &Lesson, page_id: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(items) = lesson.items(page_id) else {
        return out;
    };
    if items.is_empty() {
        return out;
    }
    let doc = lesson.loro();
    let Ok(json) = doc.export_json_updates(&VersionVector::default(), &doc.oplog_vv()) else {
        return out;
    };
    for change in &json.changes {
        let Some(agent) = change
            .msg
            .as_deref()
            .and_then(parse_commit_message)
            .map(|(a, _)| a)
        else {
            continue;
        };
        for op in &change.ops {
            if let JsonOpContent::Map(JsonMapOp::Insert { key, value }) = &op.content
                && (is_container(value) || matches!(value, LoroValue::Map(_)))
                && items.iter().any(|i| &i.id == key)
            {
                out.entry(key.clone()).or_insert_with(|| agent.clone());
            }
        }
    }
    out
}

/// Box items are inserted as child containers; strokes (immutable) as map
/// values, which `agent_items` accepts too.
fn is_container(v: &LoroValue) -> bool {
    match v {
        LoroValue::Container(_) => true,
        // Some encodings spell a child container as its id string.
        LoroValue::String(s) => s.starts_with("🦜:"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoardCtx, Op, apply};
    use rupok_core::{BoxGeom, ItemData, TextData};
    use rupok_format::Package;

    #[test]
    fn agent_items_survive_save_and_teacher_edits() {
        let mut lesson = Lesson::new("A", 1.0);
        let mut pkg = Package::from_lesson(&lesson, "t", 1.0);
        let page = lesson.page_id_at(0).unwrap();
        let teacher = lesson
            .add_item(
                &page,
                &ItemData::Text(TextData {
                    latex: false,
                    geom: BoxGeom {
                        x: 0.0,
                        y: 0.0,
                        w: 100.0,
                        h: 40.0,
                        rot: 0.0,
                    },
                    text: "mine".into(),
                    size: 32.0,
                    color: 0xff,
                }),
                2.0,
            )
            .unwrap();
        let out = apply(
            BoardCtx {
                pkg: &mut pkg,
                lesson: &mut lesson,
                page_id: &page,
                selection: &[],
                read_only: false,
            },
            &Op::AddText {
                page: None,
                text: "Quiz".into(),
                x: 10.0,
                y: 10.0,
                w: None,
                size: None,
                color: None,
            },
            "Codex",
            3.0,
        )
        .unwrap();
        let agent_id = out.result["item_id"].as_str().unwrap().to_string();
        // the teacher moves the agent's text: still the agent's
        lesson
            .set_geom(
                &page,
                &agent_id,
                BoxGeom {
                    x: 300.0,
                    y: 300.0,
                    w: 100.0,
                    h: 40.0,
                    rot: 0.0,
                },
            )
            .unwrap();
        let reloaded = Lesson::load(&lesson.save()).unwrap();
        let who = agent_items(&reloaded, &page);
        assert_eq!(who.get(&agent_id).map(String::as_str), Some("Codex"));
        assert!(!who.contains_key(&teacher));
        assert_eq!(who.len(), 1);
        assert_eq!(parse_commit_message("hello"), None);
    }
}
