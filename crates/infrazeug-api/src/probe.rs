//! Machine export for `__infrazeug-probe` (playbook discovery / agent prebuild).

use crate::Infra;
use infrazeug_core::machine::MachineKind;
use infrazeug_core::transport::TransportChoice;
use infrazeug_core::{GraphView, SshConfig};
use serde::{Deserialize, Serialize};

pub const PROBE_SUBCOMMAND: &str = "__infrazeug-probe";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeExport {
    pub host_triple: String,
    pub remotes: Vec<RemoteProbeTarget>,
    pub has_native_nodes: bool,
    /// Offline planning DAG. Lets `mcp serve` watch mode serve the `graph` tool
    /// straight from this local export, before any SSH probe or agent build.
    #[serde(default)]
    pub graph: GraphView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteProbeTarget {
    pub name: String,
    pub ssh: SshConfig,
    pub transport: TransportChoice,
    /// `uname -m` when known from `OsHint::arch`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_arch: Option<String>,
}

pub fn export_probe_targets(infra: &Infra) -> ProbeExport {
    let host_triple = infrazeug_build::host_triple().unwrap_or_else(|| "host".into());
    let has_native_nodes = infra
        .nodes
        .iter()
        .any(|n| matches!(n.body, infrazeug_core::NodeBody::Native { .. }));

    let mut remotes = Vec::new();
    for machine in &infra.machines {
        let MachineKind::Remote { ssh, os } = &machine.kind else {
            continue;
        };
        remotes.push(RemoteProbeTarget {
            name: machine.name.clone(),
            ssh: ssh.clone(),
            transport: infra.transport_for_machine(machine),
            os_arch: os.as_ref().and_then(|o| o.arch.clone()),
        });
    }

    ProbeExport {
        host_triple,
        remotes,
        has_native_nodes,
        graph: infra.graph_view().unwrap_or_default(),
    }
}
