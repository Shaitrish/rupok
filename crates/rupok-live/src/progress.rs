//! `show_progress`: the agent's "working" card on the board.
//!
//! While an agent builds something for the class (a simulation, a worked
//! solution, a diagram), the board shows a card where the result will land:
//! a soft animated outline, the agent's name, its current step ("Writing the
//! simulation…", "Testing…"), its plan as a checklist, an optional live
//! preview, and a cancel button for the teacher. When the result arrives the
//! card resolves into it (the board animates the result in).
//!
//! The in-app chat makes these cards by itself from the agent's ACP plan and
//! tool calls. Any agent can also post and update one explicitly with this
//! op, e.g. from `rupok mcp --live`:
//!
//! ```json
//! {"op": "show_progress", "id": "caffeine", "title": "Caffeine molecule",
//!  "step": "Placing the atoms…", "kind": "object", "percent": 40,
//!  "plan": [{"content": "Atoms", "status": "completed"},
//!           {"content": "Bonds", "status": "in_progress"}],
//!  "x": 320, "y": 180, "w": 1280, "h": 720}
//! ```
//!
//! then `{"op": "show_progress", "id": "caffeine", "state": "done"}` (or
//! simply place the object: a card whose area receives the result resolves
//! by itself). Cards are not saved in the lesson; they vanish after `done`,
//! `failed` + a while, `clear`, or two minutes without an update.

use serde::{Deserialize, Serialize};

use crate::apply::LiveError;

/// Most plan entries shown.
pub const MAX_PLAN: usize = 12;
/// Longest title and step, in characters.
pub const MAX_TITLE: usize = 120;
pub const MAX_STEP: usize = 200;
/// Largest live preview (HTML, rendered in the Smart Object sandbox).
pub const MAX_PREVIEW_BYTES: usize = 512 * 1024;

/// One plan entry (ACP `PlanEntry` shape: `content`, `status`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanEntry {
    #[serde(alias = "text", alias = "title")]
    pub content: String,
    /// `pending`, `in_progress` or `completed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Arguments of [`crate::Op::ShowProgress`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgressArgs {
    /// The card to create or update. Omit to create a new card; the board
    /// answers with its id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// What is being made ("Caffeine molecule").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The current step ("Writing the simulation…").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Vec<PlanEntry>>,
    /// 0–100, when the agent knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    /// `working` (default), `done`, `failed` or `clear` (remove now).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// What will land: `object`, `handwriting`, `text`, `shape` or `other`;
    /// sets the card's icon and how the result animates in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Where the result will go, slide px. Default: a free spot on the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
    /// 1-based page; default the page on screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// A partial Smart Object (HTML) to show as a live preview thumbnail
    /// while building. Runs in the same sandbox as Smart Objects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_html: Option<String>,
    /// A short message for a failed card ("The simulation did not pass the
    /// checks; fixing it").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

const STATES: [&str; 4] = ["working", "done", "failed", "clear"];
const KINDS: [&str; 5] = ["object", "handwriting", "text", "shape", "other"];

impl ProgressArgs {
    /// Check an agent's card: known state and kind, finite geometry, bounded
    /// text and preview. Long text is an error, not silently cut, so the
    /// agent learns the limits.
    pub fn validate(&self) -> Result<(), LiveError> {
        let bad = |m: String| Err(LiveError::Invalid(m));
        if let Some(s) = &self.state
            && !STATES.contains(&s.as_str())
        {
            return bad(format!("state must be one of {STATES:?}, not {s:?}"));
        }
        if let Some(k) = &self.kind
            && !KINDS.contains(&k.as_str())
        {
            return bad(format!("kind must be one of {KINDS:?}, not {k:?}"));
        }
        if let Some(id) = &self.id
            && (id.is_empty() || id.len() > 64)
        {
            return bad("id must be 1 to 64 characters".into());
        }
        if self
            .title
            .as_ref()
            .is_some_and(|t| t.chars().count() > MAX_TITLE)
        {
            return bad(format!("title is longer than {MAX_TITLE} characters"));
        }
        if self
            .step
            .as_ref()
            .is_some_and(|t| t.chars().count() > MAX_STEP)
        {
            return bad(format!("step is longer than {MAX_STEP} characters"));
        }
        if let Some(p) = &self.plan {
            if p.len() > MAX_PLAN {
                return bad(format!("at most {MAX_PLAN} plan entries"));
            }
            if p.iter().any(|e| e.content.chars().count() > MAX_STEP) {
                return bad(format!("plan entries are at most {MAX_STEP} characters"));
            }
        }
        if let Some(v) = self.percent
            && !(v.is_finite() && (0.0..=100.0).contains(&v))
        {
            return bad("percent must be between 0 and 100".into());
        }
        for (name, v) in [("x", self.x), ("y", self.y), ("w", self.w), ("h", self.h)] {
            if let Some(v) = v
                && !(v.is_finite() && v.abs() < 1e6)
            {
                return bad(format!("{name} must be a finite number of slide pixels"));
            }
        }
        if self.w.is_some_and(|w| w <= 0.0) || self.h.is_some_and(|h| h <= 0.0) {
            return bad("w and h must be positive".into());
        }
        if self
            .preview_html
            .as_ref()
            .is_some_and(|h| h.len() > MAX_PREVIEW_BYTES)
        {
            return bad(format!(
                "preview_html is larger than {} KiB; send a smaller partial object",
                MAX_PREVIEW_BYTES / 1024
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{Access, Op};
    use serde_json::json;

    fn op(v: serde_json::Value) -> Op {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn progress_op_shape_and_access() {
        let o = op(json!({
            "op": "show_progress", "id": "caffeine", "title": "Caffeine", "step": "Bonds…",
            "kind": "object", "percent": 40,
            "plan": [{"content": "Atoms", "status": "completed"}, {"text": "Bonds"}],
            "x": 320, "y": 180, "w": 1280, "h": 720
        }));
        assert_eq!(o.name(), "show_progress");
        // It changes what the class sees, not the lesson: like point_at.
        assert_eq!(o.access(), Access::Control);
        let Op::ShowProgress(a) = &o else { panic!() };
        assert_eq!(a.plan.as_ref().unwrap()[1].content, "Bonds");
        a.validate().unwrap();
        // Round trip keeps the tag.
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["op"], "show_progress");
        assert_eq!(v["id"], "caffeine");
        // The lesson document refuses it: the board app runs it.
        let mut lesson = rupok_core::Lesson::new("t", 1.0);
        let mut pkg = rupok_format::Package::from_lesson(&lesson, "t", 1.0);
        let page = lesson.page_id_at(0).unwrap();
        let v0 = lesson.version();
        let r = crate::apply(
            crate::BoardCtx {
                pkg: &mut pkg,
                lesson: &mut lesson,
                page_id: &page,
                selection: &[],
                read_only: false,
            },
            &o,
            "Claude",
            1.0,
        );
        assert!(matches!(r, Err(LiveError::Unsupported("show_progress"))));
        assert_eq!(lesson.version(), v0);
    }

    #[test]
    fn progress_validation() {
        for bad in [
            json!({"op": "show_progress", "state": "exploded"}),
            json!({"op": "show_progress", "kind": "nuke"}),
            json!({"op": "show_progress", "percent": 140}),
            json!({"op": "show_progress", "w": -4}),
            json!({"op": "show_progress", "id": ""}),
            json!({"op": "show_progress", "title": "x".repeat(500)}),
            json!({"op": "show_progress", "plan": vec![json!({"content": "a"}); 20]}),
            json!({"op": "show_progress", "preview_html": "x".repeat(MAX_PREVIEW_BYTES + 1)}),
        ] {
            let Op::ShowProgress(a) = op(bad.clone()) else {
                panic!()
            };
            assert!(a.validate().is_err(), "{bad}");
        }
        let Op::ShowProgress(a) = op(json!({"op": "show_progress", "state": "done", "id": "x"}))
        else {
            panic!()
        };
        a.validate().unwrap();
        // Unknown ops are still refused.
        assert!(serde_json::from_value::<Op>(json!({"op": "show_progress_now"})).is_err());
    }
}
