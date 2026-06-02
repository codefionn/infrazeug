---
name: infrazeug-shell-transport
description: >-
  infrazeug ShellOp DSL, argv/Out typing, SSH transport (mux, push agent, agentless
  lowering), and agent build. Use for M2, ShellOp, infrazeug-shell, infrazeug-transport,
  infrazeug-agent, infrazeug-rpc (postcard), infrazeug-build, or SOUL.md sections 3.3 and 4.
---

You implement **ShellOp**, transport, and agent paths per [SOUL.md](../../SOUL.md) §3.3 and §4.

## Scope

- Crates: `infrazeug-shell`, `infrazeug-transport`, `infrazeug-agent`, `infrazeug-rpc`, `infrazeug-build`
- ShellOp interpreter (local + agent); lowering to shell+sftp for agentless
- SSH via **system** `ssh`/`sftp` only; mux under `run_root`

## Locked behavior

- `Argv` explicit `Vec` only — use `argv!`; no free-form shell string API
- `Out<'i, T>` lifetime-tied to `Infra` / `InstantiatedTemplate`
- Native on agentless → plan-time error
- **RPC wire = postcard** (framed length-prefix on agent stdin/stdout) — not protobuf
- Become wrapping; vault identities to tmpfs 0600 under run dir
- OpenSSH ≥ 8.0; Linux controller only v1
- Progress events rate-limited; cancel per §3.8.6

## Workflow

1. Read SOUL §3.3, §4, §12 (RPC decided).
2. M2 order: agentless lowering → push agent + postcard RPC → `infrazeug-build`.
3. Integration test: container `sshd`, nginx ShellOp agentless + agented.

## Output format

- Transport mode (push / agentless / local)
- RPC codec choices
- Test command for containerized sshd
