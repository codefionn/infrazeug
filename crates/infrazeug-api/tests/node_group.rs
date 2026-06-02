//! Sync and async node groups form one connected DAG via begin/finish bookends.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{begin_node_id, finish_node_id, AsyncNodeGroup, SyncNodeGroup};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::node::NodeBody;
use infrazeug_core::Targets;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

fn nid(seed: &str) -> NodeId {
    NodeId(Uuid::parse_str(seed).unwrap())
}

fn deps_of(infra: &infrazeug_core::Infra, id: NodeId) -> Vec<NodeId> {
    infra
        .nodes
        .iter()
        .find(|n| n.id == id)
        .map(|n| n.deps.clone())
        .unwrap_or_default()
}

#[test]
fn sync_group_forms_connected_dag() {
    let machine_id = MachineId(Uuid::new_v4());
    let prep = nid("10000000-0000-4000-8000-000000000001");
    let a = nid("20000000-0000-4000-8000-000000000002");
    let b = nid("30000000-0000-4000-8000-000000000003");
    let tail = nid("40000000-0000-4000-8000-000000000004");
    let begin = begin_node_id("deploy-steps");
    let finish = finish_node_id("deploy-steps");

    let mut sync = SyncNodeGroup::new("deploy-steps", [prep]);
    let builder = InfraBuilder::new()
        .machine(builder::local(machine_id, "host"))
        .unwrap()
        .shell_on_machine(prep, "prep", machine_id, ShellOp::run(argv!["true"]))
        .unwrap()
        .begin_sync_group(&mut sync, Targets::Machine(machine_id))
        .unwrap()
        .shell_node(a, machine_id, ShellOp::run(argv!["echo", "a"]))
        .name("step-a")
        .in_sync_group(&sync)
        .build()
        .unwrap();
    sync.push(a);
    let builder = builder
        .shell_node(b, machine_id, ShellOp::run(argv!["echo", "b"]))
        .name("step-b")
        .in_sync_group(&sync)
        .build()
        .unwrap();
    sync.push(b);
    let (builder, exit) = builder
        .finish_sync_group(&mut sync, Targets::Machine(machine_id))
        .unwrap();
    assert_eq!(exit, finish);
    let infra = builder
        .shell_node(tail, machine_id, ShellOp::run(argv!["echo", "tail"]))
        .name("tail")
        .deps([sync.exit().unwrap()])
        .build()
        .unwrap()
        .build()
        .infra;

    assert_eq!(deps_of(&infra, begin), vec![prep]);
    assert_eq!(deps_of(&infra, a), vec![begin]);
    assert_eq!(deps_of(&infra, b), vec![a]);
    assert_eq!(deps_of(&infra, finish), vec![b]);
    assert_eq!(deps_of(&infra, tail), vec![finish]);
    assert!(matches!(
        infra.nodes.iter().find(|n| n.id == begin).unwrap().body,
        NodeBody::Begin
    ));
    assert!(matches!(
        infra.nodes.iter().find(|n| n.id == finish).unwrap().body,
        NodeBody::Finish
    ));
}

#[test]
fn async_group_forms_connected_dag() {
    let machine_id = MachineId(Uuid::new_v4());
    let prep = nid("10000000-0000-4000-8000-000000000011");
    let x = nid("20000000-0000-4000-8000-000000000012");
    let y = nid("30000000-0000-4000-8000-000000000013");
    let tail = nid("50000000-0000-4000-8000-000000000015");
    let begin = begin_node_id("parallel-tasks");
    let finish = finish_node_id("parallel-tasks");

    let mut async_g = AsyncNodeGroup::new("parallel-tasks", [prep]);
    let builder = InfraBuilder::new()
        .machine(builder::local(machine_id, "host"))
        .unwrap()
        .shell_on_machine(prep, "prep", machine_id, ShellOp::run(argv!["true"]))
        .unwrap()
        .begin_async_group(&mut async_g, Targets::Machine(machine_id))
        .unwrap()
        .shell_node(x, machine_id, ShellOp::run(argv!["echo", "x"]))
        .name("task-x")
        .in_async_group(&async_g)
        .build()
        .unwrap();
    async_g.push(x);
    let builder = builder
        .shell_node(y, machine_id, ShellOp::run(argv!["echo", "y"]))
        .name("task-y")
        .in_async_group(&async_g)
        .build()
        .unwrap();
    async_g.push(y);
    let (builder, exit) = builder
        .finish_async_group(&mut async_g, Targets::Machine(machine_id))
        .unwrap();
    assert_eq!(exit, finish);
    let infra = builder
        .shell_node(tail, machine_id, ShellOp::run(argv!["echo", "tail"]))
        .name("tail")
        .deps([exit])
        .build()
        .unwrap()
        .build()
        .infra;

    assert_eq!(deps_of(&infra, begin), vec![prep]);
    assert_eq!(deps_of(&infra, x), vec![begin]);
    assert_eq!(deps_of(&infra, y), vec![begin]);
    assert_eq!(deps_of(&infra, finish), vec![x, y]);
    assert_eq!(deps_of(&infra, tail), vec![finish]);
    assert!(matches!(
        infra.nodes.iter().find(|n| n.id == begin).unwrap().body,
        NodeBody::Begin
    ));
    assert!(matches!(
        infra.nodes.iter().find(|n| n.id == finish).unwrap().body,
        NodeBody::Finish
    ));
}
