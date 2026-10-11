//! Streamable HTTP transport for `mcp serve --http ADDR`.
//!
//! Security: the HTTP endpoint runs tools with the operator's real transports
//! and credentials (arbitrary `ShellOp`s on managed machines), so a token is
//! required whenever the bind address is reachable from off-host. The token is
//! read from `--http-token-file`/[`TOKEN_ENV`] and sent as
//! `Authorization: Bearer <token>`. Loopback binds may run without a token.

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

/// Env var holding the bearer token required of every HTTP client.
pub const TOKEN_ENV: &str = "INFRAZEUG_MCP_HTTP_TOKEN";

/// Resolve the MCP HTTP bearer token: explicit file path first, then env.
pub fn http_token(token_file: Option<&str>) -> anyhow::Result<Option<String>> {
    if let Some(path) = token_file {
        let token = std::fs::read_to_string(path)
            .with_context(|| format!("read MCP HTTP token file {path}"))?;
        let token = token.trim().to_string();
        if token.is_empty() {
            anyhow::bail!("MCP HTTP token file {path} is empty");
        }
        return Ok(Some(token));
    }
    match std::env::var(TOKEN_ENV) {
        Ok(token) if !token.trim().is_empty() => Ok(Some(token.trim().to_string())),
        _ => Ok(None),
    }
}

pub async fn serve(
    machines: Vec<Machine>,
    graph: GraphView,
    tools: Vec<ToolDef>,
    executor: Arc<dyn InfraExecutor>,
    server_name: String,
    bind: &str,
    token: Option<String>,
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
        token,
        None,
    )
    .await
}

/// Streamable HTTP MCP with a custom handler factory (used by watch-mode proxy).
pub async fn serve_handler<H, F>(
    factory: F,
    bind: &str,
    token: Option<String>,
    cancel: Option<CancellationToken>,
) -> anyhow::Result<()>
where
    H: ServerHandler,
    F: Fn() -> Result<H, std::io::Error> + Send + Sync + 'static,
{
    let addr = parse_bind(bind)?;
    // An unauthenticated execution-capable endpoint must not leave the host.
    if token.is_none() && !addr.ip().is_loopback() {
        anyhow::bail!(
            "refusing to serve MCP HTTP unauthenticated on non-loopback {addr}: \
             set --http-token-file or {TOKEN_ENV} (clients send \
             `Authorization: Bearer <token>`), or bind 127.0.0.1"
        );
    }
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

    let mut router = axum::Router::new().nest_service("/mcp", http_service);
    let authenticated = token.is_some();
    if let Some(token) = token {
        let token = Arc::new(token);
        router = router.layer(axum::middleware::from_fn_with_state(
            token,
            require_bearer,
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind MCP HTTP server on {addr}"))?;
    info!(%addr, authenticated, "MCP Streamable HTTP listening (JSON-RPC at /mcp)");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            shutdown.cancelled_owned().await;
        })
        .await
        .context("MCP HTTP server")?;
    Ok(())
}

/// Reject requests without a matching `Authorization: Bearer <token>` header.
async fn require_bearer(
    axum::extract::State(token): axum::extract::State<Arc<String>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let supplied = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if supplied.is_some_and(|supplied| token_eq(supplied.as_bytes(), token.as_bytes())) {
        return next.run(request).await;
    }
    (
        axum::http::StatusCode::UNAUTHORIZED,
        "missing or invalid bearer token",
    )
        .into_response()
}

/// Constant-time-ish byte comparison (no early exit on first mismatch).
fn token_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
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
