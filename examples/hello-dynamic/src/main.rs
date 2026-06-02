//! Dynamic machines: discover a set of hosts at apply time, then fan a per-machine
//! "playbook" (connect + install) out over each discovered machine.
//!
//! The discovery method emits a JSON array of `DiscoveredMachine` as its node
//! capture; the scheduler turns each into a lazy push machine and runs the
//! template on it. The per-machine `connectivity` head is what uploads the agent
//! (it is the machine's first transport use). Failures tolerate by default — a bad
//! host is skipped and the rest proceed.
//!
//! Override the discovered set with `HOSTS="web-1=10.0.0.1,web-2=10.0.0.2"`.

use async_trait::async_trait;
use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, PlaybookBundle, RunBuildContext, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::machine::{DiscoveredMachine, SshConfig};
use infrazeug_core::RuntimeConfig;
use infrazeug_native::{
    NativeError, NativeResult, NodeCtx, NodeMethod, Result as NativeMethodResult,
};
use infrazeug_shell::{argv, ShellOp};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const CONTROLLER: &str = "a1b2c3d4-e5f6-4789-a012-3456789abcde";
const PREP_NODE: &str = "b2c3d4e5-f6a7-4890-b123-456789abcdef";
const DISCOVER_NODE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";
const CONNECT_NODE: &str = "d4e5f6a7-b8c9-4012-d345-6789abcdef01";
const INSTALL_NODE: &str = "e5f6a7b8-c9d0-4123-e456-789abcdef012";

#[derive(Clone, Copy, Debug, Default)]
struct DiscoverHosts;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DiscoverInput {}

#[async_trait]
impl NodeMethod for DiscoverHosts {
    type Input = DiscoverInput;
    type Output = Vec<DiscoveredMachine>;

    fn name(&self) -> &'static str {
        "example.discover_hosts"
    }

    async fn execute(
        &self,
        _ctx: &NodeCtx,
        _input: DiscoverInput,
    ) -> NativeMethodResult<NativeResult> {
        // A real playbook would read an upstream prep node's capture (a cloud API
        // listing, an inventory file written by `prep`, etc.) via `ctx`. Here we
        // parse a simple `name=host,...` list from the environment.
        let raw =
            std::env::var("HOSTS").unwrap_or_else(|_| "web-1=10.0.0.1,web-2=10.0.0.2".to_string());
        let machines: Vec<DiscoveredMachine> = raw
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .filter_map(|entry| {
                let (name, host) = entry.trim().split_once('=')?;
                Some(DiscoveredMachine {
                    name: name.to_string(),
                    ssh: SshConfig::new(host).with_user("deploy"),
                    vars: Default::default(),
                    tags: Vec::new(),
                    os: None,
                })
            })
            .collect();
        NativeResult::changed(format!("discovered {} host(s)", machines.len()))
            .with_json_capture(&machines)
            .map_err(|e| NativeError::other(e.to_string()))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-dynamic")
            .about("Discover hosts at apply time, fan a playbook over each"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!("dynamic fan-out is apply/push-only"),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<PlaybookBundle> {
    let controller = MachineId(Uuid::parse_str(CONTROLLER)?);
    let prep = NodeId(Uuid::parse_str(PREP_NODE)?);
    let disc = NodeId(Uuid::parse_str(DISCOVER_NODE)?);
    let connect = NodeId(Uuid::parse_str(CONNECT_NODE)?);
    let install = NodeId(Uuid::parse_str(INSTALL_NODE)?);

    Ok(InfraBuilder::new()
        .machine(builder::controller(controller))?
        // Prep step: in a real run this would gather the data the discovery method
        // reads (query a cloud API, render an inventory, …).
        .shell_on_machine(
            prep,
            "prep-inventory",
            controller,
            ShellOp::run(argv!["true"]),
        )?
        .discover_machines(
            disc,
            "discover-hosts",
            controller,
            "web",
            DiscoverHosts,
            DiscoverInput {},
        )?
        .deps([prep])
        .fail_fast(false)
        .max_parallel_machines(10)
        .for_each_machine(|m| {
            // Connectivity head: first transport use → uploads the agent + ping.
            m.connectivity(connect, "connect");
            // The per-machine "playbook" body.
            m.shell(
                install,
                "install-nginx",
                ShellOp::run(argv!["echo", "installing nginx"]),
                [connect],
            );
        })?
        .build()
        .with_runtime(RuntimeConfig {
            run_root: std::env::temp_dir().join("infrazeug-hello-dynamic"),
            vault_store: None,
        }))
}
