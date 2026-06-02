use crate::id::Tag;
use crate::id::{MachineId, NodeId};
use crate::retry::{PollConfig, RetryConfig};
use infrazeug_shell::ShellOp;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub body: NodeBody,
    pub targets: Targets,
    pub deps: Vec<NodeId>,
    pub tags: Vec<Tag>,
    #[serde(flatten)]
    pub policy: NodePolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodePolicy {
    pub run_policy: RunPolicy,
    pub fail_policy: FailPolicy,
    pub timeout: Option<Duration>,
    #[serde(flatten)]
    pub locks: LockPolicy,
    #[serde(default, skip_serializing_if = "RetryConfig::is_off")]
    pub retry: RetryConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollConfig>,
    #[serde(flatten)]
    pub success: SuccessPolicy,
    #[serde(default, skip_serializing_if = "PostRunPolicy::is_none")]
    pub post_run: PostRunPolicy,
}

impl Default for NodePolicy {
    fn default() -> Self {
        Self {
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            success: SuccessPolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LockPolicy {
    #[serde(default)]
    pub local_locks: Vec<String>,
    #[serde(default)]
    pub global_locks: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SuccessPolicy {
    /// Runtime classification for successful shell output.
    ///
    /// By default, a shell node that exits `0` reports [`NodeStatus::Changed`].
    /// Add output rules when a command's stdout/stderr contains enough signal
    /// to distinguish real changes from a no-op, for example package-manager
    /// output such as `0 upgraded`.
    #[serde(default, skip_serializing_if = "OutputChangePolicy::is_default")]
    pub change_policy: OutputChangePolicy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum PostRunPolicy {
    #[default]
    None,
    /// The node is expected to reboot or otherwise disconnect the host.
    ///
    /// Core first confirms the host actually rebooted (its boot id changed), then
    /// polls `readiness_check` when present and only reports the node `Changed`
    /// once it exits `0`.
    ExpectReboot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        readiness_check: Option<ShellOp>,
    },
}

impl PostRunPolicy {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Node {
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn expects_reboot(&self) -> bool {
        matches!(self.policy.post_run, PostRunPolicy::ExpectReboot { .. })
    }

    pub fn readiness_check(&self) -> Option<&ShellOp> {
        match &self.policy.post_run {
            PostRunPolicy::None => None,
            PostRunPolicy::ExpectReboot { readiness_check } => readiness_check.as_ref(),
        }
    }
}

/// Static node facts for controller UIs (TUI node labels, §6ter).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSummary {
    pub name: String,
    pub description: Option<String>,
}

impl NodeSummary {
    pub fn from_node(node: &Node) -> Self {
        Self {
            name: node.name.clone(),
            description: node.description.clone(),
        }
    }

    /// Prefer playbook [`name`](Self::name); fall back to a short id prefix.
    pub fn display_name(&self, id: NodeId) -> String {
        if self.name.is_empty() {
            short_id_prefix(id)
        } else {
            self.name.clone()
        }
    }
}

/// First 8 characters of a uuid string (compact fallback label).
pub fn short_id_prefix(id: impl std::fmt::Display) -> String {
    id.to_string().chars().take(8).collect()
}

/// Fluent constructor for [`Node`] (optional [`Self::name`] and [`Self::description`]).
pub struct NodeBuilder {
    id: NodeId,
    name: Option<String>,
    description: Option<String>,
    body: NodeBody,
    targets: Targets,
    deps: Vec<NodeId>,
    run_policy: RunPolicy,
    fail_policy: FailPolicy,
    timeout: Option<Duration>,
    tags: Vec<Tag>,
    locks: LockPolicy,
    retry: RetryConfig,
    poll: Option<PollConfig>,
    change_policy: OutputChangePolicy,
    post_run: PostRunPolicy,
}

impl NodeBuilder {
    pub fn shell(id: NodeId, op: ShellOp, targets: Targets) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Shell(op),
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    pub fn barrier(id: NodeId, targets: Targets) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Barrier,
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    pub fn begin(id: NodeId, targets: Targets) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Begin,
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    pub fn finish(id: NodeId, targets: Targets) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Finish,
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    /// Connectivity / agent-upload head node (see [`NodeBody::Connect`]).
    ///
    /// Defaults to [`RunPolicy::Always`] so the machine's reachability and agent
    /// are verified on every apply regardless of upstream change.
    pub fn connect(id: NodeId, targets: Targets) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Connect,
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::Always,
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    pub fn native(id: NodeId, method_id: impl Into<String>, targets: Targets) -> Self {
        Self::native_with_input(id, method_id, serde_cbor::Value::Null, targets)
    }

    pub fn native_with_input(
        id: NodeId,
        method_id: impl Into<String>,
        input: serde_cbor::Value,
        targets: Targets,
    ) -> Self {
        Self {
            id,
            name: None,
            description: None,
            body: NodeBody::Native {
                method_id: method_id.into(),
                input,
            },
            targets,
            deps: Vec::new(),
            run_policy: RunPolicy::default(),
            fail_policy: FailPolicy::FailFast,
            timeout: None,
            tags: Vec::new(),
            locks: LockPolicy::default(),
            retry: RetryConfig::default(),
            poll: None,
            change_policy: OutputChangePolicy::default(),
            post_run: PostRunPolicy::None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn targets(mut self, targets: Targets) -> Self {
        self.targets = targets;
        self
    }

    pub fn deps(mut self, deps: Vec<NodeId>) -> Self {
        self.deps = deps;
        self
    }

    pub fn run_policy(mut self, run_policy: RunPolicy) -> Self {
        self.run_policy = run_policy;
        self
    }

    pub fn fail_policy(mut self, fail_policy: FailPolicy) -> Self {
        self.fail_policy = fail_policy;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn tags(mut self, tags: Vec<Tag>) -> Self {
        self.tags = tags;
        self
    }

    pub fn local_locks(mut self, local_locks: Vec<String>) -> Self {
        self.locks.local_locks = local_locks;
        self
    }

    pub fn global_locks(mut self, global_locks: Vec<String>) -> Self {
        self.locks.global_locks = global_locks;
        self
    }

    pub fn retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    pub fn poll(mut self, poll: PollConfig) -> Self {
        self.poll = Some(poll);
        self
    }

    /// Replace the successful-output change classifier for this node.
    ///
    /// The classifier is evaluated only after shell execution exits `0`.
    /// Non-zero exits still follow normal failure/retry handling.
    pub fn change_policy(mut self, change_policy: OutputChangePolicy) -> Self {
        self.change_policy = change_policy;
        self
    }

    /// Mark a successful shell node as changed when `needle` appears in `stream`.
    ///
    /// Rules are evaluated in insertion order; the first match wins.
    pub fn changed_when_contains(
        mut self,
        stream: OutputMatchStream,
        needle: impl Into<String>,
    ) -> Self {
        self.change_policy
            .rules
            .push(OutputChangeRule::changed_when_contains(stream, needle));
        self
    }

    /// Mark a successful shell node as unchanged when `needle` appears in `stream`.
    ///
    /// Use this for commands that exit `0` both for no-op and changed outcomes,
    /// but print a stable marker in stdout/stderr. If the node is unchanged,
    /// default [`RunPolicy::OnUpstreamChange`] successors are skipped.
    pub fn unchanged_when_contains(
        mut self,
        stream: OutputMatchStream,
        needle: impl Into<String>,
    ) -> Self {
        self.change_policy
            .rules
            .push(OutputChangeRule::unchanged_when_contains(stream, needle));
        self
    }

    pub fn expect_shutdown(mut self, val: bool) -> Self {
        self.post_run = if val {
            PostRunPolicy::ExpectReboot {
                readiness_check: self.post_run_readiness_check().cloned(),
            }
        } else {
            PostRunPolicy::None
        };
        self
    }

    /// App-level readiness probe run after an `expect_shutdown` reboot is
    /// confirmed; polled until it exits `0`.
    pub fn readiness_check(mut self, op: ShellOp) -> Self {
        self.post_run = PostRunPolicy::ExpectReboot {
            readiness_check: Some(op),
        };
        self
    }

    pub fn post_run(mut self, post_run: PostRunPolicy) -> Self {
        self.post_run = post_run;
        self
    }

    fn post_run_readiness_check(&self) -> Option<&ShellOp> {
        match &self.post_run {
            PostRunPolicy::None => None,
            PostRunPolicy::ExpectReboot { readiness_check } => readiness_check.as_ref(),
        }
    }

    pub fn build(self) -> Node {
        let name = self.name.unwrap_or_else(|| self.id.to_string());
        Node {
            id: self.id,
            name,
            description: self.description,
            body: self.body,
            targets: self.targets,
            deps: self.deps,
            tags: self.tags,
            policy: NodePolicy {
                run_policy: self.run_policy,
                fail_policy: self.fail_policy,
                timeout: self.timeout,
                locks: self.locks,
                retry: self.retry,
                poll: self.poll,
                success: SuccessPolicy {
                    change_policy: self.change_policy,
                },
                post_run: self.post_run,
            },
        }
    }
}

impl NodeBody {
    /// Graph-only nodes that perform no remote work (barriers and group bookends).
    pub fn is_graph_only(&self) -> bool {
        matches!(self, Self::Barrier | Self::Begin | Self::Finish)
    }

    /// Connectivity / agent-upload head node ([`NodeBody::Connect`]).
    pub fn is_connect(&self) -> bool {
        matches!(self, Self::Connect)
    }

    /// User-authored remote work (shell or native), as opposed to graph-only
    /// barriers and the system connect probe.
    pub fn is_user_work(&self) -> bool {
        matches!(self, Self::Shell(_) | Self::Native { .. })
    }

    /// Group bookend inserted by node groups ([`Begin`](Self::Begin) /
    /// [`Finish`](Self::Finish)).
    pub fn is_group_bookend(&self) -> bool {
        matches!(self, Self::Begin | Self::Finish)
    }

    /// Short kind label for graph inspection, DOT, and TUI.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Shell(_) => "shell",
            Self::Barrier => "barrier",
            Self::Begin => "begin",
            Self::Finish => "finish",
            Self::Connect => "connect",
            Self::Native { .. } => "native",
        }
    }
}

impl Node {
    /// Demand-driven node ([`RunPolicy::Lazy`]): dormant until a live dependent
    /// pulls it.
    pub fn is_lazy(&self) -> bool {
        matches!(self.policy.run_policy, RunPolicy::Lazy)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NodeBody {
    Shell(ShellOp),
    /// Graph-only dependency barrier.
    ///
    /// Barrier nodes perform no remote work. The scheduler marks them changed
    /// only when their own plan changed or an upstream dependency changed, so
    /// they preserve meaningful change propagation without fake shell commands.
    Barrier,
    /// Programmatic entry point for node groups; carries external entry deps.
    ///
    /// Like [`Barrier`], begin nodes perform no remote work. [`SyncNodeGroup`]
    /// and [`AsyncNodeGroup`] insert one via [`crate::node_group::begin_node_id`].
    Begin,
    /// Programmatic exit point for node groups; joins member work.
    ///
    /// Like [`Barrier`], finish nodes perform no remote work. Groups insert one
    /// via [`crate::node_group::finish_node_id`].
    Finish,
    /// Connectivity / agent-readiness probe — the machine's first transport use.
    ///
    /// Executing this node forces the transport to come up: for push transport it
    /// triggers an arch probe, agent upload, and RPC ping; for agentless it is an
    /// SSH reachability check; for a local machine it is a trivial success. It is
    /// the in-graph replacement for the eager pre-apply agent build/upload phase,
    /// and the per-machine head of a dynamic-machine fan-out.
    ///
    /// It reports [`NodeStatus::Changed`] on success (a fresh connection / agent
    /// upload is a state change), so machine-root successors wired through it keep
    /// the same "runs every apply" behavior they had as graph roots.
    Connect,
    /// Tier-1 native method (`NodeMethod` on Local or push agent).
    Native {
        method_id: String,
        #[serde(default = "default_native_input")]
        input: serde_cbor::Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Targets {
    Machine(MachineId),
    Machines(Vec<MachineId>),
    All,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum RunPolicy {
    #[default]
    OnUpstreamChange,
    Always,
    /// Demand-driven execution: the node stays dormant until pulled by at least
    /// one non-skipped dependent, then runs through normal dependency ordering.
    Lazy,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum FailPolicy {
    FailFast,
    Tolerate { max_failed: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanOutcome {
    Unchanged,
    Changed,
    Unknown,
}

/// Ordered rules for mapping successful shell output to changed/unchanged.
///
/// # Intent
///
/// Shell commands often use exit status only for success/failure, not for
/// idempotence. A package upgrade, service reload, or apply command may exit
/// `0` whether it changed the machine or found nothing to do. This policy lets
/// playbooks promote stable stdout/stderr markers into the node's runtime
/// [`NodeStatus`], so [`RunPolicy::OnUpstreamChange`] successors only run when
/// real work happened.
///
/// The classifier is intentionally node-level rather than a `ShellOp` transport
/// feature: transports should report raw execution results, while the node owns
/// how those results affect graph propagation.
///
/// Empty policy preserves the historical shell behavior: exit `0` means
/// [`OutputMatchStatus::Changed`]. For non-empty policies, the first matching
/// rule decides the status; if no rule matches, the status is still `Changed`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChangePolicy {
    /// Rules checked in order against captured stdout/stderr.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<OutputChangeRule>,
}

impl OutputChangePolicy {
    /// Whether this policy preserves default shell change behavior.
    pub fn is_default(&self) -> bool {
        self.rules.is_empty()
    }

    /// Classify a successful shell result from raw stdout/stderr bytes.
    pub fn classify(&self, stdout: &[u8], stderr: &[u8]) -> OutputMatchStatus {
        self.rules
            .iter()
            .find(|rule| rule.matches(stdout, stderr))
            .map(|rule| rule.status)
            .unwrap_or(OutputMatchStatus::Changed)
    }
}

/// One substring rule in an [`OutputChangePolicy`].
///
/// Matching is byte-based and case-sensitive. Empty `contains` matches any
/// successful output for the selected stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChangeRule {
    /// Which output stream is searched.
    pub stream: OutputMatchStream,
    /// Byte substring to search for, stored as UTF-8 playbook text.
    pub contains: String,
    /// Status returned when this rule is the first match.
    pub status: OutputMatchStatus,
}

impl OutputChangeRule {
    /// Build a rule that maps matching successful output to `Changed`.
    pub fn changed_when_contains(stream: OutputMatchStream, needle: impl Into<String>) -> Self {
        Self {
            stream,
            contains: needle.into(),
            status: OutputMatchStatus::Changed,
        }
    }

    /// Build a rule that maps matching successful output to `Unchanged`.
    pub fn unchanged_when_contains(stream: OutputMatchStream, needle: impl Into<String>) -> Self {
        Self {
            stream,
            contains: needle.into(),
            status: OutputMatchStatus::Unchanged,
        }
    }

    fn matches(&self, stdout: &[u8], stderr: &[u8]) -> bool {
        if self.contains.is_empty() {
            return true;
        }
        let needle = self.contains.as_bytes();
        match self.stream {
            OutputMatchStream::Stdout => contains_bytes(stdout, needle),
            OutputMatchStream::Stderr => contains_bytes(stderr, needle),
            OutputMatchStream::Any => {
                contains_bytes(stdout, needle) || contains_bytes(stderr, needle)
            }
        }
    }
}

/// Output stream selector for [`OutputChangeRule`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMatchStream {
    /// Search only stdout.
    Stdout,
    /// Search only stderr.
    Stderr,
    /// Search both stdout and stderr; a match in either stream wins.
    Any,
}

/// Status assigned by an output change rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMatchStatus {
    /// Successful shell execution changed the machine.
    Changed,
    /// Successful shell execution was a no-op.
    Unchanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Running,
    Changed,
    Unchanged,
    Skipped,
    Failed,
    Cancelled,
}

fn default_native_input() -> serde_cbor::Value {
    serde_cbor::Value::Null
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
