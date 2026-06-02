//! Watch playbook sources, rebuild, and restart `mcp serve` (stock `infrazeug` CLI only).
//!
//! A [`WatchProxy`] listens on stdio or `--http` immediately with builtin tool/resource
//! discovery while [`prepare_playbook`] runs, then forwards to the playbook child.

use crate::discover::PlaybookProject;
use crate::run::{
    build_playbook_native, prepare_agents_for_export, release_profile_from_env, run_playbook_probe,
};
use anyhow::Context;
use infrazeug_mcp::WatchProxy;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer};
use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

const DEBOUNCE_MS: u64 = 400;

enum ServeOutcome {
    Restart,
    Done,
}

/// Build on start, watch `src/` + `Cargo.toml`, rebuild and restart MCP on changes.
pub async fn run_mcp_watch(
    project: &PlaybookProject,
    playbook_argv: Vec<OsString>,
) -> anyhow::Result<()> {
    let http_bind = parse_http_bind(&playbook_argv);
    let (change_tx, mut change_rx) = mpsc::unbounded_channel();
    let _watcher = install_source_watcher(&project.manifest_dir, change_tx)?;
    let proxy = WatchProxy::new();

    loop {
        proxy.reset_to_warming().await;
        while change_rx.try_recv().is_ok() {}

        let session_cancel = CancellationToken::new();
        let proxy_serve = proxy.clone();
        let serve_bind = http_bind.clone();
        let serve_cancel = session_cancel.clone();
        let mut serve_task = tokio::spawn(async move {
            if let Some(bind) = serve_bind {
                proxy_serve.serve_http(&bind, serve_cancel).await
            } else {
                proxy_serve.serve_stdio(serve_cancel).await
            }
        });

        eprintln!("[infrazeug] MCP listening — building playbook…");
        let child_http_port = if http_bind.is_some() {
            Some(reserve_local_port().await?)
        } else {
            None
        };
        let child_argv = child_playbook_argv(&playbook_argv, child_http_port);

        let release = release_profile_from_env();
        let binary = build_playbook_native(project, release).await?;

        // Offline probe → planning DAG (no SSH). Seed the proxy so the `graph`
        // tool works immediately, before — and regardless of — the agent build.
        let export = run_playbook_probe(&binary).await?;
        proxy.set_warmup_graph(export.graph.clone()).await;
        while change_rx.try_recv().is_ok() {}

        // Cross-build agents / SSH-probe remotes. This can hang or fail when hosts
        // are unreachable; it must not take down graph/doc serving, so failure
        // keeps the warmup proxy up and waits for the next source change.
        if let Err(e) = prepare_agents_for_export(&export, release).await {
            eprintln!("[infrazeug] agent preparation failed: {e:#}");
            eprintln!(
                "[infrazeug] serving graph + API docs only — fix hosts/agents and \
                 edit a source file to retry"
            );
            match wait_for_change(&mut change_rx, &mut serve_task).await? {
                ServeOutcome::Restart => {
                    session_cancel.cancel();
                    let _ = serve_task.await;
                    continue;
                }
                // The proxy already stopped; nothing left to wind down.
                ServeOutcome::Done => return Ok(()),
            }
        }

        info!(path = %binary.display(), "starting MCP playbook");
        let mut child = spawn_mcp_child(&binary, &child_argv)?;
        if let Some(port) = child_http_port {
            let url = format!("http://127.0.0.1:{port}");
            proxy.set_live_http(&url).await?;
            eprintln!(
                "[infrazeug] MCP server ready ({}) — watching {} for changes",
                binary.display(),
                project.manifest_dir.display()
            );
        } else {
            proxy.set_live_stdio(&mut child).await?;
            eprintln!(
                "[infrazeug] MCP server ready (stdio) — watching {} for changes",
                project.manifest_dir.display()
            );
        }

        let outcome = supervise_session(&mut child, &mut change_rx, &mut serve_task).await?;
        session_cancel.cancel();
        let _ = serve_task.await;

        match outcome {
            ServeOutcome::Restart => {
                eprintln!("[infrazeug] playbook changed — rebuilding MCP server…");
                stop_child(&mut child).await;
                while change_rx.try_recv().is_ok() {}
            }
            ServeOutcome::Done => {
                stop_child(&mut child).await;
                return Ok(());
            }
        }
    }
}

async fn supervise_session(
    child: &mut Child,
    change_rx: &mut mpsc::UnboundedReceiver<()>,
    serve_task: &mut tokio::task::JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<ServeOutcome> {
    tokio::select! {
        _ = change_rx.recv() => Ok(ServeOutcome::Restart),
        status = child.wait() => {
            let status = status.context("wait for MCP playbook")?;
            if status.success() {
                Ok(ServeOutcome::Done)
            } else {
                anyhow::bail!("MCP playbook exited with {status}");
            }
        }
        serve_result = serve_task => {
            serve_result.context("MCP proxy task join")??;
            wait_child_or_change(child, change_rx).await
        }
    }
}

/// Wait for a source change or proxy shutdown without a live playbook child
/// (used when the build succeeded but agent preparation failed).
async fn wait_for_change(
    change_rx: &mut mpsc::UnboundedReceiver<()>,
    serve_task: &mut tokio::task::JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<ServeOutcome> {
    tokio::select! {
        _ = change_rx.recv() => Ok(ServeOutcome::Restart),
        serve_result = serve_task => {
            serve_result.context("MCP proxy task join")??;
            Ok(ServeOutcome::Done)
        }
    }
}

async fn wait_child_or_change(
    child: &mut Child,
    change_rx: &mut mpsc::UnboundedReceiver<()>,
) -> anyhow::Result<ServeOutcome> {
    tokio::select! {
        _ = change_rx.recv() => Ok(ServeOutcome::Restart),
        status = child.wait() => {
            let status = status.context("wait for MCP playbook")?;
            if status.success() {
                Ok(ServeOutcome::Done)
            } else {
                anyhow::bail!("MCP playbook exited with {status}");
            }
        }
    }
}

fn parse_http_bind(argv: &[OsString]) -> Option<String> {
    let mut iter = argv.iter();
    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();
        if s == "--http" {
            return iter.next().map(|a| a.to_string_lossy().into_owned());
        }
        if let Some(addr) = s.strip_prefix("--http=") {
            return Some(addr.to_string());
        }
    }
    None
}

fn child_playbook_argv(playbook_argv: &[OsString], child_http_port: Option<u16>) -> Vec<OsString> {
    let Some(port) = child_http_port else {
        return playbook_argv.to_vec();
    };
    let internal = format!("127.0.0.1:{port}");
    let mut out = Vec::with_capacity(playbook_argv.len());
    let mut iter = playbook_argv.iter();
    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();
        if s == "--http" {
            out.push(OsString::from("--http"));
            let _ = iter.next();
            out.push(OsString::from(&internal));
            continue;
        }
        if s.strip_prefix("--http=").is_some() {
            out.push(OsString::from(format!("--http={internal}")));
            continue;
        }
        out.push(arg.clone());
    }
    out
}

async fn reserve_local_port() -> anyhow::Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .context("reserve local port for playbook MCP")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn spawn_mcp_child(binary: &Path, argv: &[OsString]) -> anyhow::Result<Child> {
    Command::new(binary)
        .args(argv)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))
}

async fn stop_child(child: &mut Child) {
    if let Err(e) = child.start_kill() {
        warn!(%e, "kill MCP playbook");
    }
    let _ = child.wait().await;
}

fn install_source_watcher(
    manifest_dir: &Path,
    change_tx: mpsc::UnboundedSender<()>,
) -> anyhow::Result<Debouncer<notify::RecommendedWatcher>> {
    let manifest_dir = manifest_dir.to_path_buf();
    let mut debouncer = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            if events.iter().any(|e| is_source_change(&e.path)) {
                let _ = change_tx.send(());
            }
        },
    )
    .context("create file watcher")?;

    let src = manifest_dir.join("src");
    if src.is_dir() {
        debouncer
            .watcher()
            .watch(&src, RecursiveMode::Recursive)
            .context("watch src/")?;
    }
    for name in ["Cargo.toml", "build.rs"] {
        let path = manifest_dir.join(name);
        if path.is_file() {
            debouncer
                .watcher()
                .watch(&path, RecursiveMode::NonRecursive)
                .context("watch manifest")?;
        }
    }
    Ok(debouncer)
}

fn is_source_change(path: &Path) -> bool {
    let s = path.to_string_lossy();
    !s.contains("/target/") && !s.contains("\\target\\")
}
