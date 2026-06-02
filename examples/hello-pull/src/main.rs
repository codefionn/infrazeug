//! M6 demo: publish a pull-mode sealed slice to a local plan store and apply it on-host.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::pull_cli::{PlanCmd, PullCommand, PullCommandSet};
use infrazeug_api::{
    default_infra, init_tracing, run, ExtraSubcommand, RunBuildContext, RunCommands, RunConfig,
};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_pull::{
    machine_keygen, open_fs_store, publish_slice, register_machine_pubkey, PlanStore,
    PublishOptions,
};
use infrazeug_secrets::FsBackend;
use infrazeug_shell::{argv, ShellOp};
use std::sync::Arc;
use uuid::Uuid;

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "demo",
    about: "Keygen, register, publish slice, and serve-pull once (M6 e2e)",
    run: || Box::pin(demo()),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-pull")
            .about("M6 pull-mode plan store demo")
            .commands(RunCommands::empty())
            .pull(PullCommandSet::ALL)
            .extras(&EXTRAS),
        build_infra,
    )
    .await
}

fn build_infra(ctx: RunBuildContext<'_>) -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    match ctx {
        RunBuildContext::Pull(pull) => match &pull.command {
            PullCommand::PlanOp(PlanCmd::Publish { for_machine, .. }) => Ok(
                infrazeug_api::PlaybookBundle::from_infra(build_demo_infra(*for_machine)?),
            ),
            _ => Ok(infrazeug_api::PlaybookBundle::from_infra(default_infra())),
        },
        RunBuildContext::Playbook(_) => {
            Ok(infrazeug_api::PlaybookBundle::from_infra(default_infra()))
        }
    }
}

async fn demo() -> anyhow::Result<()> {
    let store_dir = std::env::temp_dir().join("infrazeug-hello-pull-store");
    std::fs::create_dir_all(&store_dir)?;

    let machine = Uuid::new_v4();
    let key_path = store_dir.join("machine.key");
    let pubkey = machine_keygen(machine, &key_path)?;

    let backend: Arc<dyn infrazeug_secrets::Backend> = Arc::new(FsBackend::new(&store_dir));
    let store = PlanStore::new(Arc::clone(&backend));
    register_machine_pubkey(&store, machine, pubkey).await?;

    // Demo signing seed; in production the signing key is held by the controller
    // and only its public key is distributed to hosts as the trusted signer.
    let signing_seed = [0x11u8; 32];
    let trusted = [infrazeug_secrets::verifying_key_from_seed(&signing_seed)];

    let infra = build_demo_infra(machine)?;
    publish_slice(
        &infra,
        &store,
        machine,
        PublishOptions {
            agent_digest: Some("sha256:demo".into()),
            signing_seed: Some(signing_seed),
            signer_id: "hello-pull".into(),
        },
    )
    .await?;

    let empty = default_infra();
    let plan_store = Arc::new(open_fs_store(store_dir.to_str().unwrap()));
    infrazeug_pull::run_oneshot(&empty, plan_store, machine, key_path, &trusted).await?;

    println!("hello-pull demo ok for machine {machine}");
    Ok(())
}

fn build_demo_infra(machine: Uuid) -> anyhow::Result<infrazeug_core::Infra> {
    let mid = MachineId(machine);
    let node_id = NodeId(Uuid::new_v4());

    Ok(InfraBuilder::new()
        .machine(builder::local(mid, "pull-host"))?
        .shell_on_local(
            node_id,
            "hello-pull",
            mid,
            ShellOp::run(argv!["sh", "-c", "echo hello-pull"]),
        )?
        .build()
        .infra)
}
