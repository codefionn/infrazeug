# infrazeug — agent guide

Rust Infrastructure-as-Code library/framework (code-first, not YAML). **Design authority:** [SOUL.md](SOUL.md).

## For humans and agents

| Resource | Location |
|----------|----------|
| Full design | [SOUL.md](SOUL.md) |
| Playbook authoring patterns | [docs/playbook-patterns.md](docs/playbook-patterns.md) |
| Vault on-disk format | [docs/vault-format.md](docs/vault-format.md) |
| Locked rules (cheat sheet) | [.cursor/skills/infrazeug-soul/reference-locked.md](.cursor/skills/infrazeug-soul/reference-locked.md) |
| Milestone scope | [.cursor/skills/infrazeug-soul/milestones.md](.cursor/skills/infrazeug-soul/milestones.md) |
| Implementation workflow | skill `infrazeug-implement-milestone` |
| Always-on conventions | [.cursor/rules/infrazeug-soul.mdc](.cursor/rules/infrazeug-soul.mdc) |

## Subagents (`.cursor/agents/`)

Delegate isolated exploration or focused implementation:

| Name | Domain |
|------|--------|
| `infrazeug-core` | Machine, Node, Scheduler, Plan/Apply, VarSet, `VarAcl`, `Interactor` types |
| `infrazeug-tui` | ratatui controller, `--tui`/`--watch`, `attach`, prompts (§6ter) |
| `infrazeug-shell-transport` | ShellOp DSL, SSH, agent, **postcard** RPC |
| `infrazeug-secrets` | Vault, DataKeys, VarRequest (M4) |
| `infrazeug-emulation` | OCI, BuildGraph, QEMU, `like`, test mode, RunGuard |
| `infrazeug-pull` | Bootstrap stub, sealed plans, pull daemon |
| `infrazeug-mcp` | MCP tools/resources; never expose secrets |
| `infrazeug-soul-reviewer` | Diff review against SOUL locked semantics |

Example: *Use the infrazeug-tui subagent to land the M1 TUI MVP per SOUL §6ter.10.*

## Runnable surface

Playbook CLI (`plan`, `apply`, `test`, `lint`) lives in `infrazeug-api::cli`; pull CLI (`machine`, `plan-op`, `serve-pull`, `bootstrap`) in `infrazeug-api::pull_cli`. Examples and pull binaries call `infrazeug_api::run`; the stock `infrazeug` binary delegates to the same dispatchers. Catalog: [docs/runnable.md](docs/runnable.md).

## Current phase

**M6 shipped** (pull-mode + partial plans). Next work is post-1.0 polish or deferred items in SOUL §12 (Git plan-store, stock pull agent, real IMDS).

M1–M6 vertical slices are in place; use milestone docs under `docs/impl/` for scope. RPC format is **postcard** (locked). Pull-mode rejects `WaitForHash` in slices (locked).
