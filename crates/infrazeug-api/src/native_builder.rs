//! Fluent builder for tier-1 native nodes.

use crate::builder::InfraBuilder;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::{AsyncNodeGroup, NodeBuilder, RunPolicy, SyncNodeGroup, Targets};
use infrazeug_native::{encode_input, NodeMethod};

/// Staged native-node helper returned by [`InfraBuilder::native`] and [`InfraBuilder::native_typed`].
pub struct NativeNodeBuilder {
    builder: InfraBuilder,
    node_id: NodeId,
    machine_id: MachineId,
    method_id: String,
    input: serde_cbor::Value,
    name: String,
    description: Option<String>,
    deps: Vec<NodeId>,
    run_policy: RunPolicy,
}

impl NativeNodeBuilder {
    pub(crate) fn new(
        builder: InfraBuilder,
        node_id: NodeId,
        name: impl Into<String>,
        machine_id: MachineId,
        method_id: impl Into<String>,
        input: serde_cbor::Value,
    ) -> Self {
        Self {
            builder,
            node_id,
            machine_id,
            method_id: method_id.into(),
            input,
            name: name.into(),
            deps: Vec::new(),
            description: None,
            run_policy: RunPolicy::default(),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.into();
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn deps(mut self, deps: impl IntoIterator<Item = NodeId>) -> Self {
        self.deps.extend(deps);
        self
    }

    /// Wire this node as the next member of a [`SyncNodeGroup`].
    pub fn in_sync_group(mut self, group: &SyncNodeGroup) -> Self {
        self.deps.extend(group.next_deps());
        self
    }

    /// Wire this node as a parallel member of an [`AsyncNodeGroup`].
    pub fn in_async_group(mut self, group: &AsyncNodeGroup) -> Self {
        self.deps.extend(group.next_deps());
        self
    }

    /// Run only when upstream nodes changed on this machine (default).
    pub fn on_upstream_change(mut self) -> Self {
        self.run_policy = RunPolicy::OnUpstreamChange;
        self
    }

    pub fn always(mut self) -> Self {
        self.run_policy = RunPolicy::Always;
        self
    }

    pub fn lazy(mut self) -> Self {
        self.run_policy = RunPolicy::Lazy;
        self
    }

    pub fn build(mut self) -> anyhow::Result<InfraBuilder> {
        let mut node = NodeBuilder::native_with_input(
            self.node_id,
            self.method_id,
            self.input,
            Targets::Machine(self.machine_id),
        )
        .name(self.name)
        .deps(self.deps)
        .run_policy(self.run_policy)
        .build();
        if let Some(d) = self.description {
            node = node.with_description(d);
        }
        self.builder = self.builder.add_built_node(node)?;
        Ok(self.builder)
    }
}

pub(crate) fn native_node_with_method<M: NodeMethod + 'static>(
    mut builder: InfraBuilder,
    node_id: NodeId,
    name: &str,
    machine_id: MachineId,
    method: M,
    input: M::Input,
) -> anyhow::Result<NativeNodeBuilder> {
    let method_id = method.name();
    builder.register_native_method(method);
    let input = encode_input(&input).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(NativeNodeBuilder::new(
        builder, node_id, name, machine_id, method_id, input,
    ))
}

pub(crate) fn native_node_typed<M: NodeMethod + 'static>(
    builder: InfraBuilder,
    node_id: NodeId,
    name: &str,
    machine_id: MachineId,
    input: M::Input,
) -> anyhow::Result<NativeNodeBuilder> {
    let method_id = builder
        .method_name::<M>()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "native method type `{}` is not registered — call `.method(..)` first",
                std::any::type_name::<M>()
            )
        })?
        .to_string();
    let input = encode_input(&input).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(NativeNodeBuilder::new(
        builder, node_id, name, machine_id, method_id, input,
    ))
}
