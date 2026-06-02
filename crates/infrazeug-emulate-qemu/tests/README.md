# QEMU integration tests

From the repo root, download assets and run **all** workspace tests (including these)
with [`scripts/run-infra-tests.sh`](../../../scripts/run-infra-tests.sh).

The Debian microVM lab stack lives in `src/vm_stack.rs` behind `#[cfg(test)]`.

## Debian VM stack

Four bookworm cloud guests (`iz-db`, `iz-idp`, `iz-ui`, `iz-store`) each get a multicast
socket NIC on `192.168.100.0/24` (static via cloud-init) plus SSH host-forwarding.
The test checks Debian identity and per-VM LAN addressing (cross-VM ping is not required).

Prerequisites:

- `qemu-system-x86_64` or `qemu-system-aarch64`
- `qemu-img`, `genisoimage` or `mkisofs`
- `ssh` / `ssh-keygen` on PATH
- Debian 12 generic cloud qcow2 at `INFRZEUG_DEBIAN_CLOUD_IMAGE` or
  `~/.cache/infrazeug/debian-12-generic-amd64.qcow2`

```bash
curl -Lo ~/.cache/infrazeug/debian-12-generic-amd64.qcow2 \
  https://cdimage.debian.org/cdimage/cloud/bookworm/latest/debian-12-generic-amd64.qcow2

INFRZEUG_VM_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu debian_vm_stack_internal_network -- --ignored --nocapture
```

Optional: `INFRZEUG_QEMU_SSH_PUBKEY`, `INFRZEUG_QEMU_SSH_USER` (default `debian`),
`INFRZEUG_VM_STACK_MEM_MB` (default `768` per VM).

The stack also has a longer package-update idempotence test. It runs Debian
updates on all four guests twice, reboots only guests that changed after the
first pass, and requires the second update pass to report no changes:

```bash
INFRZEUG_VM_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu debian_vm_stack_agent_updates_are_idempotent -- --ignored --nocapture
```

## k3s + Helm stack

Single Debian microVM (`iz-k3s`) bootstraps k3s without flannel/kube-proxy, then installs
via Helm: **Cilium**, **OpenEBS** (hostpath only), **CloudNativePG**, and **Open WebUI**
(backed by a CNPG cluster on `openebs-hostpath`). Lives in `src/k3s_stack.rs`.

Prerequisites match the Debian VM stack, plus ~8 GiB RAM for the guest (override with
`INFRZEUG_K3S_STACK_MEM_MB`). First run typically needs 20–40 minutes for image pulls.

```bash
INFRZEUG_K3S_STACK_TEST=1 cargo test -p infrazeug-emulate-qemu k3s_helm_stack_cilium_openebs_cnpg_openwebui -- --ignored --nocapture
```
