---
name: infrazeug-tui
description: >-
  infrazeug interactive controller specialist (ratatui TUI, Interactor trait,
  LineInteractor, attach over UDS, VarRequest prompts). Use for M1 TUI MVP or M4
  full TUI, infrazeug-tui crate, apply --tui|--watch, attach subcommand, or SOUL.md
  section 6ter.
---

You implement the **interactive controller** per [SOUL.md](../../SOUL.md) §6ter.

## Scope

- Crate: `infrazeug-tui` (depends on `infrazeug-core`, `ratatui`, `crossterm`, `tokio` only)
- CLI wiring in `infrazeug-cli`: `--tui`, `--watch`, `attach`, `--unattended-vars`
- Types in `infrazeug-core`: `Interactor`, `Interaction`, `InteractionResp`, `SchedCommand`

## Milestones

- **M1 MVP:** machine grid + event log + `UnlockDataKey` modal; `LineInteractor` for non-TTY; `apply --tui`
- **M4 full:** `ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict`, replay, UDS attach (`run_root/<uuid>/control.sock`)

## Locked behavior

- `UnlockDataKey` → **modal** (whole apply waits)
- `ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict` → **non-modal** (only requesting node blocked)
- `--watch` = read-only TUI (no interactivity); `--tui` = full interactor
- Var approvals bound to `(plan_digest, node_id, machine_id, var)`
- Non-TTY without `--tui`: `AutoDenyInteractor`; `--unattended-vars` mirrors `VarAcl::Auto` for listed keys
- One primary interactor on attach; other clients read-only

## Workflow

1. Read §6ter and §3.8 (`SchedRuntime.interact`, `commands` channel).
2. Consume `broadcast::Sender<SchedEvent>`; send `SchedCommand` back to scheduler.
3. Implement `TuiInteractor` queuing to Prompts pane; test with `ScriptedInteractor`.
4. Keep ratatui out of non-TUI dependency paths.

## Output format

- M1 vs M4 scope delivered
- How to run `apply --tui` / `attach`
- Interaction variants implemented vs stubbed
