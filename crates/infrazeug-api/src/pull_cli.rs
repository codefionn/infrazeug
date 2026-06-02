//! Canonical pull-mode CLI (`machine`, `plan-op`, `serve-pull`, `bootstrap`).

use crate::Infra;
use clap::{FromArgMatches, Subcommand};
use infrazeug_core::id::MachineId;
use infrazeug_core::slice::SliceMode;
use infrazeug_pull::{
    machine_keygen, open_fs_store, parse_bootstrap, parse_trusted_signers, publish_slice,
    register_machine_pubkey, revoke_machine, run_from_bootstrap, run_oneshot, PlanStore,
    PublishOptions,
};
use infrazeug_secrets::FsBackend;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Top-level pull subcommand names (stable catalog).
pub const PULL_SUBCOMMANDS: &[&str] = &["machine", "plan-op", "serve-pull", "bootstrap"];

/// Bitset of pull commands a binary may expose.
#[derive(Clone, Copy, Debug, Default)]
pub struct PullCommandSet {
    bits: u8,
}

impl PullCommandSet {
    pub const MACHINE: u8 = 1 << 0;
    pub const PLAN_OP: u8 = 1 << 1;
    pub const SERVE_PULL: u8 = 1 << 2;
    pub const BOOTSTRAP: u8 = 1 << 3;

    pub const ALL: Self = Self {
        bits: Self::MACHINE | Self::PLAN_OP | Self::SERVE_PULL | Self::BOOTSTRAP,
    };

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn with(mut self, flag: u8) -> Self {
        self.bits |= flag;
        self
    }

    pub fn contains(self, flag: u8) -> bool {
        self.bits & flag != 0
    }

    pub fn any(self) -> bool {
        self.bits != 0
    }
}

/// How [`PullCommands::Bootstrap`] is executed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BootstrapExec {
    /// `run_from_bootstrap` in-process (stock `infrazeug bootstrap`).
    #[default]
    InProcess,
    /// Exec `infrazeug-agent serve-pull` (first-boot stub binary).
    DelegateAgent,
}

/// Context after parsing a pull subcommand.
#[derive(Clone, Debug)]
pub struct PullContext {
    pub command: PullCommand,
}

#[derive(Clone, Debug)]
pub enum PullCommand {
    Machine(MachineCmd),
    PlanOp(PlanCmd),
    ServePull {
        store: PathBuf,
        machine: Uuid,
        key: PathBuf,
    },
    Bootstrap {
        from: PathBuf,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum PullCommands {
    Machine {
        #[command(subcommand)]
        cmd: MachineCmd,
    },
    #[command(name = "plan-op")]
    PlanOp {
        #[command(subcommand)]
        cmd: PlanCmd,
    },
    #[command(name = "serve-pull")]
    ServePull {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        machine: Uuid,
        #[arg(long)]
        key: PathBuf,
        /// Hex-encoded Ed25519 public key(s) to trust as plan signers. Repeat
        /// the flag to trust several. Required: applies are rejected otherwise.
        #[arg(long = "trust")]
        trust: Vec<String>,
    },
    Bootstrap {
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum MachineCmd {
    Keygen {
        #[arg(long)]
        machine: Uuid,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Register {
        #[arg(long)]
        machine: Uuid,
        #[arg(long)]
        pubkey: PathBuf,
        #[arg(long, default_value = ".infrazeug/plan-store")]
        store: PathBuf,
    },
}

#[derive(Subcommand, Clone, Debug)]
pub enum PlanCmd {
    Publish {
        #[arg(long)]
        for_machine: Uuid,
        #[arg(long, default_value = ".infrazeug/plan-store")]
        store: PathBuf,
        #[arg(long)]
        agent_digest: Option<String>,
        #[arg(long)]
        sign_with: Option<String>,
        #[arg(long, default_value = "cli")]
        signer_id: String,
    },
    Revoke {
        #[arg(long)]
        for_machine: Uuid,
        #[arg(long, default_value = ".infrazeug/plan-store")]
        store: PathBuf,
        #[arg(long)]
        with_teardown: bool,
    },
    Slice {
        #[arg(long)]
        for_machine: Uuid,
        #[arg(long, default_value = "push")]
        mode: String,
    },
}

impl PullCommands {
    pub fn to_context(&self) -> PullContext {
        let command = match self {
            PullCommands::Machine { cmd } => PullCommand::Machine(cmd.clone()),
            PullCommands::PlanOp { cmd } => PullCommand::PlanOp(cmd.clone()),
            PullCommands::ServePull {
                store,
                machine,
                key,
                ..
            } => PullCommand::ServePull {
                store: store.clone(),
                machine: *machine,
                key: key.clone(),
            },
            PullCommands::Bootstrap { from } => PullCommand::Bootstrap { from: from.clone() },
        };
        PullContext { command }
    }

    pub fn needs_playbook_infra(&self) -> bool {
        matches!(
            self,
            PullCommands::PlanOp {
                cmd: PlanCmd::Publish { .. } | PlanCmd::Slice { .. }
            }
        )
    }
}

pub async fn dispatch_pull(
    infra: &Infra,
    cmd: &PullCommands,
    bootstrap_exec: BootstrapExec,
) -> anyhow::Result<()> {
    match cmd {
        PullCommands::Machine { cmd } => match cmd {
            MachineCmd::Keygen { machine, out } => {
                let path = out
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(format!("machine-{machine}.key")));
                let pubkey = machine_keygen(*machine, &path)?;
                println!(
                    "wrote private key {} (pubkey {})",
                    path.display(),
                    hex::encode(pubkey)
                );
            }
            MachineCmd::Register {
                machine,
                pubkey,
                store,
            } => {
                let bytes = std::fs::read(pubkey)?;
                if bytes.len() != 32 {
                    anyhow::bail!("pubkey file must be 32 bytes");
                }
                let mut pk = [0u8; 32];
                pk.copy_from_slice(&bytes);
                let backend = Arc::new(FsBackend::new(store));
                let plan_store = PlanStore::new(backend);
                register_machine_pubkey(&plan_store, *machine, pk).await?;
                println!("registered {machine} in {}", store.display());
            }
        },
        PullCommands::PlanOp { cmd } => match cmd {
            PlanCmd::Publish {
                for_machine,
                store,
                agent_digest,
                sign_with,
                signer_id,
            } => {
                let backend = Arc::new(FsBackend::new(store));
                let plan_store = PlanStore::new(backend);
                let signing_seed = sign_with.as_ref().and_then(|h| {
                    hex::decode(h).ok().and_then(|v| {
                        if v.len() == 32 {
                            let mut s = [0u8; 32];
                            s.copy_from_slice(&v);
                            Some(s)
                        } else {
                            None
                        }
                    })
                });
                let slice = publish_slice(
                    infra,
                    &plan_store,
                    *for_machine,
                    PublishOptions {
                        agent_digest: agent_digest.clone(),
                        signing_seed,
                        signer_id: signer_id.clone(),
                    },
                )
                .await?;
                println!("published slice digest {} for {for_machine}", slice.digest);
            }
            PlanCmd::Revoke {
                for_machine,
                store,
                with_teardown,
            } => {
                let backend = Arc::new(FsBackend::new(store));
                let plan_store = PlanStore::new(backend);
                revoke_machine(&plan_store, *for_machine, *with_teardown).await?;
                println!("revoked {for_machine}");
            }
            PlanCmd::Slice { for_machine, mode } => {
                let plan = infra.plan()?;
                let slice_mode = if mode == "pull" {
                    SliceMode::Pull
                } else {
                    SliceMode::Push
                };
                let slice = plan.slice_for_machine(infra, MachineId(*for_machine), slice_mode)?;
                println!(
                    "slice digest {} ({} steps)",
                    slice.digest,
                    slice.steps.len()
                );
            }
        },
        PullCommands::ServePull {
            store,
            machine,
            key,
            trust,
        } => {
            let plan_store = Arc::new(open_fs_store(
                store
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("store path is not UTF-8"))?,
            ));
            let trusted = parse_trusted_signers(&trust.join(","))?;
            run_oneshot(infra, plan_store, *machine, key.clone(), &trusted).await?;
            println!("serve-pull finished for {machine}");
        }
        PullCommands::Bootstrap { from } => match bootstrap_exec {
            BootstrapExec::InProcess => {
                let bytes = std::fs::read(from)?;
                let bootstrap = parse_bootstrap(&bytes)?;
                run_from_bootstrap(infra, &bootstrap).await?;
                println!("bootstrap apply finished for {}", bootstrap.machine_id);
            }
            BootstrapExec::DelegateAgent => {
                let bytes = tokio::fs::read(from).await?;
                let bootstrap = parse_bootstrap(&bytes)?;
                println!(
                    "bootstrap machine={} plan_url={} agent_digest={}",
                    bootstrap.machine_id, bootstrap.plan_url, bootstrap.agent_digest
                );
                let agent =
                    std::env::var("INFRAZEUG_AGENT").unwrap_or_else(|_| "infrazeug-agent".into());
                let status = std::process::Command::new(&agent)
                    .args([
                        "serve-pull",
                        "--store",
                        bootstrap.plan_url.trim_end_matches('/'),
                        "--machine",
                        &bootstrap.machine_id.to_string(),
                        "--key",
                        bootstrap
                            .machine_key
                            .to_str()
                            .ok_or_else(|| anyhow::anyhow!("machine_key path"))?,
                    ])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("agent serve-pull failed: {status}");
                }
            }
        },
    }
    Ok(())
}

pub(crate) fn attach_pull_subcommands(
    mut root: clap::Command,
    enabled: PullCommandSet,
) -> clap::Command {
    let mut template = clap::Command::new("_pull");
    template = PullCommands::augment_subcommands(template);
    for sub in template.get_subcommands() {
        let name = sub.get_name();
        let on = match name {
            "machine" => enabled.contains(PullCommandSet::MACHINE),
            "plan-op" => enabled.contains(PullCommandSet::PLAN_OP),
            "serve-pull" => enabled.contains(PullCommandSet::SERVE_PULL),
            "bootstrap" => enabled.contains(PullCommandSet::BOOTSTRAP),
            _ => false,
        };
        if on {
            root = root.subcommand(sub.clone());
        }
    }
    root
}

pub(crate) fn parse_pull_subcommand(
    name: &str,
    matches: &clap::ArgMatches,
    enabled: PullCommandSet,
) -> anyhow::Result<PullCommands> {
    let allowed = match name {
        "machine" => enabled.contains(PullCommandSet::MACHINE),
        "plan-op" => enabled.contains(PullCommandSet::PLAN_OP),
        "serve-pull" => enabled.contains(PullCommandSet::SERVE_PULL),
        "bootstrap" => enabled.contains(PullCommandSet::BOOTSTRAP),
        _ => false,
    };
    if !allowed {
        anyhow::bail!("subcommand `{name}` is not enabled for this binary");
    }
    PullCommands::from_arg_matches(matches)
        .map_err(|e| anyhow::anyhow!("failed to parse pull subcommand `{name}`: {e}"))
}
