//! MCP CLI surface (`mcp serve`) for embedders and the stock `infrazeug` binary.

use infrazeug_mcp::McpBuilder;

/// Top-level MCP subcommand names (stable catalog).
pub const MCP_SUBCOMMANDS: &[&str] = &["mcp"];

/// Nested subcommands under `mcp`.
pub const MCP_NESTED_SUBCOMMANDS: &[&str] = &["serve"];

/// How `mcp serve` exposes the protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpServeMode {
    /// JSON-RPC over stdio (default; desktop MCP clients spawn the binary).
    Stdio,
    /// Streamable HTTP JSON-RPC server on `ADDR` (e.g. `127.0.0.1:7777`).
    Http(String),
}

impl McpServeMode {
    /// Parse `--stdio` / `--http ADDR` from clap (`--http` wins over default stdio).
    pub fn from_cli(http: Option<&str>, stdio_flag: bool) -> anyhow::Result<Self> {
        match (http, stdio_flag) {
            (Some(_), true) => {
                anyhow::bail!("`mcp serve` cannot use both --stdio and --http");
            }
            (Some(addr), false) => Ok(Self::Http(addr.to_string())),
            (None, true) => Ok(Self::Stdio),
            (None, false) => Ok(Self::Stdio),
        }
    }
}

/// Run `mcp serve` with the selected transport.
pub async fn dispatch_mcp_serve(builder: McpBuilder, mode: McpServeMode) -> anyhow::Result<()> {
    match mode {
        McpServeMode::Stdio => builder.serve_stdio().await,
        McpServeMode::Http(bind) => builder.serve_http(bind).await,
    }
}
