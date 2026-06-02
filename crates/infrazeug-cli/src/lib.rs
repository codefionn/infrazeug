//! Stock `infrazeug` CLI binary library (SOUL §7).
//!
//! When a discovered playbook project is present, argv is forwarded to its native
//! binary (build + probe + agent prep via [`infrazeug_playbook`]) for:
//!
//! - Playbook subcommands (`plan`, `apply`, …).
//! - Any other top-level subcommand not defined on this binary (e.g. `RunConfig::extras`).
//!
//! Operational subcommands (`vault`, `gc`, `mcp`, …) always stay on the stock CLI.
//!
//! - **Vault** — `vault keygen`, `vault status`, `vault edit`, `vault show-keys`, unlock helpers.
//! - **Migrate** — Ansible Vault import via [`infrazeug_migrate`].
//! - **Pull** — thin wrapper around [`infrazeug_api::pull_cli`].
//! - **MCP** — `mcp serve` (stdio/HTTP) with optional watch mode.
//!
//! Playbook parsing and apply/test scheduling remain in [`infrazeug_api::cli`]
//! so user binaries and examples share one surface.

mod init;
mod passphrase;
mod unlock;
mod vault_edit;
mod vault_show_keys;
mod vault_status;

use unlock::{unlock_data_key, UnlockArgs};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use infrazeug_api::cli::{dispatch, init_tracing, PlaybookCommands};
use infrazeug_api::default_infra;
use infrazeug_api::pull_cli::{dispatch_pull, BootstrapExec, PullCommands};
pub use infrazeug_api::pull_cli::{MachineCmd, PlanCmd};
use infrazeug_api::McpServeMode;
use infrazeug_core::id::MachineId;
use infrazeug_core::resolve_machine;
use infrazeug_core::{Plan, RuntimeConfig};
use infrazeug_migrate::{
    migrate_ansible_vault, AnsibleVaultMigrateOptions, MigrateError, MigrateReport,
};
use infrazeug_playbook::{
    discover_playbook, is_playbook_subcommand, run_mcp_watch, run_playbook_command,
};
use infrazeug_secrets::{Backend, FsBackend, PassphraseProvider, Provider, VaultStore};
use infrazeug_secrets_dav::WebDavBackend;
use infrazeug_secrets_hw::{Fido2Provider, Pkcs11Provider};
use infrazeug_secrets_kms::{AgeProvider, EnvKmsProvider, KmsConfig};
use infrazeug_secrets_s3::{S3Config, S3HttpBackend};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "infrazeug", about = "infrazeug IaC CLI")]
pub struct Cli {
    #[arg(
        long = "playbook",
        global = true,
        help = "Named playbook in the project binary (default: default)"
    )]
    pub playbook: Option<String>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(flatten)]
    Playbook(PlaybookCommands),
    Vars {
        #[command(subcommand)]
        cmd: VarsCmd,
    },
    Gc {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        older_than_hours: Option<u64>,
    },
    Agent {
        #[command(subcommand)]
        cmd: AgentCmd,
    },
    ApplySigned {
        plan: PathBuf,
        #[arg(long = "trust")]
        trust: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    Attach {
        run_id: Option<Uuid>,
    },
    Vault {
        #[command(subcommand)]
        cmd: Box<VaultCmd>,
    },
    Migrate {
        #[command(subcommand)]
        cmd: MigrateCmd,
    },
    Machine {
        #[command(subcommand)]
        cmd: MachineCmd,
    },
    #[command(name = "plan-op")]
    PlanOp {
        #[command(subcommand)]
        cmd: PlanCmd,
    },
    Bootstrap {
        #[arg(long)]
        from: PathBuf,
    },
    /// Model Context Protocol server (forwards to the project playbook when discovered).
    Mcp {
        #[command(subcommand)]
        cmd: McpCmd,
    },
    /// Scaffold a new infrazeug playbook project.
    Init {
        /// Directory name for the new project.
        name: String,
        /// LLM code agent(s) to configure MCP for (can be repeated).
        /// Omit to choose interactively.
        #[arg(long = "agent", value_name = "AGENT")]
        agents: Vec<CodeAgent>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum CodeAgent {
    /// Claude Code (.mcp.json)
    Claude,
    /// OpenCode (opencode.json)
    OpenCode,
    /// Cursor (.cursor/mcp.json)
    Cursor,
    /// VS Code / GitHub Copilot (.vscode/mcp.json)
    Vscode,
    /// Cline (.cline/mcp.json)
    Cline,
    /// Continue.dev (.continue/mcpServers/mcp.json)
    #[value(name = "continue")]
    ContinueDev,
    /// Roo Code (.roo/mcp.json)
    Roo,
    /// Zed (.zed/settings.json)
    Zed,
}

#[derive(Subcommand)]
pub enum McpCmd {
    /// Serve MCP tools; watches sources and rebuilds/restarts on change (stock CLI).
    Serve {
        /// JSON-RPC over stdio (default when `--http` is omitted).
        #[arg(long = "stdio", action = clap::ArgAction::SetTrue)]
        stdio: bool,
        /// Streamable HTTP JSON-RPC server (e.g. `127.0.0.1:7777`).
        #[arg(long = "http", value_name = "ADDR")]
        http: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum AgentCmd {
    Build {
        #[arg(long)]
        target: Vec<String>,
        #[arg(long)]
        release: bool,
    },
    ServePull {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        machine: Uuid,
        #[arg(long)]
        key: PathBuf,
        /// Hex-encoded Ed25519 public key(s) to trust as plan signers. Repeat to
        /// trust several. Required: applies are rejected without a trusted signer.
        #[arg(long = "trust")]
        trust: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum MigrateCmd {
    /// Import Ansible Vault YAML files into an infrazeug vault store.
    AnsibleVault {
        /// Ansible vault file or directory tree to import.
        #[arg(long)]
        from: PathBuf,
        /// Infrazeug vault store directory.
        #[arg(long)]
        store: PathBuf,
        /// Unlocked infrazeug data key id (see `infrazeug vault keygen`).
        #[arg(long)]
        data_key: String,
        /// Ansible vault password (otherwise password file, env, stdin, or prompt).
        #[arg(long)]
        ansible_passphrase: Option<String>,
        /// File whose first line is the Ansible vault password.
        #[arg(long)]
        ansible_password_file: Option<PathBuf>,
        /// Unlock infrazeug data key before import (otherwise stdin or prompt if locked).
        #[arg(long)]
        passphrase: Option<String>,
        /// Prefix for migrated vault file paths (`files/<prefix>…`).
        #[arg(long, default_value = "ansible/")]
        out_prefix: String,
        /// List what would be written without updating the store.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Recipient type for `vault recipients-add`.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum NewRecipientKind {
    Passphrase,
    Fido2,
    Pkcs11,
    Age,
    Kms,
}

#[derive(Subcommand)]
pub enum VaultCmd {
    Keygen {
        #[arg(long)]
        store: PathBuf,
        /// Data key id to create (e.g. `prod`).
        #[arg(long)]
        data_key: String,
        /// Passphrase for the recovery recipient (otherwise stdin or prompt).
        #[arg(long)]
        passphrase: Option<String>,
    },
    /// Add a recipient (passphrase or hardware/age/kms key) that can unlock a DataKey.
    RecipientsAdd {
        #[arg(long)]
        data_key: String,
        #[arg(long)]
        store: PathBuf,
        /// Label for the new recipient.
        #[arg(long)]
        label: String,
        // --- new recipient ---
        /// Type of recipient to add.
        #[arg(long, value_enum, default_value_t = NewRecipientKind::Passphrase)]
        new_kind: NewRecipientKind,
        /// Passphrase for a `passphrase` recipient (otherwise stdin or prompt).
        #[arg(long)]
        new_passphrase: Option<String>,
        /// Enroll against a real FIDO2 authenticator (device generates the credential id).
        #[arg(long)]
        new_fido2_device: bool,
        /// Relying-party id for `--new-fido2-device`.
        #[arg(long, default_value = unlock::DEFAULT_FIDO2_RP_ID)]
        new_fido2_rp_id: String,
        /// Credential id for the fixture FIDO2 provider (ignored with --new-fido2-device).
        #[arg(long)]
        new_fido2_credential: Option<String>,
        #[arg(long)]
        new_fido2_pin: Option<String>,
        #[arg(long)]
        new_pkcs11_slot: Option<String>,
        #[arg(long)]
        new_pkcs11_pin: Option<String>,
        #[arg(long)]
        new_age_identity: Option<String>,
        #[arg(long)]
        new_kms_key_id: Option<String>,
        /// Make the new recipient the default decryption method (move to top).
        #[arg(long)]
        set_default: bool,
        // --- how to unlock the DataKey to authorize the add ---
        #[command(flatten)]
        unlock: UnlockArgs,
    },
    /// Reorder recipients so the one with `--label` becomes the default decryption method.
    SetDefault {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        data_key: String,
        /// Label of the recipient to move to the top.
        #[arg(long)]
        label: String,
        #[command(flatten)]
        unlock: UnlockArgs,
    },
    /// List a DataKey's recipients in decryption order (first = default).
    Recipients {
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        data_key: String,
    },
    /// Summarize store layout, DataKeys, recipients, and vault files (no unlock).
    Status {
        #[arg(long)]
        store: PathBuf,
        /// Only show this DataKey.
        #[arg(long)]
        data_key: Option<String>,
    },
    DavPut {
        #[arg(long)]
        url: String,
        #[arg(long)]
        user: Option<String>,
        #[arg(long)]
        password: Option<String>,
        key: String,
        file: PathBuf,
    },
    /// List field names inside each vault file (decrypts; unlocks DataKeys as needed).
    ShowKeys {
        #[arg(long)]
        store: PathBuf,
        /// Only list files sealed with this DataKey (otherwise all files).
        #[arg(long)]
        data_key: Option<String>,
        #[command(flatten)]
        unlock: UnlockArgs,
    },
    /// Decrypt a vault file to YAML, open `$EDITOR` / `$VISUAL`, and save changes.
    Edit {
        /// Vault file id (`db/postgres.vault`) or path under `--store` (e.g. `./vault/files/db/postgres.vault`).
        file: PathBuf,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        data_key: String,
        #[command(flatten)]
        unlock: UnlockArgs,
    },
    /// Smoke-test the production HTTP S3 backend: upload then read back a file.
    S3Put {
        #[arg(long)]
        endpoint: String,
        #[arg(long, default_value = "us-east-1")]
        region: String,
        #[arg(long)]
        bucket: String,
        #[arg(long)]
        access_key: String,
        #[arg(long)]
        secret_key: String,
        /// Use virtual-hosted addressing (`bucket.endpoint`) instead of path-style.
        #[arg(long)]
        virtual_hosted: bool,
        key: String,
        file: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum VarsCmd {
    Resolve { machine: Uuid, key: Option<String> },
}

/// Operational subcommands (everything except playbook `plan`/`apply`/`test`/`lint`).
pub const OPERATIONAL_SUBCOMMANDS: &[&str] = &[
    "vars",
    "gc",
    "agent",
    "apply-signed",
    "attach",
    "vault",
    "migrate",
    "mcp",
    "init",
];

/// All subcommands on the stock binary (playbook + operational + pull).
pub fn all_subcommands() -> Vec<&'static str> {
    let mut v: Vec<_> = infrazeug_api::PLAYBOOK_SUBCOMMANDS.to_vec();
    v.extend_from_slice(infrazeug_api::PULL_SUBCOMMANDS);
    v.extend_from_slice(OPERATIONAL_SUBCOMMANDS);
    v
}

/// Whether `name` is handled by the stock `infrazeug` binary (not forwarded to a playbook).
pub fn is_stock_subcommand(name: &str) -> bool {
    all_subcommands().contains(&name)
}

/// First top-level subcommand in `args` (after the program name), skipping global flags.
pub fn first_subcommand_from_argv(args: &[OsString]) -> Option<String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();
        if s == "--playbook" {
            iter.next();
            continue;
        }
        if s.starts_with("--playbook=") {
            continue;
        }
        if s == "--debug" {
            continue;
        }
        if s.starts_with('-') {
            continue;
        }
        return Some(s.into_owned());
    }
    None
}

fn should_forward_to_playbook(sub: &str) -> bool {
    is_playbook_subcommand(sub) || !is_stock_subcommand(sub)
}

/// When cwd contains a playbook, build it and exec with `args[1..]` (playbook-only argv).
async fn try_forward_discovered_playbook(args: &[OsString]) -> anyhow::Result<bool> {
    let Some(sub) = first_subcommand_from_argv(args) else {
        return Ok(false);
    };
    if !should_forward_to_playbook(&sub) {
        return Ok(false);
    }
    let Some(project) = discover_playbook(std::env::current_dir()?)? else {
        return Ok(false);
    };
    run_playbook_command(&project, args.iter().skip(1).cloned()).await?;
    Ok(true)
}

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Playbook(pb) => {
            let bundle = infrazeug_api::PlaybookBundle::from_infra(default_infra());
            dispatch(&bundle, &pb).await?;
        }
        Commands::Mcp {
            cmd: McpCmd::Serve { http, stdio },
        } => {
            McpServeMode::from_cli(http.as_deref(), stdio)?;
            let project = discover_playbook(std::env::current_dir()?)?.context(
                "no infrazeug playbook in this directory (expected Cargo.toml with infrazeug-api dependency)",
            )?;
            let playbook_argv: Vec<_> = std::env::args_os().skip(1).collect();
            run_mcp_watch(&project, playbook_argv).await?;
        }
        Commands::Vars {
            cmd: VarsCmd::Resolve { machine, key },
        } => {
            let infra = default_infra();
            let mid = MachineId(machine);
            let m = infra.machine_by_id(mid).context("unknown machine")?;
            let resolved = resolve_machine(&infra.global_vars, &infra.groups, m, None);
            for (k, v) in resolved {
                if key.as_ref().is_some_and(|want| want != &k.0) {
                    continue;
                }
                println!("{} = {}  [{:?} / {}]", k.0, v.value, v.source, v.origin);
            }
        }
        Commands::Gc {
            dry_run,
            older_than_hours,
        } => {
            gc_runs(dry_run, older_than_hours.unwrap_or(24))?;
        }
        Commands::ApplySigned { plan, trust, force } => {
            let infra = default_infra();
            let file = Plan::read_file(&plan)?;
            let trusted: Vec<[u8; 32]> = trust
                .iter()
                .filter_map(|h| hex::decode(h).ok())
                .filter_map(|v| {
                    if v.len() == 32 {
                        let mut k = [0u8; 32];
                        k.copy_from_slice(&v);
                        Some(k)
                    } else {
                        None
                    }
                })
                .collect();
            file.verify_signatures(&trusted)?;
            let _ = infra.resolve_plan(Some(&file), force)?;
            println!("plan {} signatures ok", file.digest);
        }
        Commands::Attach { run_id } => {
            let root = RuntimeConfig::default().run_root;
            let run = run_id
                .map(|u| u.to_string())
                .or_else(|| latest_run_dir(&root).ok().flatten())
                .context("no run id and no runs in run_root")?;
            let sock = root.join(&run).join("control.sock");
            println!(
                "attach: UDS {} (use `infrazeug apply --tui` in-process for full prompts; attach client reads events)",
                sock.display()
            );
            #[cfg(unix)]
            {
                use infrazeug_core::control::{read_msg, ControlMsg};
                use tokio::net::UnixStream;
                let mut stream = UnixStream::connect(&sock).await?;
                loop {
                    match read_msg(&mut stream).await {
                        Ok(ControlMsg::Event(ev)) => println!("{ev:?}"),
                        Ok(ControlMsg::Prompt(p)) => {
                            println!("prompt: {p:?}");
                            break;
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
        }
        Commands::Migrate {
            cmd:
                MigrateCmd::AnsibleVault {
                    from,
                    store,
                    data_key,
                    ansible_passphrase,
                    ansible_password_file,
                    passphrase,
                    out_prefix,
                    dry_run,
                },
        } => {
            let ansible_pass = passphrase::resolve_passphrase(
                ansible_passphrase,
                ansible_password_file.as_deref(),
                Some("ANSIBLE_VAULT_PASSWORD_FILE"),
                "Ansible Vault password: ",
            )?;
            let backend = Arc::new(FsBackend::new(&store));
            let mut vault = VaultStore::new(backend, store.clone());
            if !vault.is_unlocked(&data_key) {
                let pp = passphrase::resolve_passphrase(
                    passphrase,
                    None,
                    None,
                    "Infrazeug data key passphrase: ",
                )?;
                vault.unlock_passphrase(&data_key, &pp, "recovery").await?;
            }
            let report = migrate_ansible_vault(
                &mut vault,
                &AnsibleVaultMigrateOptions {
                    ansible_passphrase: ansible_pass,
                    data_key: data_key.clone(),
                    out_prefix,
                    dry_run,
                },
                &from,
            )
            .await
            .map_err(|e: MigrateError| anyhow::anyhow!("{e}"))?;
            print_migrate_report(&report, dry_run);
        }
        Commands::Vault { cmd } => match *cmd {
            VaultCmd::Keygen {
                data_key,
                store,
                passphrase,
            } => {
                let passphrase = passphrase::resolve_new_passphrase(
                    passphrase,
                    None,
                    "New data key passphrase: ",
                    "Confirm data key passphrase: ",
                )?;
                let backend = Arc::new(FsBackend::new(&store));
                let mut vault = VaultStore::new(backend, store);
                vault
                    .keygen_passphrase(&data_key, &passphrase, "recovery")
                    .await?;
                println!(
                    "created data key {data_key} in {}",
                    vault.store_root.display()
                );
            }
            VaultCmd::RecipientsAdd {
                data_key,
                store,
                label,
                new_kind,
                new_passphrase,
                new_fido2_device,
                new_fido2_rp_id,
                new_fido2_credential,
                new_fido2_pin,
                new_pkcs11_slot,
                new_pkcs11_pin,
                new_age_identity,
                new_kms_key_id,
                set_default,
                unlock,
            } => {
                let backend = Arc::new(FsBackend::new(&store));
                let mut vault = VaultStore::new(backend, store.clone());
                unlock_data_key(
                    &mut vault,
                    &data_key,
                    &unlock.to_opts(),
                    "Data key passphrase: ",
                )
                .await?;
                let provider: Box<dyn Provider> = match new_kind {
                    NewRecipientKind::Passphrase => {
                        let pp = passphrase::resolve_new_passphrase(
                            new_passphrase,
                            None,
                            "New passphrase recipient: ",
                            "Confirm new passphrase: ",
                        )?;
                        Box::new(PassphraseProvider::new(&pp))
                    }
                    NewRecipientKind::Fido2 if new_fido2_device => {
                        #[cfg(feature = "fido2-device")]
                        {
                            let pin = passphrase::resolve_optional_secret(
                                new_fido2_pin,
                                "FIDO2 PIN (blank for built-in UV): ",
                            )?;
                            let mut cfg =
                                infrazeug_secrets_hw::Fido2DeviceConfig::new(new_fido2_rp_id);
                            if let Some(pin) = pin {
                                cfg = cfg.with_pin(pin);
                            }
                            println!("touch your authenticator to enroll…");
                            Box::new(infrazeug_secrets_hw::Fido2Device::new(cfg))
                        }
                        #[cfg(not(feature = "fido2-device"))]
                        {
                            let _ = new_fido2_rp_id;
                            anyhow::bail!(
                            "--new-fido2-device requires building the CLI with --features fido2-device"
                        );
                        }
                    }
                    NewRecipientKind::Fido2 => {
                        let cred = new_fido2_credential
                        .context("--new-kind fido2 requires --new-fido2-credential (or --new-fido2-device for real hardware)")?;
                        let pin = passphrase::resolve_passphrase(
                            new_fido2_pin,
                            None,
                            None,
                            "FIDO2 PIN: ",
                        )?;
                        Box::new(Fido2Provider::new(cred, pin))
                    }
                    NewRecipientKind::Pkcs11 => {
                        let slot = new_pkcs11_slot
                            .context("--new-kind pkcs11 requires --new-pkcs11-slot")?;
                        let pin = passphrase::resolve_passphrase(
                            new_pkcs11_pin,
                            None,
                            None,
                            "PKCS#11 PIN: ",
                        )?;
                        Box::new(Pkcs11Provider::new(slot, pin))
                    }
                    NewRecipientKind::Age => {
                        let id = new_age_identity
                            .context("--new-kind age requires --new-age-identity")?;
                        Box::new(AgeProvider::from_identity_str(&id)?)
                    }
                    NewRecipientKind::Kms => {
                        let key_id =
                            new_kms_key_id.context("--new-kind kms requires --new-kms-key-id")?;
                        Box::new(EnvKmsProvider::from_env(KmsConfig { key_id })?)
                    }
                };
                vault
                    .add_recipient(&data_key, provider.as_ref(), &label)
                    .await?;
                if set_default {
                    vault.set_default_recipient(&data_key, &label).await?;
                }
                println!(
                    "added recipient {label} to {data_key}{}",
                    if set_default { " (now default)" } else { "" }
                );
            }
            VaultCmd::SetDefault {
                store,
                data_key,
                label,
                unlock,
            } => {
                let backend = Arc::new(FsBackend::new(&store));
                let mut vault = VaultStore::new(backend, store);
                unlock_data_key(
                    &mut vault,
                    &data_key,
                    &unlock.to_opts(),
                    "Data key passphrase: ",
                )
                .await?;
                vault.set_default_recipient(&data_key, &label).await?;
                println!("default decryption method for {data_key} is now {label}");
                print_recipients(&vault, &data_key).await?;
            }
            VaultCmd::Recipients { store, data_key } => {
                let backend = Arc::new(FsBackend::new(&store));
                let vault = VaultStore::new(backend, store);
                print_recipients(&vault, &data_key).await?;
            }
            VaultCmd::Status { store, data_key } => {
                vault_status::vault_status(store, data_key).await?;
            }
            VaultCmd::ShowKeys {
                store,
                data_key,
                unlock,
            } => {
                vault_show_keys::vault_show_keys(store, data_key, &unlock.to_opts()).await?;
            }
            VaultCmd::Edit {
                file,
                store,
                data_key,
                unlock,
            } => {
                vault_edit::vault_edit_file(store, &data_key, &file, &unlock.to_opts()).await?;
            }
            VaultCmd::DavPut {
                url,
                user,
                password,
                key,
                file,
            } => {
                let dav = WebDavBackend::new(&url, user.as_deref(), password.as_deref())?;
                let data = tokio::fs::read(&file).await?;
                dav.put(&key, bytes::Bytes::from(data), None).await?;
                println!("uploaded {} to webdav {url}", key);
            }
            VaultCmd::S3Put {
                endpoint,
                region,
                bucket,
                access_key,
                secret_key,
                virtual_hosted,
                key,
                file,
            } => {
                let mut cfg = S3Config::new(&endpoint, &region, &bucket, &access_key, &secret_key);
                cfg.path_style = !virtual_hosted;
                let s3 = S3HttpBackend::new(cfg)?;
                let data = tokio::fs::read(&file).await?;
                s3.put(&key, bytes::Bytes::from(data), None).await?;
                let back = s3.get(&key).await?.context("object missing after put")?;
                println!(
                    "uploaded {key} to s3 {endpoint}/{bucket} ({} bytes read back)",
                    back.0.len()
                );
            }
        },
        Commands::Bootstrap { from } => {
            let infra = default_infra();
            dispatch_pull(
                &infra,
                &PullCommands::Bootstrap { from },
                BootstrapExec::InProcess,
            )
            .await?;
        }
        Commands::Machine { cmd } => {
            let infra = default_infra();
            dispatch_pull(
                &infra,
                &PullCommands::Machine { cmd },
                BootstrapExec::InProcess,
            )
            .await?;
        }
        Commands::PlanOp { cmd } => {
            let infra = default_infra();
            dispatch_pull(
                &infra,
                &PullCommands::PlanOp { cmd },
                BootstrapExec::InProcess,
            )
            .await?;
        }
        Commands::Agent {
            cmd:
                AgentCmd::ServePull {
                    store,
                    machine,
                    key,
                    trust,
                },
        } => {
            let infra = default_infra();
            dispatch_pull(
                &infra,
                &PullCommands::ServePull {
                    store,
                    machine,
                    key,
                    trust,
                },
                BootstrapExec::InProcess,
            )
            .await?;
        }
        Commands::Agent {
            cmd: AgentCmd::Build { target, release },
        } => {
            let root = std::env::current_dir()?;
            let mut opts = infrazeug_build::AgentBuildOptions {
                release,
                ..Default::default()
            };
            for t in target {
                opts = opts.with_target(t);
            }
            let paths = infrazeug_build::build_agent(&root, &opts).map_err(anyhow::Error::msg)?;
            for p in paths {
                println!("{}", p.display());
            }
        }
        Commands::Init { name, agents } => {
            init::init_project(&name, &agents)?;
        }
    }
    Ok(())
}

async fn print_recipients(vault: &VaultStore, data_key: &str) -> anyhow::Result<()> {
    let recipients = vault.list_recipients(data_key).await?;
    println!("recipients for {data_key} (first = default decryption method):");
    for (i, (kind, label)) in recipients.iter().enumerate() {
        let marker = if i == 0 { "* " } else { "  " };
        println!("{marker}{label} [{kind:?}]");
    }
    Ok(())
}

fn print_migrate_report(report: &MigrateReport, dry_run: bool) {
    let label = if dry_run { "would migrate" } else { "migrated" };
    for entry in &report.migrated {
        println!(
            "{label} {} -> files/{} ({} top-level fields)",
            entry.source.display(),
            entry.vault_file,
            entry.field_count
        );
    }
    for (path, reason) in &report.skipped {
        eprintln!("skipped {}: {reason}", path.display());
    }
    if report.migrated.is_empty() && report.skipped.is_empty() {
        println!("no files processed");
    }
}

fn latest_run_dir(root: &std::path::Path) -> anyhow::Result<Option<String>> {
    let mut dirs: Vec<_> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    Ok(dirs.pop())
}

fn gc_runs(dry_run: bool, older_than_hours: u64) -> anyhow::Result<()> {
    let root = infrazeug_core::RuntimeConfig::default().run_root;
    if !root.exists() {
        println!("no run root at {}", root.display());
        return Ok(());
    }
    let cutoff =
        std::time::SystemTime::now() - std::time::Duration::from_secs(older_than_hours * 3600);
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.modified()? < cutoff {
            if dry_run {
                println!("would remove {}", entry.path().display());
            } else {
                if meta.is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                } else {
                    std::fs::remove_file(entry.path())?;
                }
                println!("removed {}", entry.path().display());
            }
        }
    }
    Ok(())
}

pub async fn main() -> anyhow::Result<()> {
    // OpenSSH re-invokes this binary as its SSH_ASKPASS helper during interactive
    // SSH auth; print the resolved secret and exit before any normal startup.
    if infrazeug_core::ssh_askpass::is_askpass_invocation() {
        infrazeug_core::ssh_askpass::emit_secret()?;
        return Ok(());
    }
    init_tracing();
    let args: Vec<OsString> = std::env::args_os().collect();
    if try_forward_discovered_playbook(&args).await? {
        return Ok(());
    }
    let cli = Cli::parse_from(&args);
    run_cli(cli).await
}

#[cfg(test)]
mod argv_tests {
    use super::*;

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    #[test]
    fn stock_subcommands_include_plan_and_vault() {
        assert!(is_stock_subcommand("plan"));
        assert!(is_stock_subcommand("vault"));
        assert!(is_stock_subcommand("init"));
    }

    #[test]
    fn first_subcommand_skips_playbook_flag() {
        let args = vec![os("infrazeug"), os("--playbook"), os("main"), os("apply")];
        assert_eq!(first_subcommand_from_argv(&args).as_deref(), Some("apply"));
    }

    #[test]
    fn forward_policy() {
        assert!(should_forward_to_playbook("plan"));
        assert!(!should_forward_to_playbook("init"));
        assert!(!should_forward_to_playbook("vault"));
        assert!(!should_forward_to_playbook("mcp"));
    }
}
