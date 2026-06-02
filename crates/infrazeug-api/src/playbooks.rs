//! Named playbooks: register several infra factories and select one via `--playbook`.

use crate::cli::{RunBuildContext, RunContext};
use crate::PlaybookBundle;
use anyhow::Context;

/// One named playbook (`build` receives command context for transport/mode tweaks).
pub struct PlaybookEntry {
    pub name: &'static str,
    pub build: fn(&RunContext) -> anyhow::Result<PlaybookBundle>,
}

/// Static set of playbooks plus a default name when `--playbook` is omitted.
pub struct PlaybookRegistry {
    pub default: &'static str,
    pub entries: &'static [PlaybookEntry],
}

impl PlaybookRegistry {
    /// Build the playbook selected on `ctx` (or the registry default).
    pub fn resolve(&self, ctx: &RunContext) -> anyhow::Result<PlaybookBundle> {
        let name = ctx.playbook_name(self.default);
        let entry = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .with_context(|| {
                format!(
                    "unknown playbook {name:?} (available: {})",
                    self.entries
                        .iter()
                        .map(|e| e.name)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
        (entry.build)(ctx)
    }

    /// Build the registry default (for pull subcommands that need playbook infra).
    pub fn build_default(&self) -> anyhow::Result<PlaybookBundle> {
        let ctx = RunContext {
            command: crate::cli::PlaybookCommand::Plan,
            playbook: None,
        };
        self.resolve(&ctx)
    }
}

/// Infra factory for [`crate::run`] that dispatches on `--playbook`.
pub fn build_from_registry(
    registry: &'static PlaybookRegistry,
    ctx: RunBuildContext<'_>,
) -> anyhow::Result<PlaybookBundle> {
    match ctx {
        RunBuildContext::Playbook(ctx) => registry.resolve(ctx),
        RunBuildContext::Pull(_) => registry.build_default(),
    }
}
