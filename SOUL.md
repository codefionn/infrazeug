# SOUL.md — infrazeug (Rust rewrite)

Design plan for the Rust rewrite of infrazeug: a programmatic Infrastructure-as-Code library/framework. Code-first (not YAML), multi-tasking, change-aware, with first-class local emulation and an encrypted distributed secret store.

## Contents

1. Goals
2. Workspace Layout
3. Core Model
  - 3.1 Machine · 3.2 Node · 3.3 Tiers (ShellOp + NodeMethod) · 3.4 NodeTemplate · 3.5 Change semantics · 3.6 Retry/polling/timeout · 3.7 Become inheritance · 3.8 Executor & Scheduler · 3.9 Groups, VarSet & precedence · 3.10 Plan vs Apply · 3.11 Pull-mode apply
4. Transports (SSH foundation · push · agentless · pull · local · cross-compile)
5. Emulation (OCI + BuildGraph · QEMU · test mode + RunGuard · `like` workflow)
6. Secrets (key model · format · providers · onboarding · backends · multi-backend · packs · VaultStruct · audit · MCP rule · var integration)

6bis. MCP Integration
6ter. Interactive Controller (TUI)
7. CLI
8. Error Handling & Observability
9. Testing Strategy
10. Milestones M1–M6
11. Getting started on M1
12. Open Questions

## 1. Goals

- **Library-first.** A set of crates you embed in a Rust binary. The user's `main.rs` is the playbook; the binary doubles as agent + bootstrap when invoked with the right subcommand.
- **Stable.** Deterministic plans, edge-readiness scheduling, content-addressed `infrazeug.lock`, no hidden global state, no controller-side state file.
- **Concurrent by default.** Tokio + structured concurrency. Central planner + per-machine workers, global limits, per-machine + cross-machine locks. Unrelated DAG branches advance independently.
- **Skip useless work.** Every node reports `Changed | Unchanged | Failed` at execution. Downstream is conditional-by-default; predicates compose; methods can implement read-only `plan()` inspection for preview, while the canonical serializable plan stays stable and pessimistic (`Unknown`).
- **Two execution tiers.** `ShellOp` (typed, serializable, agentless-capable) covers most needs; `NodeMethod` (arbitrary Rust, agent-only) is the escape hatch. Native-on-agentless is a plan-time error.
- **Flexible transport.** Same node can run via SSH push agent (RPC), agentless SSH (`ssh`/`sftp` mux), local exec, or pull-mode (self-contained sealed plans on a `Backend`).
- **Realistic emulation.** Run the same playbook against OCI containers (`ContainerSpec` + `BuildGraph` via buildkit LLB) or QEMU/KVM microVMs. Test mode tears everything down via `RunGuard`.
- **Secrets you can actually trust.** Two-tier key model (recipients → DataKeys → files), CBOR + XChaCha20-Poly1305, pluggable providers (ssh-agent / FIDO2 hmac-secret / PKCS#11 / passphrase / age / KMS), pluggable backends with `MultiBackend` replication, `VaultStruct` typed loading with layered merge, vault-backed plan signing.
- **MCP first-class.** `infra.mcp().tool().resource().prompt()` exposes live stats from the same binary that deploys. Secrets are never exposed to MCP — locked, not configurable.

## 2. Workspace Layout

Cargo workspace under `crates/`. Small, single-purpose crates so embedders pull only what they need.

```
crates/
  infrazeug-core/         # Graph, scheduler, node trait, change semantics, facts
  infrazeug-api/          # Public builder API re-exported as `infrazeug`
  infrazeug-transport/    # Transport trait + SSH (agent + agentless) + local
  infrazeug-agent/        # On-host agent binary entrypoint (serve-rpc subcommand)
  infrazeug-rpc/          # Wire protocol: postcard (framed length-prefix over stdin/stdout)
  infrazeug-emulate/      # Emulation backends (containers, QEMU)
  infrazeug-emulate-oci/  #   OCI/containerd/podman driver
  infrazeug-emulate-qemu/ #   QEMU/KVM driver
  infrazeug-secrets/      # KV store, envelope format, provider trait, backend trait
  infrazeug-secrets-ssh/  #   ssh-agent provider
  infrazeug-secrets-hw/   #   FIDO2 / PKCS#11 provider
  infrazeug-secrets-kms/  #   age + cloud KMS provider
  infrazeug-secrets-s3/   #   S3/Swift backend
  infrazeug-secrets-dav/  #   WebDAV backend
  infrazeug-shell/        # ShellOp DSL (typed builder, lowering, agent interpreter)
  infrazeug-methods/      # Tier-1 native NodeMethod registry + helpers
  infrazeug-templates/        # Compile-time `template!` macro re-export + render/escape helpers
  infrazeug-templates-macros/ #   proc-macro crate implementing `template!` (Rust-native, rustc-typed)
  infrazeug-build/        # Cross-compile + agent binary build (zigbuild/cross/native)
  infrazeug-pull/         # Pull-mode daemon, sealed-plan codec, plan-store client
  infrazeug-bootstrap/    # Tiny static stub binary used at first boot of ephemeral hosts
  infrazeug-mcp/          # MCP server: expose tools/resources/prompts from a deployment binary
  infrazeug-tui/          # ratatui-based interactive controller; in-process or attach via UDS
  infrazeug-cli/          # `infrazeug` CLI binary: vault, plan, apply, agent, build, gc
examples/
  hello-ssh/
  emulated-cluster/
  pull-from-git/
```

Re-export pattern: `infrazeug` crate re-exports the user-facing surface from `infrazeug-api` + `infrazeug-core` so end users add one dep.

## 3. Core Model

### 3.1 Machine

```rust
pub struct Machine {
    id: MachineId,                       // Uuid v4, user-supplied via uuid!() macro
    name: Arc<str>,                      // display, unique per Infra (collision => error)
    kind: MachineKind,
    os_hint: Option<OsHint>,             // optional, facts can override
    like: Option<LikeConfig>,            // emulated twin (only emulated kinds allowed)
    vars: Arc<VarSet>,
    groups: Vec<GroupId>,                // flat, ordered for precedence
    tags: Vec<Tag>,                      // targeting filter only, no var semantics
    become: Option<Become>,              // sudo/doas/su, password from vault
    max_parallel_nodes: Option<usize>,   // per-machine concurrency throttle
    lifecycle: Lifecycle,                // Persistent | Ephemeral { owner: RunId }
}

pub struct MachineId(Uuid);              // v4, in-code literal; no state file needed

pub enum MachineKind {
    Remote   { ssh: SshConfig },
    Container(ContainerRef),             // see §5.1
    MicroVm  { image: VmImage, qemu: QemuConfig },
    Local,
    Custom(Arc<dyn MachineDriver>),      // escape hatch for BSD, Windows, k8s pods, ...
}

pub struct OsHint {
    family: OsFamily,                    // Linux | Freebsd | Windows | Macos | Other
    distro:  Option<Arc<str>>,           // "ubuntu", "alpine", ...
    version: Option<Arc<str>>,           // required for distros that need it (RHEL major, ...)
}

pub enum VmImage { LocalQcow2(PathBuf), RemoteQcow2(Url) }

pub struct SshConfig {
    host: Arc<str>,                      // "alias" or "host[:port]" — plain ssh semantics
    user: Option<Arc<str>>,
    ssh_config: Option<SshConfigSource>, // None = system ~/.ssh/config
    identity:   Option<IdentitySource>,
    extra_opts: Vec<Arc<str>>,           // raw -o KEY=VAL pass-through
}

pub enum SshConfigSource {
    File(PathBuf),
    Vault(VaultKey),                     // decrypted to tmpfs in run_root, -F it
}

pub enum IdentitySource {
    Agent,
    File(PathBuf),
    Vault(VaultKey),                     // decrypted to tmpfs in run_root, 0600, -i it
}

pub struct Become {
    method:   BecomeMethod,              // Sudo | Doas | Su
    user:     Arc<str>,                  // default "root"
    password: Option<VaultKey>,
}

pub struct LikeConfig {
    kind: MachineKind,                   // must be emulated (Container/MicroVm/Local)
    vars_override: Option<Arc<VarSet>>,
    transport_override: Option<TransportChoice>,
}

pub enum Lifecycle {
    Persistent,                          // survives across runs
    Ephemeral { owner: RunId },          // torn down by RunGuard
}
```

The `MachineDriver` trait for `Custom(...)`:

```rust
#[async_trait]
pub trait MachineDriver: Send + Sync {
    async fn provision(&self, run: &RunCtx) -> Result<ProvisionedHandle>;
    async fn teardown(&self, h: ProvisionedHandle) -> Result<()>;
    fn default_transport(&self) -> TransportChoice;
    fn os_hint(&self) -> Option<OsHint>;
    fn fact_schema(&self) -> Schema;
    async fn gather_facts(&self, h: &ProvisionedHandle) -> Result<Value>;
}
```

Notes:

- **Identity.** `MachineId` is a UUIDv4 literal in user code (`uuid!("…")`, compile-time parsed). Same name → user-chosen stable UUID, no controller-side state file required. Duplicate `name` within one `Infra` is a registration error.
- `**like` restrictions.** Only emulated kinds are accepted as twins so `infrazeug test` cannot accidentally touch real hosts.
- **Tags vs groups.** Groups carry variable precedence (`global < group(in order) < machine < like_override`). Tags are pure targeting filters (`--target tag=app=web`). Any level may hold encrypted vault-backed values; they're lazy-decrypted per machine via that machine's provider chain.
- **Transport decoupled.** `MachineKind::Remote` only carries identity/access info. *How* we talk (push agent vs agentless vs pull daemon) is a separate `TransportChoice` chosen at apply time. See §4.

### 3.2 Node

A node is one logical unit of work targeting one or more machines.

```rust
pub struct Node {
    id: NodeId,                              // Uuid v4, user-supplied via uuid!()
    name: Arc<str>,                          // display, unique per Infra
    body: NodeBody,
    targets: Targets,
    deps: Vec<NodeId>,
    run_policy: RunPolicy,                   // §3.5
    fail_policy: FailPolicy,                 // FailFast | Continue
    retry: RetryConfig,                      // §3.6
    timeout: Option<Duration>,
    become: Option<Become>,                  // node-level; inherits down (§3.7)
    tags: Vec<Tag>,
}

pub enum NodeBody {
    Shell(ShellOp),                          // tier 2 — serializable, agentless-capable
    Native { method: Arc<dyn ErasedNodeMethod>, input: Value },  // tier 1 — agent-only
}

pub enum Targets {
    Machine(MachineId),
    Machines(Vec<MachineId>),                // fan out
    Group(GroupId),                          // resolved at plan
    TagSelector(TagExpr),                    // resolved at plan
    All,
}
```

**Fan-out & propagation semantics** (locked):

- One logical `Node` runs its body on each target machine concurrently. Per-machine results are stored separately.
- A successor's `RunPolicy::OnUpstreamChange` fires if **any** target of any upstream changed.
- A node **completes** when it has finished on every assigned target; only then do successors start. This is the barrier-by-default rule — no per-machine pipelining across nodes. Use [Templates](#34-nodetemplate) to express richer sub-graphs.
- When a node has multiple successors, those successors execute in parallel (subject to their own `targets`, `RunPolicy`, and per-machine `max_parallel_nodes`).

### 3.3 Two execution tiers: ShellOp & NodeMethod

Two tiers cover the spectrum from "trivial serializable shell work" to "arbitrary Rust on the target."

#### 3.3.1 ShellOp (tier 2, default)

A typed, serializable enum DSL. Same code runs natively in the agent's interpreter (direct syscalls, no shell parsing) **and** lowers to portable shell + sftp for agentless mode.

```rust
pub enum ShellOp {
    Run        { argv: Argv, env: Env, cwd: Option<PathBuf>, stdin: Option<Source> },
    Pipe       { stages: Vec<ShellOp> },
    Seq        { steps: Vec<ShellOp>, on_error: OnError },
    All        { steps: Vec<ShellOp> },                       // parallel
    If         { cond: Box<ShellOp>, then: Box<ShellOp>, else_: Option<Box<ShellOp>> },
    Poll       { until: Box<ShellOp>, every: Duration, timeout: Duration },
    ReadFile   { path: PathBuf },
    WriteFile  { path: PathBuf, content: Source, mode: u32, atomic: bool },
    EnsureDir  { path: PathBuf, mode: u32 },
    Symlink    { target: PathBuf, link: PathBuf },
    Chmod      { path: PathBuf, mode: u32, recursive: bool },
    Chown      { path: PathBuf, user: Option<Arc<str>>, group: Option<Arc<str>>, recursive: bool },
    Download   { url: Url, to: PathBuf, sha256: Option<[u8; 32]> },
    // NOTE: the runtime `Template { src, vars, to }` variant is SUPERSEDED by the
    // compile-time `template!` macro (infrazeug-templates). Templates render to a
    // `String` controller-side at authoring/plan time and feed `WriteFile` via
    // `FileSource::Bytes` (see §3.3.2), so there is no runtime template interpreter
    // and no new serializable op. Kept here only as a record of the old plan.
    Package    { manager: PkgManager, action: PkgAction, pkgs: Vec<Arc<str>> },
    Systemd    { unit: Arc<str>, action: UnitAction },
    Sysctl     { key: Arc<str>, value: Arc<str>, persist: bool },
    Capture    { op: Box<ShellOp>, as_: Vec<CaptureSpec> },
    Process    { from: ResultRef, with: Vec<Processor> },     // declarative post-processing
}

pub enum Processor {
    Lines, JsonParse, JsonPath(Arc<str>), Regex(Arc<str>),
    Head(usize), Tail(usize), Trim, Decode(Encoding), Hash(Algo),
}
```

`Argv` is **always an explicit `Vec`** of pieces (each piece is `Arg<String>`, accepting a literal or an `Out<String>`). There is no `Run::shell("…")` convenience that parses a free-form string — prevents quoting bugs. A `shellword!` / `argv!` macro keeps writing them ergonomic:

```rust
b.run(argv!["apt-get", "install", "-y", &pkg_name]);
```

#### 3.3.2 Typed result handles `Out<'i, T>`

`ShellOp` and `Node` outputs are referenced through phantom-typed handles whose lifetime is tied to their owning `Infra`/`InstantiatedTemplate`, so dangling references are a compile error.

```rust
pub struct Out<'i, T> { node: NodeId, field: Field, _i: PhantomData<&'i ()>, _t: PhantomData<T> }

impl<'i, T: ShellValue> Out<'i, T> {
    fn eq(self, rhs: T) -> Out<'i, bool>;
    fn and(self, rhs: Out<'i, bool>) -> Out<'i, bool>;
    fn json_path(self, p: &str) -> Out<'i, Value>;
    fn as_bytes(self) -> Out<'i, Bytes>;
}

pub trait IntoArg<'i, T> { fn into_arg(self) -> Arg<'i, T>; }
impl<'i, T> IntoArg<'i, T> for T          { ... }
impl<'i, T> IntoArg<'i, T> for Out<'i, T> { ... }
```

Authoring example:

```rust
let pkg:    Out<ExitCode> = b.run(argv!["apt-get","install","-y","nginx"]).capture_exit();
let conf:   Out<Bytes>    = b.read_file("/etc/nginx/nginx.conf");
let needs:  Out<bool>     = pkg.eq(0).and(conf.contains("server_name old"));
b.if_(needs, |t| {
    // Compile-time, Rust-native template (rustc type-checks every `{{ expr }}`).
    // `template!` renders to a `String`; `write_rendered` wraps it as a WriteFile.
    t.write_rendered("/etc/nginx/nginx.conf", 0o644,
        template!("server_name {{ server_name }};\n@for u in &upstreams { server {{ u }};\n}",
                  server_name = server_name, upstreams = upstreams))
     .atomic();
    t.run(argv!["systemctl","reload","nginx"]);
});
```

Cross-template wiring: `Out<'i, T>` produced by one `InstantiatedTemplate<'i>` can be passed as an input to another instantiation under the same `'i`, giving fully typed composition.

#### 3.3.3 Captures & storage

Each `Capture` materialises its fields (stdout/stderr/exit, file content, headers, …) in `NodeResult.data`. Cap: **16 MiB per capture by default**, configurable per node. Overflow strategy is `Spill` by default → file written to 25`run_root/<run_uuid>/captures/<node>/<machine>/<field>`; the `Out<T>` handle remains valid and lowers to a streamed read on access. Alternatives: `Truncate`, `Fail`.

#### 3.3.4 NodeMethod (tier 1, agent-only)

Arbitrary Rust running inside the agent binary on the target (or **in-process on the controller** when the node targets a `MachineKind::Local` machine). Typed at the user's compile site, erased internally for storage.

```rust
#[async_trait]
pub trait NodeMethod<I, O>: Send + Sync
where I: DeserializeOwned + JsonSchema + Send,
      O: Serialize        + JsonSchema + Send,
{
    fn name(&self) -> &'static str;
    fn idempotent(&self) -> bool { false }
    async fn plan(&self, ctx: &PlanCtx, input: &I) -> Result<PlanOutcome<O>> { Ok(PlanOutcome::Unknown) }
    async fn execute(&self, ctx: &NodeCtx, input: I) -> Result<NodeOutput<O>>;
}
```

`PlanOutcome` semantics (locked):

- `Unchanged` → preview says in-sync; if present in an executable plan, node is skipped and does not propagate.
- `Changed`   → preview says would change; if present in an executable plan, execute and propagate.
- `Unknown`   → execute; **treated as Changed for downstream**. Safe default and the default outcome stored in canonical serialized plans.

`NodeMethod::plan()` is a read-only inspection hook used for preview/dry-run output. Its result is not baked into the default serialized `Plan`; that keeps plan digests byte-stable across runs and avoids turning transient remote facts, API availability, or secret-dependent reads into drift.

**Native-on-agentless = plan-time hard error.** Because the application binary declares the full DAG, plan validates every `Node { body: Native(_), targets: ... }` against per-machine `TransportChoice` and available agent arch. If any target can't run the agent (agentless or arch unsupported), plan fails before any side effect with both the node and the offending machine named.

### 3.4 NodeTemplate

A reusable sub-DAG with an `entry` injection point and one or more `exits`. This is how richer change-propagation patterns are expressed (the framework's only barrier rule between top-level nodes is "wait for all targets to finish before propagating"; templates let you stitch multiple internal nodes with their own per-machine sets).

```rust
pub struct NodeTemplate<'i, I, O> {
    name: Arc<str>,
    nodes: Vec<TemplateNode<'i>>,
    inputs: I,
    entry: TemplateNodeId,
    exits: Vec<TemplateNodeId>,
    outputs: O,                              // typed Out<'i, _> handles
}

impl Infra {
    fn instantiate<'i, I, O>(&'i mut self, tpl: &NodeTemplate<'_, I, O>, args: TemplateArgs<'i, I>)
        -> InstantiatedTemplate<'i, O>;       // exposes outputs as Out<'i, _>
}
```

Locked rules:

- Per-template-node `targets` come from `TemplateArgs`, so the same template can be instantiated against different machine sets.
- An instantiation's `entry` may be wired `.after(some_node_id)` so the template hangs off an external predecessor.
- Templates ship as crates: convention `infrazeug-template-<name>` for community, `infrazeug-templates` for first-party. Each template exposes a typed builder (`fn install_nginx() -> NodeTemplateBuilder<NginxInputs, NginxOutputs>`).

### 3.5 Change semantics (the "skip work" rule)

Every node and ShellOp emits a `Changed` value. Downstream nodes have a `RunPolicy`:

```rust
pub enum RunPolicy {
    OnUpstreamChange,                        // default: skip unless an upstream reports changed
    OnUpstreamChangeAnd(Box<dyn Predicate>), // changed AND extra condition holds
    Always,
    When(Box<dyn Predicate>),                // pure predicate, ignores change
}
```

A `Predicate` receives `&WhenContext { machine, facts, results }` and may consume multiple upstream results (multi-`When` composition). Result access is **same-machine by default**, `results.global(node_id)` for cross-machine.

Plus the **read-only preview** layer: methods that implement `plan()` can report `Changed`, `Unchanged`, or `Unknown` for `infrazeug plan` / `--dry-run` display without mutating the canonical plan. Actual skip/propagation decisions are driven by the executable plan outcome plus runtime `Changed`/`Unchanged` results.

### 3.6 Retry, polling, timeout

```rust
pub struct RetryConfig {
    enabled: RetryMode,                      // Off | Auto | Force
    max: u32,
    backoff: Backoff,
}
pub enum Backoff { Fixed(Duration), Exp { initial: Duration, max: Duration, jitter: bool } }
```

- Methods declare `idempotent() -> bool` (default `false`); `RetryMode::Auto` (default) only auto-retries methods that are idempotent. Built-ins `file.read`, `file.list`, `Download` (with `sha256`), `http.get`, and fact gathering are idempotent → default 3× exp backoff 1s→30s with jitter.
- Non-idempotent methods need `RetryMode::Force` for explicit opt-in.
- **Polling is not retry.** `ShellOp::Poll { until, every, timeout }` waits for state to become true. Distinct semantics, distinct primitive — retry is for transient failure recovery, polling is for asynchronous readiness.

### 3.7 Become inheritance

- `Node.become = Option<Become>` applies to the whole node.
- At plan time, `effective_become` is propagated to every successor via the dependency DAG. A successor without its own `Become` inherits its ancestors'.
- Override: a successor that explicitly sets `Become` wins.
- Conflict: a join with two ancestors holding incompatible `Become` and no override on the successor is a **plan error** that names both ancestors. Forces explicit resolution.
- `effective_become` is recorded in plan output so users can audit who runs as what.

### 3.8 Executor & Scheduler

The scheduler is a pluggable trait so users can swap or extend the default. Default implementation is a **central planner + per-machine worker** model: the planner advances the DAG on a per-edge readiness basis (no global level barrier); each machine has a dedicated worker that pulls tasks for its host, serialised by that host's concurrency cap.

```rust
#[async_trait]
pub trait Scheduler: Send + Sync {
    async fn run(&self, plan: Plan, runtime: SchedRuntime) -> RunReport;
}

pub struct SchedRuntime {
    pub transports: TransportFactory,
    pub limits:     GlobalLimits,
    pub events:     broadcast::Sender<SchedEvent>,
    pub commands:   mpsc::Receiver<SchedCommand>,    // inbound: cancel, pause, replay
    pub interact:   Arc<dyn Interactor>,             // prompts: unlock, approve, confirm, ...
    pub cancel:     CancellationToken,
    pub vault:      Arc<VaultSession>,
}

infra.with_scheduler(Box::new(MyCustomScheduler::new()));   // extension point
```

Default scheduler internals:

```rust
struct DefaultScheduler {
    workers: HashMap<MachineId, MachineWorker>,
    locks_local:  PerMachine<LockBag>,
    locks_global: LockBag,
}

struct MachineWorker {
    transport: Arc<dyn Transport>,
    queue:     mpsc::Receiver<Task>,
    permit:    Semaphore,                            // = Machine.max_parallel_nodes
    inflight:  HashSet<NodeId>,
}
```

#### 3.8.1 Global limits

```rust
pub struct GlobalLimits {
    pub max_ssh_connections:  usize,                 // default 32
    pub max_concurrent_nodes: usize,                 // default num_cpus * 4
    pub max_concurrent_builds: usize,                // default num_cpus
    pub max_fact_gathers:     usize,                 // default 16
}
```

Per-machine `max_parallel_nodes` (§3.1) further throttles each host. All limits enforced via tokio `Semaphore`.

#### 3.8.2 Edge-readiness, not global levels

A successor starts as soon as **its own** predecessors finish on all *their* targets. Unrelated branches advance independently — there is no global level barrier. This maximises concurrency without breaking the "wait for the whole node to finish across targets before propagating" rule.

`SyncAll` remains as the explicit "wait for everything in the infra up to this point" barrier (see §3.8.5).

#### 3.8.3 Resource locks

```rust
node.requires_locks(["pkg-manager"]);                // per-machine lock
node.requires_global_lock("rolling-deploy");         // cross-machine lock
```

The scheduler acquires named locks before dispatch and releases on completion. Common uses: serialising `apt`/`dnf`, kernel module loads, atomic rollouts that must serialize across the fleet.

#### 3.8.4 Failure semantics

A node fans out across N machines; per-machine outcomes are tracked independently.

```rust
pub enum FailPolicy {
    FailFast,                                        // default: first failure cancels siblings
    Tolerate { max_failed: usize },                  // node aggregate succeeds if ≤ N machines failed
}

pub enum NodeAggregate {
    AllChanged, AnyChanged, AllUnchanged,
    PartialFailed { ok: Vec<MachineId>, failed: Vec<MachineId> },
    AllFailed,
}
```

**Default: don't tolerate failures.** Configurable per node via `Tolerate { max_failed }`.

**Per-machine downstream propagation** (locked rule):

- Downstream `OnUpstreamChange` fires *as a whole* if any upstream target changed (existing rule).
- Downstream nodes **execute only on machines where every predecessor succeeded on that same machine**. A successor on machine M is marked `Skipped(BlockedByUpstream)` if any predecessor failed on M.
- Combined with `Tolerate { max_failed: 2 }`: the upstream succeeds in aggregate even with two failures; downstream runs on the other machines and skips on the failed two.

#### 3.8.5 `SyncAll` barriers

`SyncAll` is the framework-wide barrier: wait for every node up to this point to reach a terminal state across every machine, then continue. Default config has `SyncAll` available as an explicit node type users insert where they want it; can be configured to insert one implicitly between every top-level node (very conservative). When two ancestors disagree on `SyncAll` strictness, the **stricter setting wins** on the successor (no surprises from a relaxed branch).

#### 3.8.6 Cancellation

`CancellationToken` cascades to every worker. In-flight work:

- **RPC (agented):** send a polite `Cancel(node_id)` frame; agent acknowledges and starts shutting the operation down. After a configurable grace period (default 10 s) the controller kills the RPC channel; agent's signal handler terminates the operation.
- **Agentless ssh:** all remote commands are wrapped in `setsid timeout <node_timeout_or_default> -- <cmd>` so killing the local `ssh` child also kills the remote process tree. On cancel: send SIGTERM to the local `ssh`, wait grace period, SIGKILL.

#### 3.8.7 Per-op progress over RPC

The agent emits **structured per-step events** over RPC (`ShellOpStarted`, `ShellOpProgress`, `ShellOpFinished` with the op's `ResultRef`). Events are rate-limited: bursts > 100/s are coalesced into batch frames to avoid flooding the channel. The same event stream feeds:

- `tracing` spans on the controller.
- `RunReport` construction.
- MCP `tail_logs` / `progress` resources (§6bis).
- An optional `**--watch` TUI** built into the user's application binary: invoking `<app> apply --watch` runs the binary in scheduler mode and renders a live TUI of the event bus. Same binary, no extra tool to install.

#### 3.8.8 Event bus

```rust
pub enum SchedEvent {
    NodeQueued    { node, machine },
    NodeStarted   { node, machine, effective_become },
    NodeProgress  { node, machine, kind: ProgressKind, payload: Value },
    NodeFinished  { node, machine, status, duration, captures },
    NodeCancelled { node, machine, reason },
    PlanWarning   { ... },
    BuildEvent    { spec, kind, payload },
}
```

`tokio::sync::broadcast` channel; subscribers are lock-free.

#### 3.8.9 No resumability

A crash mid-apply means **start from scratch on next run**, with a fresh canonical plan plus fresh preview/runtime inspection. No checkpoints, no state file (consistent with §3.10 plan-is-recomputed). `RunReport` is durable for audit/post-mortem only.

#### 3.8.10 Timeouts

No default per-node timeout; nodes run until they finish or are cancelled. No inheritance. Users opt in per node (`Node.timeout = Some(Duration)`) or via `Infra::default_node_timeout` (default `None`). If a transport-level timeout (the `setsid timeout` wrapper for agentless) is set higher than the node timeout, the node timeout wins via cooperative cancel.

#### 3.8.11 Graph mechanics

- DAG built on `petgraph`; cycle detection at plan time with the cycle path named.
- Plan-time validation includes: cycle check, native-on-agentless check (§3.3.4), become conflict check (§3.7), unknown machine/group/tag selector resolution, lock declaration sanity.

### 3.9 Groups, VarSet & precedence

Groups carry variables; tags target. Both can target a node (`Targets::Group | Targets::TagSelector`), they compose (`group=prod and tag=app=web`), and they coexist without overlap in responsibility.

```rust
pub struct Group {
    id:     GroupId,                                 // Uuid v4 via uuid!()
    name:   Arc<str>,                                // unique per Infra
    vars:   Arc<VarSet>,
}

pub struct VarSet {
    entries: BTreeMap<VarKey, VarValue>,             // sorted → canonical hash for plan diff
}

pub enum VarValue {
    Scalar(serde_json::Value),                       // string/num/bool/null
    List(Vec<VarValue>),                             // REPLACE on merge
    Map(BTreeMap<String, VarValue>),                 // DEEP-MERGE on merge
    Vault(VaultRef),                                 // lazy decrypt per machine
    Computed(Arc<dyn Fn(&ResolveCtx) -> Result<VarValue> + Send + Sync>),
}

pub enum VarAcl {
    Auto,                                            // resolved without prompting (default)
    Prompt,                                          // push-mode: controller raises Interaction::ApproveVarRequest
    AutoForMachines(Vec<MachineId>),                 // auto for listed, prompt for others
}
```

Each entry can carry an ACL: `Vault::field("db.vault","password").acl(VarAcl::Prompt)`. Auto by default; `Prompt` makes the controller raise an interactive approval when a node requests the value (see §6ter).

**Groups are strictly flat.** No nesting, no inheritance. Sharing the same values across groups is done by referencing the same vault file from each group's `VarSet`, not by group-of-groups.

**Multi-membership ordering** is the order the machine lists its groups (locked):

```rust
machine.groups([base, web, prod]);                   // base lowest, prod highest within group level
```

Full precedence chain (lowest → highest):

```
global  <  group[0] < group[1] < … < group[n-1]  <  machine  <  like_override
```

Within each level, vault-backed and computed values are resolved lazily at machine-resolution time and folded in.

**Merge semantics** (locked, distinct from `VaultStruct`):

- **Maps** deep-merge: later levels override per-key, missing keys inherited from lower levels.
- **Lists** replace wholesale by default. Opt-in append via `VarKey::Append("name")` marker on the higher-level entry.
- **Scalars / Vault / Computed** replace.

(`VaultStruct` keeps its own concat-for-Vec semantics inside a single load operation — that's a different consumer with explicit derive semantics and is scoped to its own load call, so the divergence doesn't cross paths in practice.)

**Typed access** is the primary API:

```rust
let host: String              = vars.get("db.host")?;
let port: u16                 = vars.get("db.port")?;
let creds: DbCreds            = vars.load_struct()?;  // §6.8
let raw:  &serde_json::Value  = vars.raw("debug.flags")?;  // escape hatch
```

`Computed(...)` closures receive a `ResolveCtx { machine, facts, infra_meta, vault }`. Vault access from a computed value is permitted but counts as a vault dependency — graph build will not start before vault unlock completes (see §6.3).

**Framework-supplied vars** are exposed via a typed `MachineSpec` struct, not magic string keys, so user-defined vars cannot shadow them:

```rust
pub struct MachineSpec {
    pub id:     MachineId,
    pub name:   Arc<str>,
    pub os:     ResolvedOs,                          // family / distro / version
    pub tags:   Vec<Tag>,
    pub groups: Vec<GroupId>,
    pub facts:  FactView,
}
pub struct InfraMeta { pub run_id: RunId, pub run_mode: RunMode }
```

User access from `Computed`:

```rust
VarValue::Computed(Arc::new(|ctx| Ok(format!("web-{}", ctx.machine.name).into())))
```

**Construction ergonomics** — macro for literals, file loader for JSON/TOML (no YAML):

```rust
let vs = vars! {
    "db.host"     => "10.0.0.5",
    "db.port"     => 5432,
    "db.password" => Vault::field("db.vault", "password"),
    "replicas"    => [host1, host2, host3],
};
let vs2 = VarSet::from_file("vars/prod.toml")?;
```

**Environments are not first-class.** Use groups (`prod`, `staging`) and tags. Keeps the model small; can be added later without breaking changes.

**Debug tool** (M1): `infrazeug vars resolve <machine> [<key>]` prints resolved values annotated with their source level (global / which group / machine / like_override) and origin (literal / vault file / computed).

### 3.10 Plan vs Apply

Two-phase, terraform-lite:

- `plan()` resolves the graph, validates native-on-agentless and `Become` conflicts, resolves target machines, records node fingerprints, stores `PlanOutcome::Unknown` by default, and returns a canonical `Plan`. The digest is byte-stable across runs because it excludes live remote facts and preview observations.
- `preview()` / `--dry-run` may invoke per-method `plan()` hooks to perform read-only inspection and display `Changed` / `Unchanged` / `Unknown`. Preview results are display-only and are not serialized into the canonical `Plan`.
- `apply(plan)` executes. If no plan file is supplied, apply recomputes a fresh canonical plan. If a plan file is supplied, apply recomputes the canonical plan for drift detection before execution.

#### 3.10.1 Plan as a serializable value

```rust
pub struct Plan {
    pub digest: PlanDigest,                          // rollup hash (DAG + targets + node fingerprints)
    pub nodes:  Vec<PlannedNode>,
    pub signatures: Vec<PlanSignature>,              // see 3.10.3
    // ...
}
```

- Serialized as CBOR. `infrazeug plan -o plan.bin` / `infrazeug apply plan.bin`.
- `apply plan.bin` recomputes a fresh plan and refuses if its digest differs from the file's (drift detection). `--force` bypasses with a loud warning.

#### 3.10.2 Partial plans (per-machine slices with hash waits)

A `Plan` can be **sliced by machine** so that work is distributed without each host needing the full DAG:

```rust
let slice: Plan = full.slice(MachineId(uuid!("…")));
slice.write("web-01.plan")?;
```

Slicing rules:

- Keeps every node that targets the slice's machine.
- Replaces dependencies on nodes that targeted *other* machines with `WaitForHash` markers:
  ```rust
  pub enum SliceStep {
      Node(PlannedNode),
      WaitForHash { id: WaitId, expect: Sha256, sources: Vec<MachineId> },
  }
  ```
- The waiting host's runtime blocks on receiving `expect` from at least one of `sources`. Other machines' work is **not** further specified in the slice (privacy + smaller payload).
- Hash distribution: relayed via the controller in v1 (each machine reports completion hashes back over its RPC channel; controller forwards to subscribers). Direct agent-to-agent deferred.
- Use case: ship a per-machine slice to a machine that pulls its own plan from git (§4.3) without exposing the full infrastructure DAG.

#### 3.10.3 Plan signing (vault-backed)

```rust
infra.plan()
    .sign_with(SigningKey::Vault(VaultRef::field("ops/plan-signing.vault", "ed25519")))
    .write("plan.bin")?;

infra.apply_signed("plan.bin", &[trusted_signer])?;  // verifies before applying
```

- Signature covers the canonical `Plan` digest, not the raw bytes (so re-serialisation is safe).
- Multiple signatures allowed (approval workflow: developer signs, ops signs, then apply requires both).
- Signing keys live in the vault (§6): `SigningKey::{ InMemory, Vault, FidoSigningKey }`. Hardware-key signing is the recommended path.
- `apply` verifies against a configured set of trusted signers; mismatches fail loudly.

#### 3.10.4 Variable transport: push (RPC) vs pull (sealed)

Plans by default do **not** embed secret variables. The default push model keeps secrets on the controller and serves them on demand:

- Plan slice contains *references* (`VaultRef` / `VarRef`) for secret-bearing vars, not values.
- During execution, the agent emits a `VarRequest { node_id, var_ref }` over the RPC channel.
- Controller validates: this slice's signed digest, machine identity, var ACL, then resolves the value (vault decryption + var precedence) and returns it.
- Agent uses the value for the node and discards it (no on-disk caching).

For **pull-mode** the controller is not online during apply, so every secret-bearing var must be inlined into the slice and the whole slice is sealed for one specific machine. See §3.11.

#### 3.10.5 Lint vs plan

A separate **lint tool** validates the DAG without unlocking the vault: cycle check, native-on-agentless check, become conflicts, unused groups/tags, dead computed-var references. Suitable for CI on every commit. Canonical `plan` does the same validation and additionally resolves target machines, fingerprints planned nodes, and computes the drift digest. `preview` / `--dry-run` is the credentialed read-only path for live facts and best-effort diffs.

### 3.11 Pull-mode apply (sealed plans)

Pull-mode targets ephemeral/short-lived hosts (cloud VMs at boot, autoscaler-spawned containers, edge devices). The controller publishes a sealed per-machine plan to a store; the host fetches and applies it itself, with no live controller link during execution.

#### 3.11.1 Plan store = `Backend` from §6.5

Reuse the secrets `Backend` trait wholesale — FS, S3/Swift, WebDAV (and a future `GitBackend`). Same `MultiBackend` replication, same auth chain. One concept to learn.

```
<store>/
  bootstrap/<machine-uuid>.toml           # bootstrap input, see 3.11.2
  plans/<machine-uuid>.plan.sealed        # encrypted slice, see 3.11.4
  agents/<digest>/<triple>/infrazeug-agent
  agents/<digest>.sig                     # detached signature
  hashes/<wait-id>                        # NOT allowed in pull mode (see 3.11.6)
  tombstones/<machine-uuid>               # revocation marker
```

#### 3.11.2 Bootstrap input

A small config consumed by the bootstrap stub at first boot:

```toml
machine_id    = "0f3c…"
plan_url      = "s3://my-bucket/"
agent_url     = "https://artifacts/my-agent/sha256-…/"
agent_digest  = "sha256:…"
agent_signer  = "ed25519:…"
plan_signer   = "ed25519:…"
machine_key   = "/var/lib/infrazeug/machine.key"   # X25519 private key
fetch_auth    = { kind = "instance" }              # see 3.11.5
poll_interval = "30s"                              # omit for one-shot
```

Multiple input formats supported (cloud providers force the user's hand): TOML (canonical), JSON, plain cloud-init `#cloud-config` YAML wrapping a `write_files` directive, and Ignition. All deserialize to the same canonical `Bootstrap` struct.

#### 3.11.3 Per-machine keypair (sealed-plan crypto)

Pull-mode plans are sealed to the receiving machine using an **X25519** keypair generated per machine.

```
infrazeug machine keygen --machine 0f3c… --out machine.key
# prints the X25519 public key for controller registration
# private key written to machine.key (mode 0600)
```

Provisioning flow:

1. Run `machine keygen` on the host (or generate on controller and inject the private key via cloud-init secret).
2. Register the public key on the controller (file, registry, or `infrazeug machine register`).
3. Controller produces sealed slices targeting that public key.

The sealed-plan envelope **reuses the vault envelope format** (§6.2) with a single X25519 recipient — same crypto, same code path. Inside the sealed body: CBOR-encoded `Plan` slice containing inlined secret-bearing vars.

#### 3.11.4 Custom agent only

Pull-mode requires a **custom agent** (signed, content-addressed). No stock-agent path for v1 — gives us:

- One distribution story: build with `infrazeug-build`, publish to `agents/<digest>/...`, sign with the configured key.
- No tier-1 native-vs-stock split to validate (the issue from §3.3.4 doesn't exist for pull-mode).
- A future stock-agent variant can be added later without breaking the pull-mode wire format.

Plan slice declares `agent_digest`; bootstrap fetches, verifies signature, verifies digest, then executes.

#### 3.11.5 Fetch auth

```rust
pub enum FetchAuth {
    NoAuth,
    CustomHeader { name: Arc<str>, value: SecretString },
    BearerToken  { token: SecretString },
    InstanceIdentity { provider: CloudProvider },     // AWS IMDSv2, GCP metadata, Azure IMDS
}
```

`SecretString` zeroizes. `InstanceIdentity` is preferred on cloud VMs (creds rotate automatically, never in user-data). `BearerToken` is the recommended path for fixed-credential setups.

#### 3.11.6 No `WaitForHash` in pull-mode slices

Cross-machine `WaitForHash` markers from §3.10.2 are **not supported** in pull-mode slices. Slicing for pull-mode fails with a clear error if the slice would need one. Rationale: pull-mode is for independent ephemeral nodes; cross-machine coordination needs a controller (push-mode) for v1.

#### 3.11.7 Modes

```rust
pub enum PullMode {
    OneShot,
    Daemon { interval: Duration, jitter: Duration },
}
```

- **OneShot**: bootstrap → fetch → apply → exit. Typical for immutable cloud VMs that re-bake on change.
- **Daemon**: poll every `interval ± jitter`; refetch slice; if digest differs from last-applied (or a tombstone appears), re-apply. Crash recovery is "re-apply on restart" (idempotent because §3.10 plan-redo + per-node change semantics).

Both supported in v1.

#### 3.11.8 Bootstrap stub binary (`infrazeug-bootstrap`)

Separate crate, separate binary, statically linked musl, kept tiny (target ≤ 2 MB). Job:

1. Read bootstrap input (TOML/JSON/cloud-init/Ignition).
2. Fetch agent binary from `agent_url` by digest.
3. Verify detached signature against `agent_signer`.
4. Fetch sealed plan from `plan_url`.
5. Verify plan signature against `plan_signer`.
6. Decrypt plan with `machine.key` (X25519 unwrap → XChaCha20-Poly1305 body).
7. `exec` the agent with the decrypted plan on stdin (or via a tmpfs path).

The stub does no apply work itself — it's pure plumbing. Distributed via `curl | sh` from a pinned-by-digest URL, or base64-embedded in cloud-init.

#### 3.11.9 Revocation

`infrazeug plan revoke <machine-uuid> [--with-teardown]` writes a tombstone to `tombstones/<uuid>`. Daemons refuse to apply on next poll. With `--with-teardown`, the tombstone carries a small sealed plan that cleans up before exiting. OneShot already-applied installs are unaffected (no controller link to enforce).

#### 3.11.10 Workspace

Pull-mode lives in two crates:

- `infrazeug-bootstrap` — the tiny stub.
- `infrazeug-pull` — daemon-mode logic, plan-store client, sealed-plan codec. Wired into the main agent so it's the same binary running `serve-pull` vs `serve-rpc`.

#### 3.11.11 CLI

```
infrazeug machine keygen --machine UUID [--out FILE]
infrazeug machine register --machine UUID --pubkey FILE [--into BACKEND]
infrazeug plan publish --for-machine UUID [--into BACKEND] [--sign-with KEY]
infrazeug plan revoke  --for-machine UUID [--with-teardown]
infrazeug bootstrap --from /etc/infrazeug/bootstrap.toml
```

## 4. Transports

Trait in `infrazeug-transport`. `TransportChoice` is selected per-machine at apply time, independent of `MachineKind`.

```rust
pub enum TransportChoice { SshAgentPush, SshAgentless, Local, PullDaemon }

#[async_trait]
pub trait Transport: Send + Sync {
    async fn exec(&self, cmd: Command) -> Result<ExecOutput>;
    async fn write_file(&self, path: &Path, data: Bytes, mode: u32) -> Result<()>;
    async fn read_file(&self,  path: &Path) -> Result<Bytes>;
    async fn open_rpc(&self) -> Result<RpcChannel>;        // err for agentless
}
```

### 4.0 SSH foundation: shell out to `ssh` binary

All SSH transports drive the system `ssh` / `sftp` binaries — **no in-process `russh`/`libssh2`**. Rationale: respect `~/.ssh/config`, `Match`, `ProxyJump`, `Include`, hardware-key agents, `ssh-agent`, FIDO2 SK keys, OpenSSH's audited crypto.

Shared mechanics:

- **Connection mux.** `-o ControlMaster=auto -o ControlPath=<run_root>/<run_uuid>/ssh-%C -o ControlPersist=...`. First call opens, subsequent `ssh`/`sftp` reuse the mux socket. Socket lives under the run dir → reaped by `RunGuard` automatically.
- **Stderr handling.** Treat stderr as opaque human text; trust exit codes + `-o LogLevel=ERROR`.
- **Min OpenSSH 8.0** required on controller. Checked at startup, fail fast with a clear message.
- **Controller OS.** Linux only in v1 (no `ControlMaster` on Windows controllers).
- **Vault-sourced SSH config / identity.** Decrypted to tmpfs under `run_root/<run_uuid>/secrets/`, `0600`, zeroized + unlinked by `RunGuard`. Passed via `-F` / `-i`.

### 4.1 SSH push (agented)

- Controller uploads `infrazeug-agent` binary (with user's custom methods statically linked) via `sftp` to `~/.cache/infrazeug/agent-<version>`.
- Spawns it with `ssh host -- infrazeug-agent serve-rpc`.
- RPC = framed **postcard** over that child's **stdin/stdout** (the long-lived `ssh` process — itself riding the mux'd connection). Not a separate SSH channel. Framing = `uvarint(len) || postcard-bytes`. Schema evolution via `#[serde(other)]` fallback variants on enums and `#[serde(default)]` on new fields; controller and agent ship from the same workspace so versions are pinned together.
- File ops use `sftp` (atomic rename, mode bits, large files) on the same mux'd connection.
- Become (`sudo`/`doas`/`su`) is applied per-exec by wrapping the remote command; password (if any) piped on stdin from a vault decryption.

### 4.2 SSH agentless

- Same `ssh`/`sftp` plumbing, no binary upload, no RPC channel.
- Each `NodeMethod` either ships an **agentless recipe** (portable shell snippet generator) or is marked agent-only; a machine on `SshAgentless` against an agent-only method fails at plan time with a precise error.
- All built-in methods ship agentless recipes.

### 4.3 PullDaemon

Thin transport-layer face of the pull-mode apply model (§3.11). The agent runs `serve-pull` and is its own scheduler — no live controller connection during apply. Plan slices and agent binary come from a `Backend` (FS / S3 / WebDAV / future Git), sealed per-machine with X25519, signed by a trusted signer. See §3.11 for the full model.

### 4.4b Agent build & cross-compile (`infrazeug-build`)

Native-tier nodes only work if an agent binary exists for the target arch — and the application binary declares the full DAG, so plan can validate this. To keep that trivial, `infrazeug-build` ships first-class cross-compile:

```rust
infra.agent_build()
    .target("aarch64-unknown-linux-musl")
    .target("x86_64-unknown-linux-musl")
    .toolchain(Toolchain::Auto)              // Auto | Zigbuild | Cross | Native
    .add();
```

- **Default `Auto`** prefers `cargo-zigbuild` if `zig` is on PATH (single toolchain covers musl/glibc/macOS/Windows), falls back to `cross` (docker-based), then native.
- Outputs cached under `target/infrazeug-agents/<triple>/infrazeug-agent`.
- musl-static default → no glibc surprises on minimal targets.
- Per-machine arch resolved from `OsHint` or probed via agentless SSH on first contact; the right binary is uploaded.
- CLI: `infrazeug agent build [--target ...]` for CI prebuild.

### 4.4 Local

Emulated targets and `MachineKind::Local`: direct `tokio::process` exec, no SSH. Used inside containers via `nsenter` / `runc exec`; inside QEMU VMs we still use SSH (production code path stays exercised).

## 5. Emulation

`infrazeug-emulate` defines an `EmulatedHost` factory; per-backend crates implement it.

### 5.1 OCI containers, ContainerSpec & BuildGraph

A `ContainerRef` is either prebuilt or framework-built. Specs are `Arc`-shared so identical specs share a cache key by pointer (and by content digest).

```rust
pub enum ContainerRef {
    Prebuilt(ImageRef),                              // user pulled / built externally
    Spec(Arc<ContainerSpec>),                        // framework builds via buildkit
}

pub struct ContainerSpec {
    base:    ContainerBase,
    steps:   Vec<BuildStep>,
    runtime: ContainerRuntime,                       // Containerd | Podman
    build:   BuildConfig,
    outputs: Vec<BuildOutput>,                       // empty => stage-only (consumed by COPY --from)
}

pub enum ContainerBase {
    Scratch,
    Image(ImageRef),                                 // base by ref or digest
    From(Arc<ContainerSpec>),                        // multi-stage chain → forms BuildGraph
}

pub struct BuildConfig {
    builder:   Builder,                              // Local (default) | Buildkit{addr} | OnMachine(MachineId)
    platforms: Vec<Platform>,                        // linux/amd64, linux/arm64, ...
    cross:     CrossPolicy,                          // Auto (qemu-user) | PreferCross | EmulateOnly
    cache:     CacheConfig,                          // layer cache + named mount caches
}
```

#### 5.1.1 BuildStep coverage

```rust
pub enum BuildStep {
    Run    { argv: Argv, env: Env, mounts: Vec<Mount>, network: NetMode, cache_id: Option<Arc<str>> },
    Copy   { from: CopySource, src: Vec<PathBuf>, dest: PathBuf, chmod: Option<u32>, chown: Option<Owner> },
    Add    { url: Url, dest: PathBuf, sha256: Option<[u8; 32]> },
    Env    { kv: Vec<(Arc<str>, Arc<str>)> },
    Arg    { name: Arc<str>, default: Option<Arc<str>> },
    Workdir(PathBuf),
    User(Arc<str>),
    Label(Vec<(Arc<str>, Arc<str>)>),
    Expose(Vec<PortSpec>),
    Volume(Vec<PathBuf>),
    Entrypoint(Argv),
    Cmd(Argv),
    Healthcheck(HealthcheckSpec),
    Shell(Argv),                                     // override default exec shell (rare)
}

pub enum CopySource {
    Context(BuildContext),                           // explicit, typed
    Stage(Arc<ContainerSpec>),                       // multi-stage: COPY --from=<spec>
    Image(ImageRef),                                 // COPY --from=alpine:3.19 /bin/busybox
}

pub enum BuildContext {
    LocalDir { path: PathBuf, include: Vec<Glob>, exclude: Vec<Glob> },
    Whole(PathBuf),                                  // Docker-compat "take everything" escape hatch
    InlineFiles(Map<PathBuf, Source>),               // small files defined in Rust
    GitRepo { url: Url, rev: GitRev, subdir: Option<PathBuf> },
}
```

`Argv` is the same explicit-array type used by ShellOp (no shell parsing). Build contexts are first-class typed inputs: each `Copy::Context(...)` declares exactly what it pulls in, so the LLB cache key only invalidates on changes to those specific paths. `BuildContext::Whole(path)` is offered as a migration escape hatch for "behave like Docker."

#### 5.1.2 Mounts (cache, secret, bind)

```rust
pub enum Mount {
    Cache  { id: Arc<str>, target: PathBuf, sharing: CacheSharing },   // Shared | Private | Locked
    Secret { id: Arc<str>, target: PathBuf, source: SecretSource },    // build-time secret, never in layers
    Bind   { source: PathBuf, target: PathBuf, readonly: bool },
    TmpFs  { target: PathBuf, size: Option<u64> },
}

pub enum SecretSource {
    Vault(VaultRef),                                 // wired straight to §6 vault
    EnvVar(Arc<str>),
    File(PathBuf),
}
```

- `Mount::Cache` persists across builds for `~/.cargo`, apt lists, npm cache, etc.
- `Mount::Secret { source: Vault(...) }` decrypts via the current run's DataKeys, passes the value to buildkit via its native secret-mount protocol — the secret is mounted only inside the `Run` step, never written to a layer, never appears in the image. This is the v1 path for build-time secrets.

#### 5.1.3 BuildOutput

```rust
pub enum BuildOutput {
    LocalStore { runtime: ContainerRuntime, namespace: Arc<str> },     // default
    OciImage   { ref_: ImageRef, push: bool, signer: Option<SigningKey> },
    OciTarball { path: PathBuf },
    Rootfs     { path: PathBuf },                    // for QEMU base image bootstrap
}
```

**Default output = local containerd content store on the controller** (or on `Builder::OnMachine`'s target). Machines consuming `ContainerRef::Spec(s)` pull from the local store directly — no registry round-trip required for in-cluster use. Pushing to a registry is opt-in via `BuildOutput::OciImage { push: true, .. }`. A spec with `outputs: []` is stage-only and pruned by buildkit when nothing copies from it.

#### 5.1.4 Signing & provenance

`SigningKey` enum: `InMemory(Bytes) | Vault(VaultRef) | FidoSigningKey`. Sigstore/cosign-style detached signature attached on push when `signer` is set. SLSA provenance output deferred past v1.

#### 5.1.5 Builders & `OnMachine`

- `**Builder::Local`** is the default. Spawns/embeds buildkit on the controller.
- `**Builder::OnMachine(id)**` runs the build on a specific machine — natural fit for a beefy native-arch builder. Build context is streamed over the same SSH transport; outputs flow back through the same channel, or are pushed directly to a registry from that machine (faster).
- Plan-time validation: `OnMachine` target must be a `Container` host with buildkit reachable (socket or TCP) and support all requested `platforms`. Otherwise plan fails with a precise error.
- **Phased rollout pattern.** Use node `tags` + `TagSelector` to first apply only the nodes that install buildkit on the remote, then re-run with the full DAG (`--target tag=phase=full`). The framework supports this via the existing target-filter; no special machinery needed.

#### 5.1.6 Image refs & registry auth

```rust
pub struct ImageRef    { registry: Arc<str>, repo: Arc<str>, tag: Option<Arc<str>>, digest: Option<Digest> }
pub struct RegistryAuth { url: Arc<str>, creds: AuthSource }

pub enum AuthSource {
    Env,                                             // DOCKER_USERNAME / DOCKER_PASSWORD
    DockerConfig(PathBuf),                           // ~/.docker/config.json
    Vault(VaultRef),
}
```

Resolution chain: per-`Infra` registry config → docker config file → anonymous.

#### 5.1.7 BuildGraph execution

- DAG over `Arc<ContainerSpec>` edges (`Base::From(other)` and `Copy::Stage(other)`).
- Cycle detection at plan time, with cycle path named.
- Levels built in parallel; within a level, buildkit fans out internally.
- A `ContainerMachine` referencing `ContainerRef::Spec(s)` registers `s` in the BuildGraph and gates machine start on its successful build.
- Failures: a spec build failure fails every downstream spec/machine; siblings continue per `FailPolicy`.

#### 5.1.8 Caching

Two orthogonal layers:

- **LLB layer cache** handled by buildkit; key = lowered LLB digest, includes resolved base-image digest (not just tag, so tag drift doesn't poison cache), `Builder` choice, and `Platform`. Two builders → two cache entries.
- **Named mount caches** (`Mount::Cache`) persist between builds with explicit `sharing` semantics.
- Run-local lowering cache: `Arc<ContainerSpec> → LLB Definition` keyed by pointer identity → fan-out lowers once per spec, not per consumer.

#### 5.1.9 Determinism — `infrazeug.lock`

A lock file `infrazeug.lock` (sibling of `Cargo.lock`) pins:

- Resolved base-image digests (every `ImageRef` without a digest gets one).
- Build-context input digests (per `Copy::Context` materialised content hash).
- Resolved LLB digest per `ContainerSpec`.
- Top-level BuildGraph digest (rollup) so the whole build is content-addressed.

`plan`/`apply` read the lock; missing or stale entries are resolved fresh and the lock is rewritten. `--unpinned` opts out of lock enforcement. Mirrors Cargo's behaviour; matches the project's stability goal. Cross-references with `RunReport` so a deployment is traceable to the exact image graph.

#### 5.1.10 Cross-build default

`CrossPolicy::Auto` = qemu-user emulation inside buildkit (works everywhere, slow but correct). `CrossPolicy::PreferCross` uses true cross-compilation when the toolchain supports it (notably Rust/Go via `infrazeug-build` integration).

#### 5.1.11 Image GC

Built images are tagged `infrazeug.run_id=<uuid>` in the local store. Cleanup piggybacks on `infrazeug gc`: `infrazeug gc --keep-recent N` / `--older-than 7d` reaps stale images alongside other run artifacts. Images bound to live `Persistent` machines are never collected.

### 5.2 QEMU/KVM microVMs

- Backend: direct `qemu-system-x86_64`/`aarch64` spawn, virtio-vsock for control channel, cloud-init seed ISO for first-boot config.
- Snapshots for fast reset between test runs.
- Same `Transport` via SSH-into-VM, so the production code path is exercised end-to-end.

### 5.3 Test mode, run isolation & teardown

```rust
pub struct RunId(Uuid);
pub enum RunMode { Apply, Test }

impl Infra {
    pub async fn test(&self) -> Result<TestReport>;   // RunMode::Test
    pub async fn apply(&self) -> Result<RunReport>;   // RunMode::Apply
}
```

Rules:

- `RunMode::Test` swaps every machine with a `like` to its emulated twin and marks the twin `Lifecycle::Ephemeral { owner: run_id }`. Machines **without** `like` are skipped with a WARN and recorded in `TestReport.skipped` (no destructive action against real hosts in test mode).
- All transient resources (containers, VMs, build outputs scoped to the run, tmp vault unlocks, ssh mux sockets) are tagged `infrazeug.run_id=<uuid>` at creation and live under `<run_root>/<run_uuid>/`.
- **Strong isolation from day one:** per-run **network namespace** + per-run bridge inside it; veth pairs per ephemeral container; tap devices inside the netns for VMs; vsock sockets under `run_root`. User namespace used when available (warn + root fallback otherwise). Mount namespace bind-mounts `run_root/<uuid>/` as the only shared writable path.
- **Teardown** via a `RunGuard` that fires on normal end, panic, and signals (SIGINT/SIGTERM handler installed once). Idempotent. Failure during teardown is logged loudly, exit non-zero, leftovers enumerated.
- `**run_root` is configurable** (`RuntimeConfig::run_root`), default `/var/lib/infrazeug/runs/`. Override per-invocation via CLI flag or `Infra::with_runtime(...)`.

### 5.4 `like` workflow

```rust
machine
    .like(MachineKind::MicroVm { image: VmImage::RemoteQcow2("…ubuntu-24.04.qcow2".into()),
                                 qemu: QemuConfig::default() })
    .test_env();
```

`infrazeug test` builds the emulated host, runs the playbook against it, tears it down. `infrazeug apply --emulate-first` chains: emulate → on success, deploy to real target.

## 6. Secrets

`infrazeug-secrets` provides an encrypted KV store with a **two-tier key model**, pluggable recipient providers, pluggable storage backends, optional packs for high-latency remotes, and typed struct loading with layered merge.

### 6.1 Key model: recipients → DataKeys → vault files

```
recipients (ssh-ed25519 / fido2 hmac-secret / passphrase / age / kms)
     │ wrap
     ▼
DataKey  (random 32B, named — "prod", "ops", "ci")
     │ wrap
     ▼
DataKey envelope  (one .dkey file; header lists all recipients)
     │ unlock via any one recipient
     ▼
DataKey  (raw 32B in memory; zeroized on RunGuard drop)
     │ AEAD
     ▼
Vault file  (CBOR map: nested k/v, lists, blobs)
```

Consequences:

- **Onboard new device** = wrap DataKey for one new recipient, write back the `.dkey`. Vault file bodies untouched. O(1) per DataKey.
- **Revoke device/person** = `vault revoke <recipient> [--rotate]`. Always rewrites `.dkey` headers; `--rotate` additionally generates a new DataKey and re-encrypts bound files (expensive, opt-in).
- **Blast radius** = one DataKey per scope you actually care about (e.g. `prod-db`, `ops-shared`, `ci`).

### 6.2 On-disk format

Binary CBOR, magic-prefixed, versioned:

```
magic    : 8B  "INFRZVLT"
version  : 1B  0x01
header   : CBOR { data_key_id, content_type, nonce(24), aad_hash, file_salt(32) }
body     : XChaCha20-Poly1305(DEK, plaintext = CBOR map, nonce, aad = canonical(header))
```

- AEAD: **XChaCha20-Poly1305** (24-byte nonce, random-safe).
- Body is a CBOR map supporting arbitrary nesting (`map | list | bytes | string | int | bool | null`).
- AAD = canonical-encoded header → any tamper invalidates.

Store layout:

```
<store>/
  keys/                       # DataKey envelopes
    prod.dkey
    ops.dkey
  files/                      # vault files (CBOR maps, sealed under a DataKey)
    db/postgres.vault
    deploy/keys.vault
    mutable/                  # generated secrets written by runs
      cloud/images.vault
  packs/                      # optional bundles for high-latency backends
    prod.pack
```

### 6.3 Recipient providers

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    async fn wrap(&self,   dek: &[u8], ctx: &WrapCtx)   -> Result<RecipientEntry>;
    async fn unwrap(&self, entry: &RecipientEntry, ctx: &WrapCtx) -> Result<Vec<u8>>;
}
```

`WrapCtx` carries `data_key_id` and per-envelope `file_salt`, used to construct unique challenges so a captured signature can't be replayed.

- `**ssh-agent`.** Ed25519 only by default (RSA opt-in, PKCS1v15 deterministic, with WARN). Derivation:
  ```
  challenge = "infrazeug-kek-v1\0" || data_key_id || file_salt
  sig       = ssh-agent.sign(key, challenge)
  KEK       = HKDF-SHA256(salt = "infrazeug-kek-hkdf-v1", ikm = sig, info = data_key_id)
  ```
  Wrap = ChaCha20-Poly1305(KEK, DEK).
- `**fido2`.** `hmac-secret` extension only (deterministic, offline). Resident-credential mode supported for portability.
- `**pkcs11`.** PIV / HSM tokens.
- `**passphrase`.** Argon2id, defaults `m=256 MiB, t=3, p=4`, params stored in header. `vault calibrate` tunes for 500 ms target.
- `**age` / `kms`.** Thin wrappers; KMS subtrait covers AWS KMS, GCP KMS, Vault Transit.

No DEK caching across runs by default — every run prompts/touches the recipient. In-memory DEK zeroized on `RunGuard` drop.

**Unlock lifecycle.**

- DataKey envelopes are unlocked **at run start**, before the build graph and node graph are constructed. Any password-manager prompt, YubiKey touch, or PKCS#11 PIN entry happens once up-front — not per node, not per machine, not interleaved with build output.
- **Per-file DataKey by default.** Each vault file declares which DataKey it's sealed under; users typically have one DataKey per blast radius (`prod`, `ops`, `ci`). A meta-vault pattern — one DataKey that encrypts a file containing other DataKeys — is supported as an opt-in configuration for organisations that want a single human unlock to cascade.
- Vault **bodies** decrypt lazily per machine at resolution time (the DataKey is already in memory, decryption is cheap and avoids loading unused secrets).
- **Canonical plan is vault-light.** `infrazeug plan -o` writes a stable drift artifact and does not bake secret plaintext or secret-dependent remote facts into the digest. Read-only preview may decrypt as needed to inspect secret-dependent resources; if it cannot or should not inspect them, it reports `Unknown`. Apply still requires the needed DataKey unlocks at run start.
- DAG validation independent of vault content is the job of a **separate lint tool** (Rust lint + DAG analysis): it can be run in CI without touching the vault.

**Mutable vault namespace.**

- `files/mutable/**` is reserved for generated secret material that a run creates or rotates, such as bucket-scoped cloud API keys created after provisioning the bucket.
- Mutable entries use the normal vault-file wire format and DataKey envelopes. This is an encrypted secret namespace, not a controller state file and not a source of machine identity.
- Recommended blast radius: use a dedicated runtime DataKey (for example `prod-runtime`) when generated credentials should be accessible to a smaller operator set than static configuration.
- Mutation must use backend optimistic concurrency (`Etag`) where available; conflict means another run wrote the generated secret and the caller must retry or re-read.
- ShellOps can write generated secrets with a controller-side `VaultWrite`: run the cloud CLI in one node, capture stdout, apply transforms such as regex include/exclude and trim, then store the result in `files/mutable/**`.
- MCP exposure remains metadata-only under §6.10.

### 6.4 Onboarding & rotation UX

```
infrazeug vault keygen prod
infrazeug vault recipients add prod --fido2 --label "yubikey-a"
infrazeug vault recipients add prod --age age1...
infrazeug vault recipients add prod --passphrase --label "paper-recovery"
infrazeug vault recipients list prod
infrazeug vault recipients rm  prod --label "yubikey-a"      # device lost
infrazeug vault rotate-key     prod                            # full re-encrypt (rare)
infrazeug vault rotate-file    db/postgres.vault               # single file
```

- A DataKey must have ≥1 non-passphrase recipient, unless created with `--allow-passphrase-only`.
- `recipients add` requires unlocking the DataKey with an existing recipient → only authorized users can extend.
- `vault recovery-code generate` mints a printable Argon2id passphrase recipient.

### 6.5 Storage backends

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>>;
    async fn put(&self, key: &str, v: Bytes, prev: Option<Etag>) -> Result<ObjectMeta>; // optimistic CAS
    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>>;
    async fn delete(&self, key: &str) -> Result<()>;
}

pub struct ObjectMeta { pub key: String, pub etag: Option<Etag>, pub mtime: Option<SystemTime>, pub size: u64 }
```

v1: local FS, S3-compatible (incl. OpenStack Swift via S3 API), WebDAV. Git deferred to v1.1.

### 6.6 Multi-backend replication

```rust
pub struct MultiBackend {
    primary: Box<dyn Backend>,
    mirrors: Vec<Box<dyn Backend>>,
    write:   WritePolicy,            // WriteAll (default) | PrimaryOnly | Quorum(n)
    read:    ReadPolicy,             // FirstSuccess (default) | LatestByMTime | LatestByETagChain
}
```

- **Default: write to all backends, read first-success.**
- `read = LatestByMTime` reads from every backend and returns the freshest by `mtime`; used when you want eventual-consistency healing on read.
- Optimistic concurrency: `put(..., prev: Some(etag))` fails with `Conflict` on mismatch → caller retries (rotation is the only mutation path, so conflicts are rare and benign).
- Globally configurable via `RuntimeConfig::secrets`, locally overridable per-store.

### 6.7 Packs

`infrazeug vault pack <DataKey>` bundles every vault file sealed by that DataKey into one `.pack` (single envelope + inner manifest of `path → offset/length`). Reduces N small GETs to one on S3/WebDAV. Backend trait unchanged; the runtime prefers a fresher pack when present.

### 6.8 Typed struct loading with layered merge

Vault values are commonly groups of related fields — load them into typed Rust structs from one or more files, with later-wins precedence (mirrors the variable precedence chain).

```rust
#[derive(VaultStruct)]
struct DbCreds {
    user: String,
    password: SecretString,                        // zeroizing; never Debug/Display
    host: String,
    replicas: Vec<ReplicaCreds>,                   // Vec → concat in declared order
    tunables: BTreeMap<String, String>,            // Map → merge, later wins
}

let cfg: DbCreds = vault.load_layered::<DbCreds>([
    VaultRef::file("defaults/db.vault"),
    VaultRef::file("env/prod/db.vault"),
    VaultRef::file("machines/web-01/db.vault"),
]).await?;
```

- Each file contributes whichever fields it covers; missing required field = error naming the field and the files searched.
- `Option<T>` fields are optional; `T` is required.
- Decryption results cached in-memory for the run lifetime, keyed by file id.
- For ad-hoc access: `Value::Vault(VaultRef { file, field: Option<JsonPath> })` in `VarSet`.

### 6.9 Audit log

Append-only local file under `run_root/audit/<date>.log`: `who, when, op, file_id, data_key_id, host, success`. Optional `audit_sink: Box<dyn Backend>` ships entries to a remote backend. Off by default; one line in `RuntimeConfig` enables.

### 6.10 MCP exposure

**Secrets are not exposed to MCP.** No `vault.read` tool, no `secrets.`* tools — locked, not configurable. MCP can see *that* a vault entry exists (metadata only via `secrets.list` if enabled) but never plaintext. Prevents accidental exfiltration via an LLM session.

### 6.11 Variable integration

Precedence (lowest → highest): **global < group (in declared order) < machine < like_override**. Every level may hold vault-backed values; they're lazy-decrypted at resolution time per-machine using that machine's provider chain. In test mode, `like_override` wins, ensuring emulated twins can carry different creds/paths without touching production vars. Structs loaded via `VaultStruct` follow the same precedence chain when assembled from per-level vault references.

## 6bis. MCP Integration

First-class Model Context Protocol support so the same binary that deploys infrastructure can also expose (and consume) live system stats to an MCP client (Claude Desktop, IDEs, agents).

New crate: `infrazeug-mcp` — thin layer over an MCP server SDK (e.g. `rmcp`) wired to the framework's facts/results.

### 6bis.1 Trivial author experience

The user adds a few lines next to their deployment code:

```rust
let infra = infrazeug::new();
// ... machines, nodes ...

infra.mcp()
    .tool("disk_usage", |m: &Machine| async move {
        m.transport().exec(cmd!("df -B1 --output=source,size,used,target")).await
    })
    .resource("nodes/last_run", || infra.last_report())
    .prompt("incident_triage", include_str!("prompts/triage.md"))
    .serve_stdio()?;          // or .serve_http("0.0.0.0:7777")
```

Three primitives, all auto-registered with the MCP server:

- `**tool(name, fn)**` — callable from the LLM. Closure receives a `&Machine` (or `&[Machine]` for fleet-wide tools) and returns `serde_json::Value`. Schema auto-derived from the return type via `schemars`.
- `**resource(uri, fn)**` — read-only data exposed at an MCP URI (last run report, fact snapshots, current vault recipients).
- `**prompt(name, body)**` — reusable prompt templates the client can list.

### 6bis.2 Built-in tools (opt in with one call)

`infra.mcp().with_builtins()` registers:

- `list_machines`, `get_facts(machine)`, `last_run_report`, `plan_diff`
- `metrics(machine, kind=cpu|mem|disk|net)` — uses the existing transport, no extra agent.
- `tail_logs(machine, unit)` — `journalctl -f`-style streaming via MCP's streaming responses.

These run through the same `Transport` already configured for deployment, so SSH/agent/local/emulated all work transparently — no second connection path.

### 6bis.3 Modes

- **stdio:** for desktop MCP clients launching the user's binary as a subprocess.
- **HTTP/SSE:** for remote/long-running deployments; reuses the CLI's existing TLS config.
- **Subcommand:** the embedded CLI exposes `infrazeug mcp serve` automatically when `mcp()` is configured.

### 6bis.4 Security

- **Secrets are never exposed to MCP** (locked, not configurable — see §6.10). No `vault.read` or equivalent tool. Metadata-only secret tools are off by default and require explicit opt-in.
- Per-tool allowlist; default-deny destructive tools (`apply`, `shell_exec`) unless explicitly enabled.

### 6bis.5 Milestone placement

Slot as **M3.5** (after emulation, before secrets) so the MCP surface can already expose meaningful state during M4 secret work.

## 6ter. Interactive Controller (TUI)

`infrazeug-tui` is a ratatui + crossterm interactive controller. It is the **central agent's UI** for runs that benefit from human-in-the-loop: it consumes the scheduler's event bus, displays live progress, and answers prompts the scheduler raises (vault unlock, var-request approval, destructive confirmation, become-conflict resolution). Read-only `--watch` mode (§3.8.7) is the same TUI with interactivity disabled.

### 6ter.1 Process model

Two modes from one binary:

- **In-process (default).** `infrazeug apply --tui` runs the scheduler in the same process; TUI is the event consumer and `Interactor`. No IPC.
- **Attach.** Controller exposes a UDS at `run_root/<run_uuid>/control.sock`. `infrazeug attach [run-id]` opens the TUI as a client. Multiple viewers allowed; one is designated the **primary interactor**, others are read-only. Enables `ssh host -- infrazeug attach` to drive a remote controller from a laptop.

### 6ter.2 Interactor trait

```rust
#[async_trait]
pub trait Interactor: Send + Sync {
    async fn ask(&self, req: Interaction) -> Result<InteractionResp>;
}

pub enum Interaction {
    UnlockDataKey         { name: Arc<str>, provider: ProviderKind, hint: Option<Arc<str>> },
    ApproveVarRequest     { node: NodeId, machine: MachineId, var: VarKey, reason: Arc<str> },
    ConfirmDestructive    { node: NodeId, machine: MachineId, summary: Arc<str> },
    SignPlan              { plan_digest: PlanDigest, key: SigningKeyRef },
    ResolveBecomeConflict { node: NodeId, options: Vec<EffectiveBecome> },
    Custom                { id: Arc<str>, prompt: Arc<str>, schema: Schema },
}

pub enum InteractionResp { Passphrase(SecretString), Approve, Deny, Pick(usize), Json(Value), Cancel }
```

Implementations shipped:

- `TuiInteractor` — queues to the Prompts pane, blocks the requester until answered.
- `LineInteractor` — readline prompts for non-TTY / CI / SSH sessions.
- `AutoDenyInteractor` — unattended apply; any prompt = loud failure.
- `ScriptedInteractor` — tests.

### 6ter.3 Blocking semantics (locked)

- `UnlockDataKey` is **modal** — the whole apply waits (it's the first thing at run start anyway).
- `ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict` are **non-modal** — only the requesting node is blocked; unrelated work continues.
- `SignPlan` is raised at plan time, not apply time, via a separate subcommand path.

### 6ter.4 Layout

Four panes:

```
┌─ infrazeug · plan abc1234 · 3/7 nodes · 12s ──────────────────────┐
│ Machines                          │ Node detail: install-nginx    │
│ ✔ web-01  [████████░░] 8/10       │ Machine: web-02               │
│ ⏳ web-02  [██████░░░░] 6/10       │ stdout (live):                │
│ ✔ db-01   [██████████] 10/10      │   Reading package lists...    │
│ ✖ db-02   [████░░░░░░] 4/10 (1!)  │   nginx-core nginx-common ... │
│                                    │ captures: exit=0, lines=42    │
│ Prompts (2)                        │                               │
│ ▸ UnlockDataKey "prod" (fido2)     │ Events                        │
│ ▸ ApproveVarRequest db.password    │ 14:02:11 NodeStarted   web-02 │
│   for db-01:configure              │ 14:02:09 NodeFinished  web-01 │
├────────────────────────────────────┴───────────────────────────────┤
│ [q] quit  [p] pause  [c] cancel  [r] replay  [enter] answer  [/] filter │
└────────────────────────────────────────────────────────────────────┘
```

Resizable splits; vim-like + arrow-key navigation; mouse-aware. Status icons: `✔` ok, `⏳` running, `✖` failed, `⏸` skipped, `…` waiting.

### 6ter.5 Cancellation & replay

- `c` on a focused node → polite `Cancel(node_id)` over RPC, 10 s grace, then kill (§3.8.6).
- `C` on a focused machine → cancel all in-flight on that machine.
- `Ctrl-C` globally → confirm modal "cancel apply? [y/N]" → cascade global cancel.
- `r` on a finished node → re-queue with same inputs. Non-idempotent nodes show a warning banner; replay is still allowed.

### 6ter.6 Push-mode var serving (v1)

The TUI is what makes push-mode var serving (§3.10.4) useful in practice:

- Plan slice contains `VarRef`s; agent emits `VarRequest { node, var, audience }` over RPC.
- Controller looks up the var; per-`VarKey` ACL (`VarAcl::{Auto, Prompt, AutoForMachines}` — §3.9) decides:
  - `Auto` → resolve, return value, no prompt.
  - `Prompt` → raise `Interaction::ApproveVarRequest` → TUI shows it in the Prompts pane; on approve, return value; on deny, agent fails the node with `VarDenied`.
- Approval responses are bound to `(plan_digest, node_id, machine_id, var)` so they can't be replayed.

### 6ter.7 Inbound commands

The scheduler exposes a small command channel for the TUI:

```rust
pub enum SchedCommand {
    CancelNode    { node: NodeId, machine: MachineId, grace: Duration },
    CancelMachine { machine: MachineId },
    PauseAll, ResumeAll,
    ReplayNode    { node: NodeId, machine: MachineId },
    FilterChange  { selector: TagExpr },             // visual filter, doesn't affect execution
}
```

Same channel is exposed over the UDS for attach mode.

### 6ter.8 Non-TUI parity

`LineInteractor` covers every `Interaction` variant with readline prompts. `infrazeug apply` (without `--tui`) on a TTY uses the line interactor; on a non-TTY it uses `AutoDenyInteractor` and fails loudly when a prompt would have happened (use `--unattended-vars` to pre-approve specific var keys, mirrors `VarAcl::Auto`).

### 6ter.9 Crate layout

`infrazeug-tui` is an isolated crate: depends on `infrazeug-core` for types, `ratatui`, `crossterm`, `tokio`. The main CLI wires the `--tui` flag and the `attach` subcommand. Users who don't want a TUI don't pull ratatui transitively.

### 6ter.10 Milestone placement

- **M1 TUI MVP.** Machine grid + event log + `UnlockDataKey` prompt only (passphrase modal). Enough to drive the M1 `hello-local` example with `apply --tui`.
- **M4 TUI full.** `ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict`, replay, attach mode over UDS. Lands together with push-mode VarRequest + plan signing.

## 7. CLI

`infrazeug` (CLI from `infrazeug-cli`):

```
infrazeug plan         [--target MACHINE...] [-o plan.bin] [--sign-with KEY] [--slice MACHINE]
infrazeug apply        [--target ...] [--emulate-first] [--dry-run] [--tui|--watch] [--unattended-vars KEY...] [PLAN.bin] [--force]
infrazeug attach       [RUN_ID]                        # connect TUI to a running controller via UDS
infrazeug apply-signed PLAN.bin --trust SIGNER...
infrazeug lint                                         # DAG-only checks, no vault
infrazeug machine keygen   --machine UUID [--out FILE]
infrazeug machine register --machine UUID --pubkey FILE [--into BACKEND]
infrazeug plan publish     --for-machine UUID [--into BACKEND] [--sign-with KEY]
infrazeug plan revoke      --for-machine UUID [--with-teardown]
infrazeug bootstrap        --from FILE                 # invoked by the stub at first boot
infrazeug test         [--target ...]                 # emulation only
infrazeug agent serve-rpc                              # invoked over SSH
infrazeug agent serve-pull --repo URL --branch main
infrazeug vault keygen <data-key>
infrazeug vault recipients add|rm|list <data-key>
infrazeug vault rotate-key <data-key>
infrazeug vault rotate-file <path>
infrazeug vault pack|unpack <data-key>
infrazeug vault encrypt|decrypt|edit <path>
infrazeug vault calibrate
infrazeug vault recovery-code generate <data-key>
infrazeug secrets get|put|list
infrazeug mcp serve [--stdio | --http ADDR]           # auto-registered when mcp() is used
infrazeug gc [--run UUID | --older-than 24h | --all] [--dry-run]   # reap stragglers from crashes
```

The user's own binary embeds the library and exposes the same subcommands via `infrazeug::run(env::args())`.

## 8. Error Handling & Observability

- `thiserror` for typed errors per crate, `anyhow` only at the CLI boundary.
- `tracing` everywhere, with per-node/per-machine spans. JSON output for CI, pretty output for TTY.
- Structured `RunReport` artifact written at end of apply: node × machine × status × duration × diff.

## 9. Testing Strategy

- **Unit:** per-crate; mock `Transport` for scheduler tests.
- **Integration:** spin up an OCI container and run the full SSH-push path against `sshd` inside it.
- **Property:** scheduler given random DAGs must respect topological order and the change-propagation rules.
- **End-to-end example:** `examples/emulated-cluster` boots three QEMU VMs, deploys nginx + Postgres + a client, asserts HTTP.

## 10. Milestones

**M1 — Core skeleton.** Workspace, `Machine` (full struct), `Group` + `VarSet` (deep-merge maps, replace lists, typed `MachineSpec`, `vars!` macro, JSON/TOML loader, `vars resolve` debug CLI, `VarAcl`), `NodeMethod`, default `Scheduler` trait + central/per-machine-worker impl, `GlobalLimits`, per-machine + global locks, edge-readiness, `FailPolicy::FailFast`/`Tolerate`, per-machine downstream skip rules, event bus, `**Interactor` trait + `LineInteractor` + TUI MVP** (machine grid + event log + `UnlockDataKey` modal), `SyncAll`, `Local` transport, `shell` + `file.*` methods, `plan`/`apply`, `lint`, serializable `Plan` + drift detection, `tracing`, `RunGuard`, `gc` subcommand. End: deploy nginx to localhost with `apply --tui`.

**M2 — SSH transports + ShellOp lowering + agent build.** Agentless first (ShellOp → shell + sftp), then push agent + RPC + `infrazeug-build` cross-compile (zigbuild). End: same nginx ShellOp graph runs both agentless and agented against a containerized `sshd`.

**M3 — Emulation: OCI + BuildGraph.** `like` plumbing, `test` subcommand, `--emulate-first`. Full `ContainerSpec`/`BuildGraph`/buildkit LLB lowering, local-store default output, `Mount::Cache`/`Mount::Secret(Vault)` (gated on M4 vault landing for the secret path — stub providers until then), `OnMachine` builder, `infrazeug.lock`. End: example that emulates a remote target in a container built from a multi-stage spec, runs full playbook, then deploys.

**M4 — Secrets v1 + full TUI.** CBOR envelope + XChaCha20-Poly1305, two-tier key model (recipients → DataKeys → files), passphrase + ssh-agent providers, FS + S3 backends with `MultiBackend` write-all/read-first, packs, `VaultStruct` derive with layered merge, audit log, full `vault` CLI, variable integration, **plan signing + `apply-signed`**, push-mode VarRequest over RPC with `VarAcl` enforcement, vault-backed registry/signing keys for BuildGraph (closes the §5.1.2 build-secret loop), **full TUI** (`ApproveVarRequest`, `ConfirmDestructive`, `ResolveBecomeConflict`, replay, attach mode over UDS). End: encrypted vars/structs decrypted per machine at apply, multi-backend replication healing on read, signed plans verified before apply, interactive var approval over the TUI.

**M5 — QEMU emulation + FIDO2 (`hmac-secret`) / PKCS#11 / age / KMS providers + WebDAV backend.** Hardware-backed secrets and microVM emulation.

**M6 — Pull-mode apply + partial plans.** `infrazeug-bootstrap` stub binary, `infrazeug-pull` daemon, per-machine X25519 keypair + sealed-plan codec (reuses §6.2 envelope), `Backend`-based plan store, multi-format bootstrap input (TOML/JSON/cloud-init/Ignition), `FetchAuth` providers (NoAuth/CustomHeader/Bearer/InstanceIdentity), `plan publish`/`revoke`, OneShot + Daemon modes, custom-agent-only distribution with detached signature verification, partial plans + `WaitForHash` hash relay via controller for push-mode (pull-mode rejects `WaitForHash` at slice time).

## 11. Getting started on M1

A pragmatic build order so the first vertical slice deploys nginx to localhost end-to-end:

1. **Workspace skeleton.** Create the cargo workspace per §2, but only stub the M1 crates: `infrazeug-core`, `infrazeug-api`, `infrazeug-shell`, `infrazeug-methods`, `infrazeug-transport`, `infrazeug-cli`. Keep the rest empty.
2. **Types first.** Land `Machine`, `MachineId`, `OsHint`, `SshConfig`, `Become`, `Lifecycle`, `Group`, `GroupId`, `VarSet`, `VarValue`, `Tag` in `infrazeug-core`. Add the `uuid!()` re-export. No behaviour yet — pure data.
3. **Builder API.** In `infrazeug-api`: `Infra::new()`, `.local_machine()`, `.remote_linux(name)`, `.group(name)`, `.shell()`, `.template()`, `.add()`. Compiles, no execution.
4. **ShellOp DSL.** `infrazeug-shell`: full `ShellOp` enum, `ShellBuilder`, `Out<'i, T>` with lifetime tying, `argv!`/`vars!` macros, canonical CBOR serde. Round-trip tests.
5. **Local interpreter.** Direct execution of ShellOp via `tokio::process` — every primitive (`Run`, `Pipe`, `Seq`, `WriteFile`, ...). No SSH yet.
6. **Built-in methods.** `infrazeug-methods`: `shell` (wraps `ShellOp::Run`), `file.read/write/delete/list`. Each declares `idempotent()` correctly.
7. **Scheduler v1.** Default `Scheduler` impl: central planner over `petgraph`, per-machine worker, `GlobalLimits`, per-machine + global `LockBag`, edge-readiness, `FailPolicy::FailFast`/`Tolerate`, `SyncAll`, broadcast event bus.
8. **Plan/Apply.** `plan()` builds the DAG, validates (cycles, become conflicts), records target machines + node fingerprints, and stores stable `Unknown` outcomes. `preview()` / `--dry-run` invokes per-method `plan()` for display-only read inspection. `apply()` runs the scheduler. Serializable `Plan` (CBOR) + digest.
9. `**lint` subcommand.** Vault-free DAG checks — same validation suite canonical plan runs, without writing a plan artifact or running preview.
10. `**vars resolve` debug subcommand.** Prints resolved vars per machine with source-level annotations.
11. **TUI MVP (`infrazeug-tui`).** ratatui machine grid + event log + `UnlockDataKey` modal. `Interactor` trait + `LineInteractor` for non-TTY. `apply --tui` wired.
12. `**RunGuard` + `gc`.** `run_root/<run_uuid>/` lifecycle, signal-safe teardown, `infrazeug gc`.
13. **Example: `examples/hello-local`.** `Infra::new().shell(argv!["nginx","-v"]).on(local).add().run()`. M1 ships when this works end-to-end with `infrazeug apply --watch` producing a live TUI and a `RunReport`.

Anything not listed (SSH, agents, build, vault, MCP, emulation, pull) is deferred to its milestone.

## 12. Open Questions



- **Buildkit LLB lowering crate:** `buildkit-llb` vs hand-rolled. Decide during M3 spike.
- **Windows controller:** not v1. Document the `ControlMaster` gap; revisit when there's a real ask.
- **Stock agent for pull-mode:** deferred. v1 is custom-agent-only. Adding a stock agent later doesn't change the sealed-plan wire format.
- **Agent-to-agent hash relay** for `WaitForHash` in pull-mode: deferred; v1 stays push-mode-only for cross-machine coordination.
- **Shamir secret sharing:** deferred; revisit if a real "no single device can read alone" requirement appears.
- **Git plan-store backend:** deferred to v1.1 (signed commits offer a nice audit trail but add a binary dep).
- **API stability:** at 1.0 we lock `Machine`, `Node`, `NodeMethod`, `ShellOp`, `Transport`, `Scheduler` trait, vault envelope format, sealed-plan envelope, `Plan` CBOR schema. Everything else `0.x` until M6.

Decided (no longer open):

- RPC wire format = postcard (framed length-prefix over the agent's stdin/stdout).
- `plan` is always recomputed from facts; no state file.
- ShellOp covers all agentless needs; no shell-recipe escape hatch for tier-1.
- MCP never exposes secrets (locked).
- Pull-mode `WaitForHash` is a hard error at slice time.
- `Infra::default_node_timeout = None`; no timeout inheritance.
