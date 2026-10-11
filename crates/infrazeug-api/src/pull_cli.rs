//! Canonical pull-mode CLI (`machine`, `plan-op`, `serve-pull`, `bootstrap`).

use crate::Infra;
use clap::{FromArgMatches, Subcommand};
use infrazeug_core::id::MachineId;
use infrazeug_core::slice::SliceMode;
use infrazeug_pull::{
    machine_keygen, open_fs_store, parse_bootstrap, parse_trusted_signers, publish_slice,
    register_machine_pubkey, revoke_machine, run_from_bootstrap, run_oneshot, Bootstrap, PlanStore,
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
        /// Hex-encoded Ed25519 signing seed. Strongly recommended: unsigned
        /// tombstones are rejected by daemons with a trusted-signer set.
        #[arg(long)]
        sign_with: Option<String>,
        #[arg(long, default_value = "cli")]
        signer_id: String,
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
                let key = signing_seed.map(|s| infrazeug_secrets::signing_key_from_seed(&s));
                revoke_machine(
                    &plan_store,
                    *for_machine,
                    *with_teardown,
                    key.as_ref(),
                    signer_id,
                )
                .await?;
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
                // Forward the bootstrapped plan-signer trust set: without it the
                // delegated agent has no signers to trust and would either fail
                // closed (today) or, worse, a future "fix" might make it apply
                // untrusted plans. Empty trust is refused here as well.
                let signers: Vec<&str> = bootstrap
                    .plan_signer
                    .split([',', ' ', '\t', '\n', '\r'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                if signers.is_empty() {
                    anyhow::bail!(
                        "bootstrap plan_signer is empty; refusing to delegate without trusted plan signers"
                    );
                }
                let agent_path = resolve_verified_agent(&bootstrap).await?;
                let mut cmd = std::process::Command::new(&agent_path);
                cmd.args([
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
                ]);
                for signer in &signers {
                    cmd.args(["--trust", signer]);
                }
                let status = cmd.status()?;
                if !status.success() {
                    anyhow::bail!("agent serve-pull failed: {status}");
                }
            }
        },
    }
    Ok(())
}

/// Resolve the agent binary to delegate to, verifying it before execution
/// (SOUL M6: custom-agent distribution with detached signature verification).
///
/// The agent artifact is read from the plan store (`agents/{digest}/{triple}/`)
/// together with its detached signature (`agents/{digest}.sig`), checked against
/// the bootstrap's `agent_digest` and `agent_signer`, then written to a private
/// temp file for execution. Running an unverified binary from `INFRAZEUG_AGENT`
/// requires the explicit `INFRAZEUG_ALLOW_UNVERIFIED_AGENT=1` opt-in.
async fn resolve_verified_agent(bootstrap: &Bootstrap) -> anyhow::Result<PathBuf> {
    use infrazeug_pull::verify_agent_artifact;

    if let Ok(path) = std::env::var("INFRAZEUG_AGENT") {
        let opt_in = std::env::var("INFRAZEUG_ALLOW_UNVERIFIED_AGENT").as_deref() == Ok("1");
        if !opt_in {
            anyhow::bail!(
                "INFRAZEUG_AGENT is set but the bootstrapped agent must be digest- and \
                 signature-verified; set INFRAZEUG_ALLOW_UNVERIFIED_AGENT=1 to run it unverified"
            );
        }
        eprintln!("[bootstrap] WARNING: running UNVERIFIED agent from INFRAZEUG_AGENT={path}");
        return Ok(PathBuf::from(path));
    }

    let triple = infrazeug_build::host_triple().unwrap_or_else(|| "host".to_string());
    let store = open_fs_store(bootstrap.plan_url.trim_end_matches('/'));
    let digest = bootstrap.agent_digest.trim().trim_start_matches("sha256:");
    let agent_bytes = store
        .get_agent(digest, &triple)
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent artifact agents/{digest}/{triple} not in store"))?;
    let sig = store.get_agent_sig(digest).await?.ok_or_else(|| {
        anyhow::anyhow!("agent signature agents/{digest}.sig not in store; refusing to run")
    })?;
    verify_agent_artifact(&agent_bytes, &bootstrap.agent_digest, &sig, &bootstrap.agent_signer)
        .map_err(|e| anyhow::anyhow!("agent verification failed: {e}"))?;

    // Exec the verified bytes from a private temp file (0700 dir, 0755 file).
    let dir = std::env::temp_dir().join(format!("infrazeug-bootstrap-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = dir.join("infrazeug-agent");
    std::fs::write(&path, &agent_bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(path)
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
