//! First-boot bootstrap stub for pull-mode ephemeral hosts (SOUL §3.11.8).
//!
//! A minimal static binary baked into cloud-init or ISO: read local bootstrap
//! config, download and verify `infrazeug-agent`, fetch the sealed plan slice
//! from the plan store, then `exec` the agent in `serve-pull` mode. No
//! controller connection is required after first boot.
//!
//! Subcommands are delegated to [`infrazeug_api::pull_cli`] with
//! [`BootstrapExec::DelegateAgent`].

use infrazeug_api::pull_cli::PullCommandSet;
use infrazeug_api::{init_tracing, run, BootstrapExec, RunCommands, RunConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("infrazeug-bootstrap")
            .about("Pull-mode first-boot stub")
            .commands(RunCommands::empty())
            .pull(PullCommandSet::empty().with(PullCommandSet::BOOTSTRAP))
            .bootstrap_exec(BootstrapExec::DelegateAgent),
        |_| {
            Ok(infrazeug_api::PlaybookBundle::from_infra(
                infrazeug_api::default_infra(),
            ))
        },
    )
    .await
}
