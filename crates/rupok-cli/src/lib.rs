//! The `rupok` command-line tool and MCP server.
//!
//! - [`workspace`]: the path sandbox for agent-supplied paths.
//! - [`ops`]: lesson operations on resolved paths (shared by CLI and MCP).
//! - [`tools`]: the MCP tool surface as plain Rust (params + [`tools::Tools`]).
//! - [`mcp`]: the rmcp stdio server wrapping [`tools::Tools`].
//! - [`cli`]: the clap command line.
//! - [`spec`]: the Smart Object spec served to agents.

pub mod cli;
pub mod mcp;
pub mod ops;
pub mod spec;
pub mod tools;
pub mod workspace;
