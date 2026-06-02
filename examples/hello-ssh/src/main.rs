//! M2 vertical slice: `nginx -v` on a remote host via SSH (agentless or agent push).
//!
//! Set `INFRZEUG_SSH_HOST` (e.g. `root@127.0.0.1:2222`) and optional `INFRZEUG_SSH_MODE`
//! (`agentless` or `agent`, default `agentless`) before running apply.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{
    init_tracing, run, PlaybookCommand, RunBuildContext, RunConfig, SshConfig, TransportChoice,
};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const REMOTE_MACHINE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";
const NGINX_NODE: &str = "d4e5f6a7-b8c9-4012-d345-6789abcdef01";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-ssh").about("M2 remote nginx -v over SSH"),
        build_infra_for,
    )
    .await
}

fn build_infra_for(ctx: RunBuildContext<'_>) -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let RunBuildContext::Playbook(ctx) = ctx else {
        unreachable!()
    };
    let mode = match &ctx.command {
        PlaybookCommand::Apply(_) => ssh_mode_from_env()?,
        _ => TransportChoice::SshAgentless,
    };
    build_infra(mode)
}

fn ssh_mode_from_env() -> anyhow::Result<TransportChoice> {
    match std::env::var("INFRZEUG_SSH_MODE")
        .unwrap_or_else(|_| "agentless".into())
        .to_lowercase()
        .as_str()
    {
        "agent" => Ok(TransportChoice::SshAgentPush),
        "agentless" => Ok(TransportChoice::SshAgentless),
        other => anyhow::bail!("INFRZEUG_SSH_MODE must be agentless or agent, got {other}"),
    }
}

fn build_infra(choice: TransportChoice) -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let host = std::env::var("INFRZEUG_SSH_HOST")
        .map_err(|_| anyhow::anyhow!("set INFRZEUG_SSH_HOST (e.g. root@127.0.0.1:2222)"))?;
    let machine_id = MachineId(Uuid::parse_str(REMOTE_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(NGINX_NODE)?);
    let ssh = SshConfig::new(host);

    Ok(InfraBuilder::new()
        .machine(builder::remote(machine_id, "remote", ssh))?
        .shell_on_machine(
            node_id,
            "nginx-version",
            machine_id,
            ShellOp::run(argv!["nginx", "-v"]),
        )?
        .build()
        .with_transport_choice(machine_id, choice)
        .with_runtime(RuntimeConfig {
            run_root: std::env::temp_dir().join("infrazeug-hello-ssh"),
            vault_store: None,
        }))
}
