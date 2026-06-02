# hello-emulate (M3)

Remote machine (or local stand-in) with a **container `like`**: multi-stage `ContainerSpec` (alpine + nginx), `infrazeug test`, and optional `apply --emulate-first`.

Requires **podman** on the controller (`INFRZEUG_PODMAN` overrides the binary name).

```bash
# Default: machine is local; like runs nginx -v inside a built container
cargo run -p hello-emulate -- test

# Optional real SSH target after successful emulation
export INFRZEUG_SSH_HOST=root@127.0.0.1:2222
cargo run -p hello-emulate -- apply --emulate-first

cargo run -p hello-emulate -- plan -o /tmp/plan.bin
```

Writes/refreshes `infrazeug.lock` in the workspace on apply/test.
