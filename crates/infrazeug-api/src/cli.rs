//! Canonical playbook CLI (`plan`, `apply`, `test`, `lint`) for embedders and `infrazeug-cli`.
//!
//! All runnable playbook subcommands are defined here so examples and user binaries share one
//! surface instead of re-declaring `clap` parsers.

use crate::mcp_cli::{dispatch_mcp_serve, McpServeMode};
use crate::probe::{export_probe_targets, PROBE_SUBCOMMAND};
use crate::pull_cli::{
    attach_pull_subcommands, dispatch_pull, parse_pull_subcommand, BootstrapExec, PullCommandSet,
};
use crate::report_emit::{debug_requested, print_run_report, report_has_failures};
use crate::transport_env::{parse_transport_name, transport_name};
use crate::{
    apply_bundle, default_infra, test_infra, ApplyOptions, Infra, PlaybookBundle, TestOptions,
};
use anyhow::Context;
use clap::{FromArgMatches, Subcommand};
use infrazeug_core::transport::TransportChoice;
use serde_json;
use std::ffi::OsString;
use std::path::PathBuf;

/// Playbook subcommand names (stable catalog for docs and tooling).
pub const PLAYBOOK_SUBCOMMANDS: &[&str] = &["plan", "apply", "test", "lint", "graph"];

const TAG_HELP: &str =
    "Run only nodes with this tag (key=value, key, or value) and their prerequisites (repeatable)";

/// CLI flag name for selecting a named playbook (`--playbook`).
pub const PLAYBOOK_FLAG: &str = "playbook";

const PLAYBOOK_HELP: &str =
    "Named playbook defined in this binary (see PlaybookRegistry); default when omitted";

/// Bitset of playbook commands exposed by a given binary.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunCommands {
    bits: u8,
}

impl RunCommands {
    pub const PLAN: u8 = 1 << 0;
    pub const APPLY: u8 = 1 << 1;
    pub const TEST: u8 = 1 << 2;
    pub const LINT: u8 = 1 << 3;
    pub const GRAPH: u8 = 1 << 4;

    pub const ALL: Self = Self {
        bits: Self::PLAN | Self::APPLY | Self::TEST | Self::LINT | Self::GRAPH,
    };

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn with(mut self, flag: u8) -> Self {
        self.bits |= flag;
        self
    }

    pub fn contains(self, flag: u8) -> bool {
        self.bits & flag != 0
    }
}

/// Future returned by [`ExtraSubcommand::run`].
pub type ExtraSubcommandFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>>;

/// Example- or product-specific subcommand outside the playbook surface.
pub struct ExtraSubcommand {
    pub name: &'static str,
    pub about: &'static str,
    pub run: fn() -> ExtraSubcommandFuture,
}

/// What is being built when [`run`] invokes the infra factory.
#[derive(Clone, Debug)]
pub enum RunBuildContext<'a> {
    Playbook(&'a RunContext),
    Pull(&'a crate::pull_cli::PullContext),
}

/// Builds the MCP server for `mcp serve`. Returns a fully-wired
/// [`McpBuilder`](infrazeug_mcp::McpBuilder) (typically `infra.mcp().tool(..)`),
/// so the CLI can serve it without the dispatcher knowing the tool set.
pub type McpBuilderFactory = fn() -> anyhow::Result<infrazeug_mcp::McpBuilder>;

/// Configuration for [`run`].
#[derive(Clone)]
pub struct RunConfig {
    pub name: &'static str,
    pub about: Option<&'static str>,
    pub commands: RunCommands,
    pub pull: PullCommandSet,
    pub bootstrap_exec: BootstrapExec,
    pub extras: &'static [ExtraSubcommand],
    /// When set, the CLI auto-exposes `mcp serve` (SOUL §6bis.3).
    pub mcp: Option<McpBuilderFactory>,
    /// Name used when `--playbook` is not passed (must match a [`PlaybookRegistry`] entry if used).
    pub default_playbook: &'static str,
}

impl RunConfig {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            about: None,
            commands: RunCommands::ALL,
            pull: PullCommandSet::empty(),
            bootstrap_exec: BootstrapExec::InProcess,
            extras: &[],
            mcp: None,
            default_playbook: "default",
        }
    }

    pub fn default_playbook(mut self, name: &'static str) -> Self {
        self.default_playbook = name;
        self
    }

    /// Auto-expose `mcp serve`, serving the builder returned by `factory`.
    pub fn mcp(mut self, factory: McpBuilderFactory) -> Self {
        self.mcp = Some(factory);
        self
    }

    pub fn pull(mut self, pull: PullCommandSet) -> Self {
        self.pull = pull;
        self
    }

    pub fn bootstrap_exec(mut self, mode: BootstrapExec) -> Self {
        self.bootstrap_exec = mode;
        self
    }

    pub fn extras(mut self, extras: &'static [ExtraSubcommand]) -> Self {
        self.extras = extras;
        self
    }

    pub fn about(mut self, about: &'static str) -> Self {
        self.about = Some(about);
        self
    }

    pub fn commands(mut self, commands: RunCommands) -> Self {
        self.commands = commands;
        self
    }
}

/// Context passed to the infra factory after argv parsing.
#[derive(Clone, Debug)]
pub struct RunContext {
    pub command: PlaybookCommand,
    /// `--playbook` selection; `None` means use the binary's default playbook name.
    pub playbook: Option<String>,
}

impl RunContext {
    /// Effective playbook name: CLI flag or `default`.
    pub fn playbook_name<'a>(&'a self, default: &'a str) -> &'a str {
        self.playbook.as_deref().unwrap_or(default)
    }

    pub fn with_playbook(mut self, playbook: Option<String>) -> Self {
        self.playbook = playbook;
        self
    }
}

#[derive(Clone, Debug)]
pub enum PlaybookCommand {
    Plan,
    Apply(ApplyParsed),
    Test(TestParsed),
    Lint,
    Graph,
}

#[derive(Clone, Debug, Default)]
pub struct ApplyParsed {
    pub tui: bool,
    pub watch: bool,
    pub dry_run: bool,
    pub force: bool,
    pub emulate_first: bool,
    pub unpinned: bool,
    pub debug: bool,
    pub transport: Option<TransportChoice>,
    pub plan: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct TestParsed {
    pub dry_run: bool,
    pub force: bool,
    pub unpinned: bool,
    pub debug: bool,
    pub transport: Option<TransportChoice>,
    pub plan: Option<PathBuf>,
}

fn parse_transport_arg(s: &str) -> Result<TransportChoice, String> {
    parse_transport_name(s)
        .ok_or_else(|| format!("unknown transport {s:?} (use agent or agentless)"))
}

fn apply_transport_cli_override(
    bundle: &PlaybookBundle,
    cli: Option<TransportChoice>,
) -> PlaybookBundle {
    match cli {
        Some(t) => bundle.clone().with_default_remote_transport(t),
        None => bundle.clone(),
    }
}

/// Shared `clap` subcommands for playbook operations (flatten into `infrazeug-cli` or parse alone).
#[derive(Subcommand, Clone, Debug)]
pub enum PlaybookCommands {
    Plan {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long = "tag", help = TAG_HELP)]
        tags: Vec<String>,
    },
    Apply {
        #[arg(long)]
        tui: bool,
        #[arg(long)]
        watch: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        emulate_first: bool,
        #[arg(long)]
        unpinned: bool,
        #[arg(
            long,
            help = "Verbose tracing (stderr) and per-node status; set INFRAZEUG_DEBUG=1 instead of flag"
        )]
        debug: bool,
        #[arg(
            long,
            value_parser = parse_transport_arg,
            help = "Remote transport: agent (default) or agentless; env INFRZEUG_TRANSPORT"
        )]
        transport: Option<String>,
        #[arg(long = "tag", help = TAG_HELP)]
        tags: Vec<String>,
        plan: Option<PathBuf>,
    },
    Test {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        unpinned: bool,
        #[arg(long, help = "Verbose tracing and per-node status (see apply --debug)")]
        debug: bool,
        #[arg(long, value_parser = parse_transport_arg, help = "Remote transport (see apply --transport)")]
        transport: Option<String>,
        #[arg(long = "tag", help = TAG_HELP)]
        tags: Vec<String>,
        plan: Option<PathBuf>,
    },
    Lint,
    /// Inspect the planning DAG (optionally filtered by machine, start node, tags).
    Graph {
        #[arg(
            long = "machine",
            help = "Keep nodes targeting this machine (repeatable)"
        )]
        machines: Vec<String>,
        #[arg(
            long,
            help = "Keep this node (name or id) and its transitive dependents"
        )]
        start: Option<String>,
        #[arg(
            long = "tag",
            help = "Keep nodes with this tag: key=value, key, or value (repeatable)"
        )]
        tags: Vec<String>,
        #[arg(long, default_value = "text", value_parser = ["text", "json", "yaml", "toml", "dot", "html"], help = "Output format")]
        format: String,
        #[arg(
            long,
            default_value = "TB",
            value_parser = ["TB", "LR"],
            help = "Layout direction for --format dot/html (TB fits wide playbooks better than LR)"
        )]
        rankdir: String,
        #[arg(short, long, help = "Write to this file instead of stdout")]
        output: Option<PathBuf>,
    },
}

impl PlaybookCommands {
    pub fn to_context(&self) -> RunContext {
        let command = match self {
            PlaybookCommands::Plan { .. } => PlaybookCommand::Plan,
            PlaybookCommands::Apply {
                tui,
                watch,
                dry_run,
                force,
                emulate_first,
                unpinned,
                debug,
                transport,
                tags: _,
                plan,
            } => PlaybookCommand::Apply(ApplyParsed {
                tui: *tui,
                watch: *watch,
                dry_run: *dry_run,
                force: *force,
                emulate_first: *emulate_first,
                unpinned: *unpinned,
                debug: *debug,
                transport: transport.as_deref().and_then(parse_transport_name),
                plan: plan.clone(),
            }),
            PlaybookCommands::Test {
                dry_run,
                force,
                unpinned,
                debug,
                transport,
                tags: _,
                plan,
            } => PlaybookCommand::Test(TestParsed {
                dry_run: *dry_run,
                force: *force,
                unpinned: *unpinned,
                debug: *debug,
                transport: transport.as_deref().and_then(parse_transport_name),
                plan: plan.clone(),
            }),
            PlaybookCommands::Lint => PlaybookCommand::Lint,
            PlaybookCommands::Graph { .. } => PlaybookCommand::Graph,
        };
        RunContext {
            command,
            playbook: None,
        }
    }

    pub fn plan_output(&self) -> Option<&PathBuf> {
        match self {
            PlaybookCommands::Plan { output, .. } => output.as_ref(),
            _ => None,
        }
    }
}

/// Install `tracing_subscriber` with `RUST_LOG` / env-filter (idempotent for repeated calls in tests).
///
/// When `INFRAZEUG_DEBUG=1` or `--debug` is present on the command line and `RUST_LOG` is unset,
/// defaults to `infrazeug=debug` (and related crates).
pub fn init_tracing() {
    use crate::report_emit::terminal_ui_active;

    if terminal_ui_active() {
        // Tracing to stderr corrupts ratatui; progress belongs in SchedEvent → TUI log.
        return;
    }
    let filter = if debug_requested() && std::env::var_os("RUST_LOG").is_none() {
        tracing_subscriber::EnvFilter::new(
            "infrazeug=debug,infrazeug_core=debug,infrazeug_transport=debug,infrazeug_shell=debug,infrazeug_api=debug,infrazeug_playbook=debug",
        )
    } else {
        tracing_subscriber::EnvFilter::from_default_env()
    };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .try_init();
}

/// Parse argv and run a playbook command against infra from `build_infra`.
pub async fn run(
    args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    config: RunConfig,
    build_infra: impl FnOnce(RunBuildContext<'_>) -> anyhow::Result<PlaybookBundle>,
) -> anyhow::Result<()> {
    let args_vec: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if args_vec
        .get(1)
        .and_then(|a| a.to_str())
        .is_some_and(|s| s == PROBE_SUBCOMMAND)
    {
        let playbook = parse_playbook_flag(&args_vec);
        let ctx = RunContext {
            command: PlaybookCommand::Plan,
            playbook,
        };
        let bundle = build_infra(RunBuildContext::Playbook(&ctx))?;
        log_remote_transports(&bundle.infra);
        let export = export_probe_targets(&bundle.infra);
        println!("{}", serde_json::to_string(&export)?);
        return Ok(());
    }

    let mut cmd = clap::Command::new(config.name);
    if let Some(about) = config.about {
        cmd = cmd.about(about);
    }
    cmd = attach_playbook_select_arg(cmd);
    cmd = attach_playbook_subcommands(cmd, config.commands, config.extras);
    cmd = attach_pull_subcommands(cmd, config.pull);
    if config.mcp.is_some() {
        cmd = cmd.subcommand(
            clap::Command::new("mcp")
                .about("Model Context Protocol server")
                .subcommand_required(true)
                .subcommand(
                    clap::Command::new("serve")
                        .about("Serve MCP tools (default: JSON-RPC over stdio)")
                        .arg(
                            clap::Arg::new("stdio")
                                .long("stdio")
                                .action(clap::ArgAction::SetTrue)
                                .help("JSON-RPC over stdio (default when --http is omitted)"),
                        )
                        .arg(
                            clap::Arg::new("http")
                                .long("http")
                                .value_name("ADDR")
                                .help("Streamable HTTP JSON-RPC server (e.g. 127.0.0.1:7777)"),
                        ),
                ),
        );
    }
    let matches = cmd.get_matches_from(args_vec);
    // `name` selects the dispatch path; the *parent* `matches` (not the
    // subcommand-local matches) is what `Subcommand::from_arg_matches` needs —
    // its derived impl re-discovers the variant via `matches.subcommand()`.
    let name = matches
        .subcommand_name()
        .ok_or_else(|| anyhow::anyhow!("missing subcommand (try --help)"))?
        .to_string();
    if let Some(extra) = config.extras.iter().find(|e| e.name == name) {
        return (extra.run)().await;
    }
    if name == "mcp" {
        if let Some(factory) = config.mcp {
            let mcp_matches = matches
                .subcommand_matches("mcp")
                .context("missing mcp subcommand")?;
            let serve_matches = mcp_matches
                .subcommand_matches("serve")
                .context("missing mcp subcommand (try `mcp serve`)")?;
            let mode = McpServeMode::from_cli(
                serve_matches.get_one::<String>("http").map(|s| s.as_str()),
                serve_matches.get_flag("stdio"),
            )?;
            return dispatch_mcp_serve(factory()?, mode).await;
        }
    }
    if config.pull.any() {
        if let Ok(pull) = parse_pull_subcommand(&name, &matches, config.pull) {
            let ctx = pull.to_context();
            let infra = if pull.needs_playbook_infra() {
                build_infra(RunBuildContext::Pull(&ctx))?.infra
            } else {
                default_infra()
            };
            return dispatch_pull(&infra, &pull, config.bootstrap_exec).await;
        }
    }
    let playbook_cmd = parse_playbook_subcommand(&name, &matches, config.commands)?;
    let playbook_sel = matches.get_one::<String>(PLAYBOOK_FLAG).cloned();
    let ctx = playbook_cmd.to_context().with_playbook(playbook_sel);
    let bundle = build_infra(RunBuildContext::Playbook(&ctx))?;
    log_remote_transports(&bundle.infra);
    dispatch(&bundle, &playbook_cmd).await
}

fn attach_playbook_select_arg(cmd: clap::Command) -> clap::Command {
    cmd.arg(
        clap::Arg::new(PLAYBOOK_FLAG)
            .long(PLAYBOOK_FLAG)
            .global(true)
            .help(PLAYBOOK_HELP)
            .value_name("NAME"),
    )
}

/// Parse `--playbook` from argv without full clap (probe and tests).
pub fn parse_playbook_flag(args: &[std::ffi::OsString]) -> Option<String> {
    let mut iter = args.iter().map(|a| a.to_string_lossy());
    while let Some(arg) = iter.next() {
        if arg == format!("--{PLAYBOOK_FLAG}") {
            return iter.next().map(|s| s.into_owned());
        }
        if let Some(name) = arg.strip_prefix(&format!("--{PLAYBOOK_FLAG}=")) {
            return Some(name.to_string());
        }
    }
    None
}

fn log_remote_transports(infra: &Infra) {
    for machine in &infra.machines {
        if matches!(
            machine.kind,
            infrazeug_core::machine::MachineKind::Remote { .. }
        ) {
            let choice = infra.transport_for_machine(machine);
            tracing::info!(
                machine = %machine.name,
                transport = transport_name(choice),
                "remote transport"
            );
        }
    }
}

/// Execute an already-parsed playbook command (used by `infrazeug-cli` after flattening).
pub async fn dispatch(bundle: &PlaybookBundle, cmd: &PlaybookCommands) -> anyhow::Result<()> {
    match cmd {
        PlaybookCommands::Plan { output, tags } => {
            let bundle = bundle.with_tag_filter(tags);
            // Canonical plan/digest is computed offline (stable digest); the
            // optional plan file is written from it.
            let plan = bundle.plan()?;
            if let Some(path) = output {
                plan.write_file(path)?;
                println!("wrote plan {} to {}", plan.digest, path.display());
            } else {
                println!("plan digest: {}", plan.digest);
            }
            // Read-only preview of resource changes (observes live cloud state).
            crate::print_dry_run_preview(&bundle).await?;
        }
        PlaybookCommands::Apply {
            tui,
            watch,
            dry_run,
            force,
            emulate_first,
            unpinned,
            debug,
            transport,
            tags,
            plan,
        } => {
            let bundle = apply_transport_cli_override(
                bundle,
                transport.as_deref().and_then(parse_transport_name),
            );
            let bundle = bundle.with_tag_filter(tags);
            let report = apply_bundle(
                &bundle,
                ApplyOptions {
                    tui: *tui,
                    watch: *watch,
                    dry_run: *dry_run,
                    force: *force,
                    emulate_first: *emulate_first,
                    unpinned: *unpinned,
                    debug: *debug,
                    plan_path: plan.clone(),
                },
            )
            .await?;
            print_run_report(&report, *debug);
            println!("apply finished: {} entries", report.entries.len());
            if report_has_failures(&report) {
                anyhow::bail!("apply completed with failures (see stderr above)");
            }
        }
        PlaybookCommands::Test {
            dry_run,
            force,
            unpinned,
            debug,
            transport,
            tags,
            plan,
        } => {
            let bundle = apply_transport_cli_override(
                bundle,
                transport.as_deref().and_then(parse_transport_name),
            );
            let bundle = bundle.with_tag_filter(tags);
            let (report, test_report) = test_infra(
                &bundle,
                TestOptions {
                    dry_run: *dry_run,
                    force: *force,
                    unpinned: *unpinned,
                    debug: *debug,
                    plan_path: plan.clone(),
                    ..Default::default()
                },
            )
            .await?;
            print_run_report(&report, *debug);
            println!(
                "test finished: {} entries, {} skipped",
                report.entries.len(),
                test_report.skipped.len()
            );
            if report_has_failures(&report) {
                anyhow::bail!("test completed with failures (see stderr above)");
            }
        }
        PlaybookCommands::Lint => {
            let report = bundle.lint_report();
            if report.is_empty() {
                println!("lint ok");
            } else {
                print!("{report}");
                if report.has_errors() {
                    anyhow::bail!("lint found {} error(s)", report.errors().count());
                }
            }
        }
        PlaybookCommands::Graph {
            machines,
            start,
            tags,
            format,
            rankdir,
            output,
        } => {
            let select = infrazeug_core::GraphSelect {
                machines: machines.clone(),
                start: start.clone(),
                tags: tags.clone(),
            };
            let view = bundle.infra.graph_view()?.select(&select);
            let rendered = match format.as_str() {
                "json" => format!("{}\n", serde_json::to_string_pretty(&view)?),
                "yaml" => view.to_yaml()?,
                "toml" => view.to_toml()?,
                "dot" => view.to_dot_with_rankdir(rankdir),
                "html" => view.to_html_with_rankdir(rankdir)?,
                _ => view.to_text(),
            };
            if let Some(path) = output {
                std::fs::write(path, &rendered)
                    .with_context(|| format!("writing graph to {}", path.display()))?;
                println!("wrote graph to {}", path.display());
            } else {
                print!("{rendered}");
            }
        }
    }
    Ok(())
}

fn attach_playbook_subcommands(
    mut root: clap::Command,
    enabled: RunCommands,
    extras: &[ExtraSubcommand],
) -> clap::Command {
    for extra in extras {
        root = root.subcommand(clap::Command::new(extra.name).about(extra.about));
    }
    let mut template = clap::Command::new("_playbook");
    template = PlaybookCommands::augment_subcommands(template);
    for sub in template.get_subcommands() {
        let name = sub.get_name();
        let on = match name {
            "plan" => enabled.contains(RunCommands::PLAN),
            "apply" => enabled.contains(RunCommands::APPLY),
            "test" => enabled.contains(RunCommands::TEST),
            "lint" => enabled.contains(RunCommands::LINT),
            "graph" => enabled.contains(RunCommands::GRAPH),
            _ => false,
        };
        if on {
            root = root.subcommand(sub.clone());
        }
    }
    root
}

fn parse_playbook_subcommand(
    name: &str,
    matches: &clap::ArgMatches,
    enabled: RunCommands,
) -> anyhow::Result<PlaybookCommands> {
    let allowed = match name {
        "plan" => enabled.contains(RunCommands::PLAN),
        "apply" => enabled.contains(RunCommands::APPLY),
        "test" => enabled.contains(RunCommands::TEST),
        "lint" => enabled.contains(RunCommands::LINT),
        "graph" => enabled.contains(RunCommands::GRAPH),
        _ => false,
    };
    if !allowed {
        anyhow::bail!("subcommand `{name}` is not enabled for this binary");
    }
    PlaybookCommands::from_arg_matches(matches)
        .map_err(|e| anyhow::anyhow!("failed to parse subcommand `{name}`: {e}"))
}
