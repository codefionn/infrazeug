//! M1 vertical slice: `nginx -v` on localhost via apply --tui / --watch.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, RunBuildContext, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const LOCAL_MACHINE: &str = "a1b2c3d4-e5f6-4789-a012-3456789abcde";
const NGINX_NODE: &str = "b2c3d4e5-f6a7-4890-b123-456789abcdef";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-local").about("M1 local nginx -v demo"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(NGINX_NODE)?);

    Ok(InfraBuilder::new()
        .machine(builder::local(machine_id, "localhost"))?
        .shell_on_local(
            node_id,
            "nginx-version",
            machine_id,
            ShellOp::run(argv!["nginx", "-v"]),
        )?
        .build()
        .with_runtime(RuntimeConfig {
            run_root: std::env::temp_dir().join("infrazeug-hello-local"),
            vault_store: None,
        }))
}
