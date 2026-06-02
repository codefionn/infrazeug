//! Native tier-1 node on localhost: `native.echo` then a shell node gated by change.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, EchoInput, EchoMethod, RunBuildContext, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const LOCAL_MACHINE: &str = "a1b2c3d4-e5f6-4789-a012-3456789abcde";
const ECHO_NODE: &str = "b2c3d4e5-f6a7-4890-b123-456789abcdef";
const FOLLOW_NODE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-native").about("Local native.echo + downstream shell"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let echo_id = NodeId(Uuid::parse_str(ECHO_NODE)?);
    let follow_id = NodeId(Uuid::parse_str(FOLLOW_NODE)?);

    Ok(InfraBuilder::new()
        .machine(builder::controller(machine_id))?
        .native(
            echo_id,
            machine_id,
            EchoMethod,
            EchoInput {
                text: "hello-native".into(),
            },
        )?
        .on_upstream_change()
        .build()?
        .shell_node(
            follow_id,
            machine_id,
            ShellOp::run(argv!["echo", "follow-up"]),
        )
        .name("follow-shell")
        .deps([echo_id])
        .on_upstream_change()
        .build()?
        .build()
        .with_runtime(RuntimeConfig {
            run_root: std::env::temp_dir().join("infrazeug-hello-native"),
            vault_store: None,
        }))
}
