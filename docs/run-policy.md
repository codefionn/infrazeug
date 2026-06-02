# Run policy and change policy

Infrazeug has two separate mechanisms that are easy to conflate:

- `change_policy` classifies a successful shell node as `Changed` or `Unchanged`.
- `run_policy` decides whether a node is started after its dependencies are ready.

Use them together deliberately. `change_policy` is for idempotent commands that
exit `0` both when they changed the machine and when they had nothing to do.
`RunPolicy::OnUpstreamChange` is a graph gate: the node is skipped unless at
least one upstream dependency reported `Changed`. `RunPolicy::Always` bypasses
that gate and lets the node's own command decide whether there is work.

## Default behavior

The default `RunPolicy` is `OnUpstreamChange`.

- A root node with unknown plan outcome runs.
- A successor runs when any upstream target reported `Changed`.
- A successor is skipped when the relevant upstream work was `Unchanged`.
- A skipped node is not a successful `Changed`/`Unchanged` completion for later
  dependency readiness.

That last point matters: do not build a chain where node C depends on node B if
node B uses the default run policy and may be skipped, unless C is intentionally
not supposed to run in that case.

## When to use `OnUpstreamChange`

Use the default when the node is purely follow-up work and has no independent
reason to run.

Examples:

- regenerate a config after a template write changed
- reload a service only after its unit or config changed
- restart an app only after a package install changed

In this shape, a skipped successor is expected. Nothing downstream should require
that skipped node as a readiness barrier unless the downstream is also optional.

## When to use `Always`

Use `RunPolicy::Always` when the node is a required phase in a workflow, even if
it may internally decide to do nothing.

Examples:

- rolling upgrade phases: upgrade, reboot, wait-ready
- health checks and readiness barriers
- nodes using a marker file to coordinate whether a reboot/restart is needed
- cleanup/finalizer nodes that must run after successful predecessors
- any chain where later nodes depend on this node completing

For these nodes, keep the idempotence and skip logic inside the command or in
`change_policy`; do not rely on the framework's default upstream-change gate.

## Rolling update pattern

A rolling update usually has several required phases:

1. `rolling-upgrade@host`
2. `rolling-reboot@host`
3. `rolling-ready@host`

All three should use `RunPolicy::Always` when the scripts already implement
their own skip marker or output classification.

The common failure mode is:

1. `rolling-upgrade@host` runs and reports `Unchanged` because there were no
   package updates.
2. `rolling-reboot@host` keeps the default `OnUpstreamChange`, so the scheduler
   skips it.
3. `rolling-ready@host` depends on `rolling-reboot@host`, but the reboot node was
   `Skipped` rather than a successful completion.
4. The workflow stalls because the readiness node is waiting for a predecessor
   that never completed as `Changed` or `Unchanged`.

The fix is to make each required phase `RunPolicy::Always` and let the phase
script decide whether it has work:

```rust
use infrazeug_core::{RunPolicy, Targets};
use infrazeug_shell::{argv, ShellOp};

let mut upgrade = infrazeug_core::infra::shell_node(
    upgrade_id,
    "rolling-upgrade@homesrv",
    ShellOp::run(argv!["sh", "-c", "sudo /usr/local/sbin/rolling-upgrade"]),
    Targets::Machine(homesrv),
);
upgrade.run_policy = RunPolicy::Always;
upgrade.change_policy.rules.push(
    infrazeug_core::OutputChangeRule::unchanged_when_contains(
        infrazeug_core::OutputMatchStream::Stdout,
        "no updates",
    ),
);

let mut reboot = infrazeug_core::infra::shell_node(
    reboot_id,
    "rolling-reboot@homesrv",
    ShellOp::run(argv!["sh", "-c", "sudo /usr/local/sbin/rolling-reboot"]),
    Targets::Machine(homesrv),
);
reboot.deps = vec![upgrade_id];
reboot.run_policy = RunPolicy::Always;

let mut ready = infrazeug_core::infra::shell_node(
    ready_id,
    "rolling-ready@homesrv",
    ShellOp::run(argv!["sh", "-c", "sudo /usr/local/sbin/rolling-ready"]),
    Targets::Machine(homesrv),
);
ready.deps = vec![reboot_id];
ready.run_policy = RunPolicy::Always;
```

If the reboot script uses a marker file, make the script idempotent:

- upgrade writes the marker only when a reboot is needed
- reboot exits successfully without rebooting when the marker is absent
- reboot clears or updates the marker after the reboot path is committed
- ready succeeds immediately when no reboot was requested, or waits for the host
  when one was requested

That gives every phase a successful completion while preserving accurate
`Changed`/`Unchanged` reporting for later graph propagation.

## Rule of thumb

Use `OnUpstreamChange` for optional reaction nodes. Use `Always` for required
workflow phases and barriers. If a later node must depend on it, it usually
should not be skippable by the framework-level run policy.
