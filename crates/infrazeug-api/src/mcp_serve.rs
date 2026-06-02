//! API-side glue for the MCP server (SOUL §6bis).
//!
//! [`McpExt`] adds `infra.mcp()`, returning an [`McpBuilder`] pre-wired with
//! [`ApiExecutor`] — the bridge that runs a tool-built `Infra` through the real
//! apply pipeline (emulation prep, agent build, transport connect, scheduler)
//! and reports back captured stdout. This is the only place that closes the
//! `mcp -> core`/`api -> mcp` loop, so the dependency edge stays one-way.

use std::sync::Arc;

use async_trait::async_trait;
use infrazeug_core::Infra;
use infrazeug_mcp::{InfraExecutor, McpBuilder, ToolRun};

/// Executes MCP tool infras via the full `infrazeug-api` apply path.
#[derive(Default)]
pub struct ApiExecutor;

#[async_trait]
impl InfraExecutor for ApiExecutor {
    async fn run(&self, infra: Infra) -> anyhow::Result<ToolRun> {
        crate::run_infra_collect(&crate::PlaybookBundle::from_infra(infra)).await
    }
}

/// Extension trait adding `infra.mcp()` to build an MCP server over this infra.
pub trait McpExt {
    /// Start an [`McpBuilder`] over a snapshot of this infra's machines, with
    /// the API apply pipeline injected as the tool executor.
    fn mcp(&self) -> McpBuilder;
}

impl McpExt for Infra {
    fn mcp(&self) -> McpBuilder {
        McpBuilder::new(self.machines.clone())
            .with_graph(self.graph_view().unwrap_or_default())
            .with_executor(Arc::new(ApiExecutor))
    }
}
