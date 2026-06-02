# hello-local (M1)

Runs `nginx -v` on a `MachineKind::Local` host.

## Prerequisites

- [nginx](https://nginx.org/) on `PATH` (`nginx -v` must succeed)

## Commands

Subcommands and flags come from the shared playbook CLI (`infrazeug_api::run`); same `plan` / `apply` / `test` / `lint` surface as `infrazeug`.

```bash
cargo run -p hello-local -- plan -o /tmp/hello-local.plan
cargo run -p hello-local -- apply /tmp/hello-local.plan
cargo run -p hello-local -- apply --tui
cargo run -p hello-local -- apply --watch
cargo run -p hello-local -- apply --dry-run
```

`apply` with a plan file recomputes the digest and refuses drift unless you pass `--force`.

Run report is written under `$TMPDIR/infrazeug-hello-local/<run-uuid>/run-report.json`.
