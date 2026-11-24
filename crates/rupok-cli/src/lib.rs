//! The `rupok` command-line tool and MCP server.
//!
//! - [`workspace`]: the path sandbox for agent-supplied paths.
//! - [`ops`]: lesson operations on resolved paths (shared by CLI and MCP).
//! - [`tools`]: the MCP tool surface as plain Rust (params + [`tools::Tools`]).
//! - [`mcp`]: the rmcp stdio server wrapping [`tools::Tools`].
//! - [`live`], [`mcp_live`]: the live board channel (`rupok mcp --live`) and its tools;
//!   [`mcp_progress`]: `show_progress`, the agent's working card on the board.
//! - [`cli`]: the clap command line.
//! - [`spec`]: the Smart Object spec served to agents.

pub mod chat;
pub mod cli;
pub mod live;
pub mod mcp;
pub mod mcp_http;
pub mod mcp_live;
pub mod mcp_progress;
pub mod ops;
pub mod spec;
pub mod tools;
pub mod workspace;
