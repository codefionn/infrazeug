//! Interactive controller TUI (SOUL §6ter).
//!
//! ratatui front-end for `infrazeug apply --tui` and `--watch`: machine list,
//! machine-info pane, node-detail pane, scrolling event log, and operator
//! commands (pause/resume, cancel, replay) sent on the scheduler
//! [`SchedCommand`](infrazeug_core::SchedCommand) channel.
//!
//! Prompts follow locked modal rules: [`Interaction::UnlockDataKey`](infrazeug_core::Interaction::UnlockDataKey)
//! blocks the UI; approval prompts are non-modal overlays. [`TuiInteractor`]
//! implements [`Interactor`](infrazeug_core::Interactor) for the apply pipeline.

mod interactor;

use anyhow::Result;
use broadcast::error::TryRecvError;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use infrazeug_core::events::{MachineMetrics, MachinePreparePhase, SchedCommand, SchedEvent};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::interactor::{Interaction, InteractionResp};
use infrazeug_core::machine::MachineSummary;
use infrazeug_core::node::{NodeStatus, NodeSummary};
use infrazeug_core::short_id_prefix;
use infrazeug_core::{CoreError, OutputStream};
use interactor::PendingInteraction;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::DefaultTerminal;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

pub use interactor::{PendingInteraction as TuiPendingInteraction, TuiInteractor};

/// How many recent stdout/progress lines to keep per machine for the detail pane.
const DETAIL_LINES: usize = 12;

/// Grace before an in-flight node is hard-killed on `c` (§6ter.5).
const CANCEL_GRACE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct MachineRow {
    summary: Option<MachineSummary>,
    done: usize,
    total: usize,
    failed: usize,
    running: bool,
    /// Node currently executing on this machine, if any.
    current: Option<NodeId>,
    /// Most recently finished node on this machine (replay target).
    last_finished: Option<NodeId>,
    /// Recent progress/stdout lines for the node-detail pane.
    detail: Vec<String>,
    /// Transport bootstrap phase (cleared when apply units start).
    prepare: Option<MachinePreparePhase>,
    prepare_detail: Option<String>,
    /// Latest resource-usage sample pushed by the push-mode agent, if any.
    metrics: Option<MachineMetrics>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RunPhase {
    Preparing,
    Applying,
    Done,
}

enum ModalState {
    None,
    Unlock {
        name: String,
        input: String,
        pending: PendingInteraction,
    },
    /// Ctrl-C global cancel confirmation (§6ter.5).
    ConfirmQuit,
}

enum PromptState {
    ApproveVar {
        var: String,
        pending: PendingInteraction,
    },
    ConfirmDestructive {
        summary: String,
        pending: PendingInteraction,
    },
    ResolveBecome {
        options: Vec<String>,
        pick: usize,
        pending: PendingInteraction,
    },
}

/// All mutable UI state for the controller loop.
struct App {
    machines: HashMap<MachineId, MachineRow>,
    /// Playbook node labels (seeded from [`SchedEvent::RunStarted`]).
    nodes: HashMap<NodeId, NodeSummary>,
    /// Insertion order of machine ids, for stable selection.
    order: Vec<MachineId>,
    selected: usize,
    log: Vec<String>,
    modal: ModalState,
    prompt: Option<PromptState>,
    paused: bool,
    /// Active substring filter on machines + events (`/` to edit).
    filter: String,
    /// True while typing into the filter box.
    filter_edit: bool,
    quit: bool,
    phase: RunPhase,
    prepare_global: Option<String>,
    prepare_error: Option<String>,
    run_finished: bool,
    run_summary: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            machines: HashMap::new(),
            nodes: HashMap::new(),
            order: Vec::new(),
            selected: 0,
            log: Vec::new(),
            modal: ModalState::None,
            prompt: None,
            paused: false,
            filter: String::new(),
            filter_edit: false,
            quit: false,
            phase: RunPhase::Preparing,
            prepare_global: None,
            prepare_error: None,
            run_finished: false,
            run_summary: None,
        }
    }

    fn prepare_ready_count(&self) -> usize {
        self.machines
            .values()
            .filter(|r| prepare_is_ready(r.prepare.as_ref()))
            .count()
    }

    fn in_prepare(&self) -> bool {
        self.phase == RunPhase::Preparing && self.prepare_error.is_none()
    }

    fn selected_machine(&self) -> Option<MachineId> {
        self.order.get(self.selected).copied()
    }

    fn row(&mut self, id: MachineId) -> &mut MachineRow {
        if !self.machines.contains_key(&id) {
            self.order.push(id);
        }
        self.machines.entry(id).or_default()
    }

    fn node_display_name(&self, id: NodeId) -> String {
        self.nodes
            .get(&id)
            .map(|s| s.display_name(id))
            .unwrap_or_else(|| short_id_prefix(id))
    }
}

fn prepare_is_ready(phase: Option<&MachinePreparePhase>) -> bool {
    matches!(
        phase,
        Some(MachinePreparePhase::Ready) | Some(MachinePreparePhase::Skipped { .. })
    )
}

fn stream_label(stream: OutputStream) -> &'static str {
    match stream {
        OutputStream::Stdout => "stdout",
        OutputStream::Stderr => "stderr",
    }
}

fn output_display_lines(data: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(data);
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect();
    if lines.is_empty() && !data.is_empty() {
        lines.push(text.trim_end_matches(['\r', '\n']).to_string());
    }
    lines
}

fn prepare_phase_label(phase: &MachinePreparePhase) -> &'static str {
    match phase {
        MachinePreparePhase::Pending => "pending",
        MachinePreparePhase::ProbingArch => "probing arch",
        MachinePreparePhase::BuildingAgent => "building agent",
        MachinePreparePhase::UploadingAgent => "uploading agent",
        MachinePreparePhase::Connecting => "connecting",
        MachinePreparePhase::Ready => "ready",
        MachinePreparePhase::Skipped { .. } => "skipped",
        MachinePreparePhase::Failed { .. } => "failed",
    }
}

fn prepare_phase_icon(phase: Option<&MachinePreparePhase>) -> &'static str {
    match phase {
        None => "·",
        Some(MachinePreparePhase::Pending) => "○",
        Some(MachinePreparePhase::ProbingArch) => "◑",
        Some(MachinePreparePhase::BuildingAgent) => "⚙",
        Some(MachinePreparePhase::UploadingAgent) => "↑",
        Some(MachinePreparePhase::Connecting) => "⚡",
        Some(MachinePreparePhase::Ready) => "✓",
        Some(MachinePreparePhase::Skipped { .. }) => "−",
        Some(MachinePreparePhase::Failed { .. }) => "✖",
    }
}

pub async fn run_controller(
    events: broadcast::Receiver<SchedEvent>,
    watch_only: bool,
    prompts: Option<mpsc::UnboundedReceiver<PendingInteraction>>,
    commands: Option<mpsc::Sender<SchedCommand>>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        run_controller_blocking(events, watch_only, prompts, commands)
    })
    .await
    .map_err(|e| anyhow::anyhow!("tui thread panicked or was cancelled: {e}"))?
}

/// Blocking controller loop (crossterm/ratatui); runs off the async runtime.
fn run_controller_blocking(
    mut events: broadcast::Receiver<SchedEvent>,
    watch_only: bool,
    mut prompts: Option<mpsc::UnboundedReceiver<PendingInteraction>>,
    commands: Option<mpsc::Sender<SchedCommand>>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let result = run_loop(
        &mut terminal,
        &mut events,
        watch_only,
        prompts.as_mut(),
        commands.as_ref(),
    );
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    events: &mut broadcast::Receiver<SchedEvent>,
    watch_only: bool,
    mut prompts: Option<&mut mpsc::UnboundedReceiver<PendingInteraction>>,
    commands: Option<&mpsc::Sender<SchedCommand>>,
) -> Result<()> {
    let mut app = App::new();

    while !app.quit {
        drain_events(&mut app, events);

        if let Some(rx) = prompts.as_mut() {
            while let Ok(p) = rx.try_recv() {
                route_prompt(&mut app, watch_only, p);
            }
        }

        terminal.draw(|frame| draw_ui(frame, &app))?;

        let mut redraw = false;
        while event::poll(Duration::ZERO)? {
            redraw |= handle_crossterm_event(&mut app, event::read()?, commands);
        }
        if redraw {
            continue;
        }

        if event::poll(Duration::from_millis(100))?
            && handle_crossterm_event(&mut app, event::read()?, commands)
        {
            continue;
        }
    }

    Ok(())
}

/// Handle one crossterm event. Returns true when the frame should be redrawn immediately.
fn handle_crossterm_event(
    app: &mut App,
    ev: Event,
    commands: Option<&mpsc::Sender<SchedCommand>>,
) -> bool {
    match ev {
        Event::Key(key) => {
            handle_key(app, key, commands);
            false
        }
        Event::Resize(_, _) => true,
        _ => false,
    }
}

/// Route an incoming `Interaction` into the modal slot or the non-modal prompt slot.
fn drain_events(app: &mut App, events: &mut broadcast::Receiver<SchedEvent>) {
    loop {
        match events.try_recv() {
            Ok(ev) => ingest_event(app, ev),
            Err(TryRecvError::Lagged(n)) => {
                app.log
                    .push(format!("warn: TUI fell behind; skipped {n} events"));
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Closed) => {
                // Sender dropped when apply finishes; stay open until the operator quits.
                app.run_finished = true;
                break;
            }
        }
    }
}

fn route_prompt(app: &mut App, watch_only: bool, p: PendingInteraction) {
    if watch_only {
        p.respond(Err(CoreError::InteractionDenied(
            "watch mode is read-only".into(),
        )));
        return;
    }
    match p.req {
        Interaction::UnlockDataKey { ref name, .. } => {
            app.log.push(format!("UnlockDataKey: {name}"));
            app.modal = ModalState::Unlock {
                name: format!("Unlock data key {name:?}"),
                input: String::new(),
                pending: p,
            };
        }
        Interaction::SshAuthSecret {
            key_passphrase,
            ref hint,
            ..
        } => {
            let what = if key_passphrase {
                "SSH key passphrase"
            } else {
                "SSH password"
            };
            let label = match hint {
                Some(h) => format!("{what} ({h})"),
                None => what.to_string(),
            };
            app.log.push(label.clone());
            app.modal = ModalState::Unlock {
                name: label,
                input: String::new(),
                pending: p,
            };
        }
        Interaction::ApproveVarRequest { ref var, .. } => {
            app.log.push(format!("ApproveVarRequest: {var}"));
            app.prompt = Some(PromptState::ApproveVar {
                var: var.to_string(),
                pending: p,
            });
        }
        Interaction::ConfirmDestructive { ref summary, .. } => {
            app.log.push(format!("ConfirmDestructive: {summary}"));
            app.prompt = Some(PromptState::ConfirmDestructive {
                summary: summary.clone(),
                pending: p,
            });
        }
        Interaction::ResolveBecomeConflict { ref options, .. } => {
            app.log.push("ResolveBecomeConflict".into());
            app.prompt = Some(PromptState::ResolveBecome {
                options: options.clone(),
                pick: 0,
                pending: p,
            });
        }
        Interaction::SignPlan {
            ref plan_digest, ..
        } => {
            app.log.push(format!("SignPlan: {plan_digest}"));
            p.respond(Ok(InteractionResp::Approve));
        }
    }
}

fn send_cmd(commands: Option<&mpsc::Sender<SchedCommand>>, cmd: SchedCommand) {
    if let Some(tx) = commands {
        let _ = tx.try_send(cmd);
    }
}

fn handle_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    commands: Option<&mpsc::Sender<SchedCommand>>,
) {
    // 1. Modal handling takes precedence (blocks everything else).
    match &mut app.modal {
        ModalState::Unlock { .. } => {
            handle_unlock_key(app, key);
            return;
        }
        ModalState::ConfirmQuit => {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // Cascade cancel across every machine, then leave the TUI.
                    for id in app.order.clone() {
                        send_cmd(commands, SchedCommand::CancelMachine { machine: id });
                    }
                    app.quit = true;
                }
                _ => app.modal = ModalState::None,
            }
            return;
        }
        ModalState::None => {}
    }

    // 2. Filter editing captures text input.
    if app.filter_edit {
        match key.code {
            KeyCode::Enter => {
                app.filter_edit = false;
                // Mirror the visual filter to viewers over the command channel
                // (§6ter.7); execution is unaffected.
                send_cmd(
                    commands,
                    SchedCommand::FilterChange {
                        selector: app.filter.clone(),
                    },
                );
            }
            KeyCode::Esc => app.filter_edit = false,
            KeyCode::Char(c) => app.filter.push(c),
            KeyCode::Backspace => {
                app.filter.pop();
            }
            _ => {}
        }
        return;
    }

    // 3. Non-modal prompt answering.
    if let Some(ps) = app.prompt.take() {
        handle_prompt_key(app, ps, key);
        return;
    }

    // 4. Global controls.
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.modal = ModalState::ConfirmQuit;
        }
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Char('j') | KeyCode::Down if !app.order.is_empty() => {
            app.selected = (app.selected + 1).min(app.order.len() - 1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.selected = app.selected.saturating_sub(1);
        }
        KeyCode::Char('p') => {
            app.paused = !app.paused;
            send_cmd(
                commands,
                if app.paused {
                    SchedCommand::PauseAll
                } else {
                    SchedCommand::ResumeAll
                },
            );
            app.log
                .push(if app.paused { "paused" } else { "resumed" }.into());
        }
        KeyCode::Char('c') => {
            // Focused node: cancel the unit running on the selected machine with
            // a grace window (§6ter.5). Falls back to a machine cancel if idle.
            if let Some(id) = app.selected_machine() {
                match app.machines.get(&id).and_then(|r| r.current) {
                    Some(node) => {
                        send_cmd(
                            commands,
                            SchedCommand::CancelNode {
                                node,
                                machine: id,
                                grace: CANCEL_GRACE,
                            },
                        );
                        app.log
                            .push(format!("cancel {} on {id}", app.node_display_name(node)));
                    }
                    None => {
                        send_cmd(commands, SchedCommand::CancelMachine { machine: id });
                        app.log.push(format!("cancel machine {id}"));
                    }
                }
            }
        }
        KeyCode::Char('C') => {
            if let Some(id) = app.selected_machine() {
                send_cmd(commands, SchedCommand::CancelMachine { machine: id });
                app.log.push(format!("cancel machine {id}"));
            }
        }
        KeyCode::Char('r') => {
            if let Some(id) = app.selected_machine() {
                let node = app.machines.get(&id).and_then(|r| r.last_finished);
                if let Some(node) = node {
                    send_cmd(commands, SchedCommand::ReplayNode { node, machine: id });
                    app.log
                        .push(format!("replay {} on {id}", app.node_display_name(node)));
                }
            }
        }
        KeyCode::Char('/') => {
            app.filter.clear();
            app.filter_edit = true;
        }
        _ => {}
    }
}

fn handle_unlock_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if let ModalState::Unlock { input, pending, .. } =
                std::mem::replace(&mut app.modal, ModalState::None)
            {
                pending.respond(Ok(InteractionResp::Passphrase(input)));
                app.log.push("data key unlocked".into());
            }
        }
        KeyCode::Esc => {
            if let ModalState::Unlock { pending, .. } =
                std::mem::replace(&mut app.modal, ModalState::None)
            {
                pending.respond(Ok(InteractionResp::Cancel));
            }
        }
        KeyCode::Char(c) => {
            if let ModalState::Unlock { ref mut input, .. } = app.modal {
                input.push(c);
            }
        }
        KeyCode::Backspace => {
            if let ModalState::Unlock { ref mut input, .. } = app.modal {
                input.pop();
            }
        }
        _ => {}
    }
}

fn handle_prompt_key(app: &mut App, ps: PromptState, key: crossterm::event::KeyEvent) {
    match ps {
        PromptState::ApproveVar { var, pending } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                pending.respond(Ok(InteractionResp::Approve));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                pending.respond(Ok(InteractionResp::Deny));
            }
            _ => app.prompt = Some(PromptState::ApproveVar { var, pending }),
        },
        PromptState::ConfirmDestructive { summary, pending } => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                pending.respond(Ok(InteractionResp::Approve));
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                pending.respond(Ok(InteractionResp::Deny));
            }
            _ => app.prompt = Some(PromptState::ConfirmDestructive { summary, pending }),
        },
        PromptState::ResolveBecome {
            mut pick,
            options,
            pending,
        } => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                pick = pick.saturating_sub(1);
                app.prompt = Some(PromptState::ResolveBecome {
                    options,
                    pick,
                    pending,
                });
            }
            KeyCode::Down | KeyCode::Char('j') => {
                pick = (pick + 1).min(options.len().saturating_sub(1));
                app.prompt = Some(PromptState::ResolveBecome {
                    options,
                    pick,
                    pending,
                });
            }
            KeyCode::Enter => pending.respond(Ok(InteractionResp::Pick(pick))),
            KeyCode::Esc => pending.respond(Ok(InteractionResp::Cancel)),
            _ => {
                app.prompt = Some(PromptState::ResolveBecome {
                    options,
                    pick,
                    pending,
                })
            }
        },
    }
}

fn ingest_event(app: &mut App, ev: SchedEvent) {
    match ev {
        SchedEvent::PrepareStarted { machine_summaries } => {
            app.phase = RunPhase::Preparing;
            app.log.push("prepare: starting transport bootstrap".into());
            for (machine, summary) in machine_summaries {
                let row = app.row(machine);
                row.summary = Some(summary);
                row.prepare = Some(MachinePreparePhase::Pending);
            }
        }
        SchedEvent::PrepareGlobal { message } => {
            app.prepare_global = Some(message.clone());
            app.log.push(format!("prepare: {message}"));
        }
        SchedEvent::PrepareMachine {
            machine,
            phase,
            detail,
        } => {
            let row = app.row(machine);
            row.prepare = Some(phase.clone());
            row.prepare_detail = detail.clone();
            let label = machine_list_label(machine, row);
            let phase_label = prepare_phase_label(&phase);
            if let Some(d) = detail {
                app.log
                    .push(format!("prepare {label}: {phase_label} — {d}"));
            } else {
                app.log.push(format!("prepare {label}: {phase_label}"));
            }
            // Lazy machines connect mid-run; once their transport is ready,
            // return the panel to the units/metrics view instead of pinning
            // the bootstrap phase forever.
            if app.phase == RunPhase::Applying && matches!(phase, MachinePreparePhase::Ready) {
                let row = app.row(machine);
                row.prepare = None;
                row.prepare_detail = None;
            }
        }
        SchedEvent::PrepareFinished { ok, message } => {
            if ok {
                app.log.push("prepare: transports ready".into());
                app.prepare_global = Some("starting apply".into());
            } else {
                app.prepare_error = message.clone();
                app.run_finished = true;
                app.run_summary = message.clone();
                app.log.push(format!(
                    "prepare failed: {}",
                    message.as_deref().unwrap_or("unknown error")
                ));
            }
        }
        SchedEvent::RunStarted {
            total_units,
            planned_by_machine,
            machine_summaries,
            node_summaries,
        } => {
            app.phase = RunPhase::Applying;
            app.prepare_global = None;
            for row in app.machines.values_mut() {
                row.prepare = None;
                row.prepare_detail = None;
            }
            app.log
                .push(format!("run started: {total_units} units scheduled"));
            for (node_id, summary) in node_summaries {
                app.nodes.insert(node_id, summary);
            }
            for (machine, summary) in machine_summaries {
                let row = app.row(machine);
                row.summary = Some(summary);
            }
            for (machine, total) in planned_by_machine {
                let row = app.row(machine);
                row.total = total;
                row.done = 0;
                row.failed = 0;
                row.running = false;
                row.current = None;
            }
        }
        SchedEvent::RunFinished {
            total_units,
            succeeded,
            failed,
            cancelled,
        } => {
            app.phase = RunPhase::Done;
            app.run_finished = true;
            app.run_summary = Some(format!(
                "{succeeded} ok, {failed} failed, {cancelled} cancelled ({total_units} units)"
            ));
            app.log.push(format!(
                "run finished: {succeeded} ok, {failed} failed, {cancelled} cancelled"
            ));
            for row in app.machines.values_mut() {
                row.running = false;
                row.current = None;
            }
        }
        SchedEvent::UnitsAdded {
            added_units,
            planned_by_machine,
            machine_summaries,
            node_summaries,
        } => {
            app.log.push(format!(
                "discovered {added_units} new units (dynamic fan-out)"
            ));
            for (node_id, summary) in node_summaries {
                app.nodes.insert(node_id, summary);
            }
            for (machine, summary) in machine_summaries {
                let row = app.row(machine);
                row.summary = Some(summary);
            }
            for (machine, total) in planned_by_machine {
                app.row(machine).total += total;
            }
        }
        SchedEvent::NodeQueued { node, machine } => {
            let row = app.row(machine);
            if row.total == 0 {
                row.total += 1;
            }
            row.running = true;
            app.log.push(format!(
                "queued {} on {}",
                app.node_display_name(node),
                machine
            ));
        }
        SchedEvent::NodeStarted { node, machine } => {
            let row = app.row(machine);
            row.running = true;
            row.current = Some(node);
            row.detail.clear();
            app.log.push(format!(
                "started {} on {}",
                app.node_display_name(node),
                machine
            ));
        }
        SchedEvent::NodeProgress {
            node,
            machine,
            message,
        } => {
            let row = app.row(machine);
            row.detail.push(message.clone());
            if row.detail.len() > DETAIL_LINES {
                let drop = row.detail.len() - DETAIL_LINES;
                row.detail.drain(0..drop);
            }
            app.log.push(format!(
                "progress {}@{}: {}",
                app.node_display_name(node),
                machine,
                message
            ));
        }
        SchedEvent::NodeOutput {
            node,
            machine,
            stream,
            data,
        } => {
            let label = stream_label(stream);
            for line in output_display_lines(&data) {
                let detail = format!("{label}: {line}");
                let row = app.row(machine);
                row.detail.push(detail.clone());
                if row.detail.len() > DETAIL_LINES {
                    let drop = row.detail.len() - DETAIL_LINES;
                    row.detail.drain(0..drop);
                }
                app.log.push(format!(
                    "{label} {}@{}: {}",
                    app.node_display_name(node),
                    machine,
                    line
                ));
            }
        }
        SchedEvent::NodeFinished {
            node,
            machine,
            status,
            duration,
        } => {
            let row = app.row(machine);
            row.running = false;
            row.done += 1;
            row.current = None;
            row.last_finished = Some(node);
            if status == NodeStatus::Failed {
                row.failed += 1;
            }
            app.log.push(format!(
                "finished {} on {} {:?} in {:?}",
                app.node_display_name(node),
                machine,
                status,
                duration
            ));
        }
        SchedEvent::NodeCancelled {
            node,
            machine,
            reason,
        } => {
            app.log.push(format!(
                "cancelled {} on {}: {}",
                app.node_display_name(node),
                machine,
                reason
            ));
        }
        SchedEvent::MachineMetrics { machine, metrics } => {
            // High-frequency; update the row in place, no event-log spam.
            app.row(machine).metrics = Some(metrics);
        }
        SchedEvent::PlanWarning { message } => {
            app.log.push(format!("warn: {}", message));
        }
        SchedEvent::NodeRetrying {
            node,
            machine,
            attempt,
            max_attempts,
            message,
        } => {
            app.log.push(format!(
                "retrying {} on {} ({}/{})",
                app.node_display_name(node),
                machine,
                attempt,
                max_attempts,
            ));
            let _ = message;
        }
        SchedEvent::NodeReconnecting {
            node,
            machine,
            attempt,
            message,
        } => {
            app.log.push(format!(
                "reconnecting {} on {} (attempt {})",
                app.node_display_name(node),
                machine,
                attempt,
            ));
            let _ = message;
        }
        SchedEvent::NodePolling {
            node,
            machine,
            message,
        } => {
            app.log.push(format!(
                "poll {} on {}: {}",
                app.node_display_name(node),
                machine,
                message,
            ));
        }
    }
    if app.log.len() > 200 {
        app.log.drain(0..50);
    }
}

/// Compact list label: playbook name, else SSH host, else short uuid.
fn machine_list_label(id: MachineId, row: &MachineRow) -> String {
    if let Some(s) = row.summary.as_ref() {
        if !s.name.is_empty() {
            return s.name.clone();
        }
        if s.kind == "remote" {
            return s.endpoint.clone();
        }
    }
    short_id_prefix(id)
}

/// Machine info pane: identity on the left, id + runtime state on the right.
fn machine_panel_columns(id: MachineId, row: &MachineRow) -> (Vec<String>, Vec<String>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    if let Some(s) = &row.summary {
        left.push(format!("name: {}", s.name));
        left.push(format!("host: {}", s.endpoint));
        left.push(format!("kind: {}", s.kind));
        if let Some(os) = &s.os_hint {
            left.push(format!("os: {os}"));
        }
    } else {
        left.push("name: (unknown)".into());
    }
    right.push(format!("id: {id}"));
    if let Some(ref phase) = row.prepare {
        right.push(format!("transport: {}", prepare_phase_label(phase)));
        if let Some(d) = &row.prepare_detail {
            right.push(format!("detail: {d}"));
        }
        if let MachinePreparePhase::Skipped { reason } = phase {
            right.push(format!("reason: {reason}"));
        }
        if let MachinePreparePhase::Failed { message } = phase {
            right.push(format!("error: {message}"));
        }
    } else {
        right.push(format!(
            "units: {}/{}  failed {}",
            row.done,
            row.total.max(1),
            row.failed
        ));
    }
    if let Some(m) = row.metrics {
        right.push(format!(
            "cpu {} {:>3.0}%",
            meter(m.cpu_pct / 100.0),
            m.cpu_pct
        ));
        let mem_frac = frac(m.mem_used, m.mem_total);
        right.push(format!(
            "mem {} {}/{}",
            meter(mem_frac),
            fmt_bytes(m.mem_used),
            fmt_bytes(m.mem_total)
        ));
        let disk_frac = frac(m.disk_used, m.disk_total);
        right.push(format!(
            "dsk {} {}/{}",
            meter(disk_frac),
            fmt_bytes(m.disk_used),
            fmt_bytes(m.disk_total)
        ));
    }
    (left, right)
}

/// Fraction `used/total` in 0.0..=1.0, guarding a zero/unknown total.
fn frac(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// Render a fixed-width unicode bar for a 0.0..=1.0 fraction.
fn meter(frac: f32) -> String {
    const WIDTH: usize = 10;
    let filled = (frac.clamp(0.0, 1.0) * WIDTH as f32).round() as usize;
    let mut s = String::with_capacity(WIDTH + 2);
    s.push('[');
    for i in 0..WIDTH {
        s.push(if i < filled { '█' } else { '░' });
    }
    s.push(']');
    s
}

/// Compact human byte size (GiB/MiB) for the metrics readout.
fn fmt_bytes(bytes: u64) -> String {
    const GIB: f64 = (1u64 << 30) as f64;
    const MIB: f64 = (1u64 << 20) as f64;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1}G", b / GIB)
    } else {
        format!("{:.0}M", b / MIB)
    }
}

fn node_meta_lines(nodes: &HashMap<NodeId, NodeSummary>, node_id: NodeId) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(s) = nodes.get(&node_id) {
        lines.push(format!("name: {}", s.display_name(node_id)));
        if let Some(d) = &s.description {
            if !d.is_empty() {
                lines.push(format!("description: {d}"));
            }
        }
    } else {
        lines.push(format!("name: {}", short_id_prefix(node_id)));
    }
    lines.push(format!("id: {node_id}"));
    lines
}

fn prepare_detail_text(machine_id: MachineId, row: &MachineRow) -> String {
    let mut lines = vec!["Transport bootstrap".into()];
    if let Some(ref phase) = row.prepare {
        lines.push(format!("phase: {}", prepare_phase_label(phase)));
        if let Some(d) = &row.prepare_detail {
            lines.push(format!("detail: {d}"));
        }
        if let MachinePreparePhase::Skipped { reason } = phase {
            lines.push(format!("reason: {reason}"));
        }
        if let MachinePreparePhase::Failed { message } = phase {
            lines.push(format!("error: {message}"));
        }
    } else {
        lines.push("waiting…".into());
    }
    lines.push(format!("machine: {}", machine_list_label(machine_id, row)));
    lines.join("\n")
}

fn node_detail_text(
    machine_id: MachineId,
    row: &MachineRow,
    nodes: &HashMap<NodeId, NodeSummary>,
) -> String {
    if row.prepare.is_some() {
        return prepare_detail_text(machine_id, row);
    }
    let mut lines = Vec::new();
    match row.current {
        Some(n) => {
            lines.push("running:".into());
            lines.extend(node_meta_lines(nodes, n));
        }
        None => match row.last_finished {
            Some(n) => {
                lines.push("last node (idle):".into());
                lines.extend(node_meta_lines(nodes, n));
            }
            None => lines.push("idle".into()),
        },
    }
    lines.push("── output ──".into());
    lines.extend(row.detail.iter().cloned());
    if lines.len() <= 2 && row.detail.is_empty() {
        format!("no output yet for {}", machine_list_label(machine_id, row))
    } else {
        lines.join("\n")
    }
}

fn node_detail_title(row: &MachineRow, nodes: &HashMap<NodeId, NodeSummary>) -> String {
    if row.prepare.is_some() {
        return "Transport".into();
    }
    let node_id = row.current.or(row.last_finished);
    let Some(node_id) = node_id else {
        return "Node detail".into();
    };
    let label = nodes
        .get(&node_id)
        .map(|s| s.display_name(node_id))
        .unwrap_or_else(|| short_id_prefix(node_id));
    format!("Node · {label}")
}

fn machine_matches_filter(id: MachineId, row: &MachineRow, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    if id.to_string().to_lowercase().contains(filter) {
        return true;
    }
    let Some(s) = row.summary.as_ref() else {
        return false;
    };
    s.name.to_lowercase().contains(filter)
        || s.endpoint.to_lowercase().contains(filter)
        || s.kind.contains(filter)
        || s.os_hint
            .as_ref()
            .is_some_and(|o| o.to_lowercase().contains(filter))
}

fn draw_ui(frame: &mut ratatui::Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(9),
            Constraint::Length(3),
        ])
        .split(area);

    // Header: progress + pause/filter indicators.
    let total: usize = app.machines.values().map(|r| r.total).sum();
    let done: usize = app.machines.values().map(|r| r.done).sum();
    let machine_count = app.machines.len().max(1);
    let prepare_ready = app.prepare_ready_count();
    let mut header = if let Some(ref err) = app.prepare_error {
        format!("infrazeug · prepare failed · {err}")
    } else if let Some(ref summary) = app.run_summary {
        format!("infrazeug · apply complete · {summary}")
    } else if app.in_prepare() {
        let global = if matches!(app.modal, ModalState::Unlock { .. }) {
            "awaiting vault unlock"
        } else {
            app.prepare_global
                .as_deref()
                .unwrap_or("bootstrapping transports")
        };
        format!("infrazeug · preparing · {prepare_ready}/{machine_count} ready · {global}")
    } else {
        format!("infrazeug · apply · {done}/{total} units")
    };
    if app.paused {
        header.push_str(" · PAUSED");
    }
    if !app.filter.is_empty() {
        header.push_str(&format!(" · filter:{}", app.filter));
    }
    let header_color = if app.prepare_error.is_some() {
        Color::Red
    } else if app.in_prepare() {
        Color::Yellow
    } else {
        Color::Cyan
    };
    let header_w = Paragraph::new(header)
        .style(Style::default().fg(header_color))
        .block(Block::default().borders(Borders::ALL).title("status"));
    frame.render_widget(header_w, chunks[0]);

    // Middle: machines | (machine info + node detail).
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
        .split(chunks[1]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(mid[1]);

    let filter = app.filter.to_lowercase();
    let visible: Vec<(usize, &MachineId)> = app
        .order
        .iter()
        .enumerate()
        .filter(|(_, id)| machine_matches_filter(**id, &app.machines[id], &filter))
        .collect();
    let machine_items: Vec<ListItem> = visible
        .iter()
        .map(|(i, id)| {
            let row = &app.machines[id];
            let line = if app.in_prepare() || row.prepare.is_some() {
                let icon = prepare_phase_icon(row.prepare.as_ref());
                let phase = row
                    .prepare
                    .as_ref()
                    .map(prepare_phase_label)
                    .unwrap_or("pending");
                format!("{icon} {} · {phase}", machine_list_label(**id, row),)
            } else {
                let icon = if row.failed > 0 {
                    "✖"
                } else if row.running {
                    "⏳"
                } else if row.done >= row.total && row.total > 0 {
                    "✔"
                } else {
                    "…"
                };
                let fail = if row.failed > 0 {
                    format!(" ({}!)", row.failed)
                } else {
                    String::new()
                };
                format!(
                    "{icon} {} [{}/{}]{fail}",
                    machine_list_label(**id, row),
                    row.done,
                    row.total.max(1),
                )
            };
            let mut item = ListItem::new(line);
            if *i == app.selected {
                item = item.style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
            }
            item
        })
        .collect();
    let machine_list =
        List::new(machine_items).block(Block::default().borders(Borders::ALL).title("Machines"));
    frame.render_widget(machine_list, mid[0]);

    let (machine_info_left, machine_info_right, machine_info_title) = match app.selected_machine() {
        Some(id) => {
            let row = &app.machines[&id];
            let (left, right_cols) = machine_panel_columns(id, row);
            (
                left.join("\n"),
                right_cols.join("\n"),
                format!("Machine · {}", machine_list_label(id, row)),
            )
        }
        None => (
            "no machine selected".into(),
            String::new(),
            "Machine".into(),
        ),
    };
    let machine_block = Block::default()
        .borders(Borders::ALL)
        .title(machine_info_title);
    let machine_inner = machine_block.inner(right[0]);
    frame.render_widget(machine_block, right[0]);
    let machine_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(machine_inner);
    frame.render_widget(Paragraph::new(machine_info_left), machine_cols[0]);
    frame.render_widget(Paragraph::new(machine_info_right), machine_cols[1]);

    let (node_detail_body, node_detail_title) = match app.selected_machine() {
        Some(id) => {
            let row = &app.machines[&id];
            (
                node_detail_text(id, row, &app.nodes),
                node_detail_title(row, &app.nodes),
            )
        }
        None => ("select a machine".into(), "Node detail".into()),
    };
    let node_detail = Paragraph::new(node_detail_body).block(
        Block::default()
            .borders(Borders::ALL)
            .title(node_detail_title),
    );
    frame.render_widget(node_detail, right[1]);

    // Lower band: prompts | events.
    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[2]);

    let (prompt_text, prompt_title) = prompt_pane(app);
    let prompts_w = Paragraph::new(prompt_text)
        .block(Block::default().borders(Borders::ALL).title(prompt_title));
    frame.render_widget(prompts_w, lower[0]);

    let log_items: Vec<ListItem> = app
        .log
        .iter()
        .rev()
        .filter(|l| filter.is_empty() || l.to_lowercase().contains(&filter))
        .take(20)
        .map(|l| ListItem::new(l.as_str()))
        .collect();
    let log_list =
        List::new(log_items).block(Block::default().borders(Borders::ALL).title("Events"));
    frame.render_widget(log_list, lower[1]);

    // Footer: keybinding hints / filter input.
    let footer_text = if app.filter_edit {
        format!("filter: {}▌  [enter] apply  [esc] cancel", app.filter)
    } else {
        if app.run_finished {
            "Apply complete — press [q] to quit  [↑↓/jk] scroll machines  [/] filter".into()
        } else {
            "[q] quit  [p] pause  [c] cancel node  [C] cancel machine  [r] replay  [↑↓/jk] select  [/] filter  [^C] cancel all".into()
        }
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[3]);
}

/// Compute the Prompts-pane body + title from the current modal/prompt state.
fn prompt_pane(app: &App) -> (String, &'static str) {
    match &app.modal {
        ModalState::Unlock { name, input, .. } => (
            format!(
                "{name}\npassphrase: {}\n[enter] submit  [esc] cancel",
                "*".repeat(input.len())
            ),
            "Prompts · MODAL",
        ),
        ModalState::ConfirmQuit => (
            "Cancel apply? This cancels pending work on all machines.\n[y] yes  [n] no".into(),
            "Prompts · MODAL",
        ),
        ModalState::None => match &app.prompt {
            Some(PromptState::ApproveVar { var, .. }) => (
                format!("ApproveVarRequest {var}\n[y] approve  [n] deny"),
                "Prompts",
            ),
            Some(PromptState::ConfirmDestructive { summary, .. }) => (
                format!("ConfirmDestructive:\n{summary}\n[y] yes  [n] no"),
                "Prompts",
            ),
            Some(PromptState::ResolveBecome { options, pick, .. }) => {
                let opts = options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        if i == *pick {
                            format!("> {o}")
                        } else {
                            format!("  {o}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    format!("ResolveBecomeConflict:\n{opts}\n[↑↓] pick  [enter] ok"),
                    "Prompts",
                )
            }
            None => ("(no prompts)".into(), "Prompts"),
        },
    }
}
