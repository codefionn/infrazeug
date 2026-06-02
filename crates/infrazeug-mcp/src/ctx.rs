//! Read-only view of the served infra's machine catalog, handed to tool
//! closures so they can target the already-configured machines (real SSH /
//! transport settings) when building their sub-infra.

use infrazeug_core::Machine;

#[derive(Clone, Default)]
pub struct McpCtx {
    machines: Vec<Machine>,
}

impl McpCtx {
    pub fn new(machines: Vec<Machine>) -> Self {
        Self { machines }
    }

    /// All machines registered on the served infra.
    pub fn machines(&self) -> &[Machine] {
        &self.machines
    }

    /// Clone of the machine with the given name, for use as a tool target.
    pub fn machine(&self, name: &str) -> anyhow::Result<Machine> {
        self.machines
            .iter()
            .find(|m| m.name == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no machine named `{name}` on this infra"))
    }
}
