//! Always-on builtin tools: `list_machines` (metadata only) and `ping`
//! (runs a trivial capture node through the same executor as custom tools).

use infrazeug_core::infra::shell_node;
use infrazeug_core::node::Targets;
use infrazeug_core::{GraphSelect, GraphView, Infra, Machine, MachineKind, NodeId};
use infrazeug_shell::{argv, ShellOp};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Input for the `ping` builtin.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PingInput {
    /// Name of a machine registered on the served infra.
    pub machine: String,
}

/// Input for the `graph` builtin (all fields optional; empty = no filter).
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GraphInput {
    /// Keep nodes targeting any of these machine names.
    pub machines: Vec<String>,
    /// Keep this node (name or id) and its transitive dependents.
    pub start: Option<String>,
    /// Keep nodes carrying any of these tags (`key=value` or bare `key`).
    pub tags: Vec<String>,
}

pub const LIST_MACHINES: &str = "list_machines";
pub const PING: &str = "ping";
pub const PING_NODE: &str = "mcp-ping";
pub const GRAPH: &str = "graph";
pub const SEARCH_API_DOCS: &str = "search_api_docs";

/// Input for the `search_api_docs` builtin.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    /// Free-text query (matches path, name, summary, and doc body).
    pub query: String,
    /// Maximum hits to return (default 10, max 50).
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    /// Optional crate filter (e.g. `infrazeug-api`).
    #[serde(default)]
    pub crate_name: Option<String>,
}

fn default_search_limit() -> usize {
    10
}

/// Structured JSON for the `graph` tool: `graph` filtered by `input`. Shared by
/// the live [`InfraServer`](crate::server) and the watch-mode warmup proxy so
/// both render the planning DAG identically.
pub fn graph_json(graph: &GraphView, input: GraphInput) -> Result<Value, serde_json::Error> {
    let select = GraphSelect {
        machines: input.machines,
        start: input.start,
        tags: input.tags,
    };
    serde_json::to_value(graph.select(&select))
}

/// JSON description of every machine on the served infra (no execution).
pub fn list_machines(machines: &[Machine]) -> Value {
    let items: Vec<Value> = machines.iter().map(machine_json).collect();
    json!({ "machines": items })
}

fn machine_json(m: &Machine) -> Value {
    let (kind, os) = match &m.kind {
        MachineKind::Local => ("local", Value::Null),
        MachineKind::Remote { ssh, os } => (
            "remote",
            json!({
                "host": ssh.host,
                "user": ssh.user,
                "os": os.as_ref().map(|o| json!({
                    "family": format!("{:?}", o.family),
                    "distro": o.distro,
                    "version": o.version,
                })),
            }),
        ),
        MachineKind::Container(_) => ("container", Value::Null),
    };
    json!({
        "name": m.name,
        "id": m.id.0.to_string(),
        "kind": kind,
        "details": os,
    })
}

/// Single-node infra that captures `uname -n` on `machine`. The scheduler
/// stores a successful node's stdout, so the executor surfaces it as a capture.
pub fn ping_infra(machine: Machine) -> anyhow::Result<Infra> {
    let mid = machine.id;
    let node = shell_node(
        NodeId(Uuid::new_v4()),
        PING_NODE,
        ShellOp::run(argv!["uname", "-n"]),
        Targets::Machine(mid),
    );
    Ok(Infra::new().add_machine(machine)?.add_node(node)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_core::{GraphNode, GraphView};

    fn sample_graph() -> GraphView {
        GraphView {
            nodes: vec![
                GraphNode {
                    id: "n-web".into(),
                    name: "nginx".into(),
                    kind: "shell".into(),
                    machines: vec!["web".into()],
                    ..Default::default()
                },
                GraphNode {
                    id: "n-db".into(),
                    name: "postgres".into(),
                    kind: "shell".into(),
                    machines: vec!["db".into()],
                    ..Default::default()
                },
            ],
            edges: vec![],
        }
    }

    #[test]
    fn graph_json_renders_all_nodes_by_default() {
        let v = graph_json(&sample_graph(), GraphInput::default()).unwrap();
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["name"], "nginx");
    }

    #[test]
    fn graph_json_filters_by_machine() {
        let input = GraphInput {
            machines: vec!["db".into()],
            ..Default::default()
        };
        let v = graph_json(&sample_graph(), input).unwrap();
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["name"], "postgres");
    }
}
