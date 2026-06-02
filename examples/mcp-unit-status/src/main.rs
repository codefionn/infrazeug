//! MCP + graph demo. Exposes (via `mcp serve`) `list_machines`, `ping`,
//! `graph`, `infrazeug://docs`, and a custom `unit_status` tool. Also runnable as
//! `mcp-unit-status graph [--machine web] [--start base-setup] [--tag app=web]`.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, McpBuilder, McpExt, RunBuildContext, RunConfig};
use infrazeug_core::id::{MachineId, NodeId, Tag};
use infrazeug_core::infra::shell_node;
use infrazeug_core::node::Targets;
use infrazeug_core::{Infra, RuntimeConfig};
use infrazeug_shell::{argv, ShellOp};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

const WEB_MACHINE: &str = "a1b2c3d4-e5f6-4789-a012-3456789abcde";
const DB_MACHINE: &str = "b2c3d4e5-f6a7-4890-b123-456789abcdef";
const BASE_NODE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";
const NGINX_NODE: &str = "d4e5f6a7-b8c9-4012-d345-6789abcdef01";
const PG_NODE: &str = "e5f6a7b8-c9d0-4123-e456-789abcdef012";

/// Input for the `unit_status` tool.
#[derive(Deserialize, JsonSchema)]
struct UnitInput {
    /// Name of a machine registered on this infra (e.g. "web").
    machine: String,
    /// Systemd unit to query (e.g. "nginx").
    unit: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("mcp-unit-status")
            .about("MCP + graph demo")
            .mcp(mcp_server),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let web = MachineId(Uuid::parse_str(WEB_MACHINE)?);
    let db = MachineId(Uuid::parse_str(DB_MACHINE)?);
    let base_id = NodeId(Uuid::parse_str(BASE_NODE)?);

    // base-setup runs on both machines; nginx/postgres depend on it.
    let mut base = shell_node(
        base_id,
        "base-setup",
        ShellOp::run(argv!["true"]),
        Targets::Machines(vec![web, db]),
    );
    base.tags.push(Tag::new("tier", "base"));

    let mut nginx = shell_node(
        NodeId(Uuid::parse_str(NGINX_NODE)?),
        "nginx",
        ShellOp::run(argv!["true"]),
        Targets::Machine(web),
    );
    nginx.deps.push(base_id);
    nginx.tags.push(Tag::new("app", "web"));

    let mut postgres = shell_node(
        NodeId(Uuid::parse_str(PG_NODE)?),
        "postgres",
        ShellOp::run(argv!["true"]),
        Targets::Machine(db),
    );
    postgres.deps.push(base_id);
    postgres.tags.push(Tag::new("app", "db"));

    let bundle = InfraBuilder::new()
        .machine(builder::local(web, "web"))?
        .machine(builder::local(db, "db"))?
        .node(base)?
        .node(nginx)?
        .node(postgres)?
        .build();

    Ok(bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("infrazeug-mcp-unit-status"),
        vault_store: None,
    }))
}

/// Builds the MCP server served by `mcp serve`.
fn mcp_server() -> anyhow::Result<McpBuilder> {
    let bundle = build_infra()?;
    Ok(bundle.infra.mcp().tool::<UnitInput, _>(
        "unit_status",
        "Report whether a systemd unit is active on a machine",
        |inp, ctx| {
            let machine = ctx.machine(&inp.machine)?;
            let mid = machine.id;
            Ok(Infra::new().add_machine(machine)?.add_node(shell_node(
                NodeId(Uuid::new_v4()),
                "unit-status",
                ShellOp::run(argv!["systemctl", "is-active", &inp.unit]),
                Targets::Machine(mid),
            ))?)
        },
    ))
}
