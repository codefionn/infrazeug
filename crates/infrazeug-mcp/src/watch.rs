//! Warmup MCP front-end for `infrazeug mcp serve` watch mode.
//!
//! Answers `initialize`, `tools/list`, and `resources/list` immediately while the
//! playbook binary builds. Documentation resources and `search_api_docs` always
//! use the stock embedded API index (never the playbook child). Other tool calls
//! forward to the child once it is up.

use std::sync::Arc;

use anyhow::Context;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, JsonObject, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResult, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::{RequestContext, RoleClient, RoleServer, RunningService, ServiceError};
use rmcp::transport::IntoTransport;
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde_json::Value;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::api_docs;
use crate::building::BuildingExecutor;
use crate::builtins::{self, GraphInput, GRAPH, SEARCH_API_DOCS};
use crate::server::InfraServer;
use infrazeug_core::GraphView;

type LiveClient = RunningService<RoleClient, ()>;

/// MCP server that warms up with builtin discovery, then proxies to a child.
#[derive(Clone)]
pub struct WatchProxy {
    inner: Arc<WatchProxyInner>,
}

struct WatchProxyInner {
    /// Embedded docs, API index, and `search_api_docs` (never delegated to the child).
    docs: InfraServer,
    /// Planning DAG served by the builtin `graph` tool while the playbook builds.
    /// Seeded from the offline `__infrazeug-probe` export so `graph` works even
    /// when remote hosts are unreachable or agents are still cross-building.
    warmup_graph: RwLock<GraphView>,
    live: RwLock<Option<LiveClient>>,
}

impl Default for WatchProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchProxy {
    pub fn new() -> Self {
        api_docs::warm_in_background();
        Self {
            inner: Arc::new(WatchProxyInner {
                docs: warmup_server(),
                warmup_graph: RwLock::new(GraphView::default()),
                live: RwLock::new(None),
            }),
        }
    }

    /// Seed the planning DAG served by the builtin `graph` tool while the
    /// playbook builds. Sourced from the offline probe export, so `graph` works
    /// before (and regardless of) any SSH probe or agent cross-build.
    pub async fn set_warmup_graph(&self, graph: GraphView) {
        *self.inner.warmup_graph.write().await = graph;
    }

    /// Serve the builtin `graph` tool from the seeded warmup DAG.
    async fn warmup_graph_tool(
        &self,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, McpError> {
        let input: GraphInput =
            serde_json::from_value(Value::Object(arguments.unwrap_or_default()))
                .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
        let graph = self.inner.warmup_graph.read().await.clone();
        let value = builtins::graph_json(&graph, input)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::structured(value))
    }

    pub async fn reset_to_warming(&self) {
        let mut live = self.inner.live.write().await;
        if let Some(mut client) = live.take() {
            let _ = client.close().await;
        }
    }

    pub async fn set_live_stdio(&self, child: &mut Child) -> anyhow::Result<()> {
        let stdout = child.stdout.take().context("child stdout")?;
        let stdin = child.stdin.take().context("child stdin")?;
        let client = connect_stdio_client(stdout, stdin).await?;
        self.set_live(client).await?;
        Ok(())
    }

    pub async fn set_live_http(&self, child_url: &str) -> anyhow::Result<()> {
        let client = connect_http_client(child_url).await?;
        self.set_live(client).await?;
        Ok(())
    }

    async fn set_live(&self, client: LiveClient) -> anyhow::Result<()> {
        info!("playbook MCP connected — forwarding tool calls");
        let mut live = self.inner.live.write().await;
        if let Some(mut old) = live.replace(client) {
            let _ = old.close().await;
        }
        Ok(())
    }

    pub async fn serve_stdio(self, cancel: CancellationToken) -> anyhow::Result<()> {
        use rmcp::transport::io::stdio;
        let running = self.serve_with_ct(stdio().into_transport(), cancel).await?;
        running.waiting().await?;
        Ok(())
    }

    pub async fn serve_http(self, bind: &str, cancel: CancellationToken) -> anyhow::Result<()> {
        crate::http::serve_handler(move || Ok(self.clone()), bind, Some(cancel)).await
    }
}

pub fn warmup_server() -> InfraServer {
    InfraServer::new(
        vec![],
        GraphView::default(),
        vec![],
        Arc::new(BuildingExecutor),
        "infrazeug (starting)".into(),
    )
}

pub async fn connect_stdio_client(
    stdout: ChildStdout,
    stdin: ChildStdin,
) -> anyhow::Result<LiveClient> {
    let transport = (stdout, stdin).into_transport();
    Ok(().serve(transport).await?)
}

pub async fn connect_http_client(base_url: &str) -> anyhow::Result<LiveClient> {
    use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
    use rmcp::transport::StreamableHttpClientTransport;

    let uri: Arc<str> = if base_url.ends_with("/mcp") {
        base_url.into()
    } else {
        format!("{}/mcp", base_url.trim_end_matches('/')).into()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let transport = StreamableHttpClientTransport::with_client(
            reqwest::Client::new(),
            StreamableHttpClientTransportConfig::with_uri(Arc::clone(&uri)),
        );
        match ().serve(transport).await {
            Ok(client) => return Ok(client),
            Err(e) if std::time::Instant::now() < deadline => {
                tracing::debug!(%e, "waiting for playbook MCP HTTP");
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

impl ServerHandler for WatchProxy {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        info.instructions = Some(
            "infrazeug MCP (watch). Builtin tools are listed while the playbook builds; \
             playbook tools appear once the build finishes. API docs and resources are always \
             served from the stock infrazeug index. Secrets are never exposed."
                .into(),
        );
        info
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        if let Some(peer) = &*self.inner.live.read().await {
            return peer.list_tools(request).await.map_err(service_err);
        }
        self.inner.docs.list_tools(request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.inner.docs.list_resources(request, context).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        self.inner.docs.read_resource(request, context).await
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if request.name.as_ref() == SEARCH_API_DOCS {
            return self.inner.docs.call_tool(request, context).await;
        }
        if let Some(peer) = &*self.inner.live.read().await {
            return peer.call_tool(request).await.map_err(service_err);
        }
        // No child yet: answer `graph` from the offline warmup DAG instead of the
        // empty docs-server graph, so the planning DAG is available during the build.
        if request.name.as_ref() == GRAPH {
            return self.warmup_graph_tool(request.arguments).await;
        }
        self.inner.docs.call_tool(request, context).await
    }
}

fn service_err(e: ServiceError) -> McpError {
    match e {
        ServiceError::McpError(e) => e,
        other => McpError::internal_error(other.to_string(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_core::GraphNode;

    #[tokio::test]
    async fn warmup_graph_tool_serves_seeded_dag() {
        let proxy = WatchProxy::new();
        // Before seeding, the warmup graph is empty.
        let empty = proxy.warmup_graph_tool(None).await.unwrap();
        let empty_json = serde_json::to_string(&empty).unwrap();
        assert!(!empty_json.contains("nginx"));

        proxy
            .set_warmup_graph(GraphView {
                nodes: vec![GraphNode {
                    id: "n-web".into(),
                    name: "nginx".into(),
                    kind: "shell".into(),
                    machines: vec!["web".into()],
                    ..Default::default()
                }],
                edges: vec![],
            })
            .await;

        // After seeding, `graph` returns the real DAG even with no live child.
        let seeded = proxy.warmup_graph_tool(None).await.unwrap();
        let seeded_json = serde_json::to_string(&seeded).unwrap();
        assert!(seeded_json.contains("nginx"));
    }
}
