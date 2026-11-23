//! The live agent bridge between `rupok mcp --live` (the teacher's own AI
//! agent) and the lesson open on the board.
//!
//! ```text
//!  agent ──MCP/stdio──▶ rupok mcp ──WebSocket 127.0.0.1──▶ board (wasm)
//!                        │  Loro mirror ◀── updates ──┤  Lesson (authoritative)
//!                        └── op {add_text…} ─────────▶ apply() as a local change
//! ```
//!
//! - [`protocol`]: the JSON messages on the WebSocket, ports, and the
//!   origin/host checks the server applies before a board may connect.
//! - [`op`]: the agent operations ([`Op`]), shared by the server (which builds
//!   them from MCP tool calls) and the board (which runs them).
//! - [`apply`]: runs an [`Op`] on a [`rupok_format::Package`] + [`rupok_core::Lesson`] as
//!   **one** Loro commit tagged with the agent's name, so every agent action is
//!   one undo step and stays attributed in the history.
//! - [`attribution`]: which items an agent made, read back from that history.
//!
//! Agent operations run on the board, not on the server: they are local
//! changes there, so the teacher's Ctrl+Z undoes them like their own. Loro
//! updates still flow both ways: the board streams every change to the
//! server's mirror, and after a reconnect the mirror sends back whatever the
//! board is missing (e.g. after a reload from an older autosave), so edits
//! from both sides merge.
//!
//! Like the rest of the core this crate runs in `wasm32-unknown-unknown`:
//! no clock (time is passed in as `now_ms`) and no randomness.

pub mod apply;
pub mod attribution;
pub mod design;
pub mod handwriting;
pub mod op;
pub mod protocol;

pub use apply::{BoardCtx, LiveError, Outcome, apply, apply_with, doc_key};
pub use attribution::{agent_items, parse_commit_message};
pub use op::{Anchor, BrandingArgs, Op, PageDesignArgs, PatternName, Rect};

/// Prefix of the Loro commit message of every change an agent makes:
/// `rupok-agent/1 {"agent":"Claude Code","op":"add_text"}`.
pub const COMMIT_PREFIX: &str = "rupok-agent/1 ";

/// The commit message that attributes a change to `agent` running `op`.
pub fn commit_message(agent: &str, op: &str) -> String {
    format!(
        "{COMMIT_PREFIX}{}",
        serde_json::json!({ "agent": agent, "op": op })
    )
}

/// Loro sync helpers on encoded version vectors (what goes on the wire).
pub mod sync {
    use loro::VersionVector;
    use rupok_core::Lesson;

    /// The lesson's version vector, encoded.
    pub fn version(lesson: &Lesson) -> Vec<u8> {
        lesson.version().encode()
    }

    /// Everything `lesson` has that a peer at encoded version `from` lacks;
    /// an empty `from` means "everything".
    pub fn export_since(lesson: &Lesson, from: &[u8]) -> Result<Vec<u8>, String> {
        let vv = if from.is_empty() {
            VersionVector::default()
        } else {
            VersionVector::decode(from).map_err(|e| format!("bad version vector: {e}"))?
        };
        Ok(lesson.export_updates(&vv))
    }

    /// True when `lesson` has changes a peer at encoded version `peer` lacks.
    pub fn is_ahead(lesson: &Lesson, peer: &[u8]) -> bool {
        match VersionVector::decode(peer) {
            Ok(vv) => lesson.version() != vv && !vv.includes_vv(&lesson.version()),
            Err(_) => true,
        }
    }
}
