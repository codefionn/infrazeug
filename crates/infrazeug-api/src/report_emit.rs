//! Human-readable run output for non-TUI apply/test (`--debug`).

use infrazeug_core::events::SchedEvent;
use infrazeug_core::node::NodeStatus;
use infrazeug_core::report::{RunReport, RunReportEntry};
use tokio::sync::{broadcast, watch};

/// True when argv contains `--debug` or `INFRAZEUG_DEBUG` is set.
pub fn debug_requested() -> bool {
    std::env::var_os("INFRAZEUG_DEBUG").is_some() || std::env::args().any(|a| a == "--debug")
}

/// True when the operator is using the ratatui controller (`--tui` or `--watch`).
///
/// Stderr logging and subprocess output must not be written to the terminal in this
/// mode; use [`SchedEvent`] / the TUI log panel instead.
pub fn terminal_ui_active() -> bool {
    std::env::args().any(|a| a == "--tui" || a == "--watch")
}

/// Print a post-run summary to stderr. Failures are always listed; with `verbose`, every unit is shown.
pub fn print_run_report(report: &RunReport, verbose: bool) {
    let mut ok = 0usize;
    let mut failed = 0usize;
    let mut cancelled = 0usize;
    let mut other = 0usize;
    for e in &report.entries {
        match e.status {
            NodeStatus::Changed | NodeStatus::Unchanged => ok += 1,
            NodeStatus::Failed => failed += 1,
            NodeStatus::Cancelled => cancelled += 1,
            _ => other += 1,
        }
    }

    eprintln!();
    eprintln!("=== run summary ===");
    eprintln!(
        "  ok: {ok}  failed: {failed}  cancelled: {cancelled}  other: {other}  total: {}",
        report.entries.len()
    );

    let print_entry = |label: &str, e: &RunReportEntry| {
        eprintln!(
            "{label} [{:?}] {} ({}) on {}",
            e.status,
            e.node_name,
            short_id(e.node_id),
            e.machine_id
        );
        if let Some(m) = &e.message {
            for line in m.lines() {
                eprintln!("    {line}");
            }
        }
    };

    for e in report
        .entries
        .iter()
        .filter(|e| e.status == NodeStatus::Failed)
    {
        print_entry("FAIL", e);
    }
    for e in report
        .entries
        .iter()
        .filter(|e| e.status == NodeStatus::Cancelled)
    {
        print_entry("CANCEL", e);
    }

    if verbose {
        for e in report
            .entries
            .iter()
            .filter(|e| matches!(e.status, NodeStatus::Changed | NodeStatus::Unchanged))
        {
            print_entry("OK", e);
        }
    }

    eprintln!();
}

pub fn report_has_failures(report: &RunReport) -> bool {
    report
        .entries
        .iter()
        .any(|e| matches!(e.status, NodeStatus::Failed | NodeStatus::Cancelled))
}

/// Stream scheduler events to stderr while apply runs.
pub async fn debug_events_loop(mut events: broadcast::Receiver<SchedEvent>) {
    loop {
        match events.recv().await {
            Ok(ev) => eprintln!("{}", format_sched_event(&ev)),
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("warn: debug log fell behind; skipped {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Stream scheduler events after the controller vault is unlocked.
///
/// Plain CLI apply may prompt for a vault passphrase while transport preparation is
/// already running. Buffering keeps debug logs and subprocess output from
/// interleaving with the hidden passphrase prompt.
pub async fn debug_events_loop_after_unlock(
    mut events: broadcast::Receiver<SchedEvent>,
    mut unlocked: watch::Receiver<bool>,
) {
    let mut passthrough = *unlocked.borrow();
    let mut wait_for_unlock = !passthrough;
    let mut buffered = Vec::new();

    loop {
        tokio::select! {
            changed = unlocked.changed(), if wait_for_unlock => {
                let changed = changed.is_ok();
                if *unlocked.borrow() {
                    passthrough = true;
                    wait_for_unlock = false;
                    for line in buffered.drain(..) {
                        eprintln!("{line}");
                    }
                } else if !changed {
                    wait_for_unlock = false;
                }
            }
            event = events.recv() => {
                let line = match event {
                    Ok(ev) => format_sched_event(&ev),
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        format!("warn: debug log fell behind; skipped {n} events")
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                if passthrough {
                    eprintln!("{line}");
                } else {
                    buffered.push(line);
                }
            }
        }
    }
}

fn format_sched_event(ev: &SchedEvent) -> String {
    use infrazeug_core::events::MachinePreparePhase;
    match ev {
        SchedEvent::PrepareStarted { machine_summaries } => {
            let names: Vec<_> = machine_summaries
                .iter()
                .map(|(_, s)| s.name.as_str())
                .collect();
            format!(
                "prepare started: {} machine(s) [{}]",
                machine_summaries.len(),
                names.join(", ")
            )
        }
        SchedEvent::PrepareGlobal { message } => format!("prepare: {message}"),
        SchedEvent::PrepareMachine {
            machine,
            phase,
            detail,
        } => {
            let phase = match phase {
                MachinePreparePhase::Pending => "pending",
                MachinePreparePhase::ProbingArch => "probing",
                MachinePreparePhase::BuildingAgent => "building",
                MachinePreparePhase::UploadingAgent => "uploading",
                MachinePreparePhase::Connecting => "connecting",
                MachinePreparePhase::Ready => "ready",
                MachinePreparePhase::Skipped { reason } => return format!(
                    "prepare {machine}: skipped ({reason})"
                ),
                MachinePreparePhase::Failed { message } => {
                    return format!("prepare {machine}: failed ({message})");
                }
            };
            match detail {
                Some(d) => format!("prepare {machine}: {phase} — {d}"),
                None => format!("prepare {machine}: {phase}"),
            }
        }
        SchedEvent::PrepareFinished { ok, message } => {
            if *ok {
                "prepare finished".into()
            } else {
                format!(
                    "prepare failed: {}",
                    message.as_deref().unwrap_or("unknown")
                )
            }
        }
        SchedEvent::RunStarted {
            total_units,
            planned_by_machine,
            machine_summaries,
            node_summaries: _,
        } => {
            let by_name: std::collections::HashMap<_, _> = machine_summaries
                .iter()
                .map(|(id, s)| (id, s.name.as_str()))
                .collect();
            let machines: Vec<_> = planned_by_machine
                .iter()
                .map(|(m, n)| {
                    let label = by_name.get(m).copied().unwrap_or("?");
                    format!("{label}:{n}")
                })
                .collect();
            format!("run started: {total_units} units [{machines}]", machines = machines.join(", "))
        }
        SchedEvent::RunFinished {
            total_units,
            succeeded,
            failed,
            cancelled,
        } => format!(
            "run finished: {succeeded} ok, {failed} failed, {cancelled} cancelled ({total_units} units)"
        ),
        SchedEvent::UnitsAdded {
            added_units,
            machine_summaries,
            ..
        } => format!(
            "dynamic fan-out: +{added_units} units across {} machine(s)",
            machine_summaries.len()
        ),
        SchedEvent::NodeQueued { node, machine } => {
            format!("queued {} on {machine}", short_id(*node))
        }
        SchedEvent::NodeStarted { node, machine } => {
            format!("started {} on {machine}", short_id(*node))
        }
        SchedEvent::NodeProgress {
            node,
            machine,
            message,
        } => format!("progress {}@{machine}: {message}", short_id(*node)),
        SchedEvent::NodeOutput {
            node,
            machine,
            stream,
            data,
        } => {
            let label = match stream {
                infrazeug_shell::OutputStream::Stdout => "stdout",
                infrazeug_shell::OutputStream::Stderr => "stderr",
            };
            let text = String::from_utf8_lossy(data).trim_end().to_string();
            format!("{label} {}@{machine}: {text}", short_id(*node))
        }
        SchedEvent::NodeFinished {
            node,
            machine,
            status,
            duration,
        } => format!(
            "finished {} on {machine} {:?} in {:?}",
            short_id(*node),
            status,
            duration
        ),
        SchedEvent::NodeCancelled {
            node,
            machine,
            reason,
        } => format!(
            "cancelled {} on {machine}: {reason}",
            short_id(*node)
        ),
        SchedEvent::MachineMetrics { machine, metrics } => format!(
            "metrics {machine}: cpu {:.0}% mem {}/{} disk {}/{}",
            metrics.cpu_pct,
            metrics.mem_used,
            metrics.mem_total,
            metrics.disk_used,
            metrics.disk_total
        ),
        SchedEvent::PlanWarning { message } => format!("warn: {message}"),
        SchedEvent::NodeRetrying {
            node,
            machine,
            attempt,
            max_attempts,
            ..
        } => format!(
            "retrying {} on {machine} ({attempt}/{max_attempts})",
            short_id(*node)
        ),
        SchedEvent::NodeReconnecting {
            node,
            machine,
            attempt,
            ..
        } => format!(
            "reconnecting {} on {machine} (attempt {attempt})",
            short_id(*node)
        ),
        SchedEvent::NodePolling {
            node,
            machine,
            message,
        } => format!("poll {} on {machine}: {message}", short_id(*node)),
    }
}

fn short_id(id: impl std::fmt::Display) -> String {
    let s = id.to_string();
    s.chars().take(8).collect()
}
