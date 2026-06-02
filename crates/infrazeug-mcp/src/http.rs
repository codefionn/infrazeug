//! Streamable HTTP transport for `mcp serve --http ADDR`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::ServerHandler;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::builder::ToolDef;
use crate::exec::InfraExecutor;
use crate::server::InfraServer;
use infrazeug_core::{GraphView, Machine};

pub async fn serve(
    machines: Vec<Machine>,
    graph: GraphView,
    tools: Vec<ToolDef>,
    executor: Arc<dyn InfraExecutor>,
    server_name: String,
    bind: &str,
) -> anyhow::Result<()> {
    let state = Arc::new(ServerState {
        machines,
        graph,
        tools,
        executor,
        server_name,
    });
    serve_handler(
        move || {
            Ok(InfraServer::new(
                state.machines.clone(),
                state.graph.clone(),
                state.tools.clone(),
                Arc::clone(&state.executor),
                state.server_name.clone(),
            ))
        },
        bind,
        None,
    )
    .await
}

/// Streamable HTTP MCP with a custom handler factory (used by watch-mode proxy).
pub async fn serve_handler<H, F>(
    factory: F,
    bind: &str,
    cancel: Option<CancellationToken>,
) -> anyhow::Result<()>
where
    H: ServerHandler,
    F: Fn() -> Result<H, std::io::Error> + Send + Sync + 'static,
{
    let addr = parse_bind(bind)?;
    let mut http_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None);
    if addr.ip().is_unspecified() {
        http_config = http_config.disable_allowed_hosts();
    } else {
        let host = addr.ip().to_string();
        http_config =
            http_config.with_allowed_hosts(["localhost", "127.0.0.1", "::1", host.as_str(), bind]);
    }
    if let Some(ct) = cancel {
        http_config = http_config.with_cancellation_token(ct);
    }
    let shutdown = http_config.cancellation_token.clone();

    let http_service = StreamableHttpService::new(
        factory,
        Arc::new(LocalSessionManager::default()),
        http_config,
    );

    let router = axum::Router::new().nest_service("/mcp", http_service);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind MCP HTTP server on {addr}"))?;
    info!(%addr, "MCP Streamable HTTP listening (JSON-RPC at /mcp)");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown.cancelled_owned().await;
        })
        .await
        .context("MCP HTTP server")?;
    Ok(())
}

#[derive(Clone)]
struct ServerState {
    machines: Vec<Machine>,
    graph: GraphView,
    tools: Vec<ToolDef>,
    executor: Arc<dyn InfraExecutor>,
    server_name: String,
}

fn parse_bind(bind: &str) -> anyhow::Result<SocketAddr> {
    let trimmed = bind.trim();
    if trimmed.contains(':') || trimmed.starts_with('[') {
        trimmed.parse().context("parse --http listen address")
    } else {
        format!("{trimmed}:7777")
            .parse()
            .with_context(|| format!("parse --http listen address `{trimmed}` (default port 7777)"))
    }
}
