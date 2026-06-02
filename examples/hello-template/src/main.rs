//! Typed-group templating demo: render a per-machine config from a group's
//! typed var schema using the compile-time `template!` macro.
//!
//! Run `cargo run -p hello-template -- plan` to see one rendered `WriteFile`
//! node per cluster machine, each carrying that machine's resolved config.

use infrazeug_api::builder::{self, write_rendered, InfraBuilder};
use infrazeug_api::{init_tracing, run, template, RunBuildContext, RunConfig};
use infrazeug_core::id::{GroupId, MachineId};
use infrazeug_core::machine::Group;
use infrazeug_core::varset::{VarKey, VarSet, VarValue};
use infrazeug_core::{Machine, RuntimeConfig};
use serde::Deserialize;
use uuid::Uuid;

/// The group's var schema. `template!` field references are checked against it.
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

const CLUSTER_GROUP: &str = "c0000000-0000-4000-8000-000000000001";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-template").about("typed-group template! demo"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn scalar(v: serde_json::Value) -> VarValue {
    VarValue::Scalar(v)
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let group_id = GroupId(Uuid::parse_str(CLUSTER_GROUP)?);

    // Shared group vars: token + the full host list (any JSON shape works via Scalar).
    let mut group_vars = VarSet::new();
    group_vars.insert(
        VarKey::new("k3s_token"),
        scalar(serde_json::json!("s3cr3t-token")),
    );
    group_vars.insert(
        VarKey::new("hosts"),
        scalar(serde_json::json!([
            {"name": "pi-0", "ip": "10.10.0.10"},
            {"name": "pi-1", "ip": "10.10.0.11"},
        ])),
    );

    let mut builder = InfraBuilder::new().group(Group {
        id: group_id,
        name: "raspberry_cluster".into(),
        vars: group_vars,
    })?;

    // Two cluster machines, each with a per-machine `node_name`.
    for (uuid, node_name) in [
        ("a0000000-0000-4000-8000-000000000010", "pi-0"),
        ("a0000000-0000-4000-8000-000000000011", "pi-1"),
    ] {
        let mid = MachineId(Uuid::parse_str(uuid)?);
        let mut m: Machine = builder::local(mid, node_name);
        m.groups.push(group_id);
        m.vars.insert(
            VarKey::new("node_name"),
            scalar(serde_json::json!(node_name)),
        );
        builder = builder.machine(m)?;
    }

    let cluster = builder.typed_group::<ClusterVars>("raspberry_cluster")?;

    let bundle = builder
        .on_group(cluster, "write-cluster-config", |_machine, v| {
            // `v: &ClusterVars` — every field below is rustc-checked.
            let cfg = template!(
                "# k3s node {{ v.node_name }}\ntoken: {{ v.k3s_token }}\npeers:\n@for h in &v.hosts {  - {{ h.name }} = {{ h.ip }}\n}",
                v = v
            );
            vec![write_rendered("/etc/k3s/cluster.yaml", 0o640, cfg)]
        })?
        .build();

    Ok(bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("infrazeug-hello-template"),
        vault_store: None,
    }))
}
