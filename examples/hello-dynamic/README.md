# hello-dynamic

Dynamic machines: discover a set of hosts **at apply time**, then fan a per-machine
playbook out over each discovered machine.

## What it shows

- **`discover_machines(..).for_each_machine(..)`** — a discovery `NodeMethod`
  returns `Vec<DiscoveredMachine>` (as its node capture); the scheduler turns each
  into a lazy push machine and runs the template on it.
- **Connectivity node = agent upload.** The per-machine `connectivity` head is the
  machine's first transport use — it probes the arch, uploads the agent, and pings.
  There is no eager pre-apply agent phase; uploads are in the DAG.
- **Tolerate by default.** A host that fails connectivity (or a step) is skipped;
  the rest proceed and the group's exit barrier still joins. Use `.fail_fast(true)`
  to halt the fan-out at the join instead.

## Run

```sh
# Inspect the static scaffold (discovery node, exit barrier, injected connect head).
# The per-machine template instances appear at apply time, once machines are known.
cargo run -p hello-dynamic -- lint
cargo run -p hello-dynamic -- graph

# Apply. Override the discovered set with HOSTS="name=host,...".
HOSTS="web-1=10.0.0.1,web-2=10.0.0.2" cargo run -p hello-dynamic -- apply
```

## Notes

- Dynamic fan-out is **apply/push-only**: the runtime-discovered machines can't be
  folded into a signed plan digest, so sealed pull-mode rejects dynamic groups.
- `plan` / `graph` show the group as resolved-at-apply (the member count is unknown
  until the discovery node runs).
