use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::{
    connect_node_id, end_node_id, start_node_id, NodeBuilder, RunPolicy, Targets,
};
use infrazeug_shell::ShellOp;
use uuid::Uuid;

#[test]
fn lazy_nodes_do_not_receive_injected_connect_dependency() {
    let machine = MachineId(Uuid::new_v4());
    let lazy_id = NodeId(Uuid::new_v4());
    let eager_id = NodeId(Uuid::new_v4());

    let lazy = NodeBuilder::shell(
        lazy_id,
        ShellOp::run(vec!["true".into()]),
        Targets::Machine(machine),
    )
    .name("lazy")
    .run_policy(RunPolicy::Lazy)
    .build();
    let eager = NodeBuilder::shell(
        eager_id,
        ShellOp::run(vec!["true".into()]),
        Targets::Machine(machine),
    )
    .name("eager")
    .build();

    let infra = InfraBuilder::new()
        .machine(builder::controller(machine))
        .unwrap()
        .node(lazy)
        .unwrap()
        .node(eager)
        .unwrap()
        .build_infra();

    let connect = connect_node_id(machine);
    let start = start_node_id();
    let end = end_node_id();
    let lazy = infra
        .nodes
        .iter()
        .find(|node| node.id == lazy_id)
        .expect("lazy node");
    let eager = infra
        .nodes
        .iter()
        .find(|node| node.id == eager_id)
        .expect("eager node");
    let connect_node = infra
        .nodes
        .iter()
        .find(|node| node.id == connect)
        .expect("connect node");
    let end_node = infra
        .nodes
        .iter()
        .find(|node| node.id == end)
        .expect("end node");

    assert!(
        infra.nodes.iter().any(|node| node.id == start),
        "builder should inject a real execution start node"
    );
    assert!(
        connect_node.deps.contains(&start),
        "connect head should be downstream of the execution start node"
    );
    assert!(
        !lazy.deps.contains(&connect),
        "lazy node must not share the unified connect start node"
    );
    assert!(
        eager.deps.contains(&connect),
        "non-lazy machine root should still start at the connect node"
    );
    assert!(
        end_node.deps.contains(&eager_id),
        "end should join non-lazy terminal work"
    );
    assert!(
        !end_node.deps.contains(&lazy_id),
        "end must not demand dormant lazy leaves"
    );
}

#[test]
fn graph_only_dependency_does_not_cover_eager_connectivity() {
    let machine = MachineId(Uuid::new_v4());
    let begin_id = NodeId(Uuid::new_v4());
    let eager_id = NodeId(Uuid::new_v4());

    let begin = NodeBuilder::begin(begin_id, Targets::Machine(machine))
        .name("begin")
        .build();
    let eager = NodeBuilder::shell(
        eager_id,
        ShellOp::run(vec!["true".into()]),
        Targets::Machine(machine),
    )
    .name("eager")
    .deps(vec![begin_id])
    .build();

    let infra = InfraBuilder::new()
        .machine(builder::controller(machine))
        .unwrap()
        .node(begin)
        .unwrap()
        .node(eager)
        .unwrap()
        .build_infra();

    let connect = connect_node_id(machine);
    let start = start_node_id();
    let end = end_node_id();
    let begin = infra
        .nodes
        .iter()
        .find(|node| node.id == begin_id)
        .expect("begin node");
    let eager = infra
        .nodes
        .iter()
        .find(|node| node.id == eager_id)
        .expect("eager node");
    let connect_node = infra
        .nodes
        .iter()
        .find(|node| node.id == connect)
        .expect("connect node");
    let end_node = infra
        .nodes
        .iter()
        .find(|node| node.id == end)
        .expect("end node");

    assert!(
        connect_node.deps.contains(&start),
        "connect head should be downstream of the execution start node"
    );
    assert!(
        begin.deps.contains(&connect),
        "machine-targeted graph-only roots should share the machine connect head"
    );
    assert!(
        eager.deps.contains(&begin_id),
        "eager node should keep its graph-only ordering dependency"
    );
    assert!(
        !eager.deps.contains(&connect),
        "graph-only deps that already carry connectivity should avoid redundant direct connect edges"
    );
    assert!(
        end_node.deps.contains(&eager_id),
        "end should join through the real non-lazy terminal node"
    );
}
