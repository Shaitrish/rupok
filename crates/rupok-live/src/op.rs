//! Agent operations on the open lesson.
//!
//! An [`Op`] is what an MCP tool call becomes when a board is paired. The
//! server serialises it into an `op` message; the board runs it with
//! [`crate::apply`] (document operations) or in the web app (`snapshot`,
//! `point_at` and the UI half of `set_board_theme`).
//!
//! Pages are 1-based; `page: None` means the page the board is showing.
//! Coordinates are slide pixels from the top-left of the 1920 × 1080 frame.

use rupok_core::Pattern;
use serde::{Deserialize, Serialize};

/// A background pattern name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatternName {
    Plain,
    Grid,
    Dots,
    Lines,
}

impl From<PatternName> for Pattern {
    fn from(p: PatternName) -> Self {
        match p {
            PatternName::Plain => Pattern::Plain,
            PatternName::Grid => Pattern::Grid,
            PatternName::Dots => Pattern::Dots,
            PatternName::Lines => Pattern::Lines,
        }
    }
}

/// A rectangle in slide pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    // ------------------------------------------------------------- reads
    /// Outline of one page (default: the current page) with item ids, text,
    /// objects, ink bounds and the selection.
    GetPage {
        #[serde(default)]
        page: Option<u32>,
    },
    /// Outline of every page (same shape as the file `get_lesson_outline`).
    GetOutline,
    /// The items the teacher has selected on the current page.
    GetSelection,
    /// A PNG of a page, rendered by the board app.
    Snapshot {
        #[serde(default)]
        page: Option<u32>,
        /// Only this region (slide pixels).
        #[serde(default)]
        region: Option<Rect>,
        /// Frame these items (with a margin) instead of the whole page.
        #[serde(default)]
        item_ids: Option<Vec<String>>,
        /// Image width in pixels (default 1280, max 1920).
        #[serde(default)]
        width: Option<u32>,
    },

    // -------------------------------------------------------- presenting
    GoToPage {
        page: u32,
    },
    /// A temporary laser ring or highlight box (board app only, not saved).
    PointAt {
        x: f64,
        y: f64,
        #[serde(default)]
        w: Option<f64>,
        #[serde(default)]
        h: Option<f64>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        seconds: Option<f64>,
    },
    /// Page colour and pattern of the current page; `ui` (light, dark,
    /// system) is the app theme and is handled by the board app.
    SetBoardTheme {
        #[serde(default)]
        page_color: Option<String>,
        #[serde(default)]
        pattern: Option<PatternName>,
        #[serde(default)]
        ui: Option<String>,
    },

    // ------------------------------------------------------------ writes
    AddPage {
        #[serde(default)]
        title: Option<String>,
        /// 1-based position; `None` appends.
        #[serde(default)]
        index: Option<u32>,
        #[serde(default)]
        pattern: Option<PatternName>,
        #[serde(default)]
        color: Option<String>,
    },
    AddPageAfterCurrent {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        pattern: Option<PatternName>,
        #[serde(default)]
        color: Option<String>,
        /// Show the new page right away.
        #[serde(default)]
        show: bool,
    },
    AddText {
        #[serde(default)]
        page: Option<u32>,
        text: String,
        x: f64,
        y: f64,
        #[serde(default)]
        w: Option<f64>,
        #[serde(default)]
        size: Option<f64>,
        #[serde(default)]
        color: Option<String>,
    },
    AddSmartObject {
        #[serde(default)]
        page: Option<u32>,
        html: String,
        title: String,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        height: Option<f64>,
        #[serde(default)]
        steps: Option<u32>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        network_hosts: Option<Vec<String>>,
    },
    UpdateSmartObject {
        bundle_id: String,
        html: String,
        #[serde(default)]
        steps: Option<u32>,
        #[serde(default)]
        network_hosts: Option<Vec<String>>,
    },
    DeleteItem {
        #[serde(default)]
        page: Option<u32>,
        item_id: String,
    },
    SetPageBackground {
        #[serde(default)]
        page: Option<u32>,
        #[serde(default)]
        pattern: Option<PatternName>,
        #[serde(default)]
        color: Option<String>,
    },
    /// Remove items (typically handwriting) and put a text box in their place.
    ReplaceItemsWithText {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        text: String,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        size: Option<f64>,
        #[serde(default)]
        color: Option<String>,
    },
    /// Move a Smart Object's build step: absolute `step`, or `delta` (default +1).
    StepObject {
        #[serde(default)]
        page: Option<u32>,
        #[serde(default)]
        item_id: Option<String>,
        #[serde(default)]
        step: Option<u32>,
        #[serde(default)]
        delta: Option<i32>,
    },
}

/// How an op touches the board, for the teacher's "Allow AI to edit live"
/// toggle and the Student view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// Reads only; always allowed while paired.
    Read,
    /// Changes what the class sees but not the lesson (navigation, pointer, theme).
    Control,
    /// Changes the lesson document.
    Write,
}

impl Op {
    /// The snake_case name, as in the `op` tag and the commit message.
    pub fn name(&self) -> &'static str {
        match self {
            Op::GetPage { .. } => "get_page",
            Op::GetOutline => "get_outline",
            Op::GetSelection => "get_selection",
            Op::Snapshot { .. } => "snapshot",
            Op::GoToPage { .. } => "go_to_page",
            Op::PointAt { .. } => "point_at",
            Op::SetBoardTheme { .. } => "set_board_theme",
            Op::AddPage { .. } => "add_page",
            Op::AddPageAfterCurrent { .. } => "add_page_after_current",
            Op::AddText { .. } => "add_text",
            Op::AddSmartObject { .. } => "add_smart_object",
            Op::UpdateSmartObject { .. } => "update_smart_object",
            Op::DeleteItem { .. } => "delete_item",
            Op::SetPageBackground { .. } => "set_page_background",
            Op::ReplaceItemsWithText { .. } => "replace_items_with_text",
            Op::StepObject { .. } => "step_object",
        }
    }

    pub fn access(&self) -> Access {
        match self {
            Op::GetPage { .. } | Op::GetOutline | Op::GetSelection | Op::Snapshot { .. } => {
                Access::Read
            }
            Op::GoToPage { .. } | Op::PointAt { .. } => Access::Control,
            Op::SetBoardTheme {
                page_color,
                pattern,
                ..
            } => {
                if page_color.is_some() || pattern.is_some() {
                    Access::Write
                } else {
                    Access::Control
                }
            }
            _ => Access::Write,
        }
    }

    /// A short label for the board's activity feed, e.g. `added text`.
    pub fn label(&self) -> &'static str {
        match self {
            Op::GetPage { .. } | Op::GetOutline | Op::GetSelection => "read the board",
            Op::Snapshot { .. } => "looked at the board",
            Op::GoToPage { .. } => "changed page",
            Op::PointAt { .. } => "pointed at the board",
            Op::SetBoardTheme { .. } => "changed the board theme",
            Op::AddPage { .. } | Op::AddPageAfterCurrent { .. } => "added a page",
            Op::AddText { .. } => "added text",
            Op::AddSmartObject { .. } => "added a Smart Object",
            Op::UpdateSmartObject { .. } => "updated a Smart Object",
            Op::DeleteItem { .. } => "deleted an item",
            Op::SetPageBackground { .. } => "changed the background",
            Op::ReplaceItemsWithText { .. } => "turned ink into text",
            Op::StepObject { .. } => "stepped a Smart Object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape() {
        let op: Op =
            serde_json::from_str(r#"{"op":"add_text","text":"Hi","x":10,"y":20}"#).unwrap();
        assert_eq!(
            op,
            Op::AddText {
                page: None,
                text: "Hi".into(),
                x: 10.0,
                y: 20.0,
                w: None,
                size: None,
                color: None
            }
        );
        assert_eq!(op.access(), Access::Write);
        let v = serde_json::to_value(Op::GetOutline).unwrap();
        assert_eq!(v, serde_json::json!({"op": "get_outline"}));
        let t: Op = serde_json::from_str(r#"{"op":"set_board_theme","ui":"dark"}"#).unwrap();
        assert_eq!(t.access(), Access::Control);
        let t: Op =
            serde_json::from_str(r#"{"op":"set_board_theme","page_color":"chalkboard"}"#).unwrap();
        assert_eq!(t.access(), Access::Write);
        assert!(serde_json::from_str::<Op>(r#"{"op":"format_disk"}"#).is_err());
        assert_eq!(Op::GetSelection.name(), "get_selection");
    }
}
