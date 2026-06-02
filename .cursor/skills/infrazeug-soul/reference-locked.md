# Locked semantics (from SOUL.md)

Do not change without an explicit SOUL.md revision and user agreement.

## Identity & targeting

- `MachineId` / `NodeId` / `GroupId`: UUID v4 via `uuid!()`; duplicate `name` per `Infra` is an error.
- Groups are **flat** (no nesting). Precedence: `global < group[0..n] < machine < like_override`.
- Tags: targeting only; no var semantics.
- Map vars **deep-merge**; lists **replace** unless `VarKey::Append`. (`VaultStruct` Vec concat is scoped to struct load only.)
- `VarAcl`: `Auto` (default), `Prompt`, `AutoForMachines(Vec<MachineId>)` per vault field.

## Nodes & change propagation

- Fan-out: one logical node runs on each target concurrently; **barrier-by-default** — successors start only after all targets of a predecessor finish.
- `RunPolicy::OnUpstreamChange` (default): skip unless **any** upstream target changed.
- Successors run only on machines where **every** predecessor succeeded on that machine; else `Skipped(BlockedByUpstream)`.
- `PlanOutcome::Unknown` from `NodeMethod::plan()` → treated as **Changed** for downstream.
- **Native-on-agentless** → plan-time hard error (name node + machine).

## Execution tiers

- **ShellOp** (tier 2): serializable; `Argv` is always explicit `Vec` — no free-form shell string API.
- **NodeMethod** (tier 1): agent-only; arbitrary Rust on target.

## Scheduler & failures

- Default: central planner + per-machine workers; **edge-readiness** (no global level barrier except explicit `SyncAll`).
- `SchedRuntime` includes `commands` (cancel/pause/replay/filter) and `interact: Arc<dyn Interactor>`.
- `FailPolicy::FailFast` default; `Tolerate { max_failed }` optional.
- Stricter `SyncAll` wins on join when ancestors disagree.
- No resumability / checkpoints; crash → full re-apply.
- `Infra::default_node_timeout = None`; no inheritance.

## Plan & pull

- Plan CBOR + digest; `apply plan.bin` recomputes and refuses drift unless `--force`.
- Push plans: secret vars by reference + RPC `VarRequest`; pull: secrets inlined in sealed per-machine slice.
- Var approval bound to `(plan_digest, node_id, machine_id, var)` — no replay across plans.
- Pull-mode: **no `WaitForHash`** in slices; custom signed agent only (v1).
- Sealed plans reuse vault envelope crypto (X25519 recipient).

## Interactive controller (§6ter)

- `UnlockDataKey` is **modal** (whole apply waits).
- `ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict` are **non-modal** (only requesting node blocked).
- `SignPlan` at plan time, not apply.
- `--tui` = interactive; `--watch` = same TUI, read-only (no prompts).
- Non-TTY apply without `--tui`: `AutoDenyInteractor` unless `--unattended-vars` pre-approves keys.

## Secrets & MCP

- Recipients → DataKeys → vault files; CBOR + XChaCha20-Poly1305; magic `INFRZVLT`.
- DataKeys unlocked **once at run start**; bodies lazy per machine.
- **MCP never exposes secrets** — not configurable.

## Emulation & test

- `like` twins must be emulated kinds only.
- `RunMode::Test`: swap to `like`, ephemeral lifecycle, skip machines without `like` (WARN).
- `RunGuard` teardown on end/panic/signal.

## Transport

- SSH via system `ssh`/`sftp` only (no in-process libssh); connection mux under `run_root`.
- **RPC wire format = postcard** (framed length-prefix over agent stdin/stdout) — locked.
- Controller: Linux only v1; OpenSSH ≥ 8.0.
