//! Two named playbooks in one binary: `main` (nginx) and `machines` (uname).

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{
    build_from_registry, init_tracing, run, PlaybookEntry, PlaybookRegistry, RunConfig, RunContext,
};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const LOCAL_MACHINE: &str = "a1b2c3d4-e5f6-4789-a012-3456789abcde";
const NGINX_NODE: &str = "b2c3d4e5-f6a7-4890-b123-456789abcdef";
const UNAME_NODE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";

static PLAYBOOKS: PlaybookRegistry = PlaybookRegistry {
    default: "main",
    entries: &[
        PlaybookEntry {
            name: "main",
            build: build_main,
        },
        PlaybookEntry {
            name: "machines",
            build: build_machines,
        },
    ],
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-multi-playbook")
            .about("Multiple playbooks: --playbook main|machines")
            .default_playbook("main"),
        |ctx| build_from_registry(&PLAYBOOKS, ctx),
    )
    .await
}

fn build_main(_ctx: &RunContext) -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(NGINX_NODE)?);
    let bundle = InfraBuilder::new()
        .machine(builder::local(machine_id, "localhost"))?
        .shell_on_local(
            node_id,
            "nginx-version",
            machine_id,
            ShellOp::run(argv!["nginx", "-v"]),
        )?
        .build();
    Ok(with_runtime(bundle, "hello-multi-playbook-main"))
}

fn build_machines(_ctx: &RunContext) -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(UNAME_NODE)?);
    let bundle = InfraBuilder::new()
        .machine(builder::local(machine_id, "localhost"))?
        .shell_on_local(
            node_id,
            "uname",
            machine_id,
            ShellOp::run(argv!["uname", "-m"]),
        )?
        .build();
    Ok(with_runtime(bundle, "hello-multi-playbook-machines"))
}

fn with_runtime(
    bundle: infrazeug_api::PlaybookBundle,
    suffix: &str,
) -> infrazeug_api::PlaybookBundle {
    bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join(format!("infrazeug-{suffix}")),
        vault_store: None,
    })
}
