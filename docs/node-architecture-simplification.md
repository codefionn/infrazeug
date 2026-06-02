# Node Architecture Simplification Recommendations

This document recommends ways to simplify the node architecture without giving
up current features. The goal is to reduce implementation complexity around
planning, scheduling, dynamic expansion, and graph-only nodes while preserving
the SOUL semantics.

## Preserve These Semantics

Any simplification should keep these behaviors intact:

- `Node` remains the author-facing logical unit of work.
- One logical node may target multiple machines, and successors wait for the
  whole upstream node to finish across its targets.
- `ShellOp` remains the serializable, agentless-capable tier.
- `Native` remains the typed Rust escape hatch and stays illegal on agentless
  targets.
- `Plan` remains byte-stable and does not bake live preview observations into
  the digest.
- Pull slices keep their current privacy and sealed-plan behavior.
- Dynamic machine groups remain apply-time fan-out.
- `RunPolicy::Lazy`, `RunPolicy::Always`, and `RunPolicy::OnUpstreamChange`
  keep their current meanings.
- Connect nodes remain graph-visible so lazy connectivity, filtering, TUI, and
  graph inspection can explain why a machine was contacted.

## Current Complexity

The largest complexity is not the public `Node` type itself. The complexity is
that several concepts are represented by the same structures at multiple phases:

- authoring graph: `Infra.nodes`
- canonical plan: `Plan` / `PlannedNode`
- executable graph: scheduler-local `node_by_id`, `planned_by_id`, `work`
- runtime state: outcomes, inflight units, cancellation, lazy demand
- apply-time graph mutation for dynamic groups

The scheduler currently owns too many of these responsibilities. It compiles
plan data into work units, decides graph readiness, evaluates run policies,
executes work, mutates the graph for dynamic groups, records reports, and handles
interactive commands.

## Recommendation 1: Add an Internal Execution Graph

Introduce a normalized internal graph between `Plan::executable(...)` and
`DefaultScheduler::run(...)`.

Suggested shape:

```rust
pub struct ExecutionGraph {
    pub nodes: HashMap<NodeId, ExecNode>,
    pub units: HashMap<WorkKey, WorkUnit>,
    pub dependents: HashMap<NodeId, Vec<NodeId>>,
}

pub struct ExecNode {
    pub id: NodeId,
    pub summary: NodeSummary,
    pub action: NodeAction,
    pub policy: NodePolicy,
    pub deps: Vec<NodeId>,
    pub planned: PlannedNode,
}

pub struct WorkUnit {
    pub key: WorkKey,
    pub node_id: NodeId,
    pub machine_id: MachineId,
}
```

This keeps the public `Node` unchanged, but gives the scheduler one owned,
already-resolved representation. `DefaultScheduler` should receive an
`ExecutionGraph` rather than rebuilding `node_by_id`, `planned_by_id`,
`run_policy_by_id`, `dependents_by_id`, and `work` locally.

Why this helps:

- one place resolves targets into per-machine work
- one place validates fingerprints for executable plans
- dynamic expansion can return graph patches in the same representation
- tests can validate graph compilation without running the scheduler

## Recommendation 2: Split Scheduling Decisions From Dispatch

Move readiness, lazy demand, skip, block, and run-policy logic into a pure
`GraphState` or `DecisionEngine`.

Suggested API:

```rust
pub enum UnitDecision {
    Start(WorkKey),
    Skip { key: WorkKey, reason: SkipReason },
    Wait,
}

pub struct GraphState {
    outcomes: HashMap<WorkKey, NodeStatus>,
    demanded: HashSet<NodeId>,
    run_intent: HashSet<NodeId>,
}

impl GraphState {
    pub fn next_decisions(&mut self, graph: &ExecutionGraph) -> Vec<UnitDecision>;
    pub fn record_completion(&mut self, key: WorkKey, status: NodeStatus);
}
```

This extracts the logic currently spread across functions like
`deps_satisfied`, `deps_blocked`, `propagate_lazy_demand`, `should_run`, and
`will_run_ignoring_lazy`.

The scheduler then becomes mostly:

1. drain operator commands
2. ask the decision engine what can start or skip
3. dispatch starts with limits, locks, and cancellation
4. record completions
5. emit events and report entries

This preserves all features while making the hard semantics easier to test.

## Recommendation 3: Normalize Internal Node Actions

Keep public `NodeBody` variants for compatibility, but lower them internally to
a smaller action model:

```rust
pub enum NodeAction {
    Exec(ExecAction),
    System(SystemAction),
    Noop(NoopRole),
}

pub enum ExecAction {
    Shell(ShellOp),
    Native { method_id: String, input: serde_cbor::Value },
}

pub enum SystemAction {
    Connect,
}

pub enum NoopRole {
    Barrier,
    Begin,
    Finish,
    Start,
    End,
}
```

`Barrier`, `Begin`, and `Finish` are all graph-only nodes with different
display roles. Today each caller must remember which variants are graph-only.
Lowering them to one `Noop` execution path makes scheduling and execution
simpler while preserving graph labels and DOT/TUI output.

`Connect` should not become invisible. It should be a `SystemAction::Connect`:
still a node in the graph, but not mixed with user `Shell` or `Native` work.

## Recommendation 4: Move Dynamic Fan-Out Behind Graph Patches

Dynamic groups are a feature worth keeping, but the scheduler should not know
template remapping details.

Introduce a component that consumes a completed discovery node and returns an
`ExecutionGraphPatch`:

```rust
pub struct ExecutionGraphPatch {
    pub nodes: Vec<ExecNode>,
    pub units: Vec<WorkUnit>,
    pub edges: Vec<(NodeId, NodeId)>,
    pub machines: Vec<Machine>,
}
```

The scheduler would only apply the patch and emit `UnitsAdded`. The dynamic
module would own:

- deserializing discovered machines
- deterministic machine ids
- deterministic per-machine node ids
- connect head synthesis
- exit barrier dependency extension
- registering newly discovered machines with the executor

This keeps apply-time fan-out, but removes dynamic template mechanics from the
main dispatch loop.

## Recommendation 5: Make `Plan` Explicitly Non-Executable

Keep `Plan` canonical and serializable, but avoid treating it as the direct
scheduler input. The scheduler should run a compiled `ExecutionGraph` plus the
canonical `Plan` digest/report metadata.

Recommended split:

- `Plan`: stable, serialized, signed, sliced, drift-checked
- `ExecutablePlan`: validates the plan against `Infra`
- `ExecutionGraph`: owned, scheduler-ready, target-resolved, dynamic-patchable

This reduces the repeated map-building and makes it clear which layer is stable
serialization versus mutable runtime graph.

## Recommendation 6: Keep `NodePolicy` Public, Refine It Internally

`NodePolicy` is already a good simplification over many top-level `Node` fields.
Do not churn the public API unless necessary. Internally, however, it can be
viewed as three groups:

- scheduling: `run_policy`, `fail_policy`, `retry`, `poll`, `timeout`
- coordination: local and global locks
- reporting/classification: change policy and post-run behavior

An internal wrapper can expose these groupings without changing serialized
`NodePolicy`. That makes call sites more explicit:

```rust
node.policy.scheduling().run_policy
node.policy.coordination().locks
node.policy.reporting().change_policy
```

This is optional and should happen only after the execution graph split. It is a
cleanup, not the main simplification.

## Recommendation 7: Make Graph Filtering Use the Same Lowered Roles

Tag filtering and graph inspection currently need special knowledge of graph
sentinels, connect nodes, and lazy nodes. After lowering to `NodeAction`,
filtering can ask role-oriented questions:

- `action.is_graph_only()`
- `action.is_connect()`
- `action.is_user_work()`
- `node.is_lazy()`

That avoids repeating variant matches across `infra.rs`, `graph.rs`,
`secret_scan.rs`, `slice.rs`, and scheduler code.

## Recommendation 8: Add Golden Tests Before Refactoring

Before changing internals, freeze behavior with tests around these cases:

- static multi-machine fan-out waits for all targets before successor starts
- failed-upstream propagation matches the locked SOUL behavior and any existing
  compatibility expectations
- `Tolerate` graph-only join waits for terminal failure states
- lazy chain demand propagates through multiple lazy dependencies
- lazy nodes with no non-lazy dependent are skipped as not demanded
- connect nodes are retained by tag filtering when required
- dynamic group expansion produces the same deterministic node ids
- dynamic group exit barrier tolerates a failed discovered machine
- pull slicing still rejects controller-sync and unsupported cross-machine cases

Most of these behaviors already have coverage. The goal is to make the tests
target the new pure graph compiler and decision engine directly, then keep the
existing scheduler tests as integration coverage.

## Compiling Down To The Current Microarchitecture

The simplification does not need a flag-day rewrite. The first implementation
can compile the proposed `ExecutionGraph` model down to the scheduler's current
microarchitecture, then replace the internals one piece at a time.

The current scheduler already has a concrete execution shape:

```text
Plan + Infra
  -> node_by_id: HashMap<NodeId, Node>
  -> planned_by_id: HashMap<NodeId, PlannedNode>
  -> run_policy_by_id: HashMap<NodeId, RunPolicy>
  -> dependents_by_id: HashMap<NodeId, Vec<NodeId>>
  -> work: HashSet<WorkKey>
  -> outcomes / inflight / cancellation / report
```

Treat that shape as the initial bytecode for the new internal graph.

### Lowering Passes

Use explicit passes, each pure except where noted:

1. **Validate plan against infra**

   Reuse `Plan::executable(infra)` as the first pass. It checks that every
   planned node exists and that fingerprints match.

2. **Lower public nodes to internal actions**

   Convert each public `NodeBody` into a `NodeAction`:

   | Public body | Internal action | Current execution target |
   |-------------|-----------------|--------------------------|
   | `Shell(op)` | `Exec(Shell(op))` | existing `NodeBody::Shell` arm in `run_shell` |
   | `Native { .. }` | `Exec(Native { .. })` | existing native executor arm |
   | `Connect` | `System(Connect)` | existing connectivity-probe arm |
   | `Barrier` | `Noop(Barrier)` | existing graph-only barrier arm |
   | `Begin` | `Noop(Begin)` | existing graph-only barrier arm |
   | `Finish` | `Noop(Finish)` | existing graph-only barrier arm |

   In the first stage, `ExecNode` can still retain the original `Node` beside
   `NodeAction` so dispatch can call the existing `run_shell` unchanged.

3. **Resolve targets into work units**

   For each `PlannedNode`, create one `WorkUnit` per machine in
   `planned.machines`. This is exactly the current `work: HashSet<WorkKey>`.

4. **Build adjacency**

   For every `ExecNode.deps`, add `dep -> node` to `dependents_by_id`. This is
   the same map the scheduler currently builds by scanning `node.deps`.

5. **Emit the compatibility view**

   Add a method that materializes the old maps:

   ```rust
   pub struct SchedulerCompat {
       pub node_by_id: HashMap<NodeId, Node>,
       pub planned_by_id: HashMap<NodeId, PlannedNode>,
       pub run_policy_by_id: HashMap<NodeId, RunPolicy>,
       pub dependents_by_id: HashMap<NodeId, Vec<NodeId>>,
       pub work: HashSet<WorkKey>,
   }

   impl ExecutionGraph {
       pub fn to_scheduler_compat(&self) -> SchedulerCompat;
   }
   ```

   `DefaultScheduler::run` can switch from building these maps inline to calling
   this adapter. That is the smallest useful change.

### Dynamic Graph Patches

Dynamic fan-out should compile to the same compatibility bytecode.

Current dynamic expansion does four things inside the scheduler:

- registers discovered machines with the executor
- synthesizes or remaps connect nodes
- inserts per-machine template nodes
- extends the dynamic exit barrier's dependencies

Keep those exact outputs, but move the construction to a `DynamicCompiler`:

```rust
pub struct DynamicExpansion {
    pub graph_patch: ExecutionGraphPatch,
    pub machines_to_register: Vec<Machine>,
}
```

Applying the patch updates both representations during the transition:

```text
ExecutionGraph.apply_patch(...)
  -> update nodes / units / dependents
  -> refresh or incrementally update SchedulerCompat maps
```

This lets the scheduler keep its existing dispatch loop while dynamic remapping
moves out of it.

### Slices And Pull Mode

Do not compile `PlanSlice` from `ExecutionGraph` initially. Keep slicing based
on `Plan` and `Infra` as it works today:

```text
Infra + Plan -> PlanSlice -> slice_to_plan -> apply
```

The reason is stability: slices are serialized/sealed artifacts, while
`ExecutionGraph` is a runtime representation. Once the runtime compiler is
stable, a later pass can share target/dependency helpers with slicing, but the
serialized format should stay anchored on `Plan`, `PlannedNode`, and embedded
`Node` bodies.

Dynamic groups should remain rejected or unavailable for pull-mode sealed plans
unless the discovered machine set is known before sealing. That preserves the
current apply-time discovery semantics.

### Execution Adapter

After `NodeAction` exists, add an execution adapter instead of immediately
rewriting `run_shell`:

```rust
async fn execute_action(ctx: UnitCtx<'_>, action: &NodeAction) -> UnitResult {
    match action {
        NodeAction::Exec(ExecAction::Shell(op)) => execute_shell(ctx, op).await,
        NodeAction::Exec(ExecAction::Native { method_id, input }) => {
            execute_native(ctx, method_id, input).await
        }
        NodeAction::System(SystemAction::Connect) => execute_connect(ctx).await,
        NodeAction::Noop(role) => execute_noop(ctx, *role),
    }
}
```

The first version can delegate back into the current `run_shell` by reconstructing
or retaining a `Node`. Later, split `run_shell` into the four action-specific
functions shown above. That keeps behavior stable while making the target shape
clear.

### Migration Steps

1. Add `ExecutionGraph` and `SchedulerCompat`.
2. Replace the scheduler's inline map construction with
   `ExecutionGraph::from_executable(...).to_scheduler_compat()`.
3. Add `NodeAction` lowering, but keep the original `Node` in `ExecNode`.
4. Move dynamic expansion to produce `ExecutionGraphPatch`, then adapt the patch
   into current maps.
5. Extract `GraphState` while still reading from `SchedulerCompat`.
6. Replace `run_shell` dispatch with `execute_action`.
7. Remove `SchedulerCompat` once the scheduler natively consumes
   `ExecutionGraph`.

### Decision

Compile the simplified architecture to the current microarchitecture first. In
other words, make `ExecutionGraph` a source-level IR and make the existing
scheduler maps the initial backend. That gets the simplification benefits in
small steps, keeps plan/slice formats stable, and gives tests a pure compiler
target before the scheduler loop itself is simplified.

## Recommended Order

1. Add `ExecutionGraph`, `ExecNode`, `WorkUnit`, and `NodeAction` as internal
   types. Build them from `Plan::executable(...)` without changing behavior.
2. Move scheduler map construction into `ExecutionGraph::from_executable(...)`.
3. Extract the pure decision logic into `GraphState` and port existing scheduler
   tests that cover lazy/run-policy behavior.
4. Lower graph-only nodes to `NodeAction::Noop` and connect nodes to
   `NodeAction::System(SystemAction::Connect)`.
5. Move dynamic expansion into an `ExecutionGraphPatch` producer.
6. Simplify `DefaultScheduler::run` after the above pieces are covered.
7. Optionally clean up policy accessors and graph filtering helpers.

## Avoid These Simplifications

Do not simplify by removing features that are central to the design:

- Do not remove visible connect nodes. Make them system actions instead.
- Do not replace edge-readiness with global graph levels.
- Do not bake preview outcomes into canonical plans.
- Do not collapse `Shell` and `Native`; their transport and serialization rules
  are intentionally different.
- Do not make dynamic groups plan-time-only; their value is apply-time discovery.
- Do not add per-machine pipelining between top-level nodes unless SOUL changes.
- Do not hide graph-only barriers as scheduler-only metadata; users and tools
  need to inspect them.

## Expected Result

After these changes, the architecture still has the same user-visible feature
set, but the implementation has clearer layers:

```text
Infra
  -> Plan
  -> ExecutablePlan
  -> ExecutionGraph
  -> GraphState decisions
  -> Scheduler dispatch
```

That separation should make future node features cheaper: new authoring helpers
lower into `NodeAction`, new graph semantics live in the decision engine, and new
runtime behavior lives in dispatch/execution rather than being interleaved in one
large scheduler loop.
