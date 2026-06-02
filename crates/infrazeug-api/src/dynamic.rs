//! Builder surface for dynamic machine groups: a discovery native node plus a
//! per-machine template fanned out at apply time.
//!
//! ```ignore
//! builder
//!     .discover_machines(disc_id, "discover-workers", controller, "workers", DiscoverWorkers, input)?
//!         .deps([prep_id])
//!         .fail_fast(false)
//!         .max_parallel_machines(10)
//!         .for_each_machine(|m| {
//!             m.connectivity(connect_id, "connect");
//!             m.shell(install_id, "install", ShellOp::run(argv!["apt-get", "install", "-y", "nginx"]), [connect_id]);
//!         })?
//! ```

use crate::builder::InfraBuilder;
use infrazeug_core::dynamic::{dyn_exit_node_id, template_placeholder_machine, DynamicGroup};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::node::FailPolicy;
use infrazeug_core::{barrier_node, connect_node, Node, NodeBuilder, RunPolicy, Targets};
use infrazeug_native::{encode_input, NodeMethod};
use infrazeug_shell::ShellOp;

impl InfraBuilder {
    /// Begin a dynamic machine group discovered by native method `method`.
    ///
    /// `machine_id` is the seed machine the discovery node runs on (typically the
    /// controller). The method's node capture must be a JSON array of
    /// [`DiscoveredMachine`](infrazeug_core::machine::DiscoveredMachine) (e.g. via
    /// `NativeResult::changed(..).with_json_capture(&machines)`); it may read any
    /// upstream prep node's capture to assemble that list. Returns a
    /// [`DynamicGroupBuilder`] to attach deps, policy, and the per-machine
    /// template via [`for_each_machine`](DynamicGroupBuilder::for_each_machine).
    pub fn discover_machines<M: NodeMethod + 'static>(
        mut self,
        node_id: NodeId,
        name: &str,
        machine_id: MachineId,
        label: &str,
        method: M,
        input: M::Input,
    ) -> anyhow::Result<DynamicGroupBuilder> {
        let method_id = method.name().to_string();
        self.register_native_method(method);
        let input = encode_input(&input).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(DynamicGroupBuilder {
            builder: self,
            node_id,
            name: name.to_string(),
            machine_id,
            method_id,
            input,
            label: label.to_string(),
            deps: Vec::new(),
            fail_policy: FailPolicy::Tolerate {
                max_failed: usize::MAX,
            },
            max_parallel_machines: None,
        })
    }
}

/// Staged dynamic-group helper returned by [`InfraBuilder::discover_machines`].
pub struct DynamicGroupBuilder {
    builder: InfraBuilder,
    node_id: NodeId,
    name: String,
    machine_id: MachineId,
    method_id: String,
    input: serde_cbor::Value,
    label: String,
    deps: Vec<NodeId>,
    fail_policy: FailPolicy,
    max_parallel_machines: Option<usize>,
}

impl DynamicGroupBuilder {
    /// Upstream deps for the discovery node (e.g. prep nodes producing data it reads).
    pub fn deps(mut self, deps: impl IntoIterator<Item = NodeId>) -> Self {
        self.deps.extend(deps);
        self
    }

    /// Per-machine failure handling. `false` (default) tolerates a bad machine and
    /// continues the rest; `true` aborts the whole batch on the first failure.
    pub fn fail_fast(mut self, yes: bool) -> Self {
        self.fail_policy = if yes {
            FailPolicy::FailFast
        } else {
            FailPolicy::Tolerate {
                max_failed: usize::MAX,
            }
        };
        self
    }

    /// Cap concurrently-running discovered machines.
    pub fn max_parallel_machines(mut self, n: usize) -> Self {
        self.max_parallel_machines = Some(n);
        self
    }

    /// The downstream join point for the whole fan-out (depend on this to run
    /// after every discovered machine finishes its template).
    pub fn exit_id(&self) -> NodeId {
        dyn_exit_node_id(&self.label)
    }

    /// Define the per-machine template (the "playbook" run on each discovered
    /// machine) and finalize the group. Returns the [`InfraBuilder`] with the
    /// discovery node, the recorded [`DynamicGroup`], and the exit barrier added.
    pub fn for_each_machine<F>(self, body: F) -> anyhow::Result<InfraBuilder>
    where
        F: FnOnce(&mut MachineTemplate),
    {
        let DynamicGroupBuilder {
            mut builder,
            node_id,
            name,
            machine_id,
            method_id,
            input,
            label,
            deps,
            fail_policy,
            max_parallel_machines,
        } = self;

        let mut template = MachineTemplate { nodes: Vec::new() };
        body(&mut template);
        if template.nodes.is_empty() {
            anyhow::bail!("dynamic group `{label}` has an empty per-machine template");
        }

        // Discovery node: a native node whose capture yields Vec<DiscoveredMachine>.
        let discovery =
            NodeBuilder::native_with_input(node_id, method_id, input, Targets::Machine(machine_id))
                .name(&name)
                .deps(deps)
                .run_policy(RunPolicy::Always)
                .build();
        builder = builder.add_built_node(discovery)?;

        // Exit barrier: at expansion its deps extend to every per-machine leaf. It
        // carries the group's fail policy so a tolerate group still joins (and
        // downstream proceeds) even when some machine failed.
        let mut exit = barrier_node(
            dyn_exit_node_id(&label),
            format!("{label}/exit"),
            Targets::Machine(machine_id),
            vec![node_id],
        );
        exit.policy.fail_policy = fail_policy;
        builder = builder.add_built_node(exit)?;

        builder.add_dynamic_group(DynamicGroup {
            label,
            discovery_node: node_id,
            template: template.nodes,
            template_entry_deps: vec![node_id],
            fail_policy,
            max_parallel_machines,
        });
        Ok(builder)
    }
}

/// Accumulates the per-machine template nodes for a [`DynamicGroupBuilder`].
///
/// Template nodes carry a placeholder machine target and template-relative deps;
/// the scheduler remaps both when binding the template to a discovered machine.
/// Native template nodes reference methods that must already be registered on the
/// builder (via [`InfraBuilder::method`]).
pub struct MachineTemplate {
    nodes: Vec<Node>,
}

impl MachineTemplate {
    fn placeholder() -> Targets {
        Targets::Machine(template_placeholder_machine())
    }

    /// Per-machine connectivity / agent-upload head (see
    /// [`NodeBody::Connect`](infrazeug_core::node::NodeBody::Connect)).
    pub fn connectivity(&mut self, id: NodeId, name: &str) -> &mut Self {
        self.nodes
            .push(connect_node(id, name, Self::placeholder(), Vec::new()));
        self
    }

    /// A shell step in the template.
    pub fn shell(
        &mut self,
        id: NodeId,
        name: &str,
        op: ShellOp,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> &mut Self {
        self.nodes.push(
            NodeBuilder::shell(id, op, Self::placeholder())
                .name(name)
                .deps(deps.into_iter().collect())
                .build(),
        );
        self
    }

    /// A native step in the template, referencing an already-registered method id.
    pub fn native(
        &mut self,
        id: NodeId,
        name: &str,
        method_id: &str,
        input: serde_cbor::Value,
        deps: impl IntoIterator<Item = NodeId>,
    ) -> &mut Self {
        self.nodes.push(
            NodeBuilder::native_with_input(id, method_id, input, Self::placeholder())
                .name(name)
                .deps(deps.into_iter().collect())
                .build(),
        );
        self
    }

    /// Escape hatch: push a fully-built template node (placeholder target +
    /// template-relative deps are the caller's responsibility).
    pub fn node(&mut self, node: Node) -> &mut Self {
        self.nodes.push(node);
        self
    }
}
