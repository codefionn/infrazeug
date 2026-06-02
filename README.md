# infrazeug

![Infrazeug Logo: Throwing a lighter](logo.png)

Set your infrastructure on fire! Well, give it more passion than descruction.

**infrazeug** is from a new breed of infra-as-code tools. Infrastructure is
defined in actual code! Everything is basically a node, so no complicated
triggers or async machinary.

The focus of the current project is to build out:
- Extensions for easy getting started with e.g. provisioning resources on a
  cloud provider
- Push agent model (the pull model and agentless push will be focused on later)
- TUI experience
- Unified access patterns
- Tooling to visualize and explore infrastructure offline
- Fully self-contained units that the LLM can create without manual steps
  accept user review and apply

## Units

- Playbooks
- Machines
- Nodes
- Resources
- Vault
- Tags

A DAG of nodes is created, with one **Start** and one **End** node. A
playbook is planned (a node graph is build) and the executed.

## Planned

- Make it easier to reaquire resources api keys after some time (rolling api keys)
- Infrastructure validation and policy enforcement for resources

## Getting Started

### Build & Run

```bash
cargo build --workspace
cargo run -p hello-local -- plan -o /tmp/hello-local.plan   # plan
cargo run -p hello-local -- apply /tmp/hello-local.plan      # apply
```

### Test

```bash
cargo test --workspace                                     # unit + integration (fast)
```

Full infra tests (QEMU VMs, OCI containers, k3s stack — needs podman/docker, qemu-system-\*, ~12 GiB RAM):

```bash
./scripts/run-infra-tests.sh              # setup + all tests
./scripts/run-infra-tests.sh --setup-only # download images only
./scripts/run-infra-tests.sh --install-deps  # auto-install qemu/podman (Linux)
```

### Your Own Infra Project

Create a binary crate depending on `infrazeug-api`, `infrazeug-core`, and the crates you need:

```rust
use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, RunConfig, RunBuildContext};
use infrazeug_core::RuntimeConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("my-infra").about("My infrastructure"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    ).await
}

fn build_infra() -> anyhow::Result<infrazeug_core::Infra> {
    let infra = InfraBuilder::new()
        // .machine(...)?.shell_on_machine(...)?
        .build();
    Ok(infra.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("my-infra"),
        vault_store: None,
    }))
}
```

See `examples/` for local, SSH, container emulation, QEMU, pull-mode, and multi-machine cluster setups.

For a larger, real-world playbook, see the [authoring patterns guide](docs/playbook-patterns.md) — entry point, deterministic node IDs, per-host privilege, vars/feature gating, idempotent host prep, phased deployment, Helm releases, the two vault secret paths, and declarative cloud resources, distilled from a non-trivial consumer playbook.

Operational semantics worth reading before writing multi-node workflows:

- [Playbook authoring patterns](docs/playbook-patterns.md)
- [Run policy and change policy](docs/run-policy.md)
- [Runnable surface](docs/runnable.md)
- [Vault on-disk format](docs/vault-format.md)
