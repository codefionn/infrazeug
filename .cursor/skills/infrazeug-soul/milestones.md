# Milestones (SOUL §10–11)

Use the active milestone as the scope ceiling unless the user widens it.

## M1 — Core skeleton

**Crates (stub rest):** `infrazeug-core`, `infrazeug-api`, `infrazeug-shell`, `infrazeug-methods`, `infrazeug-transport`, `infrazeug-tui`, `infrazeug-cli`

**Ship:** `Machine`/`Group`/`VarSet` + **`VarAcl`**, `NodeMethod`, default `Scheduler` + `SchedCommand` channel, `GlobalLimits`, locks, edge-readiness, fail policies, event bus, **`Interactor` trait + `LineInteractor` + TUI MVP** (grid + log + `UnlockDataKey` modal), `SyncAll`, `Local` transport, `shell` + `file.*`, `plan`/`apply`, `lint`, CBOR `Plan` + drift, `tracing`, `RunGuard`, `gc`, `vars resolve`

**Done when:** `examples/hello-local` — nginx on localhost via **`apply --tui`** + `RunReport` (`--watch` = read-only TUI)

**Build order:** SOUL §11 steps 1–13 (step 11 = `infrazeug-tui` MVP)

## M2 — SSH + ShellOp + agent build

Agentless lowering first, then push agent + **postcard RPC** + `infrazeug-build` (zigbuild). End: nginx graph agentless + agented vs container `sshd`.

## M3 — Emulation OCI + BuildGraph

`like`, `test`, `--emulate-first`, `ContainerSpec`/`BuildGraph`, buildkit LLB, `infrazeug.lock`. Vault secret mounts stub until M4.

## M3.5 — MCP

`infrazeug-mcp`; builtins opt-in; secrets never exposed.

## M4 — Secrets v1 + full TUI

Envelope, providers (passphrase, ssh-agent), FS+S3 `MultiBackend`, packs, `VaultStruct`, audit, plan signing, push **VarRequest** + **`VarAcl` enforcement**, registry signing for builds, **full TUI** (`ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict`, replay, **`attach`** over UDS).

## M5 — QEMU + hardware providers

FIDO2/PKCS#11/age/KMS, WebDAV backend, microVM emulation.

## M6 — Pull-mode + partial plans

`infrazeug-bootstrap`, `infrazeug-pull`, sealed slices, publish/revoke, `WaitForHash` for push slices only.

## Decided (not open)

- RPC = **postcard** framed length-prefix (§4.1, §12)
- MCP never exposes secrets
- Pull slices reject `WaitForHash`

## Open questions (do not “resolve” in code)

Buildkit crate TBD; Windows controller not v1 — see SOUL §12.
