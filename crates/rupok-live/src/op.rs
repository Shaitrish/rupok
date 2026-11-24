//! Agent operations on the open lesson.
//!
//! An [`Op`] is what an MCP tool call becomes when a board is paired. The
//! server serialises it into an `op` message; the board runs it with
//! [`crate::apply`] (document operations), in the board engine (`add_shape`,
//! `add_table`: shape geometry lives there) or in the web app (`snapshot`,
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

/// Where `write_handwriting` writes: next to items, a region or a point.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Write next to these items (e.g. the strokes of the problem).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_ids: Option<Vec<String>>,
    /// Write next to this region (slide pixels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Rect>,
    /// Start exactly here (top-left of the writing, slide pixels).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// `below` (default) or `right` of the items or region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    /// Continue earlier writing: the `group` an earlier `write_handwriting`
    /// returned. The new lines go right below it, in its ink, and join its
    /// group (so a solution streamed line by line selects and moves as one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Arguments of [`Op::SetPageDesign`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageDesignArgs {
    #[serde(default)]
    pub page: Option<u32>,
    /// Give every page this page's resulting look.
    #[serde(default)]
    pub all_pages: bool,
    /// `white`, `paper`, `chalkboard`, `blackboard` or `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub pattern: Option<PatternName>,
    /// SVG markup (no scripts, external references or animation).
    #[serde(default)]
    pub background_svg: Option<String>,
    /// PNG or JPEG bytes, base64 (a `data:` URL is fine too).
    #[serde(default)]
    pub background_image: Option<String>,
    /// `fit`, `fill`, `stretch`, `tile` or `center`.
    #[serde(default)]
    pub background_fit: Option<String>,
    #[serde(default)]
    pub background_opacity: Option<f64>,
    /// A wash of the page colour over the image, for readable ink.
    #[serde(default)]
    pub background_dim: Option<bool>,
    #[serde(default)]
    pub remove_background: bool,
}

/// Arguments of [`Op::SetBranding`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BrandingArgs {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub logo_svg: Option<String>,
    /// PNG or JPEG bytes, base64.
    #[serde(default)]
    pub logo_image: Option<String>,
    /// `top-left`, `top-right`, `bottom-left` or `bottom-right`.
    #[serde(default)]
    pub logo_corner: Option<String>,
    /// Top-left of the logo as fractions (0..1) of the page.
    #[serde(default)]
    pub logo_x: Option<f64>,
    #[serde(default)]
    pub logo_y: Option<f64>,
    /// Width as a fraction of the page width (0.02..0.6).
    #[serde(default)]
    pub logo_width: Option<f64>,
    #[serde(default)]
    pub logo_opacity: Option<f64>,
    #[serde(default)]
    pub remove_logo: bool,
    /// Text; may use placeholders such as `{lesson}`, `{page}`, `{pages}`.
    #[serde(default)]
    pub school: Option<String>,
    #[serde(default)]
    pub teacher: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub date: Option<bool>,
    /// `top` (header) or `bottom` (footer).
    #[serde(default)]
    pub text_position: Option<String>,
    /// `left`, `center` or `right`.
    #[serde(default)]
    pub text_align: Option<String>,
    /// Hide (true) or show again (false) branding on this page.
    #[serde(default)]
    pub hidden: Option<bool>,
    /// Remove the branding (of the lesson, or the page's own).
    #[serde(default)]
    pub remove: bool,
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
    /// A YouTube video or a trusted web embed (PhET, GeoGebra, Desmos,
    /// Google Slides/Docs, Wikipedia, Khan Academy) by `url`, or a video or
    /// audio file (`file_base64`) stored in the lesson.
    AddMedia {
        #[serde(default)]
        page: Option<u32>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        file_base64: Option<String>,
        #[serde(default)]
        file_name: Option<String>,
        #[serde(default)]
        title: Option<String>,
        /// Play from / to, seconds.
        #[serde(default)]
        start: Option<f64>,
        #[serde(default)]
        end: Option<f64>,
        #[serde(default)]
        muted: Option<bool>,
        #[serde(default, rename = "loop")]
        looped: Option<bool>,
        #[serde(default)]
        autoplay: Option<bool>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        height: Option<f64>,
    },
    /// A 3D model (`.glb`, or `.gltf` with everything embedded) as the
    /// built-in viewer Smart Object.
    AddModel {
        #[serde(default)]
        page: Option<u32>,
        file_base64: String,
        #[serde(default)]
        file_name: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        height: Option<f64>,
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
    /// Page design (spec: Page design): the colour, pattern and background
    /// image of a page, or of every page with `all_pages`, as one undo step.
    /// With nothing to change it returns the page's design.
    SetPageDesign(Box<PageDesignArgs>),
    /// Branding: a logo and a header or footer line on every page
    /// (`scope: "lesson"`, default) or on one page (`scope: "page"`), as one
    /// undo step. Only the given fields change. With nothing to change it
    /// returns the branding.
    SetBranding(Box<BrandingArgs>),
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
    /// Write lines (text with inline LaTeX) on the board as handwriting
    /// strokes, placed next to the anchor without covering anything, as one
    /// undo step. The board animates them stroke by stroke.
    WriteHandwriting {
        #[serde(default)]
        page: Option<u32>,
        lines: Vec<String>,
        #[serde(default)]
        anchor: Option<Anchor>,
        /// `teacher` (the teacher's calibrated hand where taught) or `neat`.
        #[serde(default)]
        style: Option<String>,
        /// Pen speed multiplier, 0.25 to 4 (default 1).
        #[serde(default)]
        speed: Option<f64>,
        /// Ink colour `#rrggbb`. Default: the anchor ink's colour.
        #[serde(default)]
        color: Option<String>,
        /// Height of a capital or digit in slide pixels. Default: matches the
        /// anchor's handwriting.
        #[serde(default)]
        size: Option<f64>,
        /// `all` (write everything) or `one-line-per-click` (the teacher
        /// reveals each line with Next). Handled by the board's animation.
        #[serde(default)]
        step_mode: Option<String>,
    },
    /// Draw a shape (one of the Shapes picker's ids: `rect`, `circle`,
    /// `arrow`, `venn2`, …) in the box `x, y, w, h`; segment shapes (line,
    /// arrows) go from `(x, y)` to `(x + w, y + h)`. Run by the board app's
    /// engine, which owns shape geometry.
    AddShape {
        #[serde(default)]
        page: Option<u32>,
        shape: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        /// Line colour `#rrggbb`. Default: readable on the page.
        #[serde(default)]
        color: Option<String>,
        /// Fill colour `#rrggbb` or `#rrggbbaa` (closed shapes). Default: none.
        #[serde(default)]
        fill: Option<String>,
        /// Line width in slide pixels. Default 4.
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        dashed: Option<bool>,
        /// Text centred inside the shape (e.g. a flowchart step).
        #[serde(default)]
        text: Option<String>,
    },
    /// Draw a table of `rows` × `cols` (1–10 each) at `x, y`, optionally with
    /// cell text (`cells[row][col]`). Run by the board app's engine.
    AddTable {
        #[serde(default)]
        page: Option<u32>,
        x: f64,
        y: f64,
        rows: u32,
        cols: u32,
        #[serde(default)]
        w: Option<f64>,
        #[serde(default)]
        h: Option<f64>,
        #[serde(default)]
        cells: Option<Vec<Vec<String>>>,
        #[serde(default)]
        color: Option<String>,
        /// Fill of the header row `#rrggbb[aa]`. Default: none.
        #[serde(default)]
        header_fill: Option<String>,
        /// Cell text size in slide pixels. Default: fits the rows (24–40).
        #[serde(default)]
        text_size: Option<f64>,
    },
    /// A "working" card on the board while the agent builds something: the
    /// spot where the result will appear, the current step, the plan and an
    /// optional live preview. Run by the board app; not saved in the lesson.
    /// See [`crate::progress`].
    ShowProgress(Box<crate::progress::ProgressArgs>),
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
            Op::AddMedia { .. } => "add_media",
            Op::AddModel { .. } => "add_model",
            Op::DeleteItem { .. } => "delete_item",
            Op::SetPageBackground { .. } => "set_page_background",
            Op::SetPageDesign(_) => "set_page_design",
            Op::SetBranding(_) => "set_branding",
            Op::ReplaceItemsWithText { .. } => "replace_items_with_text",
            Op::StepObject { .. } => "step_object",
            Op::WriteHandwriting { .. } => "write_handwriting",
            Op::AddShape { .. } => "add_shape",
            Op::AddTable { .. } => "add_table",
            Op::ShowProgress(_) => "show_progress",
        }
    }

    pub fn access(&self) -> Access {
        match self {
            Op::GetPage { .. } | Op::GetOutline | Op::GetSelection | Op::Snapshot { .. } => {
                Access::Read
            }
            Op::GoToPage { .. } | Op::PointAt { .. } | Op::ShowProgress(_) => Access::Control,
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
            Op::AddMedia { .. } => "added media",
            Op::AddModel { .. } => "added a 3D model",
            Op::DeleteItem { .. } => "deleted an item",
            Op::SetPageBackground { .. } => "changed the background",
            Op::SetPageDesign(_) => "changed the page design",
            Op::SetBranding(_) => "changed the branding",
            Op::ReplaceItemsWithText { .. } => "turned ink into text",
            Op::StepObject { .. } => "stepped a Smart Object",
            Op::WriteHandwriting { .. } => "wrote on the board",
            Op::AddShape { .. } => "drew a shape",
            Op::AddTable { .. } => "drew a table",
            Op::ShowProgress(_) => "showed its progress",
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
        let s: Op = serde_json::from_str(
            r##"{"op":"add_shape","shape":"circle","x":1,"y":2,"w":30,"h":30,"fill":"#a7f3d0"}"##,
        )
        .unwrap();
        assert_eq!((s.name(), s.access()), ("add_shape", Access::Write));
        let t: Op =
            serde_json::from_str(r#"{"op":"add_table","x":0,"y":0,"rows":3,"cols":4}"#).unwrap();
        assert_eq!(t.name(), "add_table");
    }
}
