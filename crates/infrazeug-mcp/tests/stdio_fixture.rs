//! Drives the MCP server over an in-memory duplex transport with a fake
//! executor, asserting the builtin + custom tool round-trips.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use infrazeug_core::infra::{local_machine, shell_node};
use infrazeug_core::node::{NodeStatus, Targets};
use infrazeug_core::report::{RunReport, RunReportEntry};
use infrazeug_core::{Infra, MachineId, NodeId};
use infrazeug_mcp::{CaptureOut, InfraExecutor, McpBuilder, ToolRun, DOCS_API_INDEX_URI, DOCS_URI};
use infrazeug_shell::{argv, ShellOp};
use rmcp::model::CallToolRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::ServiceExt;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize, JsonSchema)]
struct UnitInput {
    machine: String,
    unit: String,
}

/// Returns a canned run with one capture, ignoring the infra it is handed —
/// enough to exercise the dispatch + serialization plumbing.
struct FakeExec;

#[async_trait]
impl InfraExecutor for FakeExec {
    async fn run(&self, _infra: Infra) -> anyhow::Result<ToolRun> {
        Ok(ToolRun {
            report: RunReport {
                entries: vec![RunReportEntry {
                    node_id: NodeId(Uuid::new_v4()),
                    node_name: "mcp-ping".into(),
                    machine_id: MachineId(Uuid::new_v4()),
                    status: NodeStatus::Changed,
                    duration: Duration::from_millis(1),
                    message: None,
                }],
            },
            captures: vec![CaptureOut {
                node: "mcp-ping".into(),
                machine: "web".into(),
                stdout: "test-host\n".into(),
            }],
        })
    }
}

#[tokio::test]
async fn stdio_roundtrip() -> anyhow::Result<()> {
    let machine = local_machine(MachineId(Uuid::new_v4()), "web");
    let builder = McpBuilder::new(vec![machine])
        .with_executor(Arc::new(FakeExec))
        .tool::<UnitInput, _>("unit_status", "Check a systemd unit", |inp, ctx| {
            let m = ctx.machine(&inp.machine)?;
            let mid = m.id;
            Ok(Infra::new().add_machine(m)?.add_node(shell_node(
                NodeId(Uuid::new_v4()),
                "check",
                ShellOp::run(argv!["systemctl", "is-active", &inp.unit]),
                Targets::Machine(mid),
            ))?)
        });

    let (server_t, client_t) = tokio::io::duplex(4096);
    let server = tokio::spawn(builder.serve(server_t));
    let client = ().serve(client_t).await?;

    // tools/list exposes both builtins and the custom tool.
    let tools = client.list_all_tools().await?;
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        names.contains(&"list_machines"),
        "missing list_machines: {names:?}"
    );
    assert!(names.contains(&"ping"), "missing ping: {names:?}");
    assert!(
        names.contains(&"unit_status"),
        "missing unit_status: {names:?}"
    );
    assert!(
        names.contains(&"search_api_docs"),
        "missing search_api_docs: {names:?}"
    );

    let resources = client.list_all_resources().await?;
    let uris: Vec<&str> = resources.iter().map(|r| r.uri.as_ref()).collect();
    assert!(uris.contains(&DOCS_URI), "missing docs resource: {uris:?}");
    assert!(
        uris.contains(&DOCS_API_INDEX_URI),
        "missing api-index resource: {uris:?}"
    );

    let doc = client
        .read_resource(ReadResourceRequestParams::new(DOCS_URI))
        .await?;
    let text = match &doc.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.as_str(),
        _ => panic!("expected text resource"),
    };
    assert!(text.contains("unit_status"));
    assert!(text.contains("Security"));

    let index = client
        .read_resource(ReadResourceRequestParams::new(DOCS_API_INDEX_URI))
        .await?;
    let index_text = match &index.contents[0] {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.as_str(),
        _ => panic!("expected json resource"),
    };
    let index_v: serde_json::Value = serde_json::from_str(index_text)?;
    assert!(index_v["item_count"].as_u64().unwrap_or(0) > 0);

    let mut search_args = serde_json::Map::new();
    search_args.insert("query".into(), json!("RunConfig"));
    search_args.insert("limit".into(), json!(3));
    search_args.insert("crate_name".into(), json!("infrazeug-api"));
    let search = client
        .call_tool(CallToolRequestParams::new("search_api_docs").with_arguments(search_args))
        .await?;
    let hits = search
        .structured_content
        .as_ref()
        .and_then(|v| v.get("hits"))
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(!hits.is_empty(), "search_api_docs returned no hits");

    // list_machines is metadata-only, no execution.
    let res = client
        .call_tool(CallToolRequestParams::new("list_machines"))
        .await?;
    let v = res.structured_content.expect("structured content");
    assert_eq!(v["machines"][0]["name"], "web");

    // Custom tool: input deserialized, infra built, fake executor's capture surfaced.
    let mut args = serde_json::Map::new();
    args.insert("machine".into(), json!("web"));
    args.insert("unit".into(), json!("nginx"));
    let res = client
        .call_tool(CallToolRequestParams::new("unit_status").with_arguments(args))
        .await?;
    let v = res.structured_content.expect("structured content");
    assert_eq!(v["ok"], true);
    assert_eq!(v["captures"][0]["stdout"], "test-host\n");

    // Unknown machine name is a clean tool error, not a panic.
    let mut bad = serde_json::Map::new();
    bad.insert("machine".into(), json!("nope"));
    bad.insert("unit".into(), json!("nginx"));
    let err = client
        .call_tool(CallToolRequestParams::new("unit_status").with_arguments(bad))
        .await;
    assert!(err.is_err() || err.unwrap().is_error == Some(true));

    client.cancel().await?;
    server.abort();
    Ok(())
}
