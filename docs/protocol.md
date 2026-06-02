# RPC & Planning Protocol Microarchitecture

This document describes how infrazeug encodes, transports, and applies infrastructure plans across machines. There are two operational modes—**push** (controller drives remote hosts over SSH) and **pull** (hosts fetch sealed slices from a shared store)—both built on the same plan/slice core types.

## Table of Contents

- [Wire Format](#wire-format)
- [RPC Message Types](#rpc-message-types)
- [Plan Lifecycle](#plan-lifecycle)
- [Slicing: Plan to PlanSlice](#slicing-plan-to-planslice)
- [Push-Mode Architecture](#push-mode-architecture)
- [Pull-Mode Architecture](#pull-mode-architecture)
- [Sealed Plan Binary Format](#sealed-plan-binary-format)
- [Hash Relay (Push-Mode Cross-Machine Coordination)](#hash-relay-push-mode-cross-machine-coordination)
- [Transport Routing](#transport-routing)
- [Key Type Reference](#key-type-reference)
- [Source Index](#source-index)

---

## Wire Format

The `infrazeug-rpc` crate defines a length-prefixed postcard framing protocol used exclusively over the agent's stdin/stdout pipes.

```
┌──────────────────┬──────────────────────────────────────────────┐
│  uvarint(length)  │  postcard-serialized payload                 │
│  (1–10 bytes)     │   stdin:  RpcRequest   stdout: AgentFrame    │
└──────────────────┴──────────────────────────────────────────────┘
```

- **uvarint encoding**: unsigned varint, little-endian, 7 bits per byte, MSB = continuation bit. Max 10 bytes for u64.
- **Payload**: `postcard::to_allocvec(msg)` — compact, no-alloc serde format.
- **Max frame size**: 16 MiB before `TooLarge` error.

### Multiplexed stdout

The stdin direction carries plain `RpcRequest` frames. The stdout direction is **multiplexed**: every frame is an `AgentFrame` envelope tagging the payload as either a correlated reply or an unsolicited event.

```rust
enum AgentFrame {
    Response(RpcResponse), // reply to the in-flight request, in order
    Event(AgentEvent),     // agent-initiated, not tied to any request
}

enum AgentEvent {
    Metrics(AgentMetrics), // cpu_pct + mem/disk used/total, pushed on a timer
}
```

The controller's reader task (`rpc_channel.rs`) owns stdout, decodes each `AgentFrame`, and demultiplexes: `Response` frames go to the waiting request over an mpsc channel; `Event` frames are forwarded to observers (the TUI metrics readout via `SchedEvent::MachineMetrics`). Because `request_lock` still serializes whole request→reply exchanges, the response channel has exactly one consumer at a time and delivers the current request's frames in order — while metrics keep flowing even while a long command holds the request lock. The agent serializes its own writes through a shared `FrameWriter` so the request handler and the metrics task never interleave mid-frame.

| Component | Source |
|-----------|--------|
| Frame encode/decode | `infrazeug-rpc/src/frame.rs` |
| uvarint helpers | `frame.rs:38–68` |

---

## RPC Message Types

Defined in `infrazeug-rpc/src/messages.rs`.

### RpcRequest (controller → agent)

```rust
enum RpcRequest {
    Ping,
    ExecuteShell { op: ShellOp },
    SyncNodeGraphState {
        completed: Vec<RpcNodeGraphEntry>, // node_id + terminal status known to controller
    },
    VarRequest {
        node_id: Uuid,
        machine_id: Uuid,
        var_key: String,
        plan_digest: [u8; 32],
    },
    ExecuteNative {
        method_id: String,
        input: serde_cbor::Value,
    },
}
```

### RpcResponse (agent → controller)

```rust
enum RpcResponse {
    Pong,
    ExecOutputChunk { stream, data },    // zero or more before ExecResult for ExecuteShell
    ExecResult(ExecOutput),       // { exit_code: i32, stdout: Vec<u8>, stderr: Vec<u8> }
    NodeGraphStateSynced,
    VarValue(serde_json::Value),
    VarDenied { reason: String },
    NativeResult(NativeResult),   // tier-1 method outcome (Changed/Unchanged + optional output)
    Error(String),
}
```

`RpcResponse` frames travel wrapped in `AgentFrame::Response` (see [Multiplexed stdout](#multiplexed-stdout)).

### AgentEvent (agent → controller, out-of-band)

Unsolicited frames the agent emits on its own, wrapped in `AgentFrame::Event`:

```rust
enum AgentEvent {
    Metrics(AgentMetrics), // pushed every ~2s by the serve-rpc metrics task
}

struct AgentMetrics {
    cpu_pct: f32,      // busy % over the last sampling window, 0.0–100.0
    mem_used: u64,     // bytes; from /proc/meminfo (MemTotal − MemAvailable)
    mem_total: u64,
    disk_used: u64,    // bytes for `/`; parsed from `df -kP`
    disk_total: u64,
}
```

### Frame Errors

```rust
enum FrameError {
    Io(std::io::Error),
    Postcard(postcard::Error),
    TooLarge,
    Eof,  // incomplete frame or empty buffer
}
```

---

## Plan Lifecycle

The planning pipeline flows through these stages:

```
Infra (graph) → Plan (sorted + digest) → PlanSlice (per-machine) → Apply
```

### 1. Plan Construction (`infra.plan()`)

`infrazeug-core/src/infra.rs:212`

1. Lint the node graph (detect cycles, missing deps, orphan nodes).
2. Topological-sort nodes respecting `deps`.
3. Assign each `PlannedNode` to its target machines (from `Targets`).
4. `Plan::finalize()` sorts nodes deterministically and computes the plan digest (SHA-256 of CBOR-encoded sorted nodes).

```rust
struct Plan {
    digest: PlanDigest,           // [u8; 32] SHA-256
    nodes: Vec<PlannedNode>,
    signatures: Vec<PlanSignature>,
}

struct PlannedNode {
    node_id: NodeId,
    name: String,
    machines: Vec<MachineId>,     // target machines
    outcome: PlanOutcome,         // Unchanged | Changed | Unknown
}
```

**Serialization**: Plans are encoded as CBOR (`serde_cbor::to_vec`), not postcard. The digest covers the CBOR-encoded sorted node list.

### 2. Slice Construction (`plan.slice_for_machine()`)

`infrazeug-core/src/slice.rs:91`

Each machine receives its own `PlanSlice` containing only the nodes assigned to it, plus coordination markers for cross-machine dependencies.

### 3. Apply (`infra.apply_slice()`)

`infrazeug-core/src/infra.rs:259`

- Push mode: `WaitForHash` steps are resolved through the `HashRelay`, then stripped via `slice_to_plan()` before the scheduler runs.
- Pull mode: no `WaitForHash` steps exist (forbidden at slice time); the slice is applied directly.

---

## Slicing: Plan to PlanSlice

`infrazeug-core/src/slice.rs`

### SliceMode

```rust
enum SliceMode {
    Push,   // cross-machine deps become WaitForHash markers
    Pull,   // FAILS if any cross-machine dep would be required
}
```

### PlanSlice

```rust
struct PlanSlice {
    machine_id: MachineId,
    digest: PlanDigest,
    steps: Vec<SliceStep>,
    agent_digest: Option<String>,                              // pull-mode: content-addressed agent
    inlined_vars: HashMap<String, serde_cbor::Value>,         // pull-mode: resolved vault refs
    signatures: Vec<PlanSignature>,                            // pull-mode: Ed25519 signatures
    embedded_nodes: Vec<Node>,                                 // pull-mode: node bodies
    embedded_machine: Option<Machine>,                         // pull-mode: machine definition
}
```

### SliceStep

```rust
enum SliceStep {
    Node(PlannedNode),
    WaitForHash {
        id: WaitId,
        expect: Sha256Digest,
        sources: Vec<MachineId>,
    },
}
```

### Slice Algorithm

1. Filter plan nodes assigned to `machine_id`.
2. For each node, check its `deps`:
   - **Same-machine dep**: no marker needed (already in the slice).
   - **Cross-machine dep + Push mode**: insert `SliceStep::WaitForHash` with expected completion digest.
   - **Cross-machine dep + Pull mode**: return `PullSliceNeedsWait` error immediately.
3. For pull mode: embed node bodies, machine definition, and vault var references into the slice.
4. Call `.finalize()` which computes `slice_digest` (excludes `inlined_vars` from the hash so re-sealing doesn't change the digest).

### Completion Digest

`slice.rs:209`

```
SHA-256("node-completion-v1" || node_id_bytes || source_machine_id_bytes...)
```

---

## Push-Mode Architecture

In push mode, the controller SSHes to each remote host and runs an agent binary that accepts postcard-RPC commands over stdio.

### Sequence Diagram

```
Controller                              Remote Host
──────────                              ───────────
  │                                        │
  │──── SCP agent binary ────────────────>│  ~/.cache/infrazeug/agent-0.1.0
  │                                        │
  │──── ssh -T host -- agent serve-rpc ──>│
  │       (piped stdin/stdout)             │
  │                                        │
  │──── Ping (postcard frame) ───────────>│
  │<─── Pong ─────────────────────────────│
  │                                        │
  │  [ready: RpcChannel established]       │
  │                                        │
  │──── ExecuteShell{ShellOp::Run{…}} ───>│
  │<─── ExecResult{exit_code,stdout,stderr}│
  │                                        │
  │──── ExecuteShell{ShellOp::WriteFile…}>│
  │<─── ExecResult{0} ────────────────────│
  │                                        │
  │  … more ops per slice step …          │
  │                                        │
  │  [stdin closed]                        │
  │<─── process exits ────────────────────│
```

### Component Details

#### AgentPushBackend

`infrazeug-transport/src/ssh/agent_push.rs`

1. **SCP**: Uploads the cross-compiled agent binary to `~/.cache/infrazeug/agent-{VERSION}` via `session.upload_bytes()` (staging + atomic rename to avoid ETXTBSY).
2. **SSH spawn**: Runs `ssh -T <dest> -- <remote_bin> serve-rpc` with piped stdin/stdout/stderr.
3. **RpcChannel**: Takes ownership of the child's stdin/stdout pipes and spawns the demux reader. An optional metrics sink forwards `AgentMetrics` on to the event stream.
4. **Ping handshake**: Sends `Ping`, expects `Pong` within 30 seconds.
5. **Steady state**: Each `ShellOp` from the scheduler is sent as `ExecuteShell`, response is `ExecResult`.

#### RpcChannel (Controller Side)

`infrazeug-transport/src/ssh/rpc_channel.rs`

```rust
struct RpcChannel {
    stdin: Arc<Mutex<ChildStdin>>,
    responses: Mutex<mpsc::UnboundedReceiver<Result<RpcResponse>>>, // fed by reader task
    request_lock: Mutex<()>,
}
```

- A background **reader task** owns `ChildStdout`, decodes each `AgentFrame`, and routes `Response` → `responses`, `Event(Metrics)` → the metrics sink. See [Multiplexed stdout](#multiplexed-stdout).
- `request(req)`: take `request_lock` → encode → write to stdin → flush → `recv` the reply from `responses`.
- `execute_shell(op)`: sends `ExecuteShell{op}` → `recv`s zero or more `ExecOutputChunk` frames, then `ExecResult`.
- `ping()`: sends `Ping` → expects `Pong`.

#### Agent-Side Handler

`infrazeug-agent/src/main.rs`

`serve_rpc()` wraps stdout in a shared `FrameWriter`, spawns a metrics task (`metrics_loop`), then reads stdin in 4096-byte chunks and decodes `RpcRequest` frames in an inner loop:

- **Success**: dispatch to `handle_request()` → write `AgentFrame::Response` via the shared writer.
- **Eof**: need more data, break inner loop to read more stdin.
- **Other error**: send `RpcResponse::Error(msg)` and clear buffer.
- **Metrics task**: every ~2s, samples CPU/mem (`/proc`) and disk (`df -kP /`) and writes `AgentFrame::Event(Metrics(..))` through the same shared writer.

`handle_request()` dispatch:
| Request | Response |
|---------|----------|
| `Ping` | `Pong` |
| `ExecuteShell{op}` | zero or more `ExecOutputChunk{stream,data}`, then `ExecResult(out)` or `Error(e)` |
| `VarRequest{..}` | `VarDenied{reason: "controller must serve vars"}` |

### Agentless SSH Alternative

`infrazeug-transport/src/ssh/agentless.rs`

When `TransportChoice::SshAgentless` is selected, no agent binary is uploaded. Instead, `ShellOp` values are **lowered** to raw SSH/SFTP operations:

| ShellOp | Lowered Operation |
|---------|-------------------|
| `Run{argv, cwd}` | `ssh host -- <command>` |
| `ReadFile{path}` | SFTP download |
| `WriteFile{path, content, mode}` | SFTP upload |
| `EnsureDir{path, mode}` | `ssh host -- mkdir -p -m MODE path` |

SSH sessions use OpenSSH 8.0+ ControlMaster multiplexing for connection reuse.

---

## Pull-Mode Architecture

In pull mode, the target host fetches its own sealed plan slice from a shared store (filesystem, S3, etc.) and applies it locally without any live controller connection.

### Architecture Diagram

```
Controller (build machine)          Plan Store (shared FS/S3)          Target Host
┌──────────────────────┐           ┌───────────────────────┐         ┌──────────────────┐
│  plan-op publish     │──sealed──>│ plans/{uuid}.plan     │         │ serve-pull       │
│  plan-op slice       │   slice   │   .sealed             │         │ or daemon        │
│  machine keygen      │           │ machines/{uuid}.pub   │         │ or bootstrap     │
│  machine register    │──pubkey──>│                       │<─unseal─│                  │
└──────────────────────┘           └───────────────────────┘  apply  └──────────────────┘
```

### Publish Flow (Controller → Store)

`infrazeug-pull/src/publish.rs:23`

1. `plan = infra.plan()` — compute fresh plan.
2. `slice = plan.slice_for_machine(infra, mid, SliceMode::Pull)` — **must be Pull mode** (no cross-machine deps allowed).
3. Optionally sign slice digest with Ed25519 key → `PlanSignature`.
4. `slice.to_cbor()` → plaintext bytes.
5. `seal_bytes(plaintext, machine_pubkey)` — X25519 key exchange → XChaCha20-Poly1305 AEAD.
6. `store.put_sealed_plan(machine, &sealed)`.

### Apply Flow (Target Host)

`infrazeug-pull/src/serve.rs:33`

1. Check `store.is_revoked(machine)` — tombstone check.
2. `store.get_sealed_plan(machine)` → sealed blob.
3. `MachineKeyPair::read_private_file(key_path)` → X25519 private key.
4. `unseal_bytes(sealed, secret)` → plaintext CBOR.
5. `PlanSlice::from_cbor(plaintext)` → slice.
6. Verify `slice.machine_id == MachineId(machine)`.
7. Verify all `slice.signatures` via Ed25519 `verify_signature()`.
8. Build `Infra` from `slice.embedded_nodes` and `slice.embedded_machine`.
9. `infra.apply_slice(slice, AutoDenyInteractor, ...)` — no interactive prompts.

### Daemon Mode

`infrazeug-pull/src/daemon.rs`

- Polls store at `interval ± jitter/2`.
- Tracks `last_digest` (SHA-256 of sealed blob).
- Re-applies only when digest changes.
- Stops if machine is revoked.

### Bootstrap

`infrazeug-pull/src/bootstrap.rs`

First-boot configuration parsed from TOML, JSON, `#cloud-config` YAML, or Ignition JSON:

```rust
struct Bootstrap {
    machine_id: Uuid,
    plan_url: String,
    agent_url: String,
    agent_digest: String,
    agent_signer: String,
    plan_signer: String,
    machine_key: PathBuf,
    fetch_auth: FetchAuth,     // NoAuth | BearerToken | CustomHeader | InstanceIdentity
    poll_interval: Option<Duration>,
}
```

Exec modes:
- **InProcess**: calls `run_from_bootstrap()` directly in the bootstrap binary.
- **DelegateAgent**: parses config, then execs `infrazeug-agent serve-pull --store ... --machine ... --key ...`.

---

## Sealed Plan Binary Format

`infrazeug-secrets/src/machine_key.rs:69`

```
┌──────────────┬─────────┬──────────────────┬──────────────────┬─────────────────────────────┐
│ INFRZSLD     │ version │ ephemeral_pub    │ nonce            │ XChaCha20-Poly1305          │
│ (8 bytes)    │ (1 byte)│ (32 bytes)       │ (24 bytes)       │ ciphertext + 16-byte tag    │
└──────────────┴─────────┴──────────────────┴──────────────────┴─────────────────────────────┘
```

Current `version` is `0x02`.

**Key derivation**:
1. Ephemeral X25519 key exchange: `shared = ephemeral.diffie_hellman(recipient_pubkey)`.
2. Reject a non-contributory exchange (all-zero `shared`, from a low-order `ephemeral_pub`).
3. HKDF-SHA256: `salt = b"infrazeug-sealed-plan-v1"`, `input_key = shared_secret`,
   `info = ephemeral_pub || recipient_pubkey` (both keys bound for domain separation).
4. Output: 32-byte data-encryption key.
5. AEAD: XChaCha20-Poly1305 with random 24-byte nonce.

> v1 blobs (recipient-pubkey-only `info`) are no longer accepted. Sealed plans are
> short-lived and republished, so the version bump needs no on-disk migration.

**Sealing provides confidentiality, not authenticity.** Anyone holding a machine's
public X25519 key can seal a blob to it, so the apply side
(`apply_sealed_slice`) must independently authenticate the plan: it recomputes the
slice digest from the slice contents and requires a valid Ed25519 signature from a
key in the host's bootstrapped `plan_signer` trust set (fail-closed: an empty trust
set is rejected).

---

## Hash Relay (Push-Mode Cross-Machine Coordination)

`infrazeug-core/src/hash_relay.rs`

When a push-mode slice contains `WaitForHash` steps, the controller uses a `HashRelay` to coordinate across machines:

```rust
struct HashRelay {
    inner: Arc<Mutex<HashMap<WaitId, RelaySlot>>>,
}

struct RelaySlot {
    expect: Option<[u8; 32]>,
    seen: Option<[u8; 32]>,
    notify: Arc<Notify>,
}
```

### Flow

1. **Register**: Before applying a slice, `register_wait(wait_id, expected_digest)` creates a slot.
2. **Execute**: The scheduler runs nodes. When a node completes on its source machine, `report_node_completion(node_id, sources, digest)` fills `slot.seen` and fires `notify`.
3. **Block**: `wait_for(wait_id, expected)` blocks (via `tokio::Notify`) until `seen == expect`.
4. **Proceed**: Once unblocked, the waiting machine's slice continues past the `WaitForHash` marker.

This enables safe concurrent application across multiple machines while respecting cross-machine dependency ordering.

---

## Transport Routing

`infrazeug-transport/src/factory.rs`

`TransportFactory` implements `OpExecutor` and routes per-machine via `build_backend()`:

| TransportChoice | MachineKind | Backend | Agent Binary |
|-----------------|-------------|---------|-------------|
| `Local` | `Local` | `LocalShellExecutor` | n/a |
| `SshAgentPush` | `Remote` | `AgentPushBackend` | SCP'd at connect time |
| `SshAgentless` | `Remote` | `AgentlessBackend` | none (SSH/SFTP only) |
| — | `Container` | `PodmanExec` | n/a |
| `PullDaemon` | any | Error (not yet implemented) | — |

`TransportFactory::prepare(infra)` initializes all backends before apply starts.

---

## Key Type Reference

### ShellOp DSL

`infrazeug-shell/src/op.rs`

```rust
enum ShellOp {
    Run { argv: Vec<String>, cwd: Option<PathBuf> },
    Seq { steps: Vec<ShellOp> },
    ReadFile { path: PathBuf },
    WriteFile { path: PathBuf, content: FileSource, mode: u32 },
    EnsureDir { path: PathBuf, mode: u32 },
}
```

### FileSource

`infrazeug-shell/src/source.rs`

```rust
enum FileSource {
    Bytes(Vec<u8>),
    Capture(CaptureRef),                      // stdout from upstream node
    Vault { file, field },                    // resolved at apply time
    VaultYamlSubstitute { template, substitutions },
}
```

### VarSet / VarAcl

`infrazeug-core/src/varset.rs`

```rust
struct VarSet { entries: BTreeMap<VarKey, VarEntry> }
struct VarEntry { value: VarValue, acl: VarAcl }

enum VarValue {
    Scalar(serde_json::Value),
    Vault(VaultRef),
    List(Vec<VarValue>),
    Map(BTreeMap<String, VarValue>),
}

enum VarAcl {
    Auto,
    Prompt,
    AutoForMachines(Vec<MachineId>),
}
```

### PlanSignature

`infrazeug-secrets/src/sign.rs`

```rust
struct PlanSignature {
    signer_id: String,
    public_key: [u8; 32],     // Ed25519 verifying key
    signature: Vec<u8>,        // Ed25519 signature over slice digest
}
```

### PlanStore Layout

`infrazeug-pull/src/store.rs`

```
bootstrap/{machine_id}.toml           bootstrap configuration
plans/{machine_id}.plan.sealed        sealed CBOR slice
machines/{machine_id}.pub             X25519 public key (32 bytes raw)
tombstones/{machine_id}               revocation marker
agents/{digest}/{triple}/infrazeug-agent   content-addressed agent binary
agents/{digest}.sig                        detached Ed25519 signature
```

---

## Source Index

| Component | File | Key Lines |
|-----------|------|-----------|
| RPC framing | `infrazeug-rpc/src/frame.rs` | 15–68 |
| RPC messages | `infrazeug-rpc/src/messages.rs` | 6–25 |
| RPC channel (controller) | `infrazeug-transport/src/ssh/rpc_channel.rs` | 10–99 |
| Agent main / serve-rpc | `infrazeug-agent/src/main.rs` | 12–88 |
| Plan type | `infrazeug-core/src/plan.rs` | 8–89 |
| PlanSlice + SliceStep | `infrazeug-core/src/slice.rs` | 15–233 |
| SliceMode | `infrazeug-core/src/slice.rs` | 16–21 |
| HashRelay | `infrazeug-core/src/hash_relay.rs` | 9–80 |
| ShellOp DSL | `infrazeug-shell/src/op.rs` | 7–56 |
| ShellOp lowering | `infrazeug-shell/src/lower.rs` | 7–70 |
| FileSource | `infrazeug-shell/src/source.rs` | 12–69 |
| Local executor | `infrazeug-shell/src/local.rs` | 7–122 |
| ExecOutput | `infrazeug-shell/src/local.rs` | 7–12 |
| TransportFactory | `infrazeug-transport/src/factory.rs` | 18–163 |
| AgentPushBackend | `infrazeug-transport/src/ssh/agent_push.rs` | 14–83 |
| AgentlessBackend | `infrazeug-transport/src/ssh/agentless.rs` | 6–93 |
| SSH session | `infrazeug-transport/src/ssh/session.rs` | 16–293 |
| TransportChoice | `infrazeug-core/src/transport.rs` | 4–10 |
| PlanStore | `infrazeug-pull/src/store.rs` | 7–135 |
| Publish (seal + sign) | `infrazeug-pull/src/publish.rs` | 9–78 |
| Serve (unseal + apply) | `infrazeug-pull/src/serve.rs` | 12–99 |
| Daemon | `infrazeug-pull/src/daemon.rs` | 13–64 |
| Bootstrap config | `infrazeug-pull/src/bootstrap.rs` | 9–97 |
| PullMode | `infrazeug-pull/src/mode.rs` | 4–26 |
| Sealed plan format | `infrazeug-secrets/src/machine_key.rs` | 13–129 |
| PlanSignature | `infrazeug-secrets/src/sign.rs` | 6–42 |
| VarSet / VarAcl | `infrazeug-core/src/varset.rs` | 10–56 |
| VarServe (push-mode) | `infrazeug-core/src/var_serve.rs` | 12–173 |
| Node type | `infrazeug-core/src/node.rs` | 7–69 |
| Machine type | `infrazeug-core/src/machine.rs` | 7–99 |
| Infra (plan/apply) | `infrazeug-core/src/infra.rs` | 24–544 |
| Scheduler | `infrazeug-core/src/scheduler.rs` | 26–467 |
| CLI dispatch | `infrazeug-api/src/cli.rs` | 287–341 |
| Pull CLI | `infrazeug-api/src/pull_cli.rs` | 83–329 |
| ID types | `infrazeug-core/src/id.rs` | 8–71 |
