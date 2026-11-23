//! MCP tools for the live board (`rupok mcp --live`), and the routing of the
//! lesson tools to the board when they are called with `path: "live"`.
//!
//! Every tool here runs on the lesson open on the paired board through the
//! [`LiveHub`](crate::live::LiveHub); see [`rupok_live`] for what each
//! operation does. Writes are one undo step each on the board, attributed to
//! the agent.

use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_router};
use rupok_live::{Anchor, Op, PatternName, Rect};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::mcp::McpServer;
use crate::tools::{
    AddMediaParams, AddModelParams, AddPageParams, AddSmartObjectParams, AddTextParams,
    DeleteItemParams, PatternArg, SetPageBackgroundParams, UpdateSmartObjectParams,
};

/// The `path` value that sends a lesson tool to the paired board.
pub const LIVE_PATH: &str = "live";

/// True when a lesson tool's `path` means "the board".
pub fn is_live(path: &str) -> bool {
    path.trim().eq_ignore_ascii_case(LIVE_PATH)
}

fn pattern(p: PatternArg) -> PatternName {
    match p {
        PatternArg::Plain => PatternName::Plain,
        PatternArg::Grid => PatternName::Grid,
        PatternArg::Dots => PatternName::Dots,
        PatternArg::Lines => PatternName::Lines,
    }
}

// ------------------------------------------------- lesson tools → live ops

pub fn op_add_page(p: &AddPageParams) -> Op {
    Op::AddPage {
        title: p.title.clone(),
        index: p.index,
        pattern: p.pattern.map(pattern),
        color: None,
    }
}

pub fn op_add_smart_object(p: &AddSmartObjectParams) -> Op {
    Op::AddSmartObject {
        page: Some(p.page),
        html: p.html.clone(),
        title: p.title.clone(),
        width: p.width,
        height: p.height,
        steps: p.steps,
        x: p.x,
        y: p.y,
        network_hosts: p.network_hosts.clone(),
    }
}

/// `add_media` on the board: the file (already read from the workspace) goes
/// over the pairing as base64.
pub fn op_add_media(p: &AddMediaParams, input: rupok_format::media::MediaInput) -> Op {
    Op::AddMedia {
        page: Some(p.page),
        url: input.url,
        file_base64: input
            .file
            .map(|b| base64::engine::general_purpose::STANDARD.encode(b)),
        file_name: input.file_name,
        title: input.title,
        start: input.start,
        end: input.end,
        muted: Some(input.muted),
        looped: Some(input.looped),
        autoplay: Some(input.autoplay),
        x: p.x,
        y: p.y,
        width: p.width,
        height: p.height,
    }
}

pub fn op_add_model(p: &AddModelParams, bytes: &[u8], name: String) -> Op {
    Op::AddModel {
        page: Some(p.page),
        file_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        file_name: Some(name),
        title: p.title.clone(),
        x: p.x,
        y: p.y,
        width: p.width,
        height: p.height,
    }
}

pub fn op_update_smart_object(p: &UpdateSmartObjectParams) -> Op {
    Op::UpdateSmartObject {
        bundle_id: p.bundle_id.clone(),
        html: p.html.clone(),
        steps: p.steps,
        network_hosts: p.network_hosts.clone(),
    }
}

pub fn op_add_text(p: &AddTextParams) -> Op {
    Op::AddText {
        page: Some(p.page),
        text: p.text.clone(),
        x: p.x,
        y: p.y,
        w: p.w,
        size: p.size,
        color: p.color.clone(),
    }
}

pub fn op_delete_item(p: &DeleteItemParams) -> Op {
    Op::DeleteItem {
        page: Some(p.page),
        item_id: p.item_id.clone(),
    }
}

pub fn op_set_page_background(p: &SetPageBackgroundParams) -> Op {
    Op::SetPageBackground {
        page: Some(p.page),
        pattern: Some(pattern(p.pattern)),
        color: p.color.clone(),
    }
}

// ------------------------------------------------------------------ params

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectBoardParams {
    /// The 6-digit code the teacher reads from the board (under "Connect AI"),
    /// e.g. `482913` or `482 913`.
    pub code: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageParams {
    /// 1-based page number. Omit for the page the board is showing now.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
}

/// A rectangle in slide pixels.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionArg {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnapshotParams {
    /// 1-based page number. Omit for the page the board is showing now.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
    /// Zoom in on these items (e.g. the ink strokes you want to read), with
    /// a margin. Ids come from get_current_page or get_selection.
    #[serde(default)]
    pub item_ids: Option<Vec<String>>,
    /// Zoom in on this region of the slide, in slide pixels.
    #[serde(default)]
    pub region: Option<RegionArg>,
    /// Image width in pixels. Default 1280, max 1920.
    #[serde(default)]
    #[schemars(range(min = 64, max = 1920))]
    pub width: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GoToPageParams {
    /// 1-based page number to show.
    #[schemars(range(min = 1))]
    pub page: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddPageAfterCurrentParams {
    /// Page title (shown in the pages panel, not drawn on the slide).
    #[serde(default)]
    pub title: Option<String>,
    /// Background pattern. Default: same as the current page.
    #[serde(default)]
    pub pattern: Option<PatternArg>,
    /// Page colour: `white`, `paper`, `chalkboard`, `blackboard` or `#rrggbb`.
    /// Default: same as the current page.
    #[serde(default)]
    pub color: Option<String>,
    /// Show the new page on the board right away. Default false: build the
    /// page first, then call go_to_page when it is ready.
    #[serde(default)]
    pub show: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepObjectParams {
    /// Item id of the Smart Object. Omit for the first object on the page
    /// that still has steps to reveal (or to hide, for a negative delta).
    #[serde(default)]
    pub item_id: Option<String>,
    /// Go to this build step (0 = nothing revealed).
    #[serde(default)]
    pub step: Option<u32>,
    /// Move by this many steps instead (e.g. 1 = next, -1 = back). Default 1.
    #[serde(default)]
    pub delta: Option<i32>,
    /// 1-based page number. Omit for the current page.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceItemsWithTextParams {
    /// Ids of the items to replace, usually handwriting strokes (kind
    /// `stroke`) from get_selection or get_current_page.
    pub item_ids: Vec<String>,
    /// The clean text, e.g. what the handwriting says. Use `\n` for lines.
    pub text: String,
    /// Left edge in slide pixels. Default: where the replaced items were.
    #[serde(default)]
    pub x: Option<f64>,
    /// Top edge in slide pixels. Default: where the replaced items were.
    #[serde(default)]
    pub y: Option<f64>,
    /// Font size in pixels. Default: matches the handwriting height (min 24).
    #[serde(default)]
    pub size: Option<f64>,
    /// Text colour `#rrggbb`. Default: the ink's colour.
    #[serde(default)]
    pub color: Option<String>,
    /// 1-based page number. Omit for the current page.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PointAtParams {
    /// Centre x (or left edge, with `w`/`h`) in slide pixels.
    pub x: f64,
    /// Centre y (or top edge, with `w`/`h`) in slide pixels.
    pub y: f64,
    /// Width of a highlight box. Omit both `w` and `h` for a laser ring at (x, y).
    #[serde(default)]
    pub w: Option<f64>,
    /// Height of a highlight box.
    #[serde(default)]
    pub h: Option<f64>,
    /// A short caption shown next to the pointer, e.g. "the base case".
    #[serde(default)]
    pub label: Option<String>,
    /// How long it stays, in seconds. Default 4, max 15.
    #[serde(default)]
    pub seconds: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetBoardThemeParams {
    /// Colour of the current page: `white`, `paper`, `chalkboard`,
    /// `blackboard` or `#rrggbb`. Smart Objects re-theme themselves to match.
    #[serde(default)]
    pub page_color: Option<String>,
    /// Background pattern of the current page.
    #[serde(default)]
    pub pattern: Option<PatternArg>,
    /// The app's own light or dark interface: `light`, `dark` or `system`.
    #[serde(default)]
    pub ui: Option<String>,
}

/// Where to write: next to items, a region, or an explicit point.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnchorArg {
    /// Write next to these items, usually the strokes of the problem (from
    /// get_selection or get_current_page). Default: the teacher's selection.
    #[serde(default)]
    pub item_ids: Option<Vec<String>>,
    /// Write next to this region of the slide, in slide pixels.
    #[serde(default)]
    pub region: Option<RegionArg>,
    /// Start exactly here instead: top-left of the writing, slide pixels.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// `below` (default) or `right` of the items or region. If that side is
    /// full the board tries the other, then a new page.
    #[serde(default)]
    pub side: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WriteHandwritingParams {
    /// The solution, one short step per line (at most 40 lines, 400
    /// characters each). Plain text with inline LaTeX between dollars, e.g.
    /// `"Subtract 3 from both sides:"`, `"$2x = 4$"`, `"$x = \frac{4}{2} = 2$"`.
    /// Fractions, powers, subscripts, square roots, Greek letters and common
    /// operators are written properly.
    pub lines: Vec<String>,
    /// Where to write. Default: below the teacher's selection.
    #[serde(default)]
    pub anchor: Option<AnchorArg>,
    /// `teacher` (default): the teacher's own handwriting where they have
    /// taught it to Rupok; `neat`: a neat standard hand.
    #[serde(default)]
    pub style: Option<String>,
    /// Pen speed, 0.25 to 4. Default: the teacher's setting (1 = brisk).
    #[serde(default)]
    pub speed: Option<f64>,
    /// Ink colour `#rrggbb`. Default: the colour of the problem's ink.
    #[serde(default)]
    pub color: Option<String>,
    /// Height of a capital or digit in slide pixels. Default: the size of
    /// the problem's handwriting.
    #[serde(default)]
    pub size: Option<f64>,
    /// `all` writes every line; `one-line-per-click` lets the teacher reveal
    /// one line per Next press. Default: the teacher's setting.
    #[serde(default)]
    pub step_mode: Option<String>,
    /// 1-based page number. Omit for the current page.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
}

// ------------------------------------------------------------------- tools

fn wrap(v: Value) -> Value {
    if v.is_object() {
        v
    } else {
        json!({ "result": v })
    }
}

impl McpServer {
    /// Run `op` on the paired board and turn the answer into a tool result.
    pub(crate) async fn live_call(&self, op: Op) -> CallToolResult {
        match self.live.call(op).await {
            Ok(v) => CallToolResult::structured(wrap(v)),
            Err(e) => CallToolResult::structured_error(e.to_json()),
        }
    }

    /// `get_lesson_outline` with `path: "live"`: from the board, or from the
    /// mirror while the board is away.
    pub(crate) async fn live_outline(&self) -> CallToolResult {
        if !self.live.is_paired()
            && let Some(mut o) = self.live.mirror_outline()
        {
            o["note"] = json!(
                "the board is not connected; this is the last state it sent. Edits need the board: ask the teacher to reconnect"
            );
            return CallToolResult::structured(o);
        }
        self.live_call(Op::GetOutline).await
    }
}

#[tool_router(router = live_tool_router, vis = "pub(crate)")]
impl McpServer {
    #[tool(
        description = "Pair with the Rupok board the teacher has open, so you can see and edit the lesson live \
during class. Call it when the teacher gives you a 6-digit code from the board (they get it by clicking \
\"Connect AI\" in Rupok), or says something like \"connect to my board, code 482913\". If they have not \
given a code, call get_board_status and relay its hint instead of guessing. After pairing, use path \"live\" \
with the lesson tools and the live tools (get_current_page, get_page_snapshot, …).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn connect_board(&self, Parameters(p): Parameters<ConnectBoardParams>) -> CallToolResult {
        match self.live.connect(&p.code) {
            Ok(s) => CallToolResult::structured(serde_json::to_value(s).unwrap_or_default()),
            Err(e) => CallToolResult::structured_error(e.to_json()),
        }
    }

    #[tool(
        description = "Is a Rupok board paired, and what is it showing (lesson title, page N of M, whether the \
teacher allows live edits)? Returns a `hint` saying exactly what to do next, e.g. how the teacher gets a pairing \
code. Call it first whenever the teacher asks you to do something on \"the board\" and you are not sure you are \
connected.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_board_status(&self) -> CallToolResult {
        CallToolResult::structured(serde_json::to_value(self.live.status()).unwrap_or_default())
    }

    #[tool(
        description = "Read the page the board is showing right now (or another page): page number, colour \
scheme (theme new content to match), and every item with its id, kind (text | stroke | image | object), \
geometry in slide pixels, text, Smart Object title/bundle_id/steps/step, and age_s (seconds since it was made: \
\"what I just wrote\" is the newest strokes). Also returns the teacher's current selection. Call this before \
acting on the board, e.g. for \"make a quiz from what I just wrote\". Ink strokes are listed with their \
bounds only: to read handwriting, call get_page_snapshot (optionally with their item_ids).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_current_page(&self, Parameters(p): Parameters<PageParams>) -> CallToolResult {
        self.live_call(Op::GetPage { page: p.page }).await
    }

    #[tool(
        description = "See the board: a PNG of a page as the class sees it, rendered by the board (ink, text, \
images; Smart Objects appear as labelled cards). Use it to read handwriting, check a sketch before turning it \
into a diagram, or check your own layout after editing. Pass item_ids (e.g. the newest strokes from \
get_current_page) or a region to zoom in so small handwriting is legible.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_page_snapshot(&self, Parameters(p): Parameters<SnapshotParams>) -> CallToolResult {
        let op = Op::Snapshot {
            page: p.page,
            region: p.region.map(|r| Rect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            item_ids: p.item_ids,
            width: p.width,
        };
        match self.live.call(op).await {
            Ok(mut v) => {
                let png = v
                    .get("png")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_default();
                let ok = base64::engine::general_purpose::STANDARD
                    .decode(png.as_bytes())
                    .is_ok_and(|b| b.starts_with(b"\x89PNG"));
                if !ok {
                    return CallToolResult::structured_error(
                        json!({ "error": "the board did not return a PNG image" }),
                    );
                }
                if let Some(m) = v.as_object_mut() {
                    m.remove("png");
                }
                CallToolResult::success(vec![
                    ContentBlock::image(png, "image/png"),
                    ContentBlock::text(v.to_string()),
                ])
            }
            Err(e) => CallToolResult::structured_error(e.to_json()),
        }
    }

    #[tool(
        description = "What the teacher has selected on the board (lasso): the selected items' ids, kinds, \
text and bounds. Use it when the teacher says \"this\", \"these\" or \"the selected …\", e.g. \"turn this \
into text\" or \"make a simulation of this\".",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_selection(&self) -> CallToolResult {
        self.live_call(Op::GetSelection).await
    }

    #[tool(
        description = "Show a page on the board (1-based). The whole class sees the page change, so use it when \
the teacher asks, or once a page you built is ready to present.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn go_to_page(&self, Parameters(p): Parameters<GoToPageParams>) -> CallToolResult {
        self.live_call(Op::GoToPage { page: p.page }).await
    }

    #[tool(
        description = "Insert a new blank page right after the page the board is showing, with the same look \
(colour, pattern) unless you pass others. This is the default place for new content during class (a quiz, \
practice problems, a simulation), so you never cover what the teacher wrote. Returns its page number: put \
content on it with path \"live\" and that page, then go_to_page when ready (or pass show=true).",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_page_after_current(
        &self,
        Parameters(p): Parameters<AddPageAfterCurrentParams>,
    ) -> CallToolResult {
        self.live_call(Op::AddPageAfterCurrent {
            title: p.title,
            pattern: p.pattern.map(pattern),
            color: p.color,
            show: p.show.unwrap_or(false),
        })
        .await
    }

    #[tool(
        description = "Move a Smart Object's build step on the board, like the presenter's Next/Back buttons: \
delta 1 reveals the next step, -1 goes back, or set an absolute step. Use it to walk the class through an \
object you built, or when the teacher says \"next step\" / \"show the answer\".",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn step_object(&self, Parameters(p): Parameters<StepObjectParams>) -> CallToolResult {
        self.live_call(Op::StepObject {
            page: p.page,
            item_id: p.item_id,
            step: p.step,
            delta: p.delta,
        })
        .await
    }

    #[tool(
        description = "Turn handwriting into clean typed text: removes the given items (usually ink strokes) \
and puts a text box with `text` where they were, sized to match and in the ink's colour, as ONE undo step (the \
teacher's Ctrl+Z brings the ink back). Read the handwriting first with get_page_snapshot(item_ids). Use it for \
\"convert my writing to text\" or \"clean this up\"; for maths, write it in plain Unicode (x², √, π, ½).",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            open_world_hint = false
        )
    )]
    async fn replace_items_with_text(
        &self,
        Parameters(p): Parameters<ReplaceItemsWithTextParams>,
    ) -> CallToolResult {
        self.live_call(Op::ReplaceItemsWithText {
            page: p.page,
            item_ids: p.item_ids,
            text: p.text,
            x: p.x,
            y: p.y,
            size: p.size,
            color: p.color,
        })
        .await
    }

    #[tool(
        description = "Point at something on the board for the class: a temporary laser ring at (x, y), or a \
highlight box when you give w and h, with an optional short label. Nothing is saved and it fades after a few \
seconds. Use it while explaining (\"look at this term\"), or to show the teacher where you put something. \
Coordinates are slide pixels on the page being shown.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn point_at(&self, Parameters(p): Parameters<PointAtParams>) -> CallToolResult {
        self.live_call(Op::PointAt {
            x: p.x,
            y: p.y,
            w: p.w,
            h: p.h,
            label: p.label,
            seconds: p.seconds,
        })
        .await
    }

    #[tool(
        description = "Write on the board in handwriting, animated stroke by stroke in front of the class: \
use it to solve a problem the teacher wrote (\"solve this\"). Give the solution as short lines, one step \
each, with inline LaTeX between dollars (\"$2x = 4$\", \"$x = \\frac{4}{2} = 2$\"). The board writes it \
next to the anchor (default: below the teacher's selection) without covering anything, in the problem's \
ink colour and letter size, in the teacher's own handwriting when they have taught it. It is one undo \
step. Before calling: read the problem with get_page_snapshot(item_ids), solve it, and check every \
step and the arithmetic; keep lines short (under ~30 characters) and at the class's level. If the \
result has `\"status\": \"preview\"`, the teacher is checking your solution before the class sees it: \
do NOT call it again; explain the steps in chat instead.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn write_handwriting(
        &self,
        Parameters(p): Parameters<WriteHandwritingParams>,
    ) -> CallToolResult {
        let anchor = p.anchor.map(|a| Anchor {
            item_ids: a.item_ids,
            region: a.region.map(|r| Rect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
            }),
            x: a.x,
            y: a.y,
            side: a.side,
        });
        self.live_call(Op::WriteHandwriting {
            page: p.page,
            lines: p.lines,
            anchor,
            style: p.style,
            speed: p.speed,
            color: p.color,
            size: p.size,
            step_mode: p.step_mode,
        })
        .await
    }

    #[tool(
        description = "Change the look of the board: the current page's colour (white, paper, chalkboard, \
blackboard or #rrggbb) and pattern, and/or the app interface (light, dark, system). Smart Objects follow the \
page colour automatically; new text you add picks a readable colour for it. Use it when the teacher asks for \
\"chalkboard mode\", \"dark mode\" or a grid for graphs.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn set_board_theme(
        &self,
        Parameters(p): Parameters<SetBoardThemeParams>,
    ) -> CallToolResult {
        if let Some(ui) = &p.ui
            && !matches!(ui.as_str(), "light" | "dark" | "system")
        {
            return CallToolResult::structured_error(
                json!({ "error": "ui must be light, dark or system" }),
            );
        }
        self.live_call(Op::SetBoardTheme {
            page_color: p.page_color,
            pattern: p.pattern.map(pattern),
            ui: p.ui,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_path_and_conversions() {
        assert!(is_live("live") && is_live(" LIVE ") && !is_live("live.rpk"));
        let op = op_add_text(&AddTextParams {
            path: "live".into(),
            page: 2,
            text: "Hi".into(),
            x: 1.0,
            y: 2.0,
            w: None,
            size: Some(40.0),
            color: None,
        });
        assert_eq!(op.name(), "add_text");
        let v = serde_json::to_value(&op).unwrap();
        assert_eq!(v["page"], 2);
        let bg = op_set_page_background(&SetPageBackgroundParams {
            path: "live".into(),
            page: 1,
            pattern: PatternArg::Grid,
            color: None,
        });
        assert_eq!(serde_json::to_value(&bg).unwrap()["pattern"], "grid");
    }

    #[test]
    fn schemas_describe_their_fields() {
        let s = serde_json::to_value(schemars::schema_for!(SnapshotParams)).unwrap();
        assert!(s["properties"]["item_ids"]["description"].is_string());
        let c = serde_json::to_value(schemars::schema_for!(ConnectBoardParams)).unwrap();
        assert_eq!(c["required"][0], "code");
    }
}
