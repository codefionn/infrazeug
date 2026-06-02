//! Build-time shape of a dynamic machine group: the discovery node, the recorded
//! `DynamicGroup` (template + entry deps), and the exit barrier. Runtime fan-out
//! is exercised separately in the scheduler tests.

use async_trait::async_trait;
use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_core::dynamic::dyn_exit_node_id;
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::machine::{DiscoveredMachine, SshConfig};
use infrazeug_core::node::NodeBody;
use infrazeug_native::{NativeResult, NodeCtx, NodeMethod, Result as NativeMethodResult};
use infrazeug_shell::ShellOp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default)]
struct DiscoverHosts;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DiscoverInput {
    #[serde(default)]
    count: u32,
}

#[async_trait]
impl NodeMethod for DiscoverHosts {
    type Input = DiscoverInput;
    type Output = Vec<DiscoveredMachine>;

    fn name(&self) -> &'static str {
        "test.discover_hosts"
    }

    async fn execute(
        &self,
        _ctx: &NodeCtx,
        input: DiscoverInput,
    ) -> NativeMethodResult<NativeResult> {
        let machines: Vec<DiscoveredMachine> = (0..input.count)
            .map(|i| DiscoveredMachine {
                name: format!("worker-{i}"),
                ssh: SshConfig::new(format!("10.0.0.{i}")).with_user("deploy"),
                vars: Default::default(),
                tags: Vec::new(),
                os: None,
            })
            .collect();
        Ok(
            NativeResult::changed(format!("discovered {}", machines.len()))
                .with_json_capture(&machines)
                .expect("capture"),
        )
    }
}

fn nid(seed: u8) -> NodeId {
    NodeId(Uuid::from_bytes([seed; 16]))
}

#[test]
fn dynamic_group_records_discovery_template_and_exit() {
    let controller = MachineId(Uuid::new_v4());
    let prep = nid(1);
    let disc = nid(2);
    let connect = nid(3);
    let install = nid(4);

    let bundle = InfraBuilder::new()
        .machine(builder::controller(controller))
        .unwrap()
        .shell_on_machine(prep, "prep", controller, ShellOp::run(vec!["true".into()]))
        .unwrap()
        .discover_machines(
            disc,
            "discover-workers",
            controller,
            "workers",
            DiscoverHosts,
            DiscoverInput { count: 2 },
        )
        .unwrap()
        .deps([prep])
        .fail_fast(false)
        .max_parallel_machines(5)
        .for_each_machine(|m| {
            m.connectivity(connect, "connect");
            m.shell(
                install,
                "install",
                ShellOp::run(vec!["echo".into(), "go".into()]),
                [connect],
            );
        })
        .unwrap()
        .build();

    let infra = &bundle.infra;

    // Discovery node is a native node that depends on prep (+ injected connect head).
    let d = infra
        .nodes
        .iter()
        .find(|n| n.id == disc)
        .expect("discovery node");
    assert!(matches!(d.body, NodeBody::Native { .. }));
    assert!(d.deps.contains(&prep), "discovery should depend on prep");

    // Exit barrier exists and depends on the discovery node.
    let exit_id = dyn_exit_node_id("workers");
    let exit = infra
        .nodes
        .iter()
        .find(|n| n.id == exit_id)
        .expect("exit barrier");
    assert!(matches!(exit.body, NodeBody::Barrier));
    assert!(exit.deps.contains(&disc));

    // One dynamic group recorded with the 2-node template + entry deps.
    assert_eq!(infra.dynamic_groups.len(), 1);
    let g = &infra.dynamic_groups[0];
    assert_eq!(g.label, "workers");
    assert_eq!(g.discovery_node, disc);
    assert_eq!(g.template.len(), 2);
    assert_eq!(g.template_entry_deps, vec![disc]);
    assert!(matches!(
        g.fail_policy,
        infrazeug_core::node::FailPolicy::Tolerate { .. }
    ));
    assert_eq!(g.max_parallel_machines, Some(5));

    // Template carries a connect head and a shell node chained to it.
    let head = g
        .template
        .iter()
        .find(|n| n.id == connect)
        .expect("connect template node");
    assert!(matches!(head.body, NodeBody::Connect));
    let step = g
        .template
        .iter()
        .find(|n| n.id == install)
        .expect("install template node");
    assert_eq!(step.deps, vec![connect]);

    // Lint is clean (discovery registered, template well-formed).
    bundle.lint().expect("lint clean");
}

#[test]
fn empty_template_is_rejected() {
    let controller = MachineId(Uuid::new_v4());
    let err = InfraBuilder::new()
        .machine(builder::controller(controller))
        .unwrap()
        .discover_machines(
            nid(2),
            "discover",
            controller,
            "g",
            DiscoverHosts,
            DiscoverInput { count: 0 },
        )
        .unwrap()
        .for_each_machine(|_m| {})
        .err();
    assert!(err.is_some(), "empty template must fail to build");
}
