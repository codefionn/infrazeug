//! Executor used while the playbook MCP child is still building.

use async_trait::async_trait;
use infrazeug_core::Infra;

use crate::exec::{InfraExecutor, ToolRun};

/// Rejects tool runs until the real playbook MCP server is connected.
#[derive(Debug, Default)]
pub struct BuildingExecutor;

#[async_trait]
impl InfraExecutor for BuildingExecutor {
    async fn run(&self, _infra: Infra) -> anyhow::Result<ToolRun> {
        anyhow::bail!("playbook MCP server is still building; retry tools/call in a few seconds")
    }
}
