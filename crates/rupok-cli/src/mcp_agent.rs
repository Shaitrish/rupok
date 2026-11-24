//! MCP tools that make every part of Rupok controllable by an agent
//! (docs/agent-capabilities.md): pages, items (select, move, align,
//! restyle, scale, group, reorder, delete, draw, edit text), stamps and
//! custom stamps, modes and the view, replay, PDF export, the Library,
//! settings and the Smart Dock.
//!
//! Document tools take an optional `path`: omitted or `"live"` runs on the
//! paired board (one attributed undo step there), a `.rpk` path runs the
//! same [`rupok_live`] code on a lesson file. Board-app tools (modes, view,
//! library, settings, dock, export) need the live board. The board enforces
//! "Allow AI to edit live" and Student view for all of them, and settings
//! never include AI keys, accounts, payments or permissions.

use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_router};
use rupok_live::{Op, StampBehavior, StampDef, StrokeArg};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::McpServer;
use crate::mcp_live::is_live;

// ------------------------------------------------------------------ params

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PathOnly {
    /// `"live"` (default) for the lesson open on the paired board, or a
    /// `.rpk` path in the workspace.
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PagesParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    /// 1-based page numbers, e.g. `[3, 4]`.
    pub pages: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovePagesParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    /// 1-based page numbers to move (they stay in order, side by side).
    pub pages: Vec<u32>,
    /// Where the first moved page ends up, 1-based, counted after the move
    /// (1 = the front; the page count minus the moved pages plus 1 = the end).
    #[schemars(range(min = 1))]
    pub to: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageTitleParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    /// 1-based page. Omit for the current page (page 1 in a file).
    #[serde(default)]
    pub page: Option<u32>,
    /// The page's name in the pages panel (not drawn on the slide).
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemsParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    /// 1-based page. Omit for the current page (page 1 in a file).
    #[serde(default)]
    pub page: Option<u32>,
    /// Item ids from get_current_page, get_selection or get_lesson_outline.
    pub item_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectParams {
    /// 1-based page (the board shows it). Omit for the current page.
    #[serde(default)]
    pub page: Option<u32>,
    /// Items to select; `[]` clears the selection.
    pub item_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveItemsParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    pub item_ids: Vec<String>,
    /// Move by this much (slide pixels).
    #[serde(default)]
    pub dx: Option<f64>,
    #[serde(default)]
    pub dy: Option<f64>,
    /// Or move so the items' bounds start here (top-left, slide pixels).
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AlignParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    pub item_ids: Vec<String>,
    /// `left`, `center`, `right`, `top`, `middle` or `bottom`.
    #[serde(default)]
    pub align: Option<String>,
    /// `horizontal` or `vertical`: equal gaps (three or more items).
    #[serde(default)]
    pub distribute: Option<String>,
    /// `items` (default: their common bounds) or `page` (the slide).
    #[serde(default)]
    pub relative_to: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RestyleParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    pub item_ids: Vec<String>,
    /// Ink, text and stamp colour `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    /// Line width in slide pixels (0.5–200).
    #[serde(default)]
    pub width: Option<f64>,
    /// Pen style: pen, ballpoint, fountain, marker, pencil, chalk, brush,
    /// dashed, dotted, neon or rainbow.
    #[serde(default)]
    pub brush: Option<String>,
    /// Shape fill `#rrggbb` or `#rrggbbaa` (e.g. `#20c99733`), or `none`.
    #[serde(default)]
    pub fill: Option<String>,
    /// Ink opacity 0.05–1.
    #[serde(default)]
    pub opacity: Option<f64>,
    /// Dashed shape outline.
    #[serde(default)]
    pub dashed: Option<bool>,
    /// Font size of text boxes, slide pixels.
    #[serde(default)]
    pub text_size: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScaleParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    pub item_ids: Vec<String>,
    /// 2 = twice as big, 0.5 = half (0.05–20), about the items' centre.
    pub factor: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrokeParam {
    /// `[[x, y], [x, y, pressure], …]` in slide pixels; pressure 0–1.
    pub points: Vec<Vec<f64>>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub brush: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DrawStrokeParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    /// One stroke: `[[x, y], …]` or `[[x, y, pressure], …]`, slide pixels.
    #[serde(default)]
    pub points: Option<Vec<Vec<f64>>>,
    /// Several strokes, each with its own points and optional style.
    #[serde(default)]
    pub strokes: Option<Vec<StrokeParam>>,
    /// Ink colour `#rrggbb`. Default: readable on the page.
    #[serde(default)]
    pub color: Option<String>,
    /// Pen width in slide pixels. Default 4 (highlighter 22).
    #[serde(default)]
    pub width: Option<f64>,
    /// Pen style (see restyle_items).
    #[serde(default)]
    pub brush: Option<String>,
    #[serde(default)]
    pub opacity: Option<f64>,
    /// `pen` (default) or `highlighter`.
    #[serde(default)]
    pub tool: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateTextParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    pub item_id: String,
    #[serde(default)]
    pub text: Option<String>,
    /// Font size, slide pixels (keep ≥ 24 for projectors).
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub w: Option<f64>,
}

/// A stamp's behaviour (no code).
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StampBehaviorArg {
    /// Placement animation: none, pop (default), sparkle, bounce, spin, fade-in.
    #[serde(default)]
    pub animation: Option<String>,
    /// A short burst around the stamp: none, stars, confetti, hearts, sparkles.
    #[serde(default)]
    pub burst: Option<String>,
    /// Burst colours `#rrggbb`, at most 6.
    #[serde(default)]
    pub burst_colors: Option<Vec<String>>,
    /// A number badge counting this stamp on the page (1, 2, 3…).
    #[serde(default)]
    pub counter: Option<bool>,
    /// A short sound (the board's mute wins): none, chime, pop, ding, tada, clap, whoosh.
    #[serde(default)]
    pub sound: Option<String>,
    /// Fade away this many seconds after it is placed (1–600; 0 = never).
    #[serde(default)]
    pub fade_after: Option<f64>,
}

impl From<StampBehaviorArg> for StampBehavior {
    fn from(b: StampBehaviorArg) -> Self {
        StampBehavior {
            animation: b.animation,
            burst: b.burst,
            burst_colors: b.burst_colors.unwrap_or_default(),
            counter: b.counter.unwrap_or(false),
            sound: b.sound,
            fade_after: b.fade_after,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StampParams {
    /// `"live"` (default: the board and the teacher's stamp library) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    /// For update: the stamp to change. For create: optional id (a-z, 0-9,
    /// -; generated from the name when omitted).
    #[serde(default)]
    pub id: Option<String>,
    /// Shown in the Stamps palette, e.g. "Super star".
    #[serde(default)]
    pub name: Option<String>,
    /// Search words, e.g. ["praise", "gold"].
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// `svg`, `emoji`, `text` (a pill with a short label) or `smart` (a tiny
    /// Smart Object with your logic, e.g. a dice that rolls on tap).
    #[serde(default)]
    pub art: Option<String>,
    /// SVG markup, simple and bold, viewBox about 100×100. Use
    /// `fill="currentColor"` to let the teacher recolour it. No scripts,
    /// event handlers, external references or SMIL animation (refused).
    #[serde(default)]
    pub svg: Option<String>,
    #[serde(default)]
    pub emoji: Option<String>,
    /// The pill's label (1–24 characters), e.g. "Well done!".
    #[serde(default)]
    pub text: Option<String>,
    /// `#rrggbb`: the pill colour, or the SVG's currentColor.
    #[serde(default)]
    pub color: Option<String>,
    /// Default height, slide pixels (24–600; default 96).
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub behavior: Option<StampBehaviorArg>,
    /// `smart` art: complete HTML for a small Smart Object (read_object_spec;
    /// sandboxed, offline, no network). Make the whole body tappable.
    #[serde(default)]
    pub html: Option<String>,
    /// `smart` art: placement width and height (40–800, default 200).
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
}

impl StampParams {
    fn def(self) -> (Option<String>, StampDef) {
        (
            self.path,
            StampDef {
                id: self.id,
                name: self.name,
                keywords: self.keywords.unwrap_or_default(),
                art: self.art,
                svg: self.svg,
                emoji: self.emoji,
                text: self.text,
                color: self.color,
                size: self.size,
                behavior: self.behavior.map(Into::into),
                html: self.html,
                bundle: None,
                width: self.width,
                height: self.height,
            },
        )
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteStampParams {
    /// `"live"` (default: the board and the teacher's stamp library) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    pub id: String,
    /// Also remove the placed stamps from the pages. Default false: a stamp
    /// still on a page keeps its definition in the lesson.
    #[serde(default)]
    pub remove_placements: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddStampParams {
    /// `"live"` (default) or a `.rpk` path.
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub page: Option<u32>,
    /// A stamp id from list_stamps: built-in (`star`, `tick`, `excellent`,
    /// `badge`, `emoji`, …) or custom.
    pub stamp: String,
    /// Top-left in slide pixels. Default: centred.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// Height in slide pixels (16–800). Default: the stamp's size (96).
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    /// The emoji for `emoji`, a number for `badge`, the words of a pill.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModeParams {
    /// `prepare`, `teach` or `student`. Leaving Student view is the teacher's call.
    pub mode: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ZoomParams {
    /// 1 = 100 %, 2 = 200 % (0.1–8).
    #[serde(default)]
    pub zoom: Option<f64>,
    /// Centre on this point (slide pixels). Default: the middle of the screen.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayParams {
    /// 1-based page to replay. Default: the current page.
    #[serde(default)]
    pub page: Option<u32>,
    /// 0.5–8, default 2.
    #[serde(default)]
    pub speed: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CelebrateParams {
    /// `stars` (default), `confetti` or `hearts`.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetToolParams {
    /// pen, highlighter, eraser, lasso (select), laser, text, shape, stamp, hand or interact.
    pub tool: String,
    /// Ink colour `#rrggbb` (pen, highlighter).
    #[serde(default)]
    pub color: Option<String>,
    /// Pen width in slide pixels.
    #[serde(default)]
    pub width: Option<f64>,
    /// Pen style (see restyle_items).
    #[serde(default)]
    pub brush: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportPdfParams {
    /// `all` (default), `current`, or a range like `2-5`.
    #[serde(default)]
    pub pages: Option<String>,
    /// `slides` (default), `handout-2` or `handout-3`.
    #[serde(default)]
    pub layout: Option<String>,
    /// Include the ink (default true).
    #[serde(default)]
    pub ink: Option<bool>,
    /// White pages and dark ink for printing.
    #[serde(default)]
    pub print_friendly: Option<bool>,
    /// Save the PDF here in the workspace (must end in .pdf). Default:
    /// `<lesson title>.pdf`.
    #[serde(default)]
    pub save_as: Option<String>,
    /// Also download it on the board (the teacher's browser or app).
    #[serde(default)]
    pub download: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryListParams {
    /// A folder id from a previous library_list. Default: everything.
    #[serde(default)]
    pub folder: Option<String>,
    /// Search in titles.
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryIdParams {
    /// A lesson (or folder) id from library_list.
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryCreateParams {
    pub title: String,
    /// Folder id from library_list. Default: the top of the library.
    #[serde(default)]
    pub folder: Option<String>,
    /// `blank` (default), `slides`, `lined` or `grid`.
    #[serde(default)]
    pub template: Option<String>,
    /// Open it on the board right away (the open lesson is saved first).
    #[serde(default)]
    pub open: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryMoveParams {
    pub id: String,
    /// Destination folder id; omit or null for the top of the library.
    #[serde(default)]
    pub folder: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryRenameParams {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetSettingParams {
    /// A key from get_settings, e.g. `theme`, `accent`, `density`,
    /// `pen.style`, `pen.color`, `pen.width`, `smartShapes`.
    pub key: String,
    /// The new value (a string, number or boolean as get_settings shows).
    pub value: Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DockModeParams {
    /// `prepare` or `teach`. Default: the mode on screen.
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetDockParams {
    /// `prepare` or `teach`. Default: the mode on screen.
    #[serde(default)]
    pub mode: Option<String>,
    /// The dock: `{position, orientation, size, large, groups: [{label?,
    /// items: [ids]}], split: {position, groups}}`; ids from list_dock_items.
    /// Only the given keys change. Pen, eraser and undo always stay.
    pub layout: Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveDockPresetParams {
    /// A name for the teacher's layout list, e.g. "Science lab".
    pub name: String,
    /// Which modes it stores (default both).
    #[serde(default)]
    pub modes: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DockPresetParams {
    /// "Left-handed", "Minimal teaching", "Maths class", "Default" or a
    /// layout the teacher saved.
    pub name: String,
}

// ------------------------------------------------------------------- tools

fn live_or(path: &Option<String>) -> Option<String> {
    match path.as_deref() {
        None => None,
        Some(p) if is_live(p) => None,
        Some(p) => Some(p.to_string()),
    }
}

/// Keys that are never an agent's business, refused before they reach the
/// board (the board has its own allowlist too).
fn forbidden_setting(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    [
        "key",
        "token",
        "secret",
        "password",
        "cloud",
        "account",
        "billing",
        "payment",
        "plan",
        "permission",
        "allowedit",
        "allow_edit",
        "chatpermissions",
        "chatallownoprompt",
        "connect",
    ]
    .iter()
    .any(|w| k.contains(w))
}

/// A safe file name from a lesson title.
fn pdf_name(title: &str) -> String {
    let mut s: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    s = s.trim().chars().take(80).collect();
    if s.is_empty() {
        s = "lesson".into();
    }
    format!("{s}.pdf")
}

impl McpServer {
    /// A document op: on the board (`path` omitted or `"live"`) or a file.
    async fn doc(&self, path: Option<String>, op: Op) -> CallToolResult {
        match live_or(&path) {
            None => self.live_call(op).await,
            Some(p) => self.run(move |t| t.doc_op(&p, op)).await,
        }
    }
}

#[tool_router(router = agent_tool_router, vis = "pub(crate)")]
impl McpServer {
    // ------------------------------------------------------------- pages
    #[tool(
        description = "List every page of a lesson: number, id, title (or its first text), colour, pattern \
and how many items it has. Lighter than get_lesson_outline. Omit `path` for the live board.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_pages(&self, Parameters(p): Parameters<PathOnly>) -> CallToolResult {
        self.doc(p.path, Op::ListPages).await
    }

    #[tool(
        description = "Reorder pages: move the given pages (1-based) together so the first lands at position \
`to` (counted after the move). One undo step. Example: move page 5 to the front with pages [5], to 1.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn move_pages(&self, Parameters(p): Parameters<MovePagesParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::MovePages {
                pages: p.pages,
                to: p.to,
            },
        )
        .await
    }

    #[tool(
        description = "Delete pages (1-based). A lesson keeps at least one page. Confirm with the teacher first. \
One undo step brings them all back.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_pages(&self, Parameters(p): Parameters<PagesParams>) -> CallToolResult {
        self.doc(p.path, Op::DeletePages { pages: p.pages }).await
    }

    #[tool(
        description = "Duplicate pages (1-based), each copy right after its original, with everything on it. \
One undo step; returns the copies' page numbers.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn duplicate_pages(&self, Parameters(p): Parameters<PagesParams>) -> CallToolResult {
        self.doc(p.path, Op::DuplicatePages { pages: p.pages })
            .await
    }

    #[tool(
        description = "Rename a page (its title in the pages panel; not drawn on the slide).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_page_title(&self, Parameters(p): Parameters<PageTitleParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::SetPageTitle {
                page: p.page,
                title: p.title,
            },
        )
        .await
    }

    // ------------------------------------------------------------- items
    #[tool(
        description = "Select items on the live board, as if the teacher lassoed them (the selection toolbar \
appears). `[]` clears it. Use it to show the teacher what you mean before changing it.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn select_items(&self, Parameters(p): Parameters<SelectParams>) -> CallToolResult {
        self.live_call(Op::SelectItems {
            page: p.page,
            item_ids: p.item_ids,
        })
        .await
    }

    #[tool(
        description = "Move items by (dx, dy), or so their common bounds start at (x, y), in slide pixels. \
Ink, text, shapes, images and Smart Objects all move. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn move_items(&self, Parameters(p): Parameters<MoveItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::MoveItems {
                page: p.page,
                item_ids: p.item_ids,
                dx: p.dx,
                dy: p.dy,
                x: p.x,
                y: p.y,
            },
        )
        .await
    }

    #[tool(
        description = "Align items (left, center, right, top, middle, bottom) to their common bounds or to the \
slide, and/or distribute them with equal gaps (horizontal, vertical; three or more). Grouped items (e.g. a \
handwritten word) move as one. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn align_items(&self, Parameters(p): Parameters<AlignParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::AlignItems {
                page: p.page,
                item_ids: p.item_ids,
                align: p.align,
                distribute: p.distribute,
                relative_to: p.relative_to,
            },
        )
        .await
    }

    #[tool(
        description = "Restyle items: colour (ink, text, stamps), line width, pen style (brush), shape fill \
(`#rrggbbaa` or none), dashed outline, ink opacity and text size. Highlighter ink stays translucent. Images and \
Smart Objects are skipped. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn restyle_items(&self, Parameters(p): Parameters<RestyleParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::RestyleItems {
                page: p.page,
                item_ids: p.item_ids,
                color: p.color,
                width: p.width,
                brush: p.brush,
                fill: p.fill,
                opacity: p.opacity,
                dashed: p.dashed,
                text_size: p.text_size,
            },
        )
        .await
    }

    #[tool(
        description = "Make items bigger or smaller by `factor` about their centre: ink, shapes (still exact \
shapes), text (font size too), images and Smart Objects. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn scale_items(&self, Parameters(p): Parameters<ScaleParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::ScaleItems {
                page: p.page,
                item_ids: p.item_ids,
                factor: p.factor,
            },
        )
        .await
    }

    #[tool(
        description = "Group items so they select, move and delete together (at least two). One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn group_items(&self, Parameters(p): Parameters<ItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::GroupItems {
                page: p.page,
                item_ids: p.item_ids,
            },
        )
        .await
    }

    #[tool(
        description = "Ungroup: dissolve the groups the given items belong to. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn ungroup_items(&self, Parameters(p): Parameters<ItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::UngroupItems {
                page: p.page,
                item_ids: p.item_ids,
            },
        )
        .await
    }

    #[tool(
        description = "Bring items to the front (on top of everything else on the page). One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn bring_to_front(&self, Parameters(p): Parameters<ItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::ReorderItems {
                page: p.page,
                item_ids: p.item_ids,
                to: Some("front".into()),
            },
        )
        .await
    }

    #[tool(
        description = "Send items to the back (behind everything else on the page). One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn send_to_back(&self, Parameters(p): Parameters<ItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::ReorderItems {
                page: p.page,
                item_ids: p.item_ids,
                to: Some("back".into()),
            },
        )
        .await
    }

    #[tool(
        description = "Delete several items at once (ink, text, shapes, stamps, images, Smart Objects). Never \
delete the teacher's work without asking first. One undo step brings them all back.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_items(&self, Parameters(p): Parameters<ItemsParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::DeleteItems {
                page: p.page,
                item_ids: p.item_ids,
            },
        )
        .await
    }

    #[tool(
        description = "Draw ink on the board from points: one stroke (`points`) or several (`strokes`), in a \
colour, width and pen style (pen, ballpoint, fountain, marker, pencil, chalk, brush, dashed, dotted, neon, \
rainbow), as pen or highlighter. Points are slide pixels, optionally with pressure. Use it to underline, circle \
or tick; for shapes prefer add_shape, for words add_text or write_handwriting. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn draw_stroke(&self, Parameters(p): Parameters<DrawStrokeParams>) -> CallToolResult {
        let strokes = p.strokes.map(|v| {
            v.into_iter()
                .map(|s| StrokeArg {
                    points: s.points,
                    color: s.color,
                    width: s.width,
                    brush: s.brush,
                })
                .collect()
        });
        self.doc(
            p.path,
            Op::DrawStroke {
                page: p.page,
                points: p.points,
                strokes,
                color: p.color,
                width: p.width,
                brush: p.brush,
                opacity: p.opacity,
                tool: p.tool,
            },
        )
        .await
    }

    #[tool(
        description = "Edit a text box: its text, font size, colour or position. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn update_text(&self, Parameters(p): Parameters<UpdateTextParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::UpdateText {
                page: p.page,
                item_id: p.item_id,
                text: p.text,
                size: p.size,
                color: p.color,
                x: p.x,
                y: p.y,
                w: p.w,
            },
        )
        .await
    }

    // ------------------------------------------------------------ stamps
    #[tool(
        description = "List the stamps: built-in (star, tick, cross, excellent, badge, emoji, arrows…), the \
lesson's custom stamps (with how often each is placed) and, live, the teacher's own library (\"My stamps\"). \
Also lists the animations, bursts and sounds a custom stamp can use.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_stamps(&self, Parameters(p): Parameters<PathOnly>) -> CallToolResult {
        self.doc(p.path, Op::ListStamps).await
    }

    #[tool(
        description = "Make a custom stamp for the teacher. Artwork: `svg` (bold and simple, viewBox ~100×100, \
fill=\"currentColor\" so it can be recoloured), `emoji`, or `text` (a short pill label). Behaviour without code: \
animation (pop, sparkle, bounce, spin, fade-in), a burst (stars, confetti, hearts, sparkles; colours), a counter \
badge, a sound (chime, pop, ding, tada, clap, whoosh; the board's mute wins) and fade_after seconds. Or `art: \
\"smart\"` with `html`: a tiny Smart Object with your own logic (a dice that rolls on tap, a timer, a tap counter) \
that runs in the Smart Object sandbox (offline, no network; read_object_spec). Live, it joins the teacher's \"My \
stamps\" and the lesson. One undo step. Keep stamps readable on a projector, celebratory but brief.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_stamp(&self, Parameters(p): Parameters<StampParams>) -> CallToolResult {
        let (path, def) = p.def();
        self.doc(path, Op::CreateStamp(Box::new(def))).await
    }

    #[tool(
        description = "Change a custom stamp by id: only the fields you give change (name, keywords, artwork, \
colour, size, behaviour, Smart stamp html). Every placement shows the new look. Built-in stamps cannot change.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn update_stamp(&self, Parameters(p): Parameters<StampParams>) -> CallToolResult {
        if p.id.is_none() {
            return CallToolResult::structured_error(
                json!({ "error": "id is required: the custom stamp to change (list_stamps)" }),
            );
        }
        let (path, def) = p.def();
        self.doc(
            path,
            Op::UpdateStamp {
                def: Box::new(def),
                base: None,
            },
        )
        .await
    }

    #[tool(
        description = "Delete a custom stamp from the teacher's library (live) and the lesson. A stamp still \
placed on a page keeps its definition in the lesson unless remove_placements is true (which also removes \
those stamps). Ask the teacher first.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_stamp(&self, Parameters(p): Parameters<DeleteStampParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::DeleteStamp {
                id: p.id,
                remove_placements: p.remove_placements.unwrap_or(false),
                in_library: false,
            },
        )
        .await
    }

    #[tool(
        description = "Place a stamp on a page by id (built-in or custom) at (x, y) with a size: a star for a \
good answer, a tick, \"Excellent!\", a numbered badge (`badge`, numbers itself), any emoji (`emoji` with text) \
or the teacher's own. Its behaviour (animation, burst, sound) plays on the board. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_stamp(&self, Parameters(p): Parameters<AddStampParams>) -> CallToolResult {
        self.doc(
            p.path,
            Op::AddStamp {
                page: p.page,
                stamp: p.stamp,
                x: p.x,
                y: p.y,
                size: p.size,
                color: p.color,
                text: p.text,
                def: None,
            },
        )
        .await
    }

    // ------------------------------------------------- the board (live)
    #[tool(
        description = "Switch the board's mode: `prepare` (build the lesson), `teach` (present full screen) or \
`student` (read-only). The teacher's permission is asked like an edit; leaving Student view is refused (the \
teacher does that).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_mode(&self, Parameters(p): Parameters<ModeParams>) -> CallToolResult {
        if !matches!(p.mode.as_str(), "prepare" | "teach" | "student") {
            return CallToolResult::structured_error(
                json!({ "error": "mode must be prepare, teach or student" }),
            );
        }
        self.live_call(Op::SetMode { mode: p.mode }).await
    }

    #[tool(
        description = "Zoom the board's view (1 = 100 %) around a point on the slide, e.g. to show detail. \
fit_page goes back.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn zoom_to(&self, Parameters(p): Parameters<ZoomParams>) -> CallToolResult {
        self.live_call(Op::ZoomTo {
            zoom: p.zoom,
            x: p.x,
            y: p.y,
        })
        .await
    }

    #[tool(
        description = "Fit the whole page on the screen again.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn fit_page(&self) -> CallToolResult {
        self.live_call(Op::FitPage).await
    }

    #[tool(
        description = "Replay a page stroke by stroke, as it was written (great for revising how a diagram \
was built).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn start_replay(&self, Parameters(p): Parameters<ReplayParams>) -> CallToolResult {
        self.live_call(Op::StartReplay {
            page: p.page,
            speed: p.speed,
        })
        .await
    }

    #[tool(
        description = "Stop a running replay.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn stop_replay(&self) -> CallToolResult {
        self.live_call(Op::StopReplay).await
    }

    #[tool(
        description = "A short celebration across the board (stars, confetti or hearts) for a class \
achievement. Nothing is saved.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn celebrate(&self, Parameters(p): Parameters<CelebrateParams>) -> CallToolResult {
        self.live_call(Op::Celebrate { kind: p.kind }).await
    }

    #[tool(
        description = "Pick the teacher's current tool (pen, highlighter, eraser, lasso, laser, text, shape, \
stamp, hand, interact) and, for ink, its colour, width and pen style, e.g. \"give me a red chalk pen\".",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_tool(&self, Parameters(p): Parameters<SetToolParams>) -> CallToolResult {
        self.live_call(Op::SetTool {
            tool: p.tool,
            color: p.color,
            width: p.width,
            brush: p.brush,
        })
        .await
    }

    #[tool(
        description = "Undo your own last change on the board, if it is still the newest change (it never \
undoes the teacher's work).",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn undo_last_change(&self) -> CallToolResult {
        self.live_call(Op::Undo).await
    }

    #[tool(
        description = "Export the lesson on the board as a PDF (all pages, the current one or a range; slides \
or handouts; print-friendly) and save it in the workspace (`save_as`, default `<title>.pdf`); `download` also \
saves it on the teacher's board. Returns where it was saved.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn export_pdf(&self, Parameters(p): Parameters<ExportPdfParams>) -> CallToolResult {
        if let Some(s) = &p.save_as
            && let Err(e) = self.tools.workspace().resolve_new_pdf(s)
        {
            return CallToolResult::structured_error(json!({ "error": e.to_string() }));
        }
        let op = Op::ExportPdf {
            pages: p.pages,
            layout: p.layout,
            ink: p.ink,
            print_friendly: p.print_friendly,
            download: p.download.unwrap_or(false),
        };
        let mut v = match self.live.call(op).await {
            Ok(v) => v,
            Err(e) => return CallToolResult::structured_error(e.to_json()),
        };
        let b64 = v
            .get("pdf_base64")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let bytes = match base64::engine::general_purpose::STANDARD.decode(b64.as_bytes()) {
            Ok(b) if b.starts_with(b"%PDF") => b,
            _ => {
                return CallToolResult::structured_error(
                    json!({ "error": "the board did not return a PDF" }),
                );
            }
        };
        let name = p.save_as.unwrap_or_else(|| {
            pdf_name(v.get("title").and_then(Value::as_str).unwrap_or("lesson"))
        });
        let tools = self.tools.clone();
        let len = bytes.len();
        let saved = tokio::task::spawn_blocking(move || -> Result<String, String> {
            let ws = tools.workspace();
            let abs = ws.resolve_new_pdf(&name).map_err(|e| e.to_string())?;
            if let Some(dir) = abs.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&abs, &bytes).map_err(|e| e.to_string())?;
            Ok(ws.display(&abs))
        })
        .await;
        match saved {
            Ok(Ok(path)) => {
                if let Some(m) = v.as_object_mut() {
                    m.remove("pdf_base64");
                    m.insert("saved_to".into(), json!(path));
                    m.insert("bytes".into(), json!(len));
                }
                CallToolResult::structured(v)
            }
            Ok(Err(e)) => CallToolResult::structured_error(json!({ "error": e })),
            Err(e) => {
                CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
            }
        }
    }

    // ----------------------------------------------------------- library
    #[tool(
        description = "The teacher's Rupok Library on the board (browser storage, a folder or the desktop app): \
lessons and folders with ids, titles, where they are and when they changed. Workspace .rpk files are \
list_lessons instead.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn library_list(&self, Parameters(p): Parameters<LibraryListParams>) -> CallToolResult {
        self.live_call(Op::LibraryList {
            folder: p.folder,
            query: p.query,
        })
        .await
    }

    #[tool(
        description = "Open a lesson from the Library on the board (the open lesson is saved first). The class \
sees the new lesson, so do it when asked.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn library_open(&self, Parameters(p): Parameters<LibraryIdParams>) -> CallToolResult {
        self.live_call(Op::LibraryOpen { id: p.id }).await
    }

    #[tool(
        description = "Create a new lesson in the teacher's Library (blank, slides, lined or grid), in a folder, \
optionally opening it. Returns its id.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn library_create_lesson(
        &self,
        Parameters(p): Parameters<LibraryCreateParams>,
    ) -> CallToolResult {
        self.live_call(Op::LibraryCreateLesson {
            title: p.title,
            folder: p.folder,
            template: p.template,
            open: p.open.unwrap_or(false),
        })
        .await
    }

    #[tool(
        description = "Move a lesson or folder to another folder of the Library.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn library_move(&self, Parameters(p): Parameters<LibraryMoveParams>) -> CallToolResult {
        self.live_call(Op::LibraryMove {
            id: p.id,
            folder: p.folder,
        })
        .await
    }

    #[tool(
        description = "Rename a lesson or folder in the Library.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn library_rename(
        &self,
        Parameters(p): Parameters<LibraryRenameParams>,
    ) -> CallToolResult {
        self.live_call(Op::LibraryRename {
            id: p.id,
            name: p.name,
        })
        .await
    }

    #[tool(
        description = "Move a lesson or folder to the Library's trash (the teacher can restore it; deleting \
forever is the teacher's call). Ask first.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn library_trash(&self, Parameters(p): Parameters<LibraryIdParams>) -> CallToolResult {
        self.live_call(Op::LibraryTrash { id: p.id }).await
    }

    // ---------------------------------------------------------- settings
    #[tool(
        description = "Read the board settings an agent may change (theme, accent colour, density, toolbar \
side, icon style, pages panel, pen style/colour/width, eraser, smart shapes, voice language, PDF defaults…), \
with their current values and allowed values. AI keys, the cloud account, payments and permissions are never \
included.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_settings(&self) -> CallToolResult {
        self.live_call(Op::GetSettings).await
    }

    #[tool(
        description = "Change one board setting from get_settings (e.g. theme dark, accent #20c997, pen.style \
chalk). The teacher is asked like for an edit, and the AI chip's Undo puts it back.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_setting(&self, Parameters(p): Parameters<SetSettingParams>) -> CallToolResult {
        if forbidden_setting(&p.key) {
            return CallToolResult::structured_error(json!({
                "error": format!("`{}` is the teacher's alone (AI keys, accounts, payments and permissions are never changed by an agent)", p.key)
            }));
        }
        self.live_call(Op::SetSetting {
            key: p.key,
            value: p.value,
        })
        .await
    }

    // -------------------------------------------------------------- dock
    #[tool(
        description = "Read the Smart Dock layout of a mode (Prepare or Teach): position, orientation, button \
size, large icons, the groups of buttons in order, and the optional second (split) dock.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_dock_layout(&self, Parameters(p): Parameters<DockModeParams>) -> CallToolResult {
        self.live_call(Op::GetDockLayout { mode: p.mode }).await
    }

    #[tool(
        description = "Customise the Smart Dock for the teacher (\"pen, highlighter and eraser on the left, undo \
and next page on the right, bigger buttons\"): position (left, right, top, bottom, corners), orientation, \
groups of buttons (ids from list_dock_items, each group separated), a split second dock on another edge, button \
size (small, medium, large) and large icons. Pen, eraser and undo always stay. Validated; the teacher is asked \
like for an edit and can undo it from the AI chip.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_dock_layout(&self, Parameters(p): Parameters<SetDockParams>) -> CallToolResult {
        self.live_call(Op::SetDockLayout {
            mode: p.mode,
            layout: p.layout,
        })
        .await
    }

    #[tool(
        description = "Everything that can go on the Smart Dock (tools and action buttons with ids, labels and \
whether they must stay), and the named layouts (built-in and the teacher's).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_dock_items(&self) -> CallToolResult {
        self.live_call(Op::ListDockItems).await
    }

    #[tool(
        description = "Save the current dock layout (Prepare and Teach) under a name the teacher can switch to.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn save_dock_preset(
        &self,
        Parameters(p): Parameters<SaveDockPresetParams>,
    ) -> CallToolResult {
        self.live_call(Op::SaveDockPreset {
            name: p.name,
            modes: p.modes,
        })
        .await
    }

    #[tool(
        description = "Switch to a named dock layout: \"Left-handed\", \"Minimal teaching\", \"Maths class\", \
\"Default\" or one the teacher saved.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn apply_dock_preset(
        &self,
        Parameters(p): Parameters<DockPresetParams>,
    ) -> CallToolResult {
        self.live_call(Op::ApplyDockPreset { name: p.name }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_names_and_guards() {
        assert_eq!(live_or(&None), None);
        assert_eq!(live_or(&Some("LIVE".into())), None);
        assert_eq!(live_or(&Some("a.rpk".into())), Some("a.rpk".into()));
        assert!(forbidden_setting("claudeKey"));
        assert!(forbidden_setting("ai.allowEdit"));
        assert!(forbidden_setting("cloudAccount"));
        assert!(!forbidden_setting("theme"));
        assert!(!forbidden_setting("pen.style"));
        assert_eq!(pdf_name("Ohm's law / Class 9"), "Ohm-s law - Class 9.pdf");
        assert_eq!(pdf_name("///"), "---.pdf");
        assert_eq!(pdf_name(""), "lesson.pdf");
    }

    #[test]
    fn stamp_params_become_a_definition() {
        let p: StampParams = serde_json::from_value(json!({
            "name": "Dice", "art": "smart", "html": "<p>1</p>",
            "behavior": {"animation": "pop", "counter": true}
        }))
        .unwrap();
        let (path, d) = p.def();
        assert!(path.is_none());
        assert_eq!(d.html.as_deref(), Some("<p>1</p>"));
        assert!(d.behavior.unwrap().counter);
        assert!(serde_json::from_value::<StampParams>(json!({"bogus": 1})).is_err());
    }
}
