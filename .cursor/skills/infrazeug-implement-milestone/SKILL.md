---
name: infrazeug-implement-milestone
description: >-
  Plans and implements an infrazeug milestone (M1–M6) from SOUL.md with artifacts
  under docs/impl/. Use when starting M1, landing a vertical slice, or scoping
  work to a single milestone without drifting into later crates.
---

# Implement infrazeug milestone

## Prerequisites

1. Read [SOUL.md](../../../SOUL.md) sections for the target milestone.
2. Read [.cursor/skills/infrazeug-soul/milestones.md](../infrazeug-soul/milestones.md) for acceptance criteria.
3. Apply skill `infrazeug-soul` for locked semantics.

## Artifact layout

Create `docs/impl/<milestone-slug>/` (e.g. `m1-core`, `m2-ssh`):

| File | Purpose |
|------|---------|
| `README.md` | Scope, status, links to SOUL sections |
| `research.md` | Crate map, existing code, dependencies |
| `spec.md` | Testable behavior + non-goals for this milestone only |
| `plan.md` | Ordered steps aligned with SOUL §11 when M1 |

## Workflow

1. **Confirm milestone** — If unclear, default M1 until workspace exists.
2. **Workspace skeleton** — `Cargo.toml` workspace + stub crates per SOUL §2; implement only milestone crates.
3. **Types → behavior** — Structs/enums and macros first; then interpreter/scheduler/CLI.
4. **Vertical slice** — One end-to-end path (e.g. hello-local) before polish.
5. **Validate** — `cargo test`, `cargo clippy`, milestone example binary; `lint` without vault where applicable.
6. **Update docs** — Keep `docs/impl/*` in sync if design assumptions change.

## Delegation

| Area | Subagent |
|------|----------|
| Scheduler, plan/apply, vars, `VarAcl` | `infrazeug-core` |
| TUI, `Interactor`, attach | `infrazeug-tui` |
| ShellOp, SSH, postcard RPC | `infrazeug-shell-transport` |
| Vault, VarRequest (M4) | `infrazeug-secrets` |
| OCI/QEMU/test | `infrazeug-emulation` |
| Pull/bootstrap | `infrazeug-pull` |
| MCP | `infrazeug-mcp` |
| SOUL compliance review | `infrazeug-soul-reviewer` |

## Completion checklist

- [ ] Milestone acceptance from SOUL §10 met
- [ ] No locked-semantics violations ([reference-locked.md](../infrazeug-soul/reference-locked.md))
- [ ] Later-milestone features stubbed or absent, not half-wired
- [ ] Example or integration test demonstrates the slice
- [ ] `docs/impl/<slug>/` reflects what shipped

## Output to user

Summarize: milestone, artifact path, crates touched, how to run the demo, open items deferred to later milestones.
