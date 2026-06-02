//! `infra.mcp()` builder: register custom tools, then `serve_stdio()`.

use std::sync::Arc;

use infrazeug_core::{GraphView, Infra, Machine};
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::api_docs;
use crate::ctx::McpCtx;
use crate::exec::InfraExecutor;
use crate::server::InfraServer;

type BuildFn = Arc<dyn Fn(Value, &McpCtx) -> anyhow::Result<Infra> + Send + Sync>;

/// A registered custom tool: typed input → an [`Infra`] that gets executed.
#[derive(Clone)]
pub(crate) struct ToolDef {
    pub name: String,
    pub description: String,
    pub schema: Arc<JsonObject>,
    pub build: BuildFn,
}

/// Builder produced by `infra.mcp()`. Holds the machine catalog snapshot,
/// custom tools, and the injected [`InfraExecutor`].
pub struct McpBuilder {
    machines: Vec<Machine>,
    graph: GraphView,
    tools: Vec<ToolDef>,
    executor: Option<Arc<dyn InfraExecutor>>,
    server_name: String,
}

impl McpBuilder {
    /// Create a builder over a snapshot of the infra's machine catalog.
    pub fn new(machines: Vec<Machine>) -> Self {
        api_docs::warm_in_background();
        Self {
            machines,
            graph: GraphView::default(),
            tools: Vec::new(),
            executor: None,
            server_name: "infrazeug".to_string(),
        }
    }

    /// Attach a planning-graph snapshot, exposed via the `graph` tool.
    pub fn with_graph(mut self, graph: GraphView) -> Self {
        self.graph = graph;
        self
    }

    /// Inject the executor that runs tool-built infras (supplied by the API layer).
    pub fn with_executor(mut self, executor: Arc<dyn InfraExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// Override the server name reported in `initialize`.
    pub fn server_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    /// Register a tool taking typed input `I` whose closure builds an [`Infra`]
    /// to execute. The input JSON schema is derived from `I` via `schemars`.
    ///
    /// The closure receives an [`McpCtx`] so it can pull real, already-configured
    /// machines out of the served infra (`ctx.machine("web")`).
    pub fn tool<I, F>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        f: F,
    ) -> Self
    where
        I: DeserializeOwned + JsonSchema + 'static,
        F: Fn(I, &McpCtx) -> anyhow::Result<Infra> + Send + Sync + 'static,
    {
        let schema = schema_object::<I>();
        let build: BuildFn = Arc::new(move |value, ctx| {
            let input: I = serde_json::from_value(value)
                .map_err(|e| anyhow::anyhow!("invalid arguments: {e}"))?;
            f(input, ctx)
        });
        self.tools.push(ToolDef {
            name: name.into(),
            description: description.into(),
            schema,
            build,
        });
        self
    }

    /// Serve MCP over stdio (JSON-RPC lines on stdin/stdout). Default for `mcp serve`.
    pub async fn serve_stdio(self) -> anyhow::Result<()> {
        self.serve(rmcp::transport::io::stdio()).await
    }

    /// Serve MCP over Streamable HTTP (JSON-RPC) at `bind` (e.g. `127.0.0.1:7777`).
    pub async fn serve_http(self, bind: impl AsRef<str>) -> anyhow::Result<()> {
        let executor = self
            .executor
            .ok_or_else(|| anyhow::anyhow!("no InfraExecutor configured for the MCP server"))?;
        crate::http::serve(
            self.machines,
            self.graph,
            self.tools,
            executor,
            self.server_name,
            bind.as_ref(),
        )
        .await
    }

    /// Serve the MCP protocol over an arbitrary transport. Blocks until the
    /// client disconnects. `serve_stdio` is the common entry point.
    pub async fn serve<T, E, A>(self, transport: T) -> anyhow::Result<()>
    where
        T: rmcp::transport::IntoTransport<rmcp::service::RoleServer, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let executor = self
            .executor
            .ok_or_else(|| anyhow::anyhow!("no InfraExecutor configured for the MCP server"))?;
        let server = InfraServer::new(
            self.machines,
            self.graph,
            self.tools,
            executor,
            self.server_name,
        );
        use rmcp::ServiceExt;
        let running = server.serve(transport).await?;
        running.waiting().await?;
        Ok(())
    }
}

/// Derive a JSON Schema object for `T` suitable for an MCP tool's `inputSchema`.
pub(crate) fn schema_object<T: JsonSchema>() -> Arc<JsonObject> {
    let value = serde_json::to_value(schemars::schema_for!(T)).unwrap_or_default();
    Arc::new(value.as_object().cloned().unwrap_or_default())
}
