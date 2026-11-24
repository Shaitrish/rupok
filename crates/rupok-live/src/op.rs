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

/// One stroke for [`Op::DrawStroke`]: points in slide pixels, each
/// `[x, y]` or `[x, y, pressure]` (pressure 0..1, default 0.5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StrokeArg {
    pub points: Vec<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brush: Option<String>,
}

/// Declarative behaviour of a custom stamp (no code): what happens when it
/// is placed. Unknown values are refused with a clear error.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StampBehavior {
    /// `none`, `pop` (default), `sparkle`, `bounce`, `spin` or `fade-in`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    /// A short burst around the stamp: `none`, `stars`, `confetti`,
    /// `hearts` or `sparkles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<String>,
    /// Burst colours `#rrggbb` (at most 6). Default: the stamp's colour.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub burst_colors: Vec<String>,
    /// Show a number badge counting this stamp's placements on the page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub counter: bool,
    /// A short built-in sound (respects the board's mute): `none`, `chime`,
    /// `pop`, `ding`, `tada`, `clap` or `whoosh`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    /// Fade the stamp away this many seconds after it is placed (1–600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_after: Option<f64>,
}

/// A custom stamp definition, as given to `create_stamp` / `update_stamp`
/// (every field optional for an update) and as stored (spec: Custom
/// stamps). See [`crate::stamps`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StampDef {
    /// `a-z`, `0-9`, `-`, at most 32 bytes; generated from the name when
    /// omitted. Built-in stamp ids are reserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// `svg`, `emoji`, `text` or `smart` (a tiny Smart Object).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    /// SVG markup (sanitised: no scripts, handlers, external references or
    /// animation). `currentColor` takes the stamp's colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    /// A short label drawn as a pill (`text` art).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `#rrggbb`: the pill colour, or the tint of `currentColor` in SVG.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Default height in slide pixels (24–600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, alias = "behaviour", skip_serializing_if = "Option::is_none")]
    pub behavior: Option<StampBehavior>,
    /// `smart` art: the Smart Object HTML (sandboxed, offline, no network).
    /// Not stored in the definition: it becomes a bundle in the lesson.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// `smart` art: the bundle in the lesson (set by Rupok).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    /// `smart` art: width and height of a placement (40–800, default 200).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
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

    // ------------------------------------------------------------ pages
    /// Every page: number, id, title, look and item count (lighter than
    /// `get_outline`).
    ListPages,
    /// Move pages (1-based) together so the first ends at position `to`
    /// (1-based, in the order after the move), keeping their order.
    MovePages {
        pages: Vec<u32>,
        to: u32,
    },
    /// Delete pages; a lesson keeps at least one page.
    DeletePages {
        pages: Vec<u32>,
    },
    /// Copy pages, each right after its original.
    DuplicatePages {
        pages: Vec<u32>,
    },
    SetPageTitle {
        #[serde(default)]
        page: Option<u32>,
        title: String,
    },

    // ------------------------------------------------------------ items
    /// Select items on the board (board only; the teacher sees the handles).
    SelectItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
    },
    /// Move items by (`dx`, `dy`), or so their bounds' top-left lands at
    /// (`x`, `y`).
    MoveItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        #[serde(default)]
        dx: Option<f64>,
        #[serde(default)]
        dy: Option<f64>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
    },
    /// Align (`left`, `center`, `right`, `top`, `middle`, `bottom`) and/or
    /// distribute (`horizontal`, `vertical`) items; groups move as one.
    AlignItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        #[serde(default)]
        align: Option<String>,
        #[serde(default)]
        distribute: Option<String>,
        /// `items` (default: their common bounds) or `page` (the slide frame).
        #[serde(default)]
        relative_to: Option<String>,
    },
    /// Restyle ink, shapes, text and stamps.
    RestyleItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        brush: Option<String>,
        /// Shape fill `#rrggbb[aa]`, or `none` to remove it.
        #[serde(default)]
        fill: Option<String>,
        #[serde(default)]
        opacity: Option<f64>,
        #[serde(default)]
        dashed: Option<bool>,
        /// Font size of text items.
        #[serde(default)]
        text_size: Option<f64>,
    },
    /// Scale items by `factor` about the centre of their bounds.
    ScaleItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        factor: f64,
    },
    GroupItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
    },
    UngroupItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
    },
    /// Bring items to the `front` (default) or send them to the `back`.
    ReorderItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
        #[serde(default)]
        to: Option<String>,
    },
    DeleteItems {
        #[serde(default)]
        page: Option<u32>,
        item_ids: Vec<String>,
    },
    /// Draw ink strokes from points (one stroke via `points`, or several
    /// via `strokes`), as one undo step.
    DrawStroke {
        #[serde(default)]
        page: Option<u32>,
        #[serde(default)]
        points: Option<Vec<Vec<f64>>>,
        #[serde(default)]
        strokes: Option<Vec<StrokeArg>>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        brush: Option<String>,
        #[serde(default)]
        opacity: Option<f64>,
        /// `pen` (default) or `highlighter`.
        #[serde(default)]
        tool: Option<String>,
    },
    /// Change a text box: its text, size, colour or position.
    UpdateText {
        #[serde(default)]
        page: Option<u32>,
        item_id: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        size: Option<f64>,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        w: Option<f64>,
    },

    // ----------------------------------------------------------- stamps
    /// Built-in stamps and the lesson's custom stamps (the board adds the
    /// teacher's device library).
    ListStamps,
    CreateStamp(Box<StampDef>),
    /// Change a custom stamp; only the given fields change. `base` is the
    /// definition from the board's device library when the lesson does not
    /// have it yet (set by the board, not by agents).
    UpdateStamp {
        #[serde(flatten)]
        def: Box<StampDef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<Box<StampDef>>,
    },
    /// Delete a custom stamp. Placements keep it in the lesson unless
    /// `remove_placements`. `in_library` is set by the board.
    DeleteStamp {
        id: String,
        #[serde(default)]
        remove_placements: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        in_library: bool,
    },
    /// Place a stamp (built-in or custom) at (`x`, `y`), `size` high.
    /// `def` is a custom stamp from the board's device library (set by the
    /// board, not by agents).
    AddStamp {
        #[serde(default)]
        page: Option<u32>,
        stamp: String,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
        #[serde(default)]
        size: Option<f64>,
        #[serde(default)]
        color: Option<String>,
        /// The emoji of the `emoji` stamp, a badge number, a label's words.
        #[serde(default)]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        def: Option<Box<StampDef>>,
    },

    // ------------------------------------------- board app (live only)
    /// Switch mode: `prepare`, `teach` or `student`.
    SetMode {
        mode: String,
    },
    /// Zoom the view (`zoom`: 1 = 100 %) around (`x`, `y`) in slide pixels.
    ZoomTo {
        #[serde(default)]
        zoom: Option<f64>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
    },
    /// Fit the page to the screen.
    FitPage,
    StartReplay {
        #[serde(default)]
        page: Option<u32>,
        /// 0.5–8, default 2.
        #[serde(default)]
        speed: Option<f64>,
    },
    StopReplay,
    /// A short burst of stars and confetti across the board (not saved).
    Celebrate {
        #[serde(default)]
        kind: Option<String>,
    },
    /// The teacher's current tool and its colour, width and pen style.
    SetTool {
        tool: String,
        #[serde(default)]
        color: Option<String>,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        brush: Option<String>,
    },
    /// Undo the agent's own last change, if it is still the newest.
    Undo,
    /// Make a PDF of the lesson on the board; it answers the bytes
    /// (`pdf_base64`) and, with `download`, also saves it on the board.
    ExportPdf {
        /// `all` (default), `current`, or a range like `2-5`.
        #[serde(default)]
        pages: Option<String>,
        /// `slides` (default), `handout-2` or `handout-3`.
        #[serde(default)]
        layout: Option<String>,
        #[serde(default)]
        ink: Option<bool>,
        #[serde(default)]
        print_friendly: Option<bool>,
        #[serde(default)]
        download: bool,
    },
    LibraryList {
        #[serde(default)]
        folder: Option<String>,
        #[serde(default)]
        query: Option<String>,
    },
    LibraryOpen {
        id: String,
    },
    LibraryCreateLesson {
        title: String,
        #[serde(default)]
        folder: Option<String>,
        /// `blank`, `slides`, `lined` or `grid`.
        #[serde(default)]
        template: Option<String>,
        #[serde(default)]
        open: bool,
    },
    LibraryMove {
        id: String,
        folder: Option<String>,
    },
    LibraryRename {
        id: String,
        name: String,
    },
    LibraryTrash {
        id: String,
    },
    GetSettings,
    SetSetting {
        key: String,
        value: serde_json::Value,
    },
    GetDockLayout {
        #[serde(default)]
        mode: Option<String>,
    },
    SetDockLayout {
        #[serde(default)]
        mode: Option<String>,
        layout: serde_json::Value,
    },
    ListDockItems,
    SaveDockPreset {
        name: String,
        #[serde(default)]
        modes: Option<Vec<String>>,
    },
    ApplyDockPreset {
        name: String,
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
            Op::ListPages => "list_pages",
            Op::MovePages { .. } => "move_pages",
            Op::DeletePages { .. } => "delete_pages",
            Op::DuplicatePages { .. } => "duplicate_pages",
            Op::SetPageTitle { .. } => "set_page_title",
            Op::SelectItems { .. } => "select_items",
            Op::MoveItems { .. } => "move_items",
            Op::AlignItems { .. } => "align_items",
            Op::RestyleItems { .. } => "restyle_items",
            Op::ScaleItems { .. } => "scale_items",
            Op::GroupItems { .. } => "group_items",
            Op::UngroupItems { .. } => "ungroup_items",
            Op::ReorderItems { .. } => "reorder_items",
            Op::DeleteItems { .. } => "delete_items",
            Op::DrawStroke { .. } => "draw_stroke",
            Op::UpdateText { .. } => "update_text",
            Op::ListStamps => "list_stamps",
            Op::CreateStamp(_) => "create_stamp",
            Op::UpdateStamp { .. } => "update_stamp",
            Op::DeleteStamp { .. } => "delete_stamp",
            Op::AddStamp { .. } => "add_stamp",
            Op::SetMode { .. } => "set_mode",
            Op::ZoomTo { .. } => "zoom_to",
            Op::FitPage => "fit_page",
            Op::StartReplay { .. } => "start_replay",
            Op::StopReplay => "stop_replay",
            Op::Celebrate { .. } => "celebrate",
            Op::SetTool { .. } => "set_tool",
            Op::Undo => "undo",
            Op::ExportPdf { .. } => "export_pdf",
            Op::LibraryList { .. } => "library_list",
            Op::LibraryOpen { .. } => "library_open",
            Op::LibraryCreateLesson { .. } => "library_create_lesson",
            Op::LibraryMove { .. } => "library_move",
            Op::LibraryRename { .. } => "library_rename",
            Op::LibraryTrash { .. } => "library_trash",
            Op::GetSettings => "get_settings",
            Op::SetSetting { .. } => "set_setting",
            Op::GetDockLayout { .. } => "get_dock_layout",
            Op::SetDockLayout { .. } => "set_dock_layout",
            Op::ListDockItems => "list_dock_items",
            Op::SaveDockPreset { .. } => "save_dock_preset",
            Op::ApplyDockPreset { .. } => "apply_dock_preset",
        }
    }

    pub fn access(&self) -> Access {
        match self {
            Op::GetPage { .. }
            | Op::GetOutline
            | Op::GetSelection
            | Op::Snapshot { .. }
            | Op::ListPages
            | Op::ListStamps
            | Op::LibraryList { .. }
            | Op::GetSettings
            | Op::GetDockLayout { .. }
            | Op::ListDockItems => Access::Read,
            Op::GoToPage { .. }
            | Op::PointAt { .. }
            | Op::SelectItems { .. }
            | Op::SetMode { .. }
            | Op::ZoomTo { .. }
            | Op::FitPage
            | Op::StartReplay { .. }
            | Op::StopReplay
            | Op::Celebrate { .. }
            | Op::SetTool { .. }
            | Op::ExportPdf { .. }
            | Op::LibraryOpen { .. } => Access::Control,
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

    /// Ops the board app runs itself (UI state, the library, settings):
    /// [`crate::apply`] refuses them and lesson files cannot run them.
    pub fn board_only(&self) -> bool {
        matches!(
            self,
            Op::Snapshot { .. }
                | Op::PointAt { .. }
                | Op::SetMode { .. }
                | Op::ZoomTo { .. }
                | Op::FitPage
                | Op::StartReplay { .. }
                | Op::StopReplay
                | Op::Celebrate { .. }
                | Op::SetTool { .. }
                | Op::Undo
                | Op::ExportPdf { .. }
                | Op::LibraryList { .. }
                | Op::LibraryOpen { .. }
                | Op::LibraryCreateLesson { .. }
                | Op::LibraryMove { .. }
                | Op::LibraryRename { .. }
                | Op::LibraryTrash { .. }
                | Op::GetSettings
                | Op::SetSetting { .. }
                | Op::GetDockLayout { .. }
                | Op::SetDockLayout { .. }
                | Op::ListDockItems
                | Op::SaveDockPreset { .. }
                | Op::ApplyDockPreset { .. }
        )
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
            Op::ListPages => "read the lesson",
            Op::MovePages { .. } => "moved pages",
            Op::DeletePages { .. } => "deleted pages",
            Op::DuplicatePages { .. } => "duplicated pages",
            Op::SetPageTitle { .. } => "renamed a page",
            Op::SelectItems { .. } => "selected items",
            Op::MoveItems { .. } => "moved items",
            Op::AlignItems { .. } => "aligned items",
            Op::RestyleItems { .. } => "restyled items",
            Op::ScaleItems { .. } => "resized items",
            Op::GroupItems { .. } => "grouped items",
            Op::UngroupItems { .. } => "ungrouped items",
            Op::ReorderItems { .. } => "reordered items",
            Op::DeleteItems { .. } => "deleted items",
            Op::DrawStroke { .. } => "drew on the board",
            Op::UpdateText { .. } => "edited text",
            Op::ListStamps => "read the stamps",
            Op::CreateStamp(_) => "made a stamp",
            Op::UpdateStamp { .. } => "changed a stamp",
            Op::DeleteStamp { .. } => "deleted a stamp",
            Op::AddStamp { .. } => "placed a stamp",
            Op::SetMode { .. } => "changed the mode",
            Op::ZoomTo { .. } => "zoomed",
            Op::FitPage => "fitted the page",
            Op::StartReplay { .. } => "started a replay",
            Op::StopReplay => "stopped the replay",
            Op::Celebrate { .. } => "celebrated",
            Op::SetTool { .. } => "changed the tool",
            Op::Undo => "undid its change",
            Op::ExportPdf { .. } => "exported a PDF",
            Op::LibraryList { .. } => "read the library",
            Op::LibraryOpen { .. } => "opened a lesson",
            Op::LibraryCreateLesson { .. } => "created a lesson",
            Op::LibraryMove { .. } => "moved a lesson",
            Op::LibraryRename { .. } => "renamed a lesson",
            Op::LibraryTrash { .. } => "moved a lesson to the trash",
            Op::GetSettings => "read the settings",
            Op::SetSetting { .. } => "changed a setting",
            Op::GetDockLayout { .. } => "read the dock",
            Op::SetDockLayout { .. } => "changed the dock",
            Op::ListDockItems => "read the dock",
            Op::SaveDockPreset { .. } => "saved a dock layout",
            Op::ApplyDockPreset { .. } => "changed the dock",
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
