//! Execution boundary between the MCP server and the apply pipeline.
//!
//! `infrazeug-mcp` never runs an [`Infra`] itself — it depends only on
//! `infrazeug-core`. The full apply path (emulation prep, agent build,
//! transport connect, scheduler) lives in `infrazeug-api`, which implements
//! [`InfraExecutor`] and injects it via [`crate::McpBuilder::with_executor`].
//! This keeps the dependency edge one-way (`api -> mcp`) with no cycle.

use async_trait::async_trait;
use infrazeug_core::report::RunReport;
use infrazeug_core::Infra;

/// Captured stdout for one node/machine pair after a tool's infra ran.
#[derive(Clone, Debug)]
pub struct CaptureOut {
    pub node: String,
    pub machine: String,
    pub stdout: String,
}

/// Result of executing a tool-built [`Infra`].
#[derive(Clone, Debug)]
pub struct ToolRun {
    pub report: RunReport,
    pub captures: Vec<CaptureOut>,
}

impl ToolRun {
    /// First capture whose node name matches `node`, trimmed.
    pub fn capture(&self, node: &str) -> Option<&str> {
        self.captures
            .iter()
            .find(|c| c.node == node)
            .map(|c| c.stdout.trim())
    }

    /// True if no report entry ended in a failed state.
    pub fn all_ok(&self) -> bool {
        use infrazeug_core::node::NodeStatus;
        self.report
            .entries
            .iter()
            .all(|e| !matches!(e.status, NodeStatus::Failed))
    }
}

/// Runs a tool-built [`Infra`] through the real transport/scheduler path.
#[async_trait]
pub trait InfraExecutor: Send + Sync + 'static {
    async fn run(&self, infra: Infra) -> anyhow::Result<ToolRun>;
}
