//! MCP server for infrazeug deployment binaries (SOUL §6bis).
//!
//! The same binary that deploys infrastructure can expose live tools to an MCP
//! client. Builtins include `list_machines`, `ping`, and `graph`, plus a
//! read-only documentation resource at [`docs::DOCS_URI`]. Custom tools are
//! authors register custom tools whose typed input builds an [`Infra`] that the
//! server executes through the real transport/scheduler path:
//!
//! ```ignore
//! infra.mcp()
//!     .tool::<UnitInput, _>("unit_status", "Check a systemd unit", |inp, ctx| {
//!         let m = ctx.machine(&inp.machine)?;
//!         Ok(/* an Infra with a `systemctl is-active <unit>` node */)
//!     })
//!     .serve_stdio().await?;
//! ```
//!
//! Secrets are never exposed: no vault/secret tool is compiled into this crate
//! (SOUL §6.10 / §6bis.4), and it is not configurable.
//!
//! [`Infra`]: infrazeug_core::Infra

mod api_docs;
mod builder;
mod building;
mod builtins;
mod ctx;
mod docs;
mod exec;
mod http;
mod server;
mod watch;

pub use api_docs::{DocIndex, DocItem, DocSearchResult};
pub use builder::McpBuilder;
pub use building::BuildingExecutor;
pub use ctx::McpCtx;
pub use docs::{DOCS_API_INDEX_URI, DOCS_NAME, DOCS_URI};
pub use exec::{CaptureOut, InfraExecutor, ToolRun};
pub use watch::WatchProxy;
