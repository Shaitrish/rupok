//! `rupok mcp`: the MCP server over stdio (rmcp).
//!
//! Each tool deserialises its JSON-schema'd parameters, runs the matching
//! [`Tools`] method on the blocking pool and returns the result as structured
//! JSON. Failures are tool-level errors (`isError: true`) whose JSON body
//! carries the message and, for validation failures, the diagnostics, so the
//! agent can read them, fix its HTML and retry.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use serde::Serialize;

use crate::live::LiveHub;
use crate::mcp_live::is_live;
use crate::ops::OpError;
use crate::tools::*;
use crate::workspace::Workspace;

pub fn instructions() -> String {
    format!(
        "rupok edits classroom lessons: `.rpk` files holding 1920×1080 slide pages with text, ink, images \
and live Smart Objects (self-contained interactive HTML that runs sandboxed and offline on the board).\n\
\n\
- Call `read_object_spec` once before generating any Smart Object HTML, and follow it.\n\
- Design for the 1920×1080 slide frame: coordinates are slide pixels from the top-left corner. \
Use width 1920 / height 1080 for a full-slide object.\n\
- Keep all text 24px or larger (body ~32px, headings 48px+); lessons are shown on classroom projectors.\n\
- Load libraries offline with `rupoklib://<library>/<file>` URLs instead of CDNs ({libs}). \
Remote URLs are rejected unless their host is listed in `network_hosts`.\n\
- `add_smart_object` and `update_smart_object` validate first. If they return diagnostics, nothing \
was saved: fix the HTML and call again. Use `validate_smart_object` to check HTML without saving.\n\
- Paths are relative to the workspace and must end in `.rpk`; page numbers start at 1. \
Use `get_lesson_outline` to find page numbers, item ids and bundle ids before editing.\n\
\n\
Live board (during class): the teacher can pair their open Rupok board with you. They click \
\"Connect AI\" on the board and give you a 6-digit code: call `connect_board` with it \
(`get_board_status` says whether a board is paired and what to do next). Once paired:\n\
- Pass `path: \"live\"` to the lesson tools (get_lesson_outline, add_page, add_text, add_smart_object, \
update_smart_object, delete_item, set_page_background, set_page_design, set_branding) to change the lesson \
open on the board; the class \
sees each change at once and the teacher can undo it with Ctrl+Z.\n\
- Look before you act: `get_current_page` (items and ids), `get_page_snapshot` (an image, to read \
handwriting), `get_selection` (what the teacher selected).\n\
- Put new content on a new page (`add_page_after_current`) unless the teacher asks to change the \
current one; show it with `go_to_page` when it is ready. Never delete or replace the teacher's work \
without asking first. Match the page `scheme` (light/dark) so objects stay themed.\n\
- `point_at`, `step_object`, `replace_items_with_text` (ink to text) and `set_board_theme` help \
you teach on the board.\n\
- Page design (`set_page_design`, `set_branding`): when asked for a background, a theme or a school logo. \
Draw it as SVG; keep the content area calm, ink readable (light ink on dark pages) and the logo small in a \
corner; read the current design first by calling the tool with only `path`.\n\
- Agent-first: you can do anything the teacher can. Pages (`list_pages`, `move_pages`, `duplicate_pages`, \
`delete_pages`), items (`select_items`, `move_items`, `align_items`, `restyle_items`, `scale_items`, `group_items`, \
`delete_items`, `draw_stroke`, `update_text`), stamps (`add_stamp`, `create_stamp` incl. Smart stamps with sandboxed \
logic), modes and view (`set_mode`, `zoom_to`, `start_replay`, `celebrate`), `export_pdf`, the Library \
(`library_*`), settings (`get_settings`/`set_setting`, an allowlist) and the Smart Dock (`set_dock_layout`). \
Document tools take an optional `path` (omit for the board). Settings and mode changes ask the teacher's permission \
like edits; AI keys, accounts, payments and permissions are never yours to change.\n\
- To solve a problem written on the board, read it (`get_page_snapshot` with its item_ids), check every \
step, then `write_handwriting` the solution as short lines; the board writes it in handwriting next to \
the problem. If the result says `preview`, the teacher is checking it first: do not write it again.",
        libs = rupok_format::LIBS.join(", ")
    )
}

#[derive(Clone)]
pub struct McpServer {
    pub(crate) tools: Arc<Tools>,
    /// The live board channel (see `crate::mcp_live`); disabled without `--live`.
    pub(crate) live: Arc<LiveHub>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(tools: Tools) -> Self {
        Self::with_live(
            tools,
            LiveHub::disabled("the server was started without the live channel"),
        )
    }

    /// A server whose live tools use `live`.
    pub fn with_live(tools: Tools, live: Arc<LiveHub>) -> Self {
        Self {
            tools: Arc::new(tools),
            live,
            tool_router: Self::tool_router() + Self::live_tool_router() + Self::agent_tool_router(),
        }
    }

    /// Run a tool body on the blocking pool (it does file IO) and map the result.
    pub(crate) async fn run<T, F>(&self, f: F) -> CallToolResult
    where
        T: Serialize + Send + 'static,
        F: FnOnce(&Tools) -> Result<T, OpError> + Send + 'static,
    {
        let tools = self.tools.clone();
        match tokio::task::spawn_blocking(move || f(&tools)).await {
            Ok(Ok(v)) => match serde_json::to_value(v) {
                Ok(v) => CallToolResult::structured(v),
                Err(e) => {
                    CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
                }
            },
            Ok(Err(e)) => CallToolResult::structured_error(error_json(&e)),
            Err(e) => {
                CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
            }
        }
    }
}

impl McpServer {
    async fn add_media_impl(&self, p: AddMediaParams) -> CallToolResult {
        if is_live(&p.path) {
            let tools = self.tools.clone();
            let q = p.clone();
            let input = tokio::task::spawn_blocking(move || tools.media_input(&q)).await;
            return match input {
                Ok(Ok(input)) => {
                    self.live_call(crate::mcp_live::op_add_media(&p, input))
                        .await
                }
                Ok(Err(e)) => CallToolResult::structured_error(error_json(&e)),
                Err(e) => {
                    CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
                }
            };
        }
        self.run(move |t| t.add_media(p)).await
    }
}

#[tool_router]
impl McpServer {
    #[tool(
        description = "List every rupok lesson (*.rpk) in the workspace, searching sub-folders, with its title \
and page count. Returned paths are relative to the workspace and can be passed to the other tools.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_lessons(&self) -> CallToolResult {
        let board = self.live.status().board;
        self.run(move |t| {
            let mut v = serde_json::to_value(t.list_lessons()?)
                .map_err(|e| OpError::Invalid(e.to_string()))?;
            if let Some(b) = board {
                v["live_board"] =
                    serde_json::json!({ "path": "live", "title": b.title, "pages": b.page_count });
            }
            Ok(v)
        })
        .await
    }

    #[tool(
        description = "Create a new, empty rupok lesson file (.rpk) whose pages are blank 1920×1080 slides. \
Fails if the file already exists. Returns the page ids.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_lesson(&self, Parameters(p): Parameters<CreateLessonParams>) -> CallToolResult {
        self.run(move |t| t.create_lesson(p)).await
    }

    #[tool(
        description = "Read a lesson's structure: each page (1-based number, id, title, frame size, background) \
and every item on it (id, kind = text|stroke|image|object, geometry in slide pixels, text snippet, Smart Object \
title, bundle_id, declared steps and current step). Call this before editing an existing lesson.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_lesson_outline(
        &self,
        Parameters(p): Parameters<LessonPathParams>,
    ) -> CallToolResult {
        if is_live(&p.path) {
            return self.live_outline().await;
        }
        self.run(move |t| t.get_lesson_outline(p)).await
    }

    #[tool(
        description = "Add a blank 1920×1080 slide page to a lesson, appended at the end or inserted at a \
1-based index, with an optional title and background pattern. Returns the new page number and id.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_page(&self, Parameters(p): Parameters<AddPageParams>) -> CallToolResult {
        if is_live(&p.path) {
            return self.live_call(crate::mcp_live::op_add_page(&p)).await;
        }
        self.run(move |t| t.add_page(p)).await
    }

    #[tool(
        description = "Place a Smart Object (a self-contained interactive HTML document that runs live, \
sandboxed and offline on the classroom board) on a page. Call read_object_spec first. The HTML is validated \
before anything is saved: on any error the result has isError=true, saved=false and a diagnostics list \
(severity, code, message); fix the HTML and call again. On success returns item_id, bundle_id (needed for \
update_smart_object), the placed geometry and any warnings. Default size 1280×720 centred on the slide.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_smart_object(
        &self,
        Parameters(p): Parameters<AddSmartObjectParams>,
    ) -> CallToolResult {
        if is_live(&p.path) {
            return self
                .live_call(crate::mcp_live::op_add_smart_object(&p))
                .await;
        }
        self.run(move |t| t.add_smart_object(p)).await
    }

    #[tool(
        description = "Put a YouTube video on a page by its link. It plays in class from youtube-nocookie.com \
(privacy-enhanced) with a poster picture, start/end times (a `t=` in the link sets the start), mute, loop and \
autoplay when the page is shown; offline, the board shows the poster and says it needs internet. Returns the item id.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_youtube(&self, Parameters(p): Parameters<AddYoutubeParams>) -> CallToolResult {
        self.add_media_impl(p.into()).await
    }

    #[tool(
        description = "Put a video, audio clip or trusted web embed on a page. Either `file`: an mp4, webm, mp3, \
m4a, ogg or wav in the workspace, stored inside the lesson so it plays offline (keep it small; 200 MB at most); or \
`url`: a YouTube link or a page on PhET, GeoGebra, Desmos, published Google Slides/Docs/Sheets/Forms, Wikipedia or \
Khan Academy (other sites are refused: write the link as text instead). Returns the item id, kind and stored src.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_media(&self, Parameters(p): Parameters<AddMediaParams>) -> CallToolResult {
        self.add_media_impl(p).await
    }

    #[tool(
        description = "Put a 3D model on a page: a glTF 2.0 `.glb` (or a `.gltf` with everything embedded) from \
the workspace, at most 18 MB. It becomes a built-in Smart Object viewer that works offline: orbit, zoom, pan, \
auto-rotate, reset view, the model's own animations, light/dark backdrop, and saved views the teacher steps through \
with the clicker. Returns item_id and bundle_id.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_3d_model(&self, Parameters(p): Parameters<AddModelParams>) -> CallToolResult {
        if is_live(&p.path) {
            let tools = self.tools.clone();
            let file = p.file.clone();
            let read = tokio::task::spawn_blocking(move || {
                tools.read_workspace_file(&file, rupok_format::model3d::MAX_MODEL_BYTES)
            })
            .await;
            return match read {
                Ok(Ok((bytes, name))) => {
                    self.live_call(crate::mcp_live::op_add_model(&p, &bytes, name))
                        .await
                }
                Ok(Err(e)) => CallToolResult::structured_error(error_json(&e)),
                Err(e) => {
                    CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
                }
            };
        }
        self.run(move |t| t.add_model(p)).await
    }

    #[tool(
        description = "Replace the HTML of a Smart Object that is already in a lesson, identified by its \
bundle_id (from add_smart_object or get_lesson_outline); every placement of that bundle shows the new version. \
Optionally change its step count or allowed network hosts. Re-validated first: on errors nothing is saved and \
the diagnostics are returned.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_smart_object(
        &self,
        Parameters(p): Parameters<UpdateSmartObjectParams>,
    ) -> CallToolResult {
        if is_live(&p.path) {
            return self
                .live_call(crate::mcp_live::op_update_smart_object(&p))
                .await;
        }
        self.run(move |t| t.update_smart_object(p)).await
    }

    #[tool(
        description = "Add a text box to a page at (x, y) in slide pixels from the top-left of the 1920×1080 \
frame. Font size defaults to 32px; keep it 24px or larger so it is readable on a projector. Returns the item id.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn add_text(&self, Parameters(p): Parameters<AddTextParams>) -> CallToolResult {
        if is_live(&p.path) {
            return self.live_call(crate::mcp_live::op_add_text(&p)).await;
        }
        self.run(move |t| t.add_text(p)).await
    }

    #[tool(
        description = "Replace handwritten ink strokes with a text box saying what they say (handwriting to \
text). Pass the stroke ids from get_lesson_outline and the text; the box lands over the ink, sized to the \
handwriting and in the ink's colour unless given. Saved as one change, so replay still shows the ink being \
written before it became text.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn replace_strokes_with_text(
        &self,
        Parameters(p): Parameters<ReplaceStrokesWithTextParams>,
    ) -> CallToolResult {
        self.run(move |t| t.replace_strokes_with_text(p)).await
    }

    #[tool(
        description = "Delete one item (text box, ink stroke, image or Smart Object placement) from a page, by \
the item id shown in get_lesson_outline.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn delete_item(&self, Parameters(p): Parameters<DeleteItemParams>) -> CallToolResult {
        if is_live(&p.path) {
            return self.live_call(crate::mcp_live::op_delete_item(&p)).await;
        }
        self.run(move |t| t.delete_item(p)).await
    }

    #[tool(
        description = "Set a page's background pattern (plain, grid, dots or lines) and, optionally, its colour \
as #rrggbb.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_page_background(
        &self,
        Parameters(p): Parameters<SetPageBackgroundParams>,
    ) -> CallToolResult {
        if is_live(&p.path) {
            return self
                .live_call(crate::mcp_live::op_set_page_background(&p))
                .await;
        }
        self.run(move |t| t.set_page_background(p)).await
    }

    #[tool(
        description = "Page design: a page's colour, pattern and background image, or every page's with \
`all_pages`. Draw backgrounds as SVG for the 1920×1080 page (viewBox=\"0 0 1920 1080\"); no scripts, event \
handlers, external references or animation (refused). Keep the middle calm and low-contrast so ink and text \
stay readable: on a dark page use light ink, on a light page dark ink; use `background_dim` over photos. \
Call with only `path` to read the current design. One undo step on the board.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_page_design(
        &self,
        Parameters(p): Parameters<SetPageDesignParams>,
    ) -> CallToolResult {
        let op = crate::mcp_live::op_set_page_design(&p);
        if is_live(&p.path) {
            return self.live_call(op).await;
        }
        self.run(move |t| t.design_op(&p.path, op)).await
    }

    #[tool(
        description = "Branding on every page (or one page): a logo (SVG markup or base64 PNG/JPEG) placed in a \
corner or at fractions of the page, and a header or footer line of school, teacher and class text. Text may \
use placeholders that fill themselves in per lesson and page: {lesson}, {page}, {pages}, {date}, {teacher}, \
{school}, {subject}, {course}, {chapter}. Keep the logo small (logo_width 0.08–0.12) in a corner, away from \
the content. Only the given fields change; call with only `path` to read the branding. One undo step.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_branding(&self, Parameters(p): Parameters<SetBrandingParams>) -> CallToolResult {
        let op = crate::mcp_live::op_set_branding(&p);
        if is_live(&p.path) {
            return self.live_call(op).await;
        }
        self.run(move |t| t.design_op(&p.path, op)).await
    }

    #[tool(
        description = "Check Smart Object HTML against the rupok rules without saving anything. Returns valid, \
error and warning counts and diagnostics (severity, code, message). Errors would block add_smart_object; \
warnings are advice.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn validate_smart_object(
        &self,
        Parameters(p): Parameters<ValidateSmartObjectParams>,
    ) -> CallToolResult {
        self.run(move |t| t.validate_smart_object(p)).await
    }

    #[tool(
        description = "Return the Smart Object authoring spec as Markdown: the HTML contract, sandbox and \
offline rules, the rupoklib:// libraries, the rupok steps/state API and classroom design rules. Read it once \
before generating any Smart Object HTML.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_object_spec(&self) -> CallToolResult {
        let tools = self.tools.clone();
        match tokio::task::spawn_blocking(move || tools.read_object_spec()).await {
            Ok(spec) => CallToolResult::success(vec![ContentBlock::text(spec.text)]),
            Err(e) => {
                CallToolResult::error(vec![ContentBlock::text(format!("internal error: {e}"))])
            }
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rupok", env!("CARGO_PKG_VERSION")))
            .with_instructions(instructions())
    }
}

/// Serve MCP on stdin/stdout until the client disconnects (no live channel).
pub async fn serve_stdio(workspace: Workspace) -> anyhow::Result<()> {
    serve_stdio_with(
        workspace,
        LiveHub::disabled("the server was started with --no-live"),
    )
    .await
}

/// Serve MCP on stdin/stdout with the live board channel `live` (already
/// started). The board shows the MCP client's name ("Claude Code", …).
pub async fn serve_stdio_with(workspace: Workspace, live: Arc<LiveHub>) -> anyhow::Result<()> {
    let server = McpServer::with_live(Tools::new(workspace), live.clone());
    let running = server.serve(rmcp::transport::stdio()).await?;
    if let Some(info) = running.peer_info() {
        let c = &info.client_info;
        live.set_agent(&crate::live::agent_display_name(
            &c.name,
            c.title.as_deref(),
        ));
    }
    running.waiting().await?;
    Ok(())
}
