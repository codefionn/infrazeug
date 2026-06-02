//! End-to-end check of the typed-group + `template!` flow: `on_group` resolves
//! each machine's vars into the schema struct, renders per-machine, and stores
//! the bytes in a `WriteFile` node.

use infrazeug_api::builder::{self, write_rendered, InfraBuilder};
use infrazeug_api::template;
use infrazeug_core::id::{GroupId, MachineId};
use infrazeug_core::machine::Group;
use infrazeug_core::node::NodeBody;
use infrazeug_core::varset::{VarKey, VarSet, VarValue};
use infrazeug_core::{Machine, Targets};
use infrazeug_shell::{FileSource, ShellOp};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
struct ClusterVars {
    k3s_token: String,
    node_name: String,
    hosts: Vec<HostEntry>,
}

#[derive(Deserialize)]
struct HostEntry {
    name: String,
    ip: String,
}

fn rendered_for(machine: &Machine, body_bytes: &[(String, Vec<u8>)]) -> String {
    let (_, bytes) = body_bytes
        .iter()
        .find(|(name, _)| name == &format!("write-cluster-config@{}", machine.name))
        .expect("node for machine");
    String::from_utf8(bytes.clone()).unwrap()
}

#[test]
fn on_group_renders_per_machine() {
    let group_id = GroupId(Uuid::new_v4());

    let mut group_vars = VarSet::new();
    group_vars.insert(
        VarKey::new("k3s_token"),
        VarValue::Scalar(serde_json::json!("s3cr3t")),
    );
    group_vars.insert(
        VarKey::new("hosts"),
        VarValue::Scalar(serde_json::json!([
            {"name": "pi-0", "ip": "10.10.0.10"},
            {"name": "pi-1", "ip": "10.10.0.11"},
        ])),
    );

    let mut b = InfraBuilder::new()
        .group(Group {
            id: group_id,
            name: "raspberry_cluster".into(),
            vars: group_vars,
        })
        .unwrap();

    for node_name in ["pi-0", "pi-1"] {
        let mut m: Machine = builder::local(MachineId(Uuid::new_v4()), node_name);
        m.groups.push(group_id);
        m.vars.insert(
            VarKey::new("node_name"),
            VarValue::Scalar(serde_json::json!(node_name)),
        );
        b = b.machine(m).unwrap();
    }

    let cluster = b.typed_group::<ClusterVars>("raspberry_cluster").unwrap();
    let bundle = b
        .on_group(cluster, "write-cluster-config", |_m, v: &ClusterVars| {
            let cfg = template!(
                "node={{ v.node_name }} token={{ v.k3s_token }}\n@for h in &v.hosts {peer {{ h.name }} {{ h.ip }}\n}",
                v = v
            );
            vec![write_rendered("/etc/k3s/cluster.yaml", 0o640, cfg)]
        })
        .unwrap()
        .build();
    let infra = &bundle.infra;

    // One write node per group machine (plus an injected per-machine connect head).
    let write_nodes: Vec<_> = infra
        .nodes
        .iter()
        .filter(|n| matches!(n.body, NodeBody::Shell(ShellOp::WriteFile { .. })))
        .collect();
    assert_eq!(write_nodes.len(), 2);

    // Collect (node name, rendered bytes) from each WriteFile body.
    let mut bodies: Vec<(String, Vec<u8>)> = Vec::new();
    for node in write_nodes {
        assert!(matches!(node.targets, Targets::Machine(_)));
        let NodeBody::Shell(ShellOp::WriteFile {
            path,
            content,
            mode,
        }) = &node.body
        else {
            panic!("expected WriteFile body, got {:?}", node.body);
        };
        assert_eq!(path.to_str().unwrap(), "/etc/k3s/cluster.yaml");
        assert_eq!(mode, &0o640);
        let FileSource::Bytes(bytes) = content else {
            panic!("expected inline bytes");
        };
        bodies.push((node.name.clone(), bytes.clone()));
    }

    // Per-machine rendering: each node embeds its own node_name, shared token + hosts.
    let machines: Vec<&Machine> = infra.machines.iter().collect();
    for m in machines {
        let out = rendered_for(m, &bodies);
        assert!(out.contains(&format!("node={}", m.name)), "got: {out}");
        assert!(out.contains("token=s3cr3t"), "got: {out}");
        assert!(out.contains("peer pi-0 10.10.0.10"), "got: {out}");
        assert!(out.contains("peer pi-1 10.10.0.11"), "got: {out}");
    }

    // The two nodes differ (different node_name) but share host list.
    assert_ne!(bodies[0].1, bodies[1].1);
}

#[test]
fn deterministic_node_ids() {
    // Re-building the same infra yields identical node ids (stable plans).
    fn build() -> Vec<Uuid> {
        let group_id = GroupId(Uuid::parse_str("c0000000-0000-4000-8000-000000000099").unwrap());
        let mid = MachineId(Uuid::parse_str("a0000000-0000-4000-8000-000000000099").unwrap());
        let mut b = InfraBuilder::new()
            .group(Group {
                id: group_id,
                name: "g".into(),
                vars: VarSet::new(),
            })
            .unwrap();
        let mut m = builder::local(mid, "host");
        m.groups.push(group_id);
        b = b.machine(m).unwrap();
        let g = b
            .typed_group::<std::collections::BTreeMap<String, String>>("g")
            .unwrap();
        let bundle = b
            .on_group(g, "cfg", |_m, _v| {
                vec![write_rendered("/x", 0o644, "hi".into())]
            })
            .unwrap()
            .build();
        bundle.infra.nodes.iter().map(|n| n.id.0).collect()
    }
    assert_eq!(build(), build());
}
