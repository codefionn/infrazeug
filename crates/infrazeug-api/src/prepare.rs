//! Pre-apply agent builds and per-machine triple assignment.

use crate::probe::export_probe_targets;
use crate::transport_env::transport_name;
use crate::Infra;
use futures::future::join_all;
use infrazeug_build::{build_agent, host_triple, uname_machine_to_triple, AgentBuildOptions};
use infrazeug_core::events::{MachinePreparePhase, SchedEvent};
use infrazeug_core::id::MachineId;
use infrazeug_core::machine::MachineKind;
use infrazeug_core::transport::TransportChoice;
use infrazeug_transport::{probe_uname_machine, TransportFactory};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;

fn emit_machine(
    events: Option<&broadcast::Sender<SchedEvent>>,
    machine: MachineId,
    phase: MachinePreparePhase,
    detail: Option<String>,
) {
    if let Some(tx) = events {
        let _ = tx.send(SchedEvent::PrepareMachine {
            machine,
            phase,
            detail,
        });
    }
}

/// Build agents for the controller + probed remotes; map machines → triple on `factory`.
pub async fn prepare_agents(
    infra: &Infra,
    factory: &Arc<TransportFactory>,
    agent_workspace: &Path,
    release: bool,
    events: Option<&broadcast::Sender<SchedEvent>>,
) -> anyhow::Result<()> {
    let export = export_probe_targets(infra);
    let mut triples: HashSet<String> = HashSet::new();
    triples.insert(host_triple().unwrap_or_else(|| export.host_triple.clone()));

    let probe_futs: Vec<_> = infra
        .machines
        .iter()
        .filter(|machine| !machine.lazy)
        .filter_map(|machine| {
            let MachineKind::Remote { ssh, os } = &machine.kind else {
                return None;
            };
            let choice = infra.transport_for_machine(machine);
            if choice != TransportChoice::SshAgentPush {
                tracing::debug!(
                    machine = %machine.name,
                    transport = transport_name(choice),
                    "skipping agent build (not agent push)"
                );
                return None;
            }
            let uname = os.as_ref().and_then(|hint| hint.arch.clone());
            let ssh = ssh.clone();
            let machine_id = machine.id;
            let needs_probe = uname.is_none();
            Some(async move {
                if needs_probe {
                    emit_machine(events, machine_id, MachinePreparePhase::ProbingArch, None);
                }
                let uname = match uname {
                    Some(u) => u,
                    // This eager export/pre-build probe has no resolved secret;
                    // interactive SSH auth is handled on the apply path instead.
                    None => probe_uname_machine(&ssh, None)
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                };
                Ok::<_, anyhow::Error>((machine_id, uname))
            })
        })
        .collect();

    let probe_results = join_all(probe_futs).await;
    let mut machine_triples: HashMap<MachineId, String> = HashMap::new();
    for result in probe_results {
        let (machine_id, uname) = result?;
        let triple = uname_machine_to_triple(&uname);
        emit_machine(
            events,
            machine_id,
            MachinePreparePhase::BuildingAgent,
            Some(triple.clone()),
        );
        triples.insert(triple.clone());
        machine_triples.insert(machine_id, triple);
    }

    if !triples.is_empty() {
        let targets: Vec<String> = triples.iter().cloned().collect();
        if let Some(tx) = events {
            let list = targets.join(", ");
            let _ = tx.send(SchedEvent::PrepareGlobal {
                message: format!("building infrazeug-agent ({list})"),
            });
        }
        let opts = AgentBuildOptions {
            targets,
            release,
            quiet: events.is_some(),
        };
        build_agent(agent_workspace, &opts).map_err(anyhow::Error::msg)?;
    }

    factory.set_machine_triples(machine_triples).await;
    Ok(())
}
