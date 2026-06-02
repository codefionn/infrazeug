//! rmcp [`ServerHandler`] implementation backing dynamically registered tools
//! plus the always-on builtins. Tool execution is delegated to the injected
//! [`InfraExecutor`]; this type never touches the apply pipeline directly.
//!
//! Security invariant (SOUL §6.10 / §6bis.4): no vault/secret tool exists here.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, JsonObject, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::api_docs;
use crate::builder::{schema_object, ToolDef};
use crate::builtins::{
    self, GraphInput, PingInput, SearchDocsInput, GRAPH, LIST_MACHINES, PING, PING_NODE,
    SEARCH_API_DOCS,
};
use crate::ctx::McpCtx;
use crate::docs::{self, DOCS_API_INDEX_URI, DOCS_URI};
use crate::exec::{InfraExecutor, ToolRun};
use infrazeug_core::GraphView;

pub(crate) struct InfraServer {
    ctx: McpCtx,
    graph: GraphView,
    tools: Vec<ToolDef>,
    executor: Arc<dyn InfraExecutor>,
    server_name: String,
}

impl InfraServer {
    pub(crate) fn new(
        machines: Vec<infrazeug_core::Machine>,
        graph: GraphView,
        tools: Vec<ToolDef>,
        executor: Arc<dyn InfraExecutor>,
        server_name: String,
    ) -> Self {
        Self {
            ctx: McpCtx::new(machines),
            graph,
            tools,
            executor,
            server_name,
        }
    }

    fn tool_list(&self) -> Vec<Tool> {
        let mut out = vec![
            Tool::new(
                LIST_MACHINES,
                "List the machines on this infra",
                empty_schema(),
            ),
            Tool::new(
                PING,
                "Check a machine is reachable (runs `uname -n`)",
                schema_object::<PingInput>(),
            ),
            Tool::new(
                GRAPH,
                "Inspect the planning DAG, optionally filtered by machine(s), start node, and tags",
                schema_object::<GraphInput>(),
            ),
            Tool::new(
                SEARCH_API_DOCS,
                "Search embedded rustdoc for infrazeug-api and extension crates (LLM-readable JSON)",
                schema_object::<SearchDocsInput>(),
            ),
        ];
        for t in &self.tools {
            out.push(Tool::new(
                t.name.clone(),
                t.description.clone(),
                Arc::clone(&t.schema),
            ));
        }
        out
    }
}

impl ServerHandler for InfraServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        info.server_info = Implementation::new(self.server_name.clone(), env!("CARGO_PKG_VERSION"));
        info.instructions =
            Some("infrazeug MCP server. Tools never expose secrets (locked).".to_string());
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tool_list()))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(docs::doc_resources()))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.as_str();
        let body = if uri == DOCS_URI {
            docs::docs_contents(&self.server_name, &self.tools)
        } else if uri == DOCS_API_INDEX_URI {
            docs::api_index_contents()
        } else if let Some(path) = docs::parse_api_item_uri(uri) {
            docs::api_item_contents(path).ok_or_else(|| {
                McpError::invalid_params(format!("unknown API path `{path}`"), None)
            })?
        } else {
            return Err(McpError::invalid_params(
                format!(
                    "unknown resource URI `{uri}` (try `{DOCS_URI}`, `{DOCS_API_INDEX_URI}`, or `{}<rust_path>`)",
                    docs::DOCS_API_ITEM_PREFIX
                ),
                None,
            ));
        };
        Ok(ReadResourceResult::new(vec![body]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.as_ref();
        match name {
            LIST_MACHINES => Ok(structured(builtins::list_machines(self.ctx.machines()))),
            GRAPH => {
                let input: GraphInput = parse_args(request.arguments)?;
                let value = builtins::graph_json(&self.graph, input)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(structured(value))
            }
            SEARCH_API_DOCS => {
                let input: SearchDocsInput = parse_args(request.arguments)?;
                let value =
                    api_docs::search_json(&input.query, input.limit, input.crate_name.as_deref());
                Ok(structured(value))
            }
            PING => {
                let input: PingInput = parse_args(request.arguments)?;
                let machine = self
                    .ctx
                    .machine(&input.machine)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let infra = builtins::ping_infra(machine)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                let run = self
                    .executor
                    .run(infra)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(structured(json!({
                    "reachable": run.all_ok(),
                    "hostname": run.capture(PING_NODE),
                })))
            }
            other => {
                let tool = self.tools.iter().find(|t| t.name == other).ok_or_else(|| {
                    McpError::invalid_params(format!("unknown tool `{other}`"), None)
                })?;
                let args = Value::Object(request.arguments.unwrap_or_default());
                let infra = (tool.build)(args, &self.ctx)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let run = self
                    .executor
                    .run(infra)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                Ok(structured(tool_run_json(&run)))
            }
        }
    }
}

fn parse_args<T: DeserializeOwned>(args: Option<JsonObject>) -> Result<T, McpError> {
    let value = Value::Object(args.unwrap_or_default());
    serde_json::from_value(value).map_err(|e| McpError::invalid_params(e.to_string(), None))
}

fn structured(value: Value) -> CallToolResult {
    CallToolResult::structured(value)
}

fn empty_schema() -> Arc<JsonObject> {
    let mut map = JsonObject::new();
    map.insert("type".to_string(), json!("object"));
    map.insert("properties".to_string(), json!({}));
    Arc::new(map)
}

fn tool_run_json(run: &ToolRun) -> Value {
    json!({
        "ok": run.all_ok(),
        "nodes": run.report.entries.iter().map(|e| json!({
            "node": e.node_name,
            "machine": e.machine_id.0.to_string(),
            "status": format!("{:?}", e.status),
            "message": e.message,
        })).collect::<Vec<_>>(),
        "captures": run.captures.iter().map(|c| json!({
            "node": c.node,
            "machine": c.machine,
            "stdout": c.stdout,
        })).collect::<Vec<_>>(),
    })
}
