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
Use `get_lesson_outline` to find page numbers, item ids and bundle ids before editing.",
        libs = rupok_format::LIBS.join(", ")
    )
}

#[derive(Clone)]
pub struct McpServer {
    tools: Arc<Tools>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(tools: Tools) -> Self {
        Self {
            tools: Arc::new(tools),
            tool_router: Self::tool_router(),
        }
    }

    /// Run a tool body on the blocking pool (it does file IO) and map the result.
    async fn run<T, F>(&self, f: F) -> CallToolResult
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

#[tool_router]
impl McpServer {
    #[tool(
        description = "List every rupok lesson (*.rpk) in the workspace, searching sub-folders, with its title \
and page count. Returned paths are relative to the workspace and can be passed to the other tools.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_lessons(&self) -> CallToolResult {
        self.run(|t| t.list_lessons()).await
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
        self.run(move |t| t.add_smart_object(p)).await
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
        self.run(move |t| t.add_text(p)).await
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
        self.run(move |t| t.set_page_background(p)).await
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

/// Serve MCP on stdin/stdout until the client disconnects.
pub async fn serve_stdio(workspace: Workspace) -> anyhow::Result<()> {
    let server = McpServer::new(Tools::new(workspace));
    let running = server.serve(rmcp::transport::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
