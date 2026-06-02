# OCI integration tests

From the repo root, download assets and run **all** workspace tests (including this stack)
with [`scripts/run-infra-tests.sh`](../../../scripts/run-infra-tests.sh).

The example Podman/Docker stack (PostgreSQL, Keycloak, Open WebUI, RustFS) lives in
`src/stack.rs` behind `#[cfg(test)]` and is not part of the normal library build.

## Example stack

Runtime resolution (first match wins):

1. `INFRZEUG_CONTAINER_RUNTIME` — explicit binary path or name
2. `INFRZEUG_PODMAN` or `podman` on `PATH`
3. `INFRZEUG_DOCKER` or `docker` on `PATH`

```bash
INFRZEUG_STACK_TEST=1 cargo test -p infrazeug-emulate-oci example_stack_internal_network -- --ignored --nocapture
```

Teardown runs even when assertions fail (`stack.down()` after checks).
