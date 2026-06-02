---
name: infrazeug-soul
description: >-
  Implements and reviews the infrazeug Rust IaC framework per SOUL.md. Use when
  working in this repo, adding crates, Machine/Node/Scheduler/ShellOp/TUI/vault/MCP
  code, or when the user mentions SOUL, milestones M1–M6, or locked semantics.
---

# infrazeug — SOUL.md

**Canonical design:** [SOUL.md](../../../SOUL.md) at the repo root. Read the sections relevant to your task before coding. Do not contradict locked semantics in [reference-locked.md](reference-locked.md).

## Project shape

- **Library-first:** user `main.rs` is the playbook; binary is agent + CLI when invoked with subcommands.
- **Workspace:** `crates/*` per §2 (includes `infrazeug-tui`, `infrazeug-rpc` postcard); public surface via `infrazeug-api`.
- **Stack:** Rust, Tokio, `petgraph` DAG, `tracing`, `thiserror` per crate / `anyhow` at CLI only.
- **No controller state file.** Plans recomputed from facts; `infrazeug.lock` pins build graph only.

## When to use sibling skills / subagents

| Task | Delegate to |
|------|-------------|
| M1 core, scheduler, plan/apply, vars, `VarAcl` | `infrazeug-core` or `infrazeug-implement-milestone` |
| TUI, `Interactor`, attach, prompts | `infrazeug-tui` |
| ShellOp, SSH, agent, postcard RPC | `infrazeug-shell-transport` |
| Vault, DataKeys, providers | `infrazeug-secrets` |
| OCI, BuildGraph, `like`, test mode | `infrazeug-emulation` |
| Pull-mode, bootstrap, sealed plans | `infrazeug-pull` |
| MCP (no secret exfil) | `infrazeug-mcp` |
| SOUL compliance review | `infrazeug-soul-reviewer` |

## Implementation discipline

1. **Smallest vertical slice** — [milestones.md](milestones.md); defer later crates unless the task spans milestones.
2. **Types before behavior** — SOUL §11 order; M1 includes `infrazeug-tui` step 11.
3. **Plan-time errors** — native-on-agentless, become conflicts, cycles, pull `WaitForHash`.
4. **TUI optional dep** — `infrazeug-tui` isolated; embedders avoid ratatui unless they use `--tui`.

## Quick API anchors

- `Machine`, `Node`, `Scheduler`, `SchedRuntime`, `SchedCommand` → §3
- `VarAcl`, `VarSet` → §3.9
- `ShellOp`, `Out<'i, T>`, `argv!` → §3.3
- `Transport`, postcard RPC → §4
- `Interactor`, `--tui`/`--watch`, attach → §6ter
- Vault, `VaultStruct` → §6
- CLI → §7

## Additional resources

- [reference-locked.md](reference-locked.md)
- [milestones.md](milestones.md)
