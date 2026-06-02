# Playbook authoring patterns

These patterns are distilled from the reference consumer playbook [infra-infrazeug](../infra-infrazeug) — an external example that drives real infrastructure (a Nebula/k3s Raspberry-Pi cluster plus a standalone external server), not part of this repo. They show how a non-trivial playbook is structured on top of `infrazeug-api`/`infrazeug-core`: how the binary is wired, how the node graph is composed, how secrets flow from the vault, and the idioms that keep `plan`/`apply` deterministic.

All `src/...` paths below are inside the reference playbook, not this framework repo.

---

## 0. Minimal playbook & inner loop

Before the (subsystem-scale) patterns below, start from the smallest thing that runs. A playbook is a `main` that calls `infrazeug_api::run` with a `RunConfig` and a factory that builds one `InfraBuilder` and `.build()`s it. See `examples/hello-local` and `docs/runnable.md`.

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    infrazeug_api::init_tracing();
    infrazeug_api::run(
        std::env::args(),
        RunConfig::new("my-stack").about("…").default_playbook("main"),
        |ctx| match ctx {
            RunBuildContext::Playbook(ctx) => build_main(&ctx),
            RunBuildContext::Pull(_) => build_main_default(),
        },
    )
    .await
}
```

**Your inner loop is `cargo run -- plan`** — it builds and topologically validates the whole graph offline (no SSH, no unlocked vault), so it catches dangling deps, mistyped seeds, and unregistered vault markers without touching infrastructure. Run it after every change.

**Reading guide.** §§1–6 are core (entry point, graph composition, inventory, node helpers, vars/gating, idempotent host prep) and apply to any playbook. §§7–12 are subsystem-scale patterns (Nebula, phased k3s, Helm, secrets, cloud APIs, codegen) you reach for as a playbook grows; read `docs/runnable.md` and `docs/run-policy.md` first. **§2's deterministic-seed contract is *the* must-read idiom** — get it wrong and edges dangle silently. §13 is an API reference map; §14 a pitfalls checklist.

---

## 1. Project skeleton & entry point

### Thin binary over `run` + a static `PlaybookRegistry`

**When:** you want the stock infrazeug CLI (`plan`/`apply`/`test`/`lint`/`graph`/`vault`/`mcp`) for your playbook, optionally exposing more than one named graph selectable with `--playbook`.

**Mechanism:** `main.rs` stays a ~15-line shim. Declare a `static PlaybookRegistry` of `PlaybookEntry { name, build }` where each `build` is a plain `fn(&RunContext) -> anyhow::Result<PlaybookBundle>` into `lib.rs`. `run(args, RunConfig::new("infra")…, |ctx| build_from_registry(&PLAYBOOKS, ctx))` drives the whole CLI; the same registry feeds the MCP server. `RunConfig` is a builder (`.about/.commands/.default_playbook/.extras/.mcp`). See `src/main.rs`.

```rust
static EXTRAS: [ExtraSubcommand; 0] = [];
static PLAYBOOKS: PlaybookRegistry = PlaybookRegistry {
    default: "default",
    entries: &[
        PlaybookEntry { name: "default",        build: infra_infrazeug::build_default },
        PlaybookEntry { name: "rolling-update", build: infra_infrazeug::build_rolling_update },
    ],
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("infra")
            .about("fionn-router infrastructure (infrazeug, migrated from Ansible)")
            .commands(RunCommands::ALL)
            .default_playbook("default")
            .extras(&EXTRAS)
            .mcp(mcp_server),
        |ctx| build_from_registry(&PLAYBOOKS, ctx),
    )
    .await
}
```

`ExtraSubcommand { name, about, run }` is the slot for genuinely non-graph commands; the array can be empty.

### Three-stage factory: `base_builder` → register step → `finalize`

**When:** multiple playbooks must share identical inventory/machine/group wiring and identical runtime/vault attachment, differing only in which node subsystems they register.

**Mechanism:** `base_builder(ctx)` constructs the `InfraBuilder`, picks the transport from the command context, attaches global vars + vault data keys, and loops the inventory registering groups and machines. `finalize(builder)` calls `.build()` then `.with_runtime(...)`. Each public playbook is `base_builder -> <its register step> -> finalize`, so the only difference is the middle call. See `src/lib.rs`.

```rust
fn base_builder(ctx: &RunContext) -> anyhow::Result<InfraBuilder> {
    let transport = transport_for_run(ctx).unwrap_or_else(default_remote_transport);
    let group_map = inventory::group_id_map();
    let mut builder = InfraBuilder::new()
        .default_remote_transport(transport)
        .global_vars(vars::global_varset())
        .vault_data_keys(vars::vault_data_keys());
    for group in inventory::all_groups() { builder = builder.group(group)?; }
    for host in inventory::HOSTS {
        let mut machine = inventory::machine_for(host, &group_map);
        attach_nebula_vault_vars(host.name, &mut machine);
        builder = builder.machine(machine)?;
    }
    Ok(builder)
}

pub fn build_default(ctx: &RunContext) -> anyhow::Result<PlaybookBundle> {
    let builder = base_builder(ctx)?;
    let builder = nodes::register_all(builder)?;
    finalize(builder)
}

pub fn build_rolling_update(ctx: &RunContext) -> anyhow::Result<PlaybookBundle> {
    let mut builder = base_builder(ctx)?;
    builder = crate::nodes::k3s::register_post_cluster_rolling(builder)?;
    builder = rolling_update::register(builder)?;
    finalize(builder)
}
```

### `RuntimeConfig` via `PlaybookBundle::with_runtime`

**When:** run-artifact location and vault-store path are site/environment-specific and must not be baked into nodes.

**Mechanism:** after `builder.build()`, attach `RuntimeConfig { run_root, vault_store }`, resolving env-var overrides. Note the override prefix is `INFRZEUG_` (no "A") — typing `INFRAZEUG_…` silently gets the default. See `finalize` in `src/lib.rs`.

```rust
Ok(builder.build().with_runtime(RuntimeConfig {
    run_root: std::env::var("INFRZEUG_RUN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_run_root()),
    vault_store,
}))
```

---

## 2. Graph composition & deterministic node IDs

### Dependency-ordered subsystem registration

**When:** a large playbook has dozens of node families with cross-family dependencies and you want one obvious place that defines global ordering.

**Mechanism:** every subsystem module exposes `pub fn register(mut builder) -> anyhow::Result<InfraBuilder>`. A single `register_all` threads the builder through each subsystem by reassignment. Ordering here is **documentation/readability** — the scheduler runs the topological sort of each node's `deps`, so a later-registered subsystem still needs explicit deps. When one family must straddle another (some prerequisites before nebula, some after), it splits its entry points (`register` + `register_after_nebula`). See `src/nodes/mod.rs`.

```rust
pub fn register_all(mut builder: InfraBuilder) -> anyhow::Result<InfraBuilder> {
    builder = connectivity::register(builder)?;
    builder = cloudflare_dns::register(builder)?;
    builder = ubiquity::register(builder)?;
    builder = prerequisites::register(builder)?;
    builder = external::register(builder)?;
    builder = nebula::register(builder)?;
    builder = prerequisites::register_after_nebula(builder)?;
    builder = k3s::register(builder)?;
    builder = cloud::register(builder)?;     // must precede apps (CNPG backup secret dep)
    builder = keycloak::register(builder)?;  // must precede apps (hermes client secret dep)
    builder = apps::register(builder)?;
    Ok(builder)
}
```

### Deterministic UUIDv5 node IDs from a single seeds module — the dep-wiring contract

**When:** node/machine/group IDs must be stable across every plan run (so re-applies match state and deps resolve by reconstruction), but you don't want to hand-maintain a UUID table. This is the single most load-bearing idiom in the playbook.

**Mechanism:** `node_id(seed)` hashes a human-readable, prefix-namespaced seed into a UUIDv5. A dependency is expressed by recomputing the **same** seed string — no registry of live ids is threaded around. Because a mistyped seed is a *dangling edge caught only at `plan`*, not a compile error, the infra/host/group seeds are funneled through one `seeds.rs` module: both the creator and every dependent call the same typed function, so a renamed/removed seed fn is a compile error. The identical idiom builds `machine_id(hostname)` and `group_id(name)`. See `src/nodes/mod.rs` and `src/nodes/seeds.rs`.

```rust
// src/nodes/mod.rs — the one hash function
pub fn node_id(seed: &str) -> NodeId {
    NodeId(Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("infra-infrazeug/node/{seed}").as_bytes(),
    ))
}

// src/nodes/seeds.rs — single owner; producer AND every dependent call these
pub fn connectivity(host: &str, machine: MachineId) -> NodeId {
    node_id(&format!("connectivity/{}/{}", host, machine.0))
}
pub fn nebula_install(host: &str) -> NodeId { node_id(&format!("nebula-install/{host}")) }
pub fn app(name: &str) -> NodeId { node_id(&format!("app-{name}")) }
```

Seed strings are **byte-frozen forever**: changing a seed (even a separator) silently re-creates the node under a new UUID and dangles every edge pointing at the old id. The seeds module doc bans editing existing seeds and directs new nodes to new functions.

### App / phase-6 node ids: matching string literals (not compile-checked)

The typed `seeds::*` funnel covers infra/host/group nodes. The **app layer** (Helm releases, kubectl manifests, localhost writes) instead keys nodes on a bare `&str`: `register_release`/`register_kubectl_node`/`register_local_write_node` take a `node_seed: &str`, derive the id via `seeds::app(node_seed)`, and consumers depend through `dep("name") = seeds::app("name")` (`dep` lives in `helm_release.rs`, not `seeds.rs`). Both ends are free string literals — a typo is a dangling edge caught only at `plan`, **not** a compile error. Mitigate by exporting cross-module app seeds as a `pub(super) const` (e.g. `LITELLM_VAULT_WRITE_SEED`); short-lived names shared within one batch (`dep("keycloak")`, `dep("harbor")`, `dep("synapse")`) are repeated bare. See `src/nodes/k3s/helm_release.rs`.

### Connectivity entry-gate nodes + reusable fan-in dep accessors

**When:** you want a cheap reachability gate per machine before touching it, and the first prerequisite batch needs to depend on the whole set of gates without rebuilding the id list.

**Mechanism:** the first registered subsystem emits one trivial `shell_node` per remote host (skipping localhost) running `host_op(host, &["true"])` (so it is `sudo -n true` on `become_root` Pi hosts), targeted at `Targets::Machine(mid)` and tagged `connectivity`. A companion accessor returns the `Vec<NodeId>` of all those gates so a later node can fan-in with one call — here it is consumed by exactly one node (`prerequisites/timesyncd.rs`); the rest of the graph chains off the prereq batches. The same accessor idiom (`batch1_node_ids`, `batch2_node_ids`, `k3s_marker_deps`) lets a subsystem publish exactly which of its ids consumers should depend on. See `src/nodes/connectivity.rs`, `src/nodes/prerequisites/mod.rs`.

```rust
// prerequisites/mod.rs — reusable fan-in accessor, single-sourced
pub fn connectivity_deps() -> Vec<NodeId> {
    HOSTS.iter().filter(|h| h.name != "localhost")
        .map(|h| seeds::connectivity(h.name, crate::inventory::machine_id(h.name)))
        .collect()
}
```

> The framework auto-inserts begin/finish/connect bookend nodes around the graph, so node-count assertions in plan tests must exclude them (an off-by-2 usually means the bookends).

---

## 3. Inventory & per-host configuration

### Static typed `HOSTS` table as the single source of truth

**When:** porting an Ansible `inventory.yml` to one compile-time, type-checked declaration that every `register` fn can iterate.

**Mechanism:** declare hosts as a `const` slice of a `Copy` struct. Each `HostSpec` carries the hostname (which seeds the `MachineId`), ssh user, `become_root` privilege flag, a `hosttype` (drives arch + behavioural branches), and the group names it belongs to. The `base_builder` loop turns each into a `Machine` via `machine_for`; every subsystem re-iterates `HOSTS` to decide which hosts get its node. See `src/inventory.rs`.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostSpec {
    pub name: &'static str,
    pub ssh_user: &'static str,
    pub become_root: bool,
    pub hosttype: &'static str,
    pub groups: &'static [&'static str],
}
pub const HOSTS: &[HostSpec] = &[
    HostSpec { name: "os.codefionn.eu", ssh_user: "root", become_root: false,
        hosttype: "debian-external", groups: &["global", "debian", "nebula_vpn"] },
    HostSpec { name: "raspberrypi-5-0.fionn-router.internal", ssh_user: "user",
        become_root: true, hosttype: "raspberrypiwithssd",
        groups: &["global", "debian", "raspberry_cluster", "nebula_vpn"] },
    // ...
];
```

> `all_groups()` and `group_id_map()` are two parallel sources of truth that must list the same group names — add a group to one and not the other and machines silently lose membership.

### Deterministic Machine/Group IDs

Same UUIDv5 idiom as node ids, over a fixed namespace and a formatted seed string. Any module recomputes `inventory::machine_id(host.name)` to target a host without threading a registry. The seed *prefix* (`"fionn-router.internal/machine/{hostname}"`) is load-bearing — change it and a hardcoded cross-reference breaks silently. See `src/inventory.rs`.

```rust
pub fn machine_id(hostname: &str) -> MachineId {
    MachineId(Uuid::new_v5(&Uuid::NAMESPACE_DNS,
        format!("fionn-router.internal/machine/{hostname}").as_bytes()))
}
pub fn group_id(name: &str) -> GroupId {
    GroupId(Uuid::new_v5(&Uuid::NAMESPACE_OID,
        format!("fionn-router.internal/group/{name}").as_bytes()))
}
```

### `HostSpec` → `Machine` translation with per-host quirks

**Mechanism:** `machine_for` builds `builder::local` for localhost else `builder::remote` with an `SshConfig` from `ssh_for` (which special-cases a host to `.ipv6_only()`). It mutates `MachineKind::Remote { os, .. }` to attach an `OsHint` derived from `hosttype` (skips a runtime `uname` probe), resolves group names to `GroupId`s, and tags the machine with `Tag::new("hosttype", host.hosttype)` so nodes/CLI can select by hardware class. See `src/inventory.rs`.

```rust
pub fn machine_for(host: &HostSpec, group_ids: &HashMap<&str, GroupId>) -> Machine {
    let id = machine_id(host.name);
    let mut m = if host.name == "localhost" {
        builder::local(id, host.name)
    } else {
        builder::remote(id, host.name, ssh_for(host))
    };
    if let MachineKind::Remote { os, .. } = &mut m.kind { *os = os_hint_for(host); }
    for g in host.groups { if let Some(gid) = group_ids.get(g) { m.groups.push(*gid); } }
    m.tags.push(Tag::new("hosttype", host.hosttype));
    m
}
```

### Per-host config as host-keyed `Option` lookups gating node emission

**When:** porting Ansible `host_vars`, where only some hosts have a given setting (static IPv6, a bond, a Nebula role).

**Mechanism:** each host_var becomes a `fn(hostname: &str) -> Option<SomeCopyStruct>` implemented as a `match`. A register fn does `let Some(cfg) = host_config::x(host.name) else { continue; }` — or emits a no-op skip node under the same seed (see §6) so the graph only contains nodes for hosts carrying the setting. A hostname typo yields `None`, which most register fns treat as "skip" with no error, so `plan` won't catch a missing-but-expected host. See `src/host_config.rs`.

```rust
pub fn static_ipv6(hostname: &str) -> Option<StaticIpv6> {
    match hostname {
        "os.codefionn.eu" => Some(StaticIpv6 {
            interface: "ens6", address: "2a01:239:38c:3a00::1/64", gateway: Some("fe80::1"),
        }),
        _ => None,
    }
}
```

When one node must render the *entire* fleet (a Nebula `static_host_map`, a peer list) rather than its own host, expose a whole-fleet table instead of a per-host `Option`: `nebula_peers() -> &'static [(&str, NebulaPeer)]` returns every peer for the renderer to walk. See `src/host_config.rs`.

### Group/role membership predicates to gate fan-out

`in_group(host, name)` is a `contains` over `HostSpec.groups`. Derived roles with no inventory group (k3s control-plane, external-server matched by hosttype) get their own boolean helpers. Register fns iterate `HOSTS` and `continue` past rejected hosts — the direct translation of an Ansible `hosts: <group>` selector. Group-fan-out loops must explicitly exclude `localhost` (it is in `HOSTS` for controller-side work but is not a real cluster member). See `src/inventory.rs`, `src/host_config.rs`.

```rust
for host in HOSTS {
    if !in_group(host, "raspberry_cluster") || host.name == "localhost" { continue; }
    /* emit node for this host */
}
```

### Bridging per-host config into machine vars as vault references

**When:** a per-host setting is secret-shaped (each Nebula host needs its own cert/key under per-host vault fields).

**Mechanism:** after `machine_for`, a per-host hook reads the same host-keyed config struct, pulls the host-specific vault field names off it, and inserts `VarValue::Vault(VaultRef::field(path, field))` into `machine.vars` (resolved at apply — see §10). Field-name strings live on the per-host config struct so inventory data and vault layout stay co-located. See `attach_nebula_vault_vars` in `src/lib.rs`.

```rust
fn attach_nebula_vault_vars(hostname: &str, machine: &mut Machine) {
    let Some(cfg) = host_config::nebula_config(hostname) else { return; };
    // three per-host vault refs: CA cert, host cert, host key
    machine.vars.insert(VarKey::new("nebula_ca_crt"),
        VarValue::Vault(VaultRef::field(vault_paths::NEBULA, vault_paths::nebula_ca_field())));
    machine.vars.insert(VarKey::new("nebula_host_crt"),
        VarValue::Vault(VaultRef::field(vault_paths::NEBULA,
            vault_paths::nebula_cert_field(cfg.cert_vault_field))));
    machine.vars.insert(VarKey::new("nebula_host_key"),
        VarValue::Vault(VaultRef::field(vault_paths::NEBULA,
            vault_paths::nebula_cert_field(cfg.key_vault_field))));
}
```

---

## 4. Node-construction helpers

Most of these live in `src/nodes/helpers.rs` and are the chokepoint every node passes through.

### One-call registration tail: `NodeSpec` + `add_node`

**When:** the common case — build an op, then describe/tag/dep/register it without repeating five steps inline (which invites omissions like a node registered without tags).

**Mechanism:** bundle everything besides the already-built `ShellOp` into a plain `NodeSpec { id, name, op, target, deps, tags, description }` and hand it to `add_node`, which builds the shell node, applies the optional description, tags, deps, and registers it. `name` is byte-exact (it is what shows in TUI/plan); `target` is always `Targets::Machine(mid)` here (the `Machines`/`All` variants exist but are unused in this playbook).

```rust
pub fn add_node(builder: InfraBuilder, spec: NodeSpec) -> anyhow::Result<InfraBuilder> {
    let mut node = shell_node(spec.id, spec.name, spec.op, spec.target);
    if let Some(description) = spec.description { node = node.with_description(description); }
    tags::apply(&mut node, &spec.tags);
    node.deps = spec.deps;
    builder.node(node)
}

// call site (external/acme.rs)
add_node(builder, NodeSpec {
    id: seeds::external_acme(),
    name: "external-acme@os.codefionn.eu".into(),
    op: op(host),
    target: Targets::Machine(mid),
    deps: vec![seeds::external_haproxy()],
    tags: ansible::ACME.to_vec(),
    description: Some("acme.sh: register account, issue ECC certs, deploy ...".into()),
})
```

### Manual per-machine path: `shell_host_node` + chained describe/tag/deps

**When:** deps must be computed conditionally before registering. `shell_host_node(id, name, host, mid, op)` is a thin wrapper over `shell_node` that hard-codes `Targets::Machine(mid)`. From there chain `.with_description`, `tags::apply`, mutate `node.deps`, and finish with `builder.node(node)?`.

```rust
let mut node = shell_host_node(
    seeds::nebula_install(host.name),
    format!("nebula-install@{}", host.name),
    host, mid, ShellOp::Seq { steps },
).with_description("Install Nebula VPN: download binary, deploy certs, ...");
tags::apply(&mut node, ansible::NEBULA);
let mut deps = prerequisites::batch1_node_ids(host.name);
if host.name == "os.codefionn.eu" && defaults().raspberry_cluster_enable_archlinux_mirror {
    deps.push(seeds::prereq_archlinux_mirror(host.name));
}
node.deps = deps;
builder = builder.node(node)?;
```

### Per-host privilege — never hardcode `sudo`

**Privilege is data, not code.** The root/sudo decision lives in `HostSpec.become_root` (Pi hosts SSH as an unprivileged user and need `sudo -n`; root/local hosts run verbatim). `sudo_wrap(host, inner)` branches on the flag; `host_op` is its public alias; `host_script(host, script)` specializes to `sh -ec <script>` for shell features (pipes, here-docs). Every ShellOp on a managed host is built through these — a literal `"sudo"` double-sudoes on Pi hosts and is wrong on root hosts.

```rust
pub fn sudo_wrap(host: &HostSpec, inner: &[&str]) -> ShellOp {
    if host.become_root {
        let mut argv: Vec<String> = vec!["sudo".into(), "-n".into()];
        argv.extend(inner.iter().map(|s| (*s).to_string()));
        ShellOp::run(argv)
    } else {
        ShellOp::run(inner.iter().map(|s| (*s).to_string()).collect())
    }
}
pub fn host_op(host: &HostSpec, inner: &[&str]) -> ShellOp { sudo_wrap(host, inner) }
pub fn host_script(host: &HostSpec, script: &str) -> ShellOp { host_op(host, &["sh", "-ec", script]) }
```

### Root-owned file writes: stage-in-`/tmp`-then-`sudo install`

**When:** `ShellOp::write_file` writes as the SSH user, which on `become_root` hosts cannot reach `/etc/...`. `write_root_path_file(host, path, mode, FileSource)` is the only correct way to land controller-side bytes or a vault secret on a root-owned path.

**Mechanism:** on `become_root` hosts it emits a `ShellOp::Seq` of four steps — `mkdir -p /tmp/infrazeug-stage`, `write_file` to a slugged stage path, a sudo-wrapped `install -m <mode> -D` (paths single-quote-escaped), then `rm -f`. On root hosts it collapses to a plain `write_file`. The staged content is delivered as a `FileSource` (never interpolated into the shell string), so secrets never hit a command line. `write_root_file` (`&str`) and `write_root_bytes` (`impl AsRef<[u8]>`) are convenience wrappers; use `write_root_path_file` directly for a `FileSource::vault` secret (resolved at apply — see §10).

```rust
pub fn write_root_path_file(host: &HostSpec, path: &str, mode: u32, content: FileSource) -> ShellOp {
    if host.become_root {
        let stage = stage_path(path);
        ShellOp::Seq { steps: vec![
            ShellOp::run(vec!["mkdir".into(), "-p".into(), "/tmp/infrazeug-stage".into()]),
            ShellOp::write_file(stage.clone(), content, mode),
            host_op(host, &["sh", "-ec",
                &format!("sudo -n install -m {mode:o} -D '{stage_esc}' '{path_esc}'")]),
            ShellOp::run(vec!["rm".into(), "-f".into(), stage]),
        ]}
    } else {
        ShellOp::write_file(path, content, mode)
    }
}
```

### Config & secrets as environment variables

**Mechanism:** `ShellOp::run(argv).env(key, FileSource)` attaches an env var resolved at apply; with a vault source the secret never lands on the command line (`cnpg.rs` loops a `DbApp` table adding one `.env(app.password_env, FileSource::vault(file, field))` per app). For non-secret config, build an env-prefix string from typed defaults and prepend it to an embedded script (`setup_env_for(host, role)` → `K3S_ROLE=… K3S_VERSION=… /usr/local/bin/k3s-setup.sh` in `src/nodes/k3s/common.rs`).

```rust
let mut run = ShellOp::run(argv!["sh", "-ec", &shell_script]);
for app in &apps {
    run = run.env(app.password_env,
        FileSource::vault(vault_paths::RASPBERRY_CLUSTER, app.password_vault_field));
}
```

### Tags as `key==value` for bare-key `--tag` matching

**When:** you want `apply --tag nginx` / `graph --tag nginx` Ansible-style, but infrazeug matches a bare needle against `tag.key`.

**Mechanism:** declare the tag vocabulary as `pub const FOO: &[&str]` slices in `src/nodes/tags.rs` (mirroring old Ansible tag names). `tags::apply(node, &[..])` pushes each as `Tag::new(name, name)` — identical key and value — so the bare-key rule matches. `with_tags(node, tags)` is the standalone, ownership-taking variant for the manual node path; `add_node` calls `tags::apply` internally.

```rust
pub fn apply(node: &mut Node, tags: &[&str]) {
    for name in tags { node.tags.push(Tag::new(*name, *name)); }
}
pub mod ansible {
    pub const CONNECTIVITY: &[&str] = &["connectivity"];
    pub const NEBULA: &[&str] = &["nebula"];
    pub const NGINX: &[&str] = &["nginx", "web"];
}
```

---

## 5. Vars, defaults & feature gating

### Single typed defaults struct + memoized TOML accessor

**When:** one typo-safe source of truth for hundreds of non-secret knobs (image tags, namespaces, sizes, domains, feature flags), parsed once, reachable from any module.

**Mechanism:** one flat `#[derive(Debug, Deserialize, Serialize, Clone)]` struct (`PlaybookDefaults`) whose fields map 1:1 to keys in a checked-in `vars/defaults.toml`. `defaults()` lazily `include_str!`s and `toml::from_str`s once into a `OnceLock`, returning `&'static`. A missing/renamed key panics at startup; a misspelled field access is a compile error. See `src/vars.rs`.

```rust
static DEFAULTS: OnceLock<PlaybookDefaults> = OnceLock::new();
pub fn defaults() -> &'static PlaybookDefaults {
    DEFAULTS.get_or_init(|| {
        let raw = include_str!("../vars/defaults.toml");
        toml::from_str(raw).expect("vars/defaults.toml must parse")
    })
}
```

### Typed defaults flattened into the runtime `VarSet` (`global_varset`)

**Mechanism:** `global_varset()` serializes the whole `defaults()` struct to JSON, iterates the object, and inserts each `(key, value)` into a `VarSet` as `VarValue::Scalar`. Wired in once via `InfraBuilder::new().global_vars(vars::global_varset())`. Every TOML key automatically becomes a global var addressable by string key across the graph. See `src/vars.rs`.

```rust
pub fn global_varset() -> VarSet {
    let d = defaults();
    let mut vars = VarSet::new();
    if let Ok(json) = serde_json::to_value(d) {
        if let Some(obj) = json.as_object() {
            for (k, v) in obj { vars.insert(VarKey::new(k), VarValue::Scalar(v.clone())); }
        }
    }
    // ... vault fields appended below (see §10) ...
    vars
}
```

> Single flat namespace: every struct field == one TOML key == one VarSet key. Prefix every knob with its component (`litellm_*`, `hermes_webui_*`) — `global_varset()` silently overwrites on a duplicate `VarKey`.

### Per-subsystem / per-app `*_enable` gating

**When:** subsystems and individual apps must be switchable on/off from config, and a disabled component must not appear in the scheduled graph (no half-built nodes, no dangling deps).

**Mechanism (two layers):** (1) a subsystem's `register` does an early `if !defaults().<x>_enable { return Ok(builder); }`; (2) a parent dispatcher conditionally calls each app's registrar. Flipping a flag in `defaults.toml` cleanly includes/excludes a whole branch — verified offline by `cargo run -- plan`.

```rust
pub fn register(mut builder: InfraBuilder) -> anyhow::Result<InfraBuilder> {
    if !defaults().nebula_enable { return Ok(builder); }
    // ... add nodes ...
}
// parent dispatch, gate per app:
if v.litellm_enable {
    specs.push(HelmReleaseSpec::local("litellm", &v.litellm_namespace, values::litellm(v))
        .deps(vec![pg.clone()])
        .describe("Deploy LiteLLM proxy with multi-provider LLM routing via Helm"));
}
```

### Decoupled provisioning gates, compound gates & `flag.then(seed)`

**When:** a prerequisite (an SSO client + sealed secret, a cloud bucket/IAM user) must be created *before* the app that consumes it is turned on, or one feature legitimately depends on another. A single boolean per app can't express that ordering.

**Mechanism:** introduce a separate enable flag for the prerequisite, independent of the app's deploy flag. Helper accessors return `Option<NodeId>` keyed on the provisioning flag (`flag.then(seed)`), so a downstream registrar adds a dep **only when the prerequisite exists**, avoiding dangling deps when the producer is gated off. Compound gates AND two flags. See `src/nodes/keycloak.rs`, `src/nodes/k3s/etcd_backup.rs`.

```rust
pub fn hermes_webui_secret_node() -> Option<NodeId> {
    defaults().hermes_webui_oauth2_client_enable
        .then(seeds::keycloak_hermes_webui_secret)
}
// consumer:
deps.extend(crate::nodes::keycloak::hermes_webui_secret_node());
// compound gate:
if v.nextcloud_backup_enable && v.ovh_backup_enable { /* ... */ }
```

> Some hard orderings are runtime asserts, not type-level guarantees (`k3s_etcd_backup_enable requires ovh_backup_enable`).

### Typed defaults rendered into node payloads via `template!`

**When:** a node payload (Helm values YAML, config, script) interpolates dozens of config values and you want compile-checked field substitution.

**Mechanism:** a per-app `values::<app>(v: &PlaybookDefaults) -> String` uses `template!(r#"..."#, v = v)` with `{{ v.field }}` placeholders bound to the typed struct. Secrets are **not** substituted here — they stay as the `"" # vault:key` marker resolved later (see §9/§10). See `src/nodes/apps/values.rs`.

```rust
pub fn litellm(v: &PlaybookDefaults) -> String {
    template!(r#"namespace: {{ v.litellm_namespace }}
image: {{ v.litellm_image }}
masterKey: "" # vault:litellm_master_key
postgres:
  host: {{ v.litellm_postgres_host }}
"#, v = v)
}
```

---

## 6. Idempotent host prep

Because `apply` re-runs the whole graph against a live host, **idempotency is the author's responsibility inside the shell**, not the framework's.

### Idempotent `ShellOp::Seq` recipe: ensure → write → validate → enable → verify

**Mechanism:** build the body as an ordered `ShellOp::Seq { steps }` of sudo-aware helper ops. The Seq aborts on the first non-zero exit, so order matters: (1) ensure the tool exists (`command -v X || install`), (2) `mkdir -p`, (3) write config (`write_root_file`), (4) **validate** (`nft -f`, `haproxy -c`, `nginx -t`) before any restart, (5) `systemctl enable --now`, (6) a cheap verify. Idempotency lives in shell idioms: `command -v || install`, `grep -q '^line' file || echo line >> file` (append-once), a version-marker file written next to a binary and re-installed only when missing or stale (`.promtail-version`), cross-distro fallbacks. See `src/nodes/prerequisites/*.rs`, `src/nodes/external/haproxy.rs`, `src/nodes/external/promtail.rs`.

```rust
ShellOp::Seq { steps: vec![
    host_op(host, &["sh", "-ec", "apt-get update -qq && apt-get install -y -qq haproxy"]),
    write_root_file(host, "/etc/haproxy/haproxy.cfg", 0o644, &cfg),
    // Validate BEFORE (re)starting; a bad config must not take the service down.
    host_op(host, &["haproxy", "-c", "-f", "/etc/haproxy/haproxy.cfg"]),
    host_op(host, &["systemctl", "enable", "--now", "haproxy"]),
    host_op(host, &["systemctl", "reload-or-restart", "haproxy"]),
] }
```

### Retry loop (Ansible `until`/`retries`/`delay`)

`retry_shell(script, retries, delay_secs)` wraps a check in a bounded `while` loop that re-runs until success or the budget is exhausted; `k3s_until(script)` specializes it with `k3s_until_retries`/`k3s_until_delay_secs` from defaults. This is the direct translation of an Ansible `until:` task — distinct from the stdout-sentinel `OutputChangePolicy` below (which is change classification, not waiting). See `src/nodes/k3s/common.rs`.

### Stdout-sentinel idempotence: `RunPolicy::Always` + `OutputChangePolicy`

**When:** a command exits 0 whether or not it changed anything (`apt-get upgrade`, `pacman -Syu`, a kernel check), so exit status alone can't drive `OnUpstreamChange` successors.

**Mechanism:** set `RunPolicy::Always` so the node always runs, print a known marker (`__INFRAZEUG_UNCHANGED__`) on the no-op branch, and attach `OutputChangePolicy` with one `OutputChangeRule::unchanged_when_contains(Stdout, MARKER)` to `policy.success.change_policy`. The node-level classifier promotes the marker to `Unchanged`, so default-`OnUpstreamChange` successors are skipped. **You need both** — `Always` without the policy gives unconditional re-fires; the policy without `Always` means the check may itself be skipped. See `docs/run-policy.md` and `src/rolling_update.rs`, `src/nodes/prerequisites/kernel_rebuild.rs`. (The current field path is `node.policy.{run_policy, success.change_policy}`; `run-policy.md`'s code predates the `policy` grouping and won't compile verbatim.)

```rust
const UNCHANGED_MARKER: &str = "__INFRAZEUG_UNCHANGED__";
fn skip_change_policy() -> OutputChangePolicy {
    OutputChangePolicy { rules: vec![OutputChangeRule::unchanged_when_contains(
        OutputMatchStream::Stdout, UNCHANGED_MARKER)] }
}
upgrade.policy.run_policy = RunPolicy::Always;
upgrade.policy.success.change_policy = skip_change_policy();
// shell side: if [ -n "$(pacman -Qu)" ]; then pacman -Syu --noconfirm; else echo '__INFRAZEUG_UNCHANGED__'; fi
```

This also drives the **check-only vs mutating split**: a cheap `RunPolicy::Always` check node emits the marker when satisfied; an expensive `RunPolicy::OnUpstreamChange` install node only runs when the check reported `Changed` (and re-checks at its own top, emitting the marker so a redundant run still classifies `Unchanged` and the reboot is skipped).

### Lazy, deduplicated shared work fanned-in from per-host checks

**When:** expensive work that produces a shared artifact (one kernel build per board family serves N Pis) must run at most once per target, only when at least one consuming host needs it.

**Mechanism:** key the shared node by a `target.key`, register it once behind a `HashSet` guard, give it `RunPolicy::Lazy` (dormant until pulled), and wire its `deps` to all per-host check ids for that target (collected in a first pass into a `HashMap<target_key, Vec<NodeId>>`). If every check classifies `Unchanged`, the build is never demanded. See `src/nodes/prerequisites/kernel_rebuild.rs`.

```rust
if registered_targets.insert(target.key.clone()) {
    let mut cross_build = shell_node(cross_build_id, name, cross_build_op(&target),
        Targets::Machine(local_mid));
    cross_build.policy.run_policy = RunPolicy::Lazy;
    cross_build.policy.timeout = Some(Duration::from_secs(defaults().kernel_build_timeout_secs));
    cross_build.deps = checks_by_target.get(&target.key).cloned().unwrap_or_default();
    builder = builder.node(cross_build)?;
}
```

### Reboot gate via `PostRunPolicy::ExpectReboot`

**When:** a step needs the host to reboot. A naive remote reboot kills the SSH transport and looks like a failure.

**Mechanism:** reuse the long-standing downstream-referenced seed for the reboot node (so dependents need no rewiring), apply the UNCHANGED marker policy (default `OnUpstreamChange`, fires only when the install changed), and set `policy.post_run = PostRunPolicy::ExpectReboot { readiness_check: Some(verify_op) }`. Core tolerates the connection drop, waits for the host, then runs the readiness check (re-inspecting `/proc/config.gz`, or `kubectl wait --for=condition=Ready`) before releasing the rest of the DAG. See `src/nodes/prerequisites/kernel_rebuild.rs`, `crates/infrazeug-core/src/node.rs`.

```rust
reboot.policy.success.change_policy = skip_change_policy();  // skip when install was Unchanged
reboot.policy.post_run = PostRunPolicy::ExpectReboot {
    readiness_check: Some(verify_op(host)),
};
```

### No-op gate node under the canonical seed for skipped hosts/branches

**When:** downstream code references `seeds::prereq_X(host)` for *every* host. If a host needs no real work, simply not registering the node leaves a dangling dep that `plan` rejects.

**Mechanism:** always register *something* under the canonical seed. For a no-work host/branch, register a placeholder with `op: ShellOp::run(argv!["true"])` using the exact same seed function and the upstream deps appropriate for that host (often the same the real node would have, e.g. `static_ipv6.rs`; the non-Pi kernel placeholder instead deps only on the systemd-timeout node). Optionally suffix the name (`-skip@`, as `static_ipv6.rs` does) for readability while the id stays canonical. See `kernel_rebuild.rs` (non-Pi host), `static_ipv6.rs` (no config).

```rust
builder = add_node(builder, NodeSpec {
    id: seeds::prereq_kernel(host.name),
    name: format!("prereq-kernel@{}", host.name),   // static_ipv6.rs uses "...-skip@{}"
    op: ShellOp::run(argv!["true"]),
    target: Targets::Machine(mid),
    deps: vec![timeout_node],
    tags: ansible::KERNEL_REBUILD.to_vec(),
    description: Some("No kernel rebuild required (non-Pi host)".into()),
})?;
```

### Two-batch register split + node-id aggregators

**When:** some prep must run before a mid-graph barrier (Nebula must come up first) and some only after; both belong to the same subsystem. Other subsystems need to depend on "all prerequisites" without knowing the internal list.

**Mechanism:** split into `register(builder)` (batch 1, before `nebula::register`) and `register_after_nebula(builder)` (batch 2, after). Expose `batch1_node_ids`/`batch2_node_ids`/`k3s_marker_deps(host)` aggregators so a downstream barrier depends on the whole batch via one call. The **same predicate** that gates node creation must gate dep-membership in the aggregator, or a barrier depends on a node that was never registered. See `src/nodes/prerequisites/mod.rs`.

```rust
pub fn k3s_marker_deps(host: &str) -> Vec<NodeId> {
    let mut deps = batch2_node_ids(host);
    if defaults().nebula_enable
        && host_by_name(host).is_some_and(|h| in_group(h, "nebula_vpn")) {
        deps.push(seeds::nebula_ready(host));
    }
    deps
}
```

---

## 7. The canonical module shape (nebula)

`src/nodes/nebula/` is the reference example of a full subsystem; it composes the idioms above.

**Thin `mod.rs` aggregator + gated leaf `register`s.** `mod.rs` declares private submodules and chains their `register` calls; each leaf early-returns on `!defaults().nebula_enable`, loops `HOSTS` (filtering by `in_group` and skipping `localhost`), and threads the builder back in.

```rust
// nebula/mod.rs
pub fn register(mut builder: InfraBuilder) -> anyhow::Result<InfraBuilder> {
    builder = install::register(builder)?;
    builder = readiness::register(builder)?;
    Ok(builder)
}
```

**Compose host work as `ShellOp::Seq` with conditional steps.** Build a `Vec<ShellOp>` of privilege-aware helper ops, `push` optional sections behind `if defaults().nebula_ping_enable` / `if cfg.masquerade`, then wrap in `ShellOp::Seq { steps }`. **Vault secrets** flow through `FileSource::vault(...)` into `write_root_path_file` (see §10). See `src/nodes/nebula/install.rs`.

```rust
let mut steps = vec![
    host_op(host, &["sh", "-ec", &format!("curl -fsSL -o /tmp/nebula.tar.gz '{url}' && ...")]),
    host_op(host, &["mkdir", "-p", "/etc/nebula"]),
    write_root_path_file(host, "/etc/nebula/host.key", 0o600,
        FileSource::vault(vault_paths::NEBULA, vault_paths::nebula_cert_field(cfg.key_vault_field))),
    write_root_file(host, "/etc/systemd/system/nebula.service", 0o644, NEBULA_SERVICE),
    host_op(host, &["systemctl", "enable", "--now", "nebula.service"]),
];
if defaults().nebula_ping_enable {
    steps.push(host_op(host, &["systemctl", "enable", "--now", "nebula-ping.timer"]));
}
```

**Separate readiness/barrier node.** Installing is not the same as proving health. A sibling `readiness.rs` registers a second per-host node whose op is purely verification (`systemctl is-active`, interface-up, ping the lighthouse) with `node.deps = vec![seeds::nebula_install(host.name)]`. Because its id is `seeds::nebula_ready(host)`, later subsystems depend on **readiness** rather than install, making the health check a graph barrier.

```rust
let mut node = shell_node(
    seeds::nebula_ready(host.name),
    format!("nebula-ready@{}", host.name),
    ShellOp::Seq { steps },
    Targets::Machine(mid),
);
node.deps = vec![seeds::nebula_install(host.name)];
```

---

## 8. Phased deployment & barriers

The k3s rollout is staged (control plane → CNI → join nodes → storage → infra services → apps). See `src/nodes/k3s/`.

### Phase-barrier nodes (fan-in / fan-out no-op markers)

**When:** stages live in separate modules with many-to-many edges; wiring every downstream node to every upstream is unmaintainable.

**Mechanism:** each phase ends in one **barrier** node that fans-in (depends on every node in the phase) and fans-out (is the single id the next phase depends on). It is either a no-op shell node (`ShellOp::run(argv!["true"])`) with description/tags, or a dep-only `barrier_node(...)`. Downstream code reconstructs the barrier's seed to depend on the whole phase without naming its internals — so "phase N done" is a stable single dependency vertex.

```rust
let mut node = shell_node(
    seeds::k3s_phase_6(), "k3s-phase-6-applications",
    ShellOp::run(argv!["true"]), Targets::Machine(mid),
).with_description("Barrier: all K3s infrastructure is ready, applications may deploy");
tag_phase(&mut node, ansible::K3S_PHASE_6);
node.deps = vec![seeds::k3s_phase_5()];
builder.node(node)
```

> Barriers are load-bearing across module boundaries: `apps/phase6.rs` depends on `seeds::k3s_phase_6()`, and many phase-5 services chain on `seeds::k3s_cert_manager()`. Removing/renaming a barrier breaks distant modules with no local indication.

### Feature-flag-gated registration with conditionally-built dep lists

**When:** optional infrastructure (Prometheus, Loki, CNPG) must be addable/removable without rewiring, and the verify/barrier node must wait only on whatever was actually registered.

**Mechanism:** every optional `register_*` early-returns the unchanged builder when disabled. Dependency lists are built imperatively — start from a base `Vec<NodeId>`, then `push` extra deps under the **same** `if d.<x>_enable` guard that gated registration, keeping the deps vector exactly in sync with which nodes exist. See `src/nodes/k3s/phase5.rs`.

```rust
let mut deps = vec![seeds::k3s_coredns(), seeds::k3s_cert_manager(), seeds::k3s_metrics_server()];
if d.prometheus_enable { deps.push(seeds::k3s_prometheus()); }
if d.loki_enable { deps.push(seeds::k3s_loki()); }
if d.cloudnative_pg_enable { deps.push(seeds::k3s_cloudnative_pg()); }
```

### Sequential cross-host rollout via a `prev` dependency edge + locks

**When:** a per-host operation that disrupts service (restarting k3s on every control-plane, a rolling OS upgrade) must not run concurrently across the cluster.

**Mechanism:** sort hosts into the desired order (server first via a rank function), then loop building one node per host while threading `prev: Option<NodeId>` — each node depends on its own prerequisites **plus** the previous host's node id, serializing an otherwise-parallel set into a chain. For destructive rollouts, set the lock fields under `node.policy.locks` (a global lock = one rollout cluster-wide; a local lock = serialize the package manager on a host), and pair the reboot node with `PostRunPolicy::ExpectReboot`. See `src/nodes/k3s/etcd_backup.rs`, `src/rolling_update.rs`.

```rust
let mut prev: Option<NodeId> = None;
for host in hosts {                       // sorted: server first
    let nid = seeds::k3s_etcd_backup(host.name);
    let mut node = shell_node(nid, ..., backup_config_op(host), Targets::Machine(...));
    let mut deps = vec![seeds::k3s_phase_2_wait_ready()];
    deps.extend(prev);                    // chain on predecessor -> one host at a time
    node.policy.locks.global_locks = vec!["rolling-deploy".into()];
    node.policy.locks.local_locks = vec!["pkg-manager".into()];
    node.deps = deps;
    builder = builder.node(node)?;
    prev = Some(nid);
}
```

---

## 9. Helm releases & values

### Declarative `HelmReleaseSpec` builder over `helm upgrade --install`

**When:** installing many charts that each need the same scaffolding (repo add/update, namespace, staged values, adopt pre-existing resources, version pin, wait/timeout, skip-if-healthy) but local and remote charts differ in defaults.

**Mechanism:** `HelmReleaseSpec` is a fluent builder with two constructors encoding the common cases — `local(name, ns, values)` (chart subpath under the synced charts dir; `adopt`+`wait` on, `skip_if_healthy` **off** so config edits aren't silently skipped) and `remote(name, ns, chart_ref, repo, values)` (adds repo, `skip_if_healthy` on). Setters: `.deps`, `.pre`, `.no_wait`, `.wait_timeout`, `.version`, `.describe`. `build_op` composes the ordered `ShellOp::Seq`; `register_release` attaches phase-6 base deps + tags. Simpler nodes call `server_helm_op`, which calls the `Helm::upgrade_install` method directly. See `src/nodes/k3s/helm_release.rs`, `src/nodes/k3s/helm.rs`.

```rust
pub fn local(name: &'static str, namespace: &str, values_yaml: String) -> Self {
    Self { release: name.into(), namespace: namespace.into(),
        chart: ChartRef::Local { subpath: name }, values_yaml,
        adopt: true, wait: true, skip_if_healthy: false, // local: never skip; config edits must apply
        wait_timeout: Some("15m".into()), .. }
}
```

A `common.rs` exposes pinned kubectl/helm constructors so call sites don't repeat flags: `kubectl()` adds `--kubeconfig`, while `kubectl_no_kubeconfig()` omits it (`k3s kubectl` resolves the config itself — the generated argv differs, so pick deliberately); `helm_in_namespace(ns)` pins kubeconfig + namespace. See `src/nodes/k3s/common.rs`.

### Spec-table of releases joined as one `AsyncNodeGroup`

**When:** N optional apps, each a release, that should install concurrently (blocked only by real deps), be toggleable, and expose a single "all apps up" join point.

**Mechanism:** build a `Vec<HelmReleaseSpec>` (always-on pushed unconditionally, optional ones wrapped in `if v.<app>_enable`). Loop: `register_release` registers each as a phase-6 node and `group.push(dep(seed))` collects ids. `builder.finish_async_group(&mut group, Targets::Machine(...))` emits a finish node that joins the whole batch. See `src/nodes/apps/helm_apps.rs`.

```rust
let mut group = AsyncNodeGroup::new("helm-apps", Vec::<infrazeug_core::id::NodeId>::new());
for spec in specs {
    let seed = spec.node_seed;
    builder = register_release(builder, spec)?;
    group.push(dep(seed));
}
let (builder, _all) =
    builder.finish_async_group(&mut group, Targets::Machine(k3s_server_machine_id()?))?;
```

### Compile-checked YAML via `template!`, with conditional & list sections

Render the static body with `template!(r#"..."#, v = v)`; `{{ v.field }}` placeholders are type-checked at compile time. For dynamic structure, keep the fn returning `String`: `push_str(&template!(...))` for optional blocks gated by `if v.<x>_enable`, and `push_str(&format!(...))` in a loop for list entries. Values computed in Rust are passed as extra named bindings (`v = v, tool_server_connections = ...`). See `src/nodes/apps/values.rs`.

```rust
let mut out = template!(r#"namespace: {{ v.openwebui_namespace }}
mcp:
  toolServerConnections: '{{ tool_server_connections }}'
"#, v = v, tool_server_connections = tool_server_connections);
if v.litellm_enable {
    out.push_str(&template!(r#"litellm:
  apiKey: "" # vault:litellm_openwebui_api_key
"#, v = v));
}
for d in &v.hermes_proxy_allowlist { out.push_str(&format!("    - {}\n", d)); }
```

### Idempotent / re-apply-safe shell steps (presence checks before mutate)

Each mutating step short-circuits with `exit 0` when the desired state already holds. The Helm install script optionally greps `helm list` + `readyReplicas` and skips the upgrade when healthy; StorageClass apply checks `kubectl get storageclass` first (parameters immutable); the etcd-backup node renders a drop-in to a temp file and only `install`s + restarts when `cmp -s` shows a change. The chart-presence probe `helm list -n NS -q | grep -qx RELEASE` is the reusable "already done?" idiom. See `src/nodes/k3s/helm_release.rs`, `etcd_backup.rs`.

### `helm template | transform | kubectl apply` and reusable helm helpers

**When:** a chart needs post-render manifest surgery, or a node needs the helm chores without the full `HelmReleaseSpec`.

**Mechanism:** render with `helm template`, pipe through an embedded transform, then apply — the GitLab node `include_str!`s a Python script, runs `helm template … > raw.yaml`, transforms (`GITLAB_NAMESPACE=… transform.py < raw > out`), then applies. Reusable helpers compose the common helm chores outside the spec: `helm_uninstall_stray_default_release(release)`, `helm_adopt_resources`/`helm_adopt_cluster_resources(release, ns[, grep])`, and `apply_storageclass_if_missing(name, yaml)` (StorageClass params are immutable, so it checks first). Generated scripts shell-quote interpolated values with `infrazeug_k8s::shell_escape`. See `src/nodes/apps/gitlab.rs`, `src/nodes/k3s/helm_release.rs`.

### Build & push a container image as a node

**When:** an app needs an image built and pushed before its release deploys.

**Mechanism:** `register_hermes_image_build` (gated on `hermes_image_build_enable`) embeds its Dockerfile via `include_str!`, builds with podman (docker fallback), HEAD-probes the remote registry's v2 manifest to skip when the tag already exists (unless `force_rebuild`), stages the registry password from vault to a file, and pushes — a build/push node distinct from helm/kubectl nodes. See `src/nodes/apps/hermes.rs`.

### Vault-marker + YAML-validity regression tests

**When:** the `"" # vault:key` markers and `{{ v.* }}` interpolations are stringly-typed.

**Mechanism:** `helm_vault::values_file_source` (the marker scan) runs at **graph-build time** — so for an *enabled* app an unregistered marker already panics at `plan`. The test's real value is covering render fns for apps whose flag is **off**: their node is never built during `plan`, so their markers would otherwise go unchecked until someone flips the flag. A `#[cfg(test)]` table lists every `(name, render_fn(defaults()))` regardless of flags; `catch_unwind` asserts `values_file_source` doesn't panic, so a marker missing from `vars.rs` fails the test with the app name. `serde_yaml::from_str` asserts validity and structural facts. **Add new render fns to that table.** See `src/nodes/apps/values.rs`.

```rust
#[test]
fn all_values_vault_markers_are_registered() {
    for (name, yaml) in [("keycloak", keycloak(defaults())), ("openwebui", openwebui(defaults()))] {
        let result = std::panic::catch_unwind(|| crate::helm_vault::values_file_source(&yaml));
        assert!(result.is_ok(), "{name} has an unregistered vault marker");
    }
}
```

Other test idioms worth copying: structural assertions on a build.rs-rendered template (cnpg asserts no leftover `{{`, that interpolated values appear, and that spliced blocks land at the right place — `src/nodes/apps/cnpg.rs`), and inventory-invariant tests (`machine_for` produces the expected ssh user / address family — `src/inventory.rs`).

### Raw kubectl / manifest nodes via `register_kubectl_node`

**When:** not everything is a Helm release — CRDs/manifests, K8s secrets from vault, wait loops, render-only steps. They still need to be phase-6 nodes on the K3s server, tagged, and wired to standard base deps.

**Mechanism:** wrap the work in a `ShellOp` (usually `ShellOp::run(argv!["sh","-ec", &script])`, or `ShellOp::Seq` chaining `ensure_namespace` + a vault `write_file` + the script) and pass it to `register_kubectl_node(builder, seed, app_tag, op, extra_deps, desc)`, which prepends `phase6_base_deps()` (chart-sync + phase-6 marker), builds the node, applies phase/app tags, and registers it. Secrets are created idempotently with `create ... --dry-run=client -o yaml | kubectl apply -f -`. See `src/nodes/k3s/helm_release.rs`, `src/nodes/apps/cnpg.rs`.

```rust
register_kubectl_node(
    builder, "cnpg-barman-plugin", "cnpg-barman-plugin",
    ShellOp::run(argv!["sh", "-ec", &script]),
    vec![seeds::k3s_cloudnative_pg(), seeds::k3s_cert_manager()],
    Some("Install the CloudNativePG Barman Cloud Plugin for object-store backups"),
)
```

> Hand-rolling a node (not via a `register_*` wrapper) means you must add `phase6_base_deps()` and `tag_phase(...)` yourself, or it won't honor `--tag` filters or ordering.

---

## 10. Secrets & vault (two paths)

The store layout: `vault-store/{files,keys,meta}`. Static secrets migrated from ansible-vault live under `files/infra/group_vars/{all,global,raspberry_cluster}/*.vault`; secrets generated at apply live under `files/mutable/...`. `keys/prod.dkey` is the single DataKey "prod" that seals everything (so `vault_data_keys()` returns `["prod"]`).

### Centralized `vault_paths` registry

Vault file paths and field names are bare strings consumed all over the playbook; scattering them as literals lets them drift and a typo only surfaces as an apply-time miss. One module (`src/vault_paths.rs`) exports one `pub const &str` per vault file and per well-known field, plus small fns that build dotted/dynamic field names. Doc comments record the consumer node and sealing DataKey. Every call site references these consts so `FileSource::vault(...)` and `VaultRef::field(...)` always agree.

```rust
pub const RASPBERRY_CLUSTER: &str = "infra/group_vars/raspberry_cluster/vault.vault";
pub const NEBULA: &str = "infra/group_vars/all/vault-nebula.vault";
pub const GLOBAL: &str = "infra/group_vars/global/vault.vault";
pub fn nebula_cert_field(host_cert_field: &str) -> String {
    format!("vault_nebula_certificates.{host_cert_field}")
}
```

### Path A — secret to a root-owned host file via `FileSource::vault`

**When:** land a secret (TLS key, CA cert, rsyncd credential) as a root-owned file without the plaintext appearing in source, the graph, or plan output.

**Mechanism:** build the content as `FileSource::vault(file, field)` (serializes to `FileSource::Vault { file, field }`, materialized only at apply from the unlocked store) and pass it to `write_root_path_file(host, path, mode, source)` (§4). The author never writes `sudo` and never sees the secret. See `src/nodes/nebula/install.rs`, `src/nodes/external/blog.rs`.

```rust
write_root_path_file(host, "/etc/nebula/ca.crt", 0o600,
    FileSource::vault(vault_paths::NEBULA, vault_paths::nebula_ca_field()))
```

### Path B — secret to Helm values via `"" # vault:key` markers

**When:** Helm charts need confidential values merged at apply without baking secrets into source, and a missing secret should fail loudly rather than deploy an empty password.

**Mechanism:** in the values YAML the author writes `key: "" # vault:<var_key>`. `helm_vault::values_file_source(yaml)` scans for the `"" # vault:` marker, looks each key up in `vars::global_varset()`, and builds a `FileSource::VaultYamlSubstitute { template, substitutions }` resolved at apply. The lookup **fails closed** — it panics if the key is absent or not a `VarValue::Vault`. So `vars.rs` is the single, enforced key→(file,field) mapping: every marker key must be pre-registered. See `src/helm_vault.rs`, `src/vars.rs`.

```rust
// vars.rs — register every marker key as a vault ref
fn vault_field(vars: &mut VarSet, file: &str, key: &str, field: &str) {
    vars.insert(VarKey::new(key), VarValue::Vault(VaultRef::field(file, field)));
}
vault_fields(&mut vars, crate::vault_paths::RASPBERRY_CLUSTER, &[
    ("litellm_master_key", "vault_litellm_master_key"),
    ("synapse_form_secret", "vault_synapse_form_secret"),
]);

// helm_vault.rs — turn a marked template into a vault-substituting source
pub fn values_file_source(template: &str) -> FileSource {
    FileSource::VaultYamlSubstitute {
        template: template.to_string(),
        substitutions: collect_substitutions(template), // panics if a key is unregistered
    }
}
```

### Secret as an environment variable

Per-app passwords are injected as env vars resolved from the vault at apply, never on a command line: `ShellOp::run(argv).env(KEY, FileSource::vault(file, field))` (see §4). The non-secret env-prefix idiom (`setup_env_for`) is the same `.env`-free string-prefix mechanism for config.

### Capture apply-generated secrets into the mutable vault

**When:** a secret does not exist until a resource is created at apply (a Keycloak OIDC client secret, OVH S3 keys, a generated LiteLLM API key) and cannot be pre-sealed.

**Mechanism:** the provider node emits its result on stdout; a downstream shell node seals it with `ShellOp::mutable_vault_write(DATA_KEY, file, field, source)`, where `source = FileSource::capture_same_machine(producer_node).json_pointer_optional("/secret")` extracts a JSON field and skips cleanly when absent. Mark `.deps([producer]).on_upstream_change()`. Literal connection metadata (bucket/endpoint/region) is sealed the same way with `FileSource::bytes(...)` and a node that `.always()` re-writes (the controller vault mutex serializes concurrent writes to one file). See `src/nodes/keycloak.rs`, `src/nodes/cloud.rs`.

```rust
let capture = FileSource::capture_same_machine(client_node.0).json_pointer_optional("/secret");
builder.shell_node(seeds::keycloak_hermes_webui_secret(), mid,
        ShellOp::mutable_vault_write(DATA_KEY,
            vault_paths::HERMES_WEBUI_OAUTH2,
            vault_paths::HERMES_WEBUI_OAUTH2_SECRET_FIELD, capture))
    .name("keycloak-hermes-webui-vault-secret")
    .deps([client_node])
    .on_upstream_change()
    .build()
```

### Read-after-write ordering of mutable-vault secrets

**When:** a consumer reads a mutable-vault file (directly via `FileSource::vault` or because a Helm marker resolves to it). It would read stale/empty data if scheduled before the writer — and provider stacks seed **random** UUIDv4 node ids, so there is no stable id to depend on.

**Mechanism:** override the writer's generated ids with deterministic ones from `seeds::` (e.g. `stack.vault_access_key_node_id = seeds::cloud_ovh_backup_vault_access_key()`), then have the consumer list those exact seed ids in `node.deps`, forcing write-before-read. Pair with repointing the Helm marker var in `vars.rs` from the static file to the mutable file so both the data edge and the dep edge point at the same place. See `src/nodes/cloud.rs`, `src/nodes/k3s/etcd_backup.rs`.

```rust
stack.vault_access_key_node_id = seeds::cloud_ovh_backup_vault_access_key();
// consumer:
deps.extend([
    seeds::cloud_ovh_etcd_backup_vault_access_key(),
    seeds::cloud_ovh_etcd_backup_vault_bucket(),
    // ...
]);
node.deps = deps;
```

### Provider/admin credentials from the vault at apply, never env

Provider builder extensions take the vault file plus credential field names and resolve them from the unlocked controller vault at apply — never `OVH_*`/`KEYCLOAK_*` env vars. On `plan` (no unlocked vault) native nodes degrade to `Unknown` instead of hitting the API, so the graph still validates offline. See `src/nodes/cloud.rs`, `src/nodes/keycloak.rs`.

```rust
let mut ovh = builder
    .ovh_vault_oauth2(
        crate::vault_paths::GLOBAL,
        crate::vault_paths::OVH_ADMIN_USER,
        crate::vault_paths::OVH_ADMIN_PASSWORD, mid)
    .ensure_backup_stack(stack)?;
```

---

## 11. Declarative cloud/API resources & secret hand-off

DNS records, S3 buckets, OIDC clients, network appliances are managed as **localhost-native API nodes** — controller-side, in-process, idempotent, plan-safe. See `src/nodes/cloudflare_dns.rs`, `ubiquity.rs`, `keycloak.rs`, `cloud.rs`.

### Localhost-native resource nodes via provider extension traits

**Mechanism:** each provider crate adds an `InfraBuilder` extension trait that, given a credential *source* and the localhost `MachineId`, returns a typed sub-builder whose `ensure_*` methods register one native node per resource. You take ids/display names from `seeds`, pass typed `Input` structs (`Default` + `..Default::default()`), then `.into_builder()` to fold the nodes back in. All nodes run on `inventory::machine_id("localhost")`, so `become_root`/`sudo` wrappers do **not** apply. (UniFi is the exception: it has no builder method — construct it directly with `UnifiInfraBuilder::new(builder, source, mid)`.)

```rust
let mid = inventory::machine_id("localhost");
let source = CloudflareClientSource::vault(vault_paths::GLOBAL)
    .with_api_token_field(vault_paths::CLOUDFLARE_API_TOKEN);
let mut cf = builder.cloudflare_source(source, mid);
for rec in records() {
    cf = cf.ensure_dns_record(rec.id, rec.display, rec.input)?;
}
Ok(cf.into_builder())
```

### Credentials sourced by field, never env

Every provider exposes a `*ClientSource` built from a vault file path plus the field name(s) holding the credential (Cloudflare API token; UniFi username/password + optional API key; Keycloak password grant; OVH OAuth2). Paths/fields are centralized in `vault_paths.rs`. Reusing an existing credential field means no new secret to seal.

```rust
let source = UnifiClientSource::vault_fields(
    &d.ubiquity_router_host, crate::vault_paths::GLOBAL,
    crate::vault_paths::UBIQUITY_ROUTER_USERNAME,
    crate::vault_paths::UBIQUITY_ROUTER_PASSWORD,
)
.with_api_key_field(crate::vault_paths::UBIQUITY_ROUTER_API_KEY)
.with_site(&d.ubiquity_router_site)
.insecure();
```

### Override framework-random node ids; disambiguate with name prefixes

Some framework stack builders (e.g. OVH `BackupStack`) seed internal ids with random UUIDv4s, making the graph unstable and un-dependable. After constructing the stack, assign each public `*_node_id` field from `seeds` (deterministic UUIDv5). When reusing the same stack type multiple times (cnpg / nextcloud / etcd backups), also set `with_node_name_prefix` to avoid duplicate display names. See `src/nodes/cloud.rs`.

```rust
let mut stack = BackupStack::new(/* project, bucket, region, desc */)
    .with_mutable_vault(d.ovh_backup_data_key.clone(), mutable_file.clone());
stack.bucket_node_id            = seeds::cloud_ovh_backup_bucket();
stack.vault_access_key_node_id  = seeds::cloud_ovh_backup_vault_access_key();
let ovh = builder.ovh_vault_oauth2(/* ... */).ensure_backup_stack(stack)?;
```

### Optional-node accessors

Forward-wiring variant of §5's `flag.then(seed)`: a producer module exposes `pub fn -> Option<NodeId>` (returning `Some(seed)` only when its `defaults()` flag is set); consumers do `deps.extend(producer::secret_node())`, adding the edge iff the producer exists. This keeps `plan` valid whether or not the feature is on.

### Read-back, rewrite, write-to-localhost (`register_local_write_node`)

**When:** you must pull a file off a host, transform it, and land it controller-side (e.g. a kubeconfig that must be rewritten for off-cluster use).

**Mechanism:** a `register_kubectl_node` whose op is `ShellOp::read_file(path)` captures the remote file; then `register_local_write_node(builder, seed, tag, local_path, source, mode, deps, desc)` writes it to localhost, where `source = FileSource::capture_on_machine(reader, server_mid).replace(regex, repl)` regex-rewrites the bytes before the write (the restricted-admin kubeconfig rewrites `127.0.0.1` to the routable host and pins `tls-server-name`). See `src/nodes/apps/restricted_admin.rs`, `register_local_write_node` in `helm_release.rs`.

```rust
let source = FileSource::capture_on_machine(dep("restricted-admin-kubeconfig-fetch").0, server_mid.0)
    .replace(r"127\.0\.0\.1", server_host)
    .replace(r"(?m)^(    server: https://\S+\n)",
             format!("${{1}}    tls-server-name: {tls_server_name}\n"));
register_local_write_node(builder, "restricted-admin-kubeconfig-local", "restricted-admin",
    &local_path, source, 0o600, vec![dep("restricted-admin-kubeconfig-fetch")], Some("..."))
```

### Cross-machine secret capture consumed by a later Helm release

**When:** a secret is minted by the app itself on the server, must be captured into the mutable vault, then injected into another app's Helm values on the same run.

**Mechanism:** a generator node prints JSON on the server; a localhost node calls `ShellOp::mutable_vault_write(...)` with `FileSource::capture_on_machine(generator_id, server_mid).json_pointer_optional("/key")`, marked `.on_upstream_change()`. Export the vault-write node's seed as a `pub(super) const` so the consuming Helm spec references the exact same seed in `.deps(...)`, guaranteeing the captured key is in the vault before its marker resolves. See `src/nodes/apps/litellm.rs`.

```rust
pub(super) const LITELLM_VAULT_WRITE_SEED: &str = "litellm-openwebui-vault-key";
builder = builder.shell_node(dep(LITELLM_VAULT_WRITE_SEED), localhost_mid,
        ShellOp::mutable_vault_write(DATA_KEY,
            crate::vault_paths::LITELLM_API_KEY, crate::vault_paths::LITELLM_API_KEY_FIELD,
            FileSource::capture_on_machine(keygen_node_id.0, k3s_mid.0).json_pointer_optional("/key")))
    .name("litellm-openwebui-vault-key").deps([keygen_node_id]).on_upstream_change().build()?;
// consumer (helm_apps.rs):
if v.litellm_enable { openwebui_deps.push(dep(LITELLM_VAULT_WRITE_SEED)); }
```

> Many `ensure_dns_record` impls key on `(name, record_type)`, so only one record per name+type can be managed declaratively — round-robin/secondary records must be managed out of band. Captured secrets use `json_pointer_optional` (not strict), so a missing field (a public OIDC client has no `/secret`) skips the write instead of failing apply.

---

## 12. Build-time template codegen, asset embedding & directory sync

### Compile-time template validation: file → raw-string-literal → `template!`

**When:** `template!` type-checks every embedded `{expr}` at compile time but requires a string **literal**, not a runtime `String`. You want large YAML value-templates in real `.yaml.template` files (editable, diffable) yet still get rustc to verify the interpolations.

**Mechanism:** a `build.rs` reads the template, wraps it in a Rust raw string literal whose `#` delimiter count grows until it can't collide with the content, and writes a `render_*` fn into `OUT_DIR`. The consumer `include!`s it. A `cargo:rerun-if-changed` line keeps codegen incremental. See `build.rs`, `src/nodes/apps/values.rs`.

```rust
// build.rs
fn raw_string_literal(content: &str) -> String {
    let mut hashes = 1usize;
    loop {
        let delim = "#".repeat(hashes);
        if !content.contains(&format!("\"{delim}")) {
            return format!("r{delim}\"{content}\"{delim}");
        }
        hashes += 1;
    }
}
let lit = raw_string_literal(&fs::read_to_string(&template_path).unwrap());
println!("cargo:rerun-if-changed={}", template_path.display());
let generated = format!(r#"pub fn render_gitlab_values(v: &crate::vars::PlaybookDefaults, email: &str) -> String {{
    infrazeug_api::template!({lit}, v = v, email = email,)
}}"#);
fs::write(out_path, generated).unwrap();

// consumer: src/nodes/apps/values.rs
include!(concat!(env!("OUT_DIR"), "/gitlab_values_render.rs"));
```

A table-driven variant drives codegen from a `&[(template_path, fn_name, &[extra_binding_names])]` slice, synthesizing each render fn's parameter list and `name = name,` bindings — one source of truth for which templates exist and what they bind.

### Embedding static assets via `include_str!`

**When:** the payload is a fixed shell script, systemd unit, Dockerfile, or CA cert — no `{{ }}` substitution wanted.

**Mechanism:** embed it as `&'static str` at compile time and install at apply. `asset_script(name)` is a `match` over `include_str!` of `assets/scripts/*.sh|.service`, and `install_script(host, path, name)` writes the embedded bytes via `write_root_file`. Files from the sibling Ansible repo are embedded by relative path (`include_str!("../../../files/hermes/Dockerfile")`; CA PEMs from `../infra`). Use this instead of build.rs codegen whenever you don't need interpolation type-checking. See `src/nodes/k3s/common.rs`, `src/nodes/apps/hermes.rs`, `src/nodes/external/promtail.rs`.

```rust
pub fn asset_script(name: &str) -> &'static str {
    match name {
        "k3s-setup.sh"      => include_str!("../../../assets/scripts/k3s-setup.sh"),
        "k3s-node-drain.sh" => include_str!("../../../assets/scripts/k3s-node-drain.sh"),
        // ...
        other => panic!("unknown asset script: {other}"),
    }
}
pub fn install_script(host: &HostSpec, path: &str, name: &str) -> ShellOp {
    write_root_file(host, path, 0o755, asset_script(name))
}
```

### Directory sync as a first-class DAG node (charts sync)

**When:** pushing a local tree (a vendored Helm `charts/` dir) to a host. As a standalone subcommand it has no ordering or change tracking, and helm installs can race it.

**Mechanism:** wrap `ShellOp::sync_dir_with_options(src, dest, SyncDirOptions { delete: true, hard_links: false })` in a `shell_node` with a deterministic seed `seeds::k3s_sync_charts()`, target the K3s server, and give it `deps = vec![seeds::k3s_phase_4()]`. Every helm release lists the same seed in its deps so charts land before any chart installs. The local source dir honors `INFRZEUG_CHARTS_DIR` (no "A"). **`ShellOp::SyncDir` is controller-side only** and cannot be lowered to the remote agent. See `src/nodes/k3s/mod.rs`, `src/charts_sync.rs`.

```rust
let op = ShellOp::sync_dir_with_options(
    crate::charts_sync::charts_local_dir(), &d.charts_remote_dir,
    SyncDirOptions { delete: true, hard_links: false });
let mut node = shell_node(seeds::k3s_sync_charts(), "k3s-sync-charts", op, Targets::Machine(mid))
    .with_description("Sync vendored Helm charts from local repo to the K3s server");
node.deps = vec![seeds::k3s_phase_4()];
builder = builder.node(node)?;
```

---

## 13. Framework API reference map

Symbols the playbook consumes from the framework, grouped by concern. The crate path is the *definition* site; most symbols are also re-exported at the crate root, so import from the root (`use infrazeug_shell::{argv, FileSource, ShellOp};`, `use infrazeug_core::Targets;`).

| infrazeug symbol | defining crate::path | purpose |
|---|---|---|
| `run` | `infrazeug-api::cli` | parse argv and drive the selected command |
| `RunConfig` / `RunConfig::new` | `infrazeug-api::cli` | builder for the CLI (`.about/.commands/.default_playbook/.extras/.mcp`) |
| `RunCommands::ALL` | `infrazeug-api::cli` | the full playbook command set |
| `RunBuildContext` / `RunContext` | `infrazeug-api::cli` | build-closure input: `Playbook(&RunContext)` vs `Pull` |
| `init_tracing` | `infrazeug-api` | logging setup |
| `PlaybookRegistry` / `PlaybookEntry` | `infrazeug-api::playbooks` | static registry of named playbooks |
| `build_from_registry` | `infrazeug-api::playbooks` | dispatch to an entry by `--playbook` |
| `PlaybookBundle` / `::with_runtime` | `infrazeug-api` | finalized `Infra` + method registry; attach `RuntimeConfig` |
| `ExtraSubcommand` | `infrazeug-api` | non-graph subcommand slot |
| `InfraBuilder` | `infrazeug-api::builder` | central graph assembler |
| `InfraBuilder::new/global_vars/vault_data_keys/group/machine/node/default_remote_transport/build` | `infrazeug-api::builder` | configure + register members + freeze the DAG |
| `builder::local` / `builder::remote` | `infrazeug-api::builder` | `Machine` constructors |
| `InfraBuilder::finish_async_group` / `AsyncNodeGroup` | `infrazeug-api` | join a batch of concurrent nodes into one finish vertex |
| `RuntimeConfig` | `infrazeug-core::runtime` | `run_root` + optional `vault_store` |
| `infra::shell_node` / `infra::barrier_node` | `infrazeug-core::infra` | node constructor / pure ordering node |
| `node::Node` / `::with_description` / `.deps` / `.tags` / `.policy` | `infrazeug-core::node` | the node struct and its mutable fields |
| `Targets` (`Machine`/`Machines`/`All`) | `infrazeug-core::node` | where a node executes (playbook uses `Machine` only) |
| `id::NodeId` / `MachineId` / `GroupId` | `infrazeug-core::id` | strongly-typed UUID newtypes |
| `Tag` / `Tag::new` | `infrazeug-core::id` | `--tag` filtering (bare-key match) |
| `node::RunPolicy` (`Always`/`OnUpstreamChange`/`Lazy`) | `infrazeug-core::node` | start gate for a node |
| `node::OutputChangePolicy` / `OutputChangeRule::unchanged_when_contains` / `OutputMatchStream::Stdout` | `infrazeug-core::node` | classify success as `Changed`/`Unchanged` from stdout |
| `node::PostRunPolicy::ExpectReboot` | `infrazeug-core::node` | tolerate transport drop, wait, run readiness check |
| `node::LockPolicy` (`node.policy.locks.{local_locks,global_locks}`) | `infrazeug-core::node` | serialize nodes per-host / cluster-wide |
| `varset::{VarSet, VarKey, VarValue}` / `::insert` | `infrazeug-core::varset` | string-keyed vars; `Scalar` (literal) / `Vault` (secret) |
| `VaultRef` / `VaultRef::field` | `infrazeug-secrets::vault_ref` | name a vault file+field, resolved at apply |
| `mutable_vault_path` | `infrazeug-secrets` | prefix a path into the mutable vault tree |
| `ShellOp` (`Seq`/`Run`/`run`/`write_file`/`read_file`/`env`/`sync_dir_with_options`/`mutable_vault_write`) | `infrazeug-shell::op` | the unit of work (serde enum) |
| `FileSource` (`bytes`/`vault`/`VaultYamlSubstitute`/`capture_same_machine`/`capture_on_machine`/`json_pointer_optional`/`replace`) | `infrazeug-shell::source` | file/env content incl. secrets, resolved at apply |
| `SyncDirOptions` | `infrazeug-shell` | options for controller-side directory sync |
| `argv!` | `infrazeug-shell` | macro: `argv![a,b]` → `vec![a.to_string(), ...]` |
| `shell_escape` | `infrazeug-k8s` | shell-quote a value for embedding in a generated script |
| `template` / `escape` | `infrazeug-api` (re-export of `infrazeug-templates`) | compile-checked rendering |
| `Helm` / `HelmChart` / `UpgradeOptions` / `upgrade_install` (method) / `repo_add` / `repo_update` / `with_kubeconfig` / `with_namespace` | `infrazeug-helm` | build a `helm upgrade --install` ShellOp |
| `Kubectl` / `ApplyOptions` / `RolloutOptions` / `apply_manifest` / `rollout_status` | `infrazeug-kubectl` | build kubectl apply/rollout ShellOps |
| `CloudflareInfraExt` / `CloudflareClientSource` / `EnsureDnsRecordInput` | `infrazeug-cloudflare` | declarative DNS records as nodes |
| `OvhInfraExt` / `BackupStack` (`with_mutable_vault`/`with_node_name_prefix`/`*_node_id`/`ensure_backup_stack`) | `infrazeug-ovh` | declarative OVH backup buckets + IAM + vault capture |
| `UnifiInfraExt` / `UnifiInfraBuilder::new` / `UnifiClientSource` / `EnsureDnsRecordInput` | `infrazeug-unifi` | declarative UniFi DNS / appliance config (constructed via `::new`, not a builder method) |
| `KeycloakInfraExt` / `keycloak_vault_password` | `infrazeug-keycloak` | declarative Keycloak clients/roles + secret capture |

---

## 14. Cross-cutting pitfalls

A terse checklist; each item links to where it is explained.

- **Seeds are byte-frozen.** Reconstruct ids by calling the same `seeds::*` fn; never edit an existing seed string. App-layer `dep("…")` strings are *not* compile-checked — only `cargo run -- plan` catches a typo. (§2)
- **`register_all` ordering is documentation, not scheduling** — execution follows each node's `deps`. (§2)
- **Bookends.** begin/finish + per-machine connect nodes are auto-injected; exclude them in node-count asserts (off-by-2 ⇒ bookends). (§2)
- **Two parallel group lists** (`all_groups()` / `group_id_map()`) must agree, or machines silently lose membership. (§3)
- **Secrets resolve at apply, never plan, never env.** A `"" # vault:key` marker fails *closed* — register it in `vars.rs` first; native cloud/keycloak nodes degrade to `Unknown` on `plan`. (§10)
- **Privilege is data.** Route every command through `host_op`/`host_script` and every root file write through `write_root_*`; never hardcode `sudo`. Localhost-native API nodes use none of these. (§4)
- **Idempotence is the author's job.** Guard every mutating step; for exit-0-either-way commands set `RunPolicy::Always` *and* attach an `OutputChangePolicy` marker. (§6)
- **Reboots need `PostRunPolicy::ExpectReboot`** or the transport drop is read as failure. (§6)
- **Node policy fields are grouped** under `node.policy.{run_policy, success.change_policy, locks, post_run}`; `run-policy.md`'s examples predate this grouping. (§6, §8)
- **`ShellOp::SyncDir` is controller-side only** and errors if lowered to the remote agent. (§12)
- **Env-var overrides use the `INFRZEUG_` prefix** (no "A"); `INFRAZEUG_…` silently gets the default. (§1, §12)
- **Wire-type caution.** `ShellOp`/`FileSource` deliberately avoid `skip_serializing_if` (postcard is non-self-describing — omitted fields corrupt deserialization); keep `serde(default)` alone on these types.
