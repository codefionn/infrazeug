//! M5 slice: remote machine with QEMU `like`, `infrazeug test` when image + pubkey are set.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, ExtraSubcommand, RunBuildContext, RunCommands, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_emulate::spec::{EmulatedKind, LikeConfig, QemuConfig, VmImage};
use infrazeug_emulate_qemu::qemu_available;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const REMOTE_MACHINE: &str = "e5f6a7b8-c9d0-4123-e456-7890abcdef01";
const CHECK_NODE: &str = "f6a7b8c9-d0e1-4234-f567-890abcdef012";

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "probe",
    about: "Print whether qemu-system-* is on PATH",
    run: || Box::pin(async { probe_qemu().await }),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-qemu")
            .about("M5 QEMU like + test")
            .commands(RunCommands::empty().with(RunCommands::TEST))
            .extras(&EXTRAS),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => {
                if !qemu_available() {
                    anyhow::bail!("qemu-system-* not in PATH");
                }
                if std::env::var("INFRZEUG_QEMU_IMAGE").is_err() {
                    anyhow::bail!("set INFRZEUG_QEMU_IMAGE to a cloud-friendly qcow2 path");
                }
                if std::env::var("INFRZEUG_QEMU_SSH_PUBKEY").is_err() {
                    anyhow::bail!("set INFRZEUG_QEMU_SSH_PUBKEY for cloud-init SSH");
                }
                build_infra()
            }
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

async fn probe_qemu() -> anyhow::Result<()> {
    println!(
        "qemu: {} (set INFRZEUG_QEMU_IMAGE + INFRZEUG_QEMU_SSH_PUBKEY to run test)",
        qemu_available()
    );
    Ok(())
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let image = std::env::var("INFRZEUG_QEMU_IMAGE")?;
    let machine_id = MachineId(Uuid::parse_str(REMOTE_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(CHECK_NODE)?);

    let like = LikeConfig {
        kind: EmulatedKind::MicroVm {
            image: VmImage::RemoteQcow2(image),
            qemu: QemuConfig {
                memory_mb: std::env::var("INFRZEUG_QEMU_MEM_MB")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1024),
            },
        },
        vars: Default::default(),
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
            "uname",
            machine_id,
            ShellOp::run(argv!["uname", "-a"]),
        )?
        .build();

    Ok(bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("infrazeug-hello-qemu"),
        vault_store: None,
    }))
}
