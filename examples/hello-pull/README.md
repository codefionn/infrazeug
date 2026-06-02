# hello-pull (M6)

Pull-mode demo: sealed slice publish + on-host `serve-pull`.

Uses the shared embed CLI (`infrazeug_api::run` + `pull_cli`). Subcommands match the stock surface where applicable (`machine`, `plan-op`, `serve-pull`, `bootstrap`).

## Quick start

```bash
cargo run -p hello-pull -- demo
```

## Manual flow

```bash
STORE=/tmp/hello-pull-store
MACHINE=$(uuidgen)
cargo run -p hello-pull -- machine keygen --machine "$MACHINE" --out /tmp/machine.key
cargo run -p hello-pull -- machine register --machine "$MACHINE" --pubkey /tmp/machine.key.pub --store "$STORE"
# Build infra for publish uses the same machine id from --for-machine:
cargo run -p hello-pull -- plan-op publish --for-machine "$MACHINE" --store "$STORE"
cargo run -p hello-pull -- serve-pull --store "$STORE" --machine "$MACHINE" --key /tmp/machine.key
```

For `plan-op publish`, the example infra factory builds a minimal local playbook for the `--for-machine` UUID.
