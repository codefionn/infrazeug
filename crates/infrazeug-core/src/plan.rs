use crate::id::{MachineId, NodeId};
use crate::infra::Infra;
use crate::node::{Node, NodeBody, NodePolicy, PlanOutcome};
use infrazeug_native::PlanMethodOutcome;
use infrazeug_secrets::{verify_signature, PlanSignature};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Map a native method's plan outcome onto the graph-level [`PlanOutcome`].
pub fn map_plan_outcome(outcome: PlanMethodOutcome) -> PlanOutcome {
    match outcome {
        PlanMethodOutcome::Unchanged => PlanOutcome::Unchanged,
        PlanMethodOutcome::Changed => PlanOutcome::Changed,
        PlanMethodOutcome::Unknown => PlanOutcome::Unknown,
    }
}

/// One node's read-only preview (display only — never persisted, see [`Preview`]).
#[derive(Clone, Debug)]
pub struct PreviewNode {
    pub node_id: NodeId,
    pub name: String,
    pub machines: Vec<MachineId>,
    pub outcome: PlanOutcome,
    /// `false` when the node could not be inspected (shell node, a native node not
    /// on a Local target, or a failed read) — `outcome` is
    /// [`PlanOutcome::Unknown`] in that case.
    pub previewable: bool,
    /// Why a previewable node fell back to `Unknown` (e.g. an API/auth error).
    pub note: Option<String>,
}

/// Result of [`Infra::preview`](crate::Infra::preview): a dry-run that actually
/// observes live state for previewable (Local native) nodes.
///
/// Distinct from [`Plan`] on purpose: a [`Plan`]'s digest folds in every node's
/// `outcome`, so the canonical plan keeps outcomes `Unknown` for a stable digest;
/// a preview carries *real* outcomes and must not be written to disk or used for
/// digest-drift checks.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub nodes: Vec<PreviewNode>,
}

/// `change` / `in_sync` / `unknown` tallies over a [`Preview`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewCounts {
    pub change: usize,
    pub in_sync: usize,
    pub unknown: usize,
}

impl Preview {
    pub fn counts(&self) -> PreviewCounts {
        let mut c = PreviewCounts::default();
        for n in &self.nodes {
            match n.outcome {
                PlanOutcome::Changed => c.change += 1,
                PlanOutcome::Unchanged => c.in_sync += 1,
                PlanOutcome::Unknown => c.unknown += 1,
            }
        }
        c
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlanDigest(pub [u8; 32]);

impl PlanDigest {
    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl std::fmt::Display for PlanDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.hex())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    pub digest: PlanDigest,
    pub nodes: Vec<PlannedNode>,
    #[serde(default)]
    pub signatures: Vec<PlanSignature>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedNode {
    pub node_id: NodeId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub machines: Vec<MachineId>,
    pub outcome: PlanOutcome,
    #[serde(default)]
    pub fingerprint: NodeFingerprint,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeFingerprint(pub [u8; 32]);

impl NodeFingerprint {
    pub fn is_zero(&self) -> bool {
        self.0 == [0; 32]
    }
}

impl std::fmt::Display for NodeFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

#[derive(Serialize)]
struct NodeFingerprintInput<'a> {
    body: &'a NodeBody,
    deps: &'a [NodeId],
    tags: &'a [crate::id::Tag],
    policy: &'a NodePolicy,
}

pub fn node_fingerprint(node: &Node) -> NodeFingerprint {
    let input = NodeFingerprintInput {
        body: &node.body,
        deps: &node.deps,
        tags: &node.tags,
        policy: &node.policy,
    };
    let bytes = serde_cbor::to_vec(&input).expect("node fingerprint cbor");
    let hash = Sha256::digest(&bytes);
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&hash);
    NodeFingerprint(digest)
}

pub struct ExecutablePlan<'a> {
    pub plan: &'a Plan,
    pub planned_by_id: FxHashMap<NodeId, &'a PlannedNode>,
    pub node_by_id: FxHashMap<NodeId, &'a Node>,
}

impl<'a> ExecutablePlan<'a> {
    pub fn planned(&self, id: NodeId) -> Option<&'a PlannedNode> {
        self.planned_by_id.get(&id).copied()
    }

    pub fn node(&self, id: NodeId) -> Option<&'a Node> {
        self.node_by_id.get(&id).copied()
    }
}

pub fn plan_digest(plan: &Plan) -> PlanDigest {
    let mut nodes = plan.nodes.clone();
    nodes.sort_by_key(|n| n.node_id);
    let bytes = serde_cbor::to_vec(&nodes).expect("plan cbor");
    let hash = Sha256::digest(&bytes);
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&hash);
    PlanDigest(digest)
}

impl Plan {
    pub fn finalize(mut self) -> Self {
        self.signatures.clear();
        self.digest = plan_digest(&self);
        self
    }

    /// Verify the plan carries at least one cryptographically valid signature
    /// from a key in `trusted`.
    ///
    /// Fail-closed: an empty `trusted` set is rejected. Each [`PlanSignature`]
    /// embeds its own verifying key, so a signature that merely verifies against
    /// its bundled key proves nothing about authorization — the key must be
    /// independently trusted by the caller.
    pub fn verify_signatures(&self, trusted: &[[u8; 32]]) -> crate::error::Result<()> {
        if trusted.is_empty() {
            return Err(crate::error::CoreError::other(
                "no trusted signer keys configured; refusing to verify plan signatures",
            ));
        }
        // Bind the signed digest to the plan's actual contents. Signatures cover
        // `digest` only, so without this check `nodes` can be rewritten around an
        // untouched digest+signature and still verify.
        let recomputed = plan_digest(self);
        if recomputed != self.digest {
            return Err(crate::error::CoreError::other(format!(
                "plan digest {} does not match plan contents (recomputed {}); \
                 plan was modified after signing",
                self.digest, recomputed
            )));
        }
        let trusted_ok = self.signatures.iter().any(|sig| {
            trusted.contains(&sig.public_key) && verify_signature(&self.digest.0, sig).is_ok()
        });
        if !trusted_ok {
            return Err(crate::error::CoreError::other(
                "plan has no valid signature from a trusted signer",
            ));
        }
        Ok(())
    }

    pub fn to_cbor(&self) -> crate::error::Result<Vec<u8>> {
        Ok(serde_cbor::to_vec(self)?)
    }

    pub fn from_cbor(bytes: &[u8]) -> crate::error::Result<Self> {
        Ok(serde_cbor::from_slice(bytes)?)
    }

    pub fn executable<'a>(&'a self, infra: &'a Infra) -> crate::error::Result<ExecutablePlan<'a>> {
        let planned_by_id: FxHashMap<NodeId, &PlannedNode> =
            self.nodes.iter().map(|p| (p.node_id, p)).collect();
        let all_nodes_by_id: FxHashMap<NodeId, &Node> =
            infra.nodes.iter().map(|n| (n.id, n)).collect();
        let mut node_by_id = FxHashMap::default();

        for planned in &self.nodes {
            let node = all_nodes_by_id
                .get(&planned.node_id)
                .copied()
                .ok_or_else(|| {
                    crate::error::CoreError::other(format!(
                        "planned node `{}` is missing from infra",
                        planned.node_id
                    ))
                })?;
            let fresh = node_fingerprint(node);
            if !planned.fingerprint.is_zero() && planned.fingerprint != fresh {
                return Err(crate::error::CoreError::other(format!(
                    "planned node `{}` fingerprint drift: file {} != recomputed {}",
                    planned.name, planned.fingerprint, fresh
                )));
            }
            node_by_id.insert(planned.node_id, node);
        }

        Ok(ExecutablePlan {
            plan: self,
            planned_by_id,
            node_by_id,
        })
    }

    pub fn write_file(&self, path: impl AsRef<Path>) -> crate::error::Result<()> {
        std::fs::write(path, self.to_cbor()?)?;
        Ok(())
    }

    pub fn read_file(path: impl AsRef<Path>) -> crate::error::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_cbor(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{MachineId, NodeId};
    use crate::node::PlanOutcome;
    use uuid::Uuid;

    fn planned(name: &str) -> PlannedNode {
        PlannedNode {
            node_id: NodeId(Uuid::new_v4()),
            name: name.into(),
            description: None,
            machines: vec![MachineId(Uuid::new_v4())],
            outcome: PlanOutcome::Unknown,
            fingerprint: NodeFingerprint::default(),
        }
    }

    #[test]
    fn finalize_sets_stable_digest() {
        let plan = Plan {
            digest: PlanDigest([0; 32]),
            nodes: vec![planned("a"), planned("b")],
            signatures: vec![],
        }
        .finalize();
        assert_ne!(plan.digest.0, [0u8; 32]);
        let again = Plan {
            digest: PlanDigest([0; 32]),
            nodes: plan.nodes.clone(),
            signatures: vec![],
        }
        .finalize();
        assert_eq!(plan.digest, again.digest);
    }

    #[test]
    fn verify_rejects_tampered_nodes_with_intact_signature() {
        use infrazeug_secrets::{sign_digest, signing_key_from_seed};

        let mut plan = Plan {
            digest: PlanDigest([0; 32]),
            nodes: vec![planned("a"), planned("b")],
            signatures: vec![],
        }
        .finalize();
        let key = signing_key_from_seed(&[7u8; 32]);
        let sig = sign_digest(&plan.digest.0, &key, "test-signer");
        let trusted = [sig.public_key];
        plan.signatures.push(sig);
        plan.verify_signatures(&trusted).unwrap();

        // Rewrite nodes while keeping digest + signature byte-identical.
        let mut tampered = plan.clone();
        tampered.nodes[0].machines.push(MachineId(Uuid::new_v4()));
        assert!(tampered.verify_signatures(&trusted).is_err());
    }

    #[test]
    fn cbor_roundtrip() {
        let plan = Plan {
            digest: PlanDigest([1u8; 32]),
            nodes: vec![planned("echo")],
            signatures: vec![],
        };
        let bytes = plan.to_cbor().unwrap();
        let decoded = Plan::from_cbor(&bytes).unwrap();
        assert_eq!(decoded, plan);
    }
}
