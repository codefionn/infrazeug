# hello-ssh (M2)

Runs `nginx -v` on a remote host over SSH using the same ShellOp graph as `hello-local`.

## Prerequisites

- OpenSSH ≥ 8.0 on the controller (Linux).
- A reachable `sshd` target (container or VM).
- For agent push: build the agent first (`cargo run -p infrazeug-cli -- agent build`).

## Usage

Playbook flags match the shared embed CLI (`plan`, `apply`, `test`, `lint`; see `infrazeug_api::PLAYBOOK_SUBCOMMANDS`).

```bash
export INFRZEUG_SSH_HOST=root@127.0.0.1:2222
export INFRZEUG_SSH_MODE=agentless   # or agent
cargo run -p hello-ssh -- apply
```

Optional docker sshd (example):

```bash
docker run --rm -p 2222:22 -e ROOT_PASSWORD=root lscr.io/linuxserver/openssh-server:latest
export INFRZEUG_SSH_HOST=root@127.0.0.1:2222
```
