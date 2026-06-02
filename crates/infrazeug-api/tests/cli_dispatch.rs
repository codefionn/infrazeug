//! Regression: `run()` must dispatch playbook subcommands. The derived
//! `Subcommand::from_arg_matches` needs the *parent* matches, not the
//! subcommand-local matches — passing the latter made every invocation fail
//! with "a subcommand is required".

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{
    build_from_registry, run, PlaybookBundle, PlaybookEntry, PlaybookRegistry, RunBuildContext,
    RunConfig, RunContext,
};
use infrazeug_core::id::MachineId;
use uuid::Uuid;

fn tiny_bundle() -> anyhow::Result<PlaybookBundle> {
    Ok(InfraBuilder::new()
        .machine(builder::local(MachineId(Uuid::new_v4()), "localhost"))?
        .build())
}

fn build_alpha(_ctx: &RunContext) -> anyhow::Result<PlaybookBundle> {
    tiny_bundle()
}

fn build_beta(_ctx: &RunContext) -> anyhow::Result<PlaybookBundle> {
    tiny_bundle()
}

static REGISTRY: PlaybookRegistry = PlaybookRegistry {
    default: "alpha",
    entries: &[
        PlaybookEntry {
            name: "alpha",
            build: build_alpha,
        },
        PlaybookEntry {
            name: "beta",
            build: build_beta,
        },
    ],
};

async fn dispatch(sub: &str) -> anyhow::Result<()> {
    run(
        ["bin", sub].into_iter().map(String::from),
        RunConfig::new("cli-dispatch-test"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => tiny_bundle(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

async fn dispatch_with_playbook(sub: &str, playbook: &str) -> anyhow::Result<()> {
    run(
        ["bin", sub, "--playbook", playbook]
            .into_iter()
            .map(String::from),
        RunConfig::new("cli-dispatch-test").default_playbook("alpha"),
        |ctx| build_from_registry(&REGISTRY, ctx),
    )
    .await
}

#[tokio::test]
async fn run_dispatches_plan() {
    dispatch("plan").await.expect("plan should dispatch");
}

#[tokio::test]
async fn run_dispatches_lint() {
    dispatch("lint").await.expect("lint should dispatch");
}

#[tokio::test]
async fn run_selects_named_playbook() {
    dispatch_with_playbook("plan", "beta")
        .await
        .expect("named playbook should dispatch");
}

#[tokio::test]
async fn run_rejects_unknown_playbook() {
    let err = dispatch_with_playbook("plan", "missing")
        .await
        .expect_err("unknown playbook");
    assert!(err.to_string().contains("unknown playbook"));
}
