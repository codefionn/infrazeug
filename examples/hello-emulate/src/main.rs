//! M3 slice: remote machine with container `like`, multi-stage build, `test` + emulate-first.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, RunBuildContext, RunCommands, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_emulate::like::LikeVars;
use infrazeug_emulate::spec::{
    BuildStep, ContainerBase, ContainerRef, ContainerSpec, EmulatedKind, ImageRef, LikeConfig,
};
use infrazeug_shell::{argv, ShellOp};
use std::sync::Arc;
use uuid::Uuid;

const REMOTE_MACHINE: &str = "c3d4e5f6-a7b8-4901-c234-567890abcdef";
const NGINX_NODE: &str = "d4e5f6a7-b8c9-4012-d345-678901bcdef0";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-emulate")
            .about("M3 container like + test/apply")
            .commands(RunCommands::ALL),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let machine_id = MachineId(Uuid::parse_str(REMOTE_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(NGINX_NODE)?);

    let base = Arc::new(ContainerSpec {
        base: ContainerBase::Image(ImageRef::docker_io("library/alpine", "3.19")),
        steps: vec![BuildStep::Run {
            argv: argv!["apk", "add", "--no-cache", "nginx"],
            env: vec![],
            mounts: vec![],
            network: Default::default(),
            cache_id: None,
        }],
        runtime: Default::default(),
        build: Default::default(),
        outputs: vec![],
    });

    let app = Arc::new(ContainerSpec {
        base: ContainerBase::From(Arc::clone(&base)),
        steps: vec![BuildStep::Cmd(argv!["nginx", "-v"])],
        runtime: Default::default(),
        build: Default::default(),
        outputs: vec![infrazeug_emulate::spec::BuildOutput::LocalStore {
            runtime: Default::default(),
            namespace: "hello-emulate".into(),
        }],
    });

    let like = LikeConfig {
        kind: EmulatedKind::Container(ContainerRef::Spec(app)),
        vars: LikeVars::default(),
    };

    let mut remote = if let Ok(host) = std::env::var("INFRZEUG_SSH_HOST") {
        builder::remote(machine_id, "remote", infrazeug_core::SshConfig::new(host))
    } else {
        builder::local(machine_id, "remote-standin")
    };
    remote.like = Some(like);

    let bundle = InfraBuilder::new()
        .machine(remote)?
        .shell_on_machine(
            node_id,
            "nginx-version",
            machine_id,
            ShellOp::run(argv!["nginx", "-v"]),
        )?
        .build();

    Ok(bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("infrazeug-hello-emulate"),
        vault_store: None,
    }))
}
