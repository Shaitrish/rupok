//! `show_progress`: the agent's "working" card on the live board, so the
//! class sees what the AI is doing while it builds something ("Planning…",
//! "Writing the simulation…", "Testing…") right where the result will land.
//! See [`rupok_live::progress`] and docs/ai-agents.md § "AI in action".

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{tool, tool_router};
use rupok_live::Op;
use rupok_live::progress::{PlanEntry, ProgressArgs};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::mcp::McpServer;

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanEntryArg {
    /// What this step does, e.g. "Draw the atoms".
    pub content: String,
    /// `pending`, `in_progress` or `completed`.
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowProgressParams {
    /// The card to update (from an earlier call's result). Omit to start a
    /// new card.
    #[serde(default)]
    pub id: Option<String>,
    /// What you are making, e.g. "Caffeine molecule (3D)".
    #[serde(default)]
    pub title: Option<String>,
    /// What you are doing now, e.g. "Writing the simulation…", "Testing…".
    #[serde(default)]
    pub step: Option<String>,
    /// Your plan as a short checklist (at most 12 entries).
    #[serde(default)]
    pub plan: Option<Vec<PlanEntryArg>>,
    /// 0 to 100, if you know how far along you are.
    #[serde(default)]
    pub percent: Option<f64>,
    /// `working` (default), `done`, `failed` or `clear`.
    #[serde(default)]
    pub state: Option<String>,
    /// What will land: `object`, `handwriting`, `text`, `shape` or `other`.
    #[serde(default)]
    pub kind: Option<String>,
    /// Where the result will go (slide pixels). Use the same box you will
    /// pass to add_smart_object; default: a free spot on the page.
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub w: Option<f64>,
    #[serde(default)]
    pub h: Option<f64>,
    /// 1-based page. Omit for the page on screen.
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<u32>,
    /// A partial Smart Object (HTML, ≤ 512 KiB) to show as a live preview
    /// thumbnail on the card while you build. Sandboxed like Smart Objects.
    #[serde(default)]
    pub preview_html: Option<String>,
    /// For `failed`: one short, friendly sentence for the teacher.
    #[serde(default)]
    pub error: Option<String>,
}

/// The op for `show_progress` (validated here so the agent gets a clear
/// error before anything reaches the board).
pub fn op_show_progress(p: ShowProgressParams) -> Result<Op, rupok_live::LiveError> {
    let args = ProgressArgs {
        id: p.id,
        title: p.title,
        step: p.step,
        plan: p.plan.map(|v| {
            v.into_iter()
                .map(|e| PlanEntry {
                    content: e.content,
                    status: e.status,
                })
                .collect()
        }),
        percent: p.percent,
        state: p.state,
        kind: p.kind,
        x: p.x,
        y: p.y,
        w: p.w,
        h: p.h,
        page: p.page,
        preview_html: p.preview_html,
        error: p.error,
    };
    args.validate()?;
    Ok(Op::ShowProgress(Box::new(args)))
}

#[tool_router(router = progress_tool_router, vis = "pub(crate)")]
impl McpServer {
    #[tool(
        description = "Show the class what you are working on, live, on the board: a 'working' card at the \
spot where your result will appear, with your current step (\"Planning…\", \"Writing the simulation…\", \
\"Testing…\"), an optional checklist plan, percent and a live preview (partial Smart Object HTML). Call it \
at the start of anything that takes more than a few seconds (a Smart Object, a worked solution, a diagram), \
update it as you go (same `id`), and place the result in the card's box: the card turns into the result \
with a short animation. Use state \"failed\" with a friendly `error` if something did not work (the board \
offers a retry), \"done\" or \"clear\" to remove it. Nothing is saved in the lesson; it needs no permission.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn show_progress(&self, Parameters(p): Parameters<ShowProgressParams>) -> CallToolResult {
        match op_show_progress(p) {
            Ok(op) => self.live_call(op).await,
            Err(e) => CallToolResult::structured_error(e.to_json()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_params_become_a_validated_op() {
        let p: ShowProgressParams = serde_json::from_value(serde_json::json!({
            "title": "Caffeine", "step": "Bonds…", "kind": "object",
            "plan": [{"content": "Atoms", "status": "completed"}], "x": 10, "y": 20, "w": 800, "h": 450
        }))
        .unwrap();
        let op = op_show_progress(p).unwrap();
        assert_eq!(op.name(), "show_progress");
        let bad: ShowProgressParams =
            serde_json::from_value(serde_json::json!({"state": "boom"})).unwrap();
        assert!(op_show_progress(bad).is_err());
        assert!(
            serde_json::from_value::<ShowProgressParams>(serde_json::json!({"bogus": 1})).is_err()
        );
    }
}
