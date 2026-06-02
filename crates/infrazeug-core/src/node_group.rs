//! Helpers for wiring child nodes to run sequentially or in parallel.
//!
//! Each group is a single connected DAG slice:
//!
//! ```text
//! entry_deps → Begin → members → Finish → downstream
//! ```
//!
//! [`SyncNodeGroup`] chains members; [`AsyncNodeGroup`] fans them out in parallel.
//! [`Infra::finish_sync_group`] / [`Infra::finish_async_group`] insert the begin
//! node automatically when it is not present yet. Call
//! [`Infra::begin_sync_group`] / [`Infra::begin_async_group`] earlier only when
//! members must be wired through begin before finish runs.

use crate::id::NodeId;
use crate::infra::{begin_node, finish_node, Infra};
use crate::node::Targets;
use uuid::Uuid;

/// Deterministic id for a group's programmatic begin node.
pub fn begin_node_id(label: &str) -> NodeId {
    let seed = format!("infrazeug/node-group/begin/{label}");
    NodeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()))
}

/// Deterministic id for a group's programmatic finish node.
pub fn finish_node_id(label: &str) -> NodeId {
    let seed = format!("infrazeug/node-group/finish/{label}");
    NodeId(Uuid::new_v5(&Uuid::NAMESPACE_OID, seed.as_bytes()))
}

/// Members the finish node should depend on: group leaf members — nodes no
/// other group member depends on. Depending on leaves (rather than roots)
/// guarantees finish waits for every member transitively.
pub fn finish_member_deps(infra: &Infra, members: &[NodeId]) -> Vec<NodeId> {
    group_leaf_members(infra, members)
}

fn group_leaf_members(infra: &Infra, members: &[NodeId]) -> Vec<NodeId> {
    members
        .iter()
        .copied()
        .filter(|&id| {
            !members.iter().any(|&other| {
                other != id
                    && infra
                        .nodes
                        .iter()
                        .find(|n| n.id == other)
                        .is_some_and(|n| n.deps.contains(&id))
            })
        })
        .collect()
}

fn push_node(infra: &mut Infra, node: crate::node::Node) -> crate::error::Result<NodeId> {
    let id = node.id;
    if infra.nodes.iter().any(|n| n.name == node.name) {
        return Err(crate::error::CoreError::DuplicateName {
            kind: "node",
            name: node.name.clone(),
        });
    }
    infra.nodes.push(node);
    Ok(id)
}

fn push_begin(
    infra: &mut Infra,
    label: &str,
    entry_deps: &[NodeId],
    targets: Targets,
) -> crate::error::Result<NodeId> {
    let begin = begin_node(
        begin_node_id(label),
        format!("{label}/begin"),
        targets,
        entry_deps.to_vec(),
    );
    push_node(infra, begin)
}

/// Point group entry members at begin instead of raw entry deps.
fn rewire_members_to_begin<G: NodeGroup>(infra: &mut Infra, group: &G, begin_id: NodeId) {
    let entry_deps = group.entry_deps();
    let member_set: std::collections::HashSet<_> = group.members().iter().copied().collect();
    for &member in group.members() {
        let Some(node) = infra.nodes.iter_mut().find(|n| n.id == member) else {
            continue;
        };
        if node.deps == entry_deps {
            node.deps = vec![begin_id];
            continue;
        }
        if node.deps.is_empty() {
            node.deps = vec![begin_id];
            continue;
        }
        let has_internal_dep = node.deps.iter().any(|d| member_set.contains(d));
        if !has_internal_dep && !node.deps.contains(&begin_id) {
            node.deps.insert(0, begin_id);
        }
    }
}

fn push_finish(
    infra: &mut Infra,
    label: &str,
    members: &[NodeId],
    targets: Targets,
) -> crate::error::Result<NodeId> {
    let finish_deps = finish_member_deps(infra, members);
    if finish_deps.is_empty() {
        return Err(crate::error::CoreError::other(
            "node group has no members for finish node",
        ));
    }
    let finish = finish_node(
        finish_node_id(label),
        format!("{label}/finish"),
        targets,
        finish_deps,
    );
    push_node(infra, finish)
}

trait NodeGroup {
    fn label(&self) -> &str;
    fn entry_deps(&self) -> &[NodeId];
    fn members(&self) -> &[NodeId];
    fn begun(&self) -> bool;
    fn finished(&self) -> bool;
    fn mark_begun(&mut self);
    fn mark_finished(&mut self);
}

/// Sequential child nodes: each new member depends on the previous one.
#[derive(Clone, Debug)]
pub struct SyncNodeGroup {
    label: String,
    entry_deps: Vec<NodeId>,
    members: Vec<NodeId>,
    begun: bool,
    finished: bool,
}

impl NodeGroup for SyncNodeGroup {
    fn label(&self) -> &str {
        &self.label
    }

    fn entry_deps(&self) -> &[NodeId] {
        &self.entry_deps
    }

    fn members(&self) -> &[NodeId] {
        &self.members
    }

    fn begun(&self) -> bool {
        self.begun
    }

    fn finished(&self) -> bool {
        self.finished
    }

    fn mark_begun(&mut self) {
        self.begun = true;
    }

    fn mark_finished(&mut self) {
        self.finished = true;
    }
}

impl SyncNodeGroup {
    pub fn new(label: impl Into<String>, entry_deps: impl IntoIterator<Item = NodeId>) -> Self {
        Self {
            label: label.into(),
            entry_deps: entry_deps.into_iter().collect(),
            members: Vec::new(),
            begun: false,
            finished: false,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Upstream deps for the next member: the previous member when one exists
    /// (members chain regardless of whether begin was inserted yet); otherwise
    /// begin after [`Infra::begin_sync_group`], or the raw entry deps.
    pub fn next_deps(&self) -> Vec<NodeId> {
        if let Some(last) = self.members.last() {
            return vec![*last];
        }
        if self.begun {
            vec![begin_node_id(&self.label)]
        } else {
            self.entry_deps.clone()
        }
    }

    pub fn push(&mut self, node_id: NodeId) {
        self.members.push(node_id);
    }

    pub fn begin_node_id(&self) -> NodeId {
        begin_node_id(&self.label)
    }

    pub fn finish_node_id(&self) -> NodeId {
        finish_node_id(&self.label)
    }

    /// Upstream dep id after [`Infra::begin_sync_group`].
    pub fn entry(&self) -> Option<NodeId> {
        if self.begun {
            Some(self.begin_node_id())
        } else {
            None
        }
    }

    /// Downstream dep id after [`Infra::finish_sync_group`].
    pub fn exit(&self) -> Option<NodeId> {
        if self.finished && !self.members.is_empty() {
            Some(self.finish_node_id())
        } else {
            None
        }
    }

    pub fn members(&self) -> &[NodeId] {
        &self.members
    }

    pub fn entry_deps(&self) -> &[NodeId] {
        &self.entry_deps
    }
}

/// Parallel child nodes: every member shares the same upstream deps.
#[derive(Clone, Debug)]
pub struct AsyncNodeGroup {
    label: String,
    entry_deps: Vec<NodeId>,
    members: Vec<NodeId>,
    begun: bool,
    finished: bool,
}

impl NodeGroup for AsyncNodeGroup {
    fn label(&self) -> &str {
        &self.label
    }

    fn entry_deps(&self) -> &[NodeId] {
        &self.entry_deps
    }

    fn members(&self) -> &[NodeId] {
        &self.members
    }

    fn begun(&self) -> bool {
        self.begun
    }

    fn finished(&self) -> bool {
        self.finished
    }

    fn mark_begun(&mut self) {
        self.begun = true;
    }

    fn mark_finished(&mut self) {
        self.finished = true;
    }
}

impl AsyncNodeGroup {
    pub fn new(label: impl Into<String>, entry_deps: impl IntoIterator<Item = NodeId>) -> Self {
        Self {
            label: label.into(),
            entry_deps: entry_deps.into_iter().collect(),
            members: Vec::new(),
            begun: false,
            finished: false,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Upstream deps for the next member: begin after [`Infra::begin_async_group`].
    pub fn next_deps(&self) -> Vec<NodeId> {
        if self.begun {
            vec![begin_node_id(&self.label)]
        } else {
            self.entry_deps.clone()
        }
    }

    pub fn push(&mut self, node_id: NodeId) {
        self.members.push(node_id);
    }

    pub fn begin_node_id(&self) -> NodeId {
        begin_node_id(&self.label)
    }

    pub fn finish_node_id(&self) -> NodeId {
        finish_node_id(&self.label)
    }

    pub fn entry(&self) -> Option<NodeId> {
        if self.begun {
            Some(self.begin_node_id())
        } else {
            None
        }
    }

    pub fn exit(&self) -> Option<NodeId> {
        if self.finished && !self.members.is_empty() {
            Some(self.finish_node_id())
        } else {
            None
        }
    }

    pub fn members(&self) -> &[NodeId] {
        &self.members
    }

    pub fn entry_deps(&self) -> &[NodeId] {
        &self.entry_deps
    }
}

impl Infra {
    /// Insert the programmatic begin node for a sync group.
    pub fn begin_sync_group(
        &mut self,
        group: &mut SyncNodeGroup,
        targets: Targets,
    ) -> crate::error::Result<NodeId> {
        begin_group(self, group, targets)
    }

    /// Insert the programmatic finish node for a sync group.
    pub fn finish_sync_group(
        &mut self,
        group: &mut SyncNodeGroup,
        targets: Targets,
    ) -> crate::error::Result<NodeId> {
        finish_group(self, group, targets)
    }

    /// Insert the programmatic begin node for an async group.
    pub fn begin_async_group(
        &mut self,
        group: &mut AsyncNodeGroup,
        targets: Targets,
    ) -> crate::error::Result<NodeId> {
        begin_group(self, group, targets)
    }

    /// Insert the programmatic finish node for an async group.
    pub fn finish_async_group(
        &mut self,
        group: &mut AsyncNodeGroup,
        targets: Targets,
    ) -> crate::error::Result<NodeId> {
        finish_group(self, group, targets)
    }
}

fn begin_group<G: NodeGroup>(
    infra: &mut Infra,
    group: &mut G,
    targets: Targets,
) -> crate::error::Result<NodeId> {
    if group.begun() {
        return Err(crate::error::CoreError::other(
            "node group begin node already inserted",
        ));
    }
    let id = push_begin(infra, group.label(), group.entry_deps(), targets)?;
    group.mark_begun();
    Ok(id)
}

fn finish_group<G: NodeGroup>(
    infra: &mut Infra,
    group: &mut G,
    targets: Targets,
) -> crate::error::Result<NodeId> {
    if group.finished() {
        return Err(crate::error::CoreError::other(
            "node group finish node already inserted",
        ));
    }
    if group.members().is_empty() {
        return Err(crate::error::CoreError::other("node group has no members"));
    }
    if !group.begun() {
        let begin_id = push_begin(infra, group.label(), group.entry_deps(), targets.clone())?;
        group.mark_begun();
        rewire_members_to_begin(infra, group, begin_id);
    }
    let id = push_finish(infra, group.label(), group.members(), targets)?;
    group.mark_finished();
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nid(seed: u8) -> NodeId {
        NodeId(Uuid::from_bytes([seed; 16]))
    }

    #[test]
    fn sync_chains_members_through_begin() {
        let upstream = nid(1);
        let mut group = SyncNodeGroup::new("deploy", [upstream]);
        assert_eq!(group.next_deps(), vec![upstream]);

        group.mark_begun();
        assert_eq!(group.next_deps(), vec![begin_node_id("deploy")]);

        group.push(nid(2));
        assert_eq!(group.next_deps(), vec![nid(2)]);

        group.push(nid(3));
        assert_eq!(group.members(), &[nid(2), nid(3)]);
        assert_eq!(group.exit(), None);
    }

    #[test]
    fn async_members_share_begin() {
        let upstream = nid(1);
        let mut group = AsyncNodeGroup::new("parallel", [upstream]);
        assert_eq!(group.next_deps(), vec![upstream]);

        group.mark_begun();
        assert_eq!(group.next_deps(), vec![begin_node_id("parallel")]);
        group.push(nid(2));
        assert_eq!(group.next_deps(), vec![begin_node_id("parallel")]);
    }

    #[test]
    fn begin_and_finish_ids_are_stable() {
        assert_eq!(begin_node_id("deploy"), begin_node_id("deploy"));
        assert_eq!(finish_node_id("deploy"), finish_node_id("deploy"));
    }

    #[test]
    fn finish_auto_inserts_begin() {
        use crate::node::{NodeBuilder, Targets};
        use infrazeug_shell::ShellOp;

        let mut infra = Infra::new();
        let mut group = AsyncNodeGroup::new("helm-apps", Vec::<NodeId>::new());
        let x = nid(2);
        infra = infra
            .add_node(
                NodeBuilder::shell(x, ShellOp::run(vec!["true".into()]), Targets::All)
                    .name("x")
                    .build(),
            )
            .unwrap();
        group.push(x);
        infra.finish_async_group(&mut group, Targets::All).unwrap();
        assert!(infra
            .nodes
            .iter()
            .any(|n| n.id == begin_node_id("helm-apps")));
        assert!(infra
            .nodes
            .iter()
            .any(|n| n.id == finish_node_id("helm-apps")));
    }

    #[test]
    fn begin_carries_entry_deps() {
        let mut infra = Infra::new();
        let prep = nid(1);
        let mut group = SyncNodeGroup::new("deploy", [prep]);
        let begin = infra.begin_sync_group(&mut group, Targets::All).unwrap();
        assert_eq!(begin, begin_node_id("deploy"));
        assert_eq!(
            infra.nodes.iter().find(|n| n.id == begin).unwrap().deps,
            vec![prep]
        );
    }

    #[test]
    fn finish_depends_on_leaf_not_root() {
        use crate::node::{NodeBuilder, Targets};
        use infrazeug_shell::ShellOp;

        let mut infra = Infra::new();
        let root = nid(2);
        let chained = nid(3);
        infra = infra
            .add_node(
                NodeBuilder::shell(root, ShellOp::run(vec!["true".into()]), Targets::All)
                    .name("root")
                    .build(),
            )
            .unwrap();
        infra = infra
            .add_node(
                NodeBuilder::shell(chained, ShellOp::run(vec!["true".into()]), Targets::All)
                    .name("chained")
                    .deps(vec![root])
                    .build(),
            )
            .unwrap();

        assert_eq!(finish_member_deps(&infra, &[root, chained]), vec![chained]);
    }

    #[test]
    fn sync_chains_members_without_explicit_begin() {
        let prep = nid(1);
        let mut group = SyncNodeGroup::new("deploy", [prep]);
        assert_eq!(group.next_deps(), vec![prep]);

        group.push(nid(2));
        assert_eq!(group.next_deps(), vec![nid(2)]);

        group.push(nid(3));
        assert_eq!(group.next_deps(), vec![nid(3)]);
    }

    #[test]
    fn finish_falls_back_to_group_leaves() {
        use crate::node::{NodeBuilder, Targets};
        use infrazeug_shell::ShellOp;

        let mut infra = Infra::new();
        let begin = nid(1);
        let x = nid(2);
        let y = nid(3);
        for (id, name, deps) in [
            (begin, "begin", vec![]),
            (x, "x", vec![begin]),
            (y, "y", vec![begin]),
        ] {
            infra = infra
                .add_node(
                    NodeBuilder::shell(id, ShellOp::run(vec!["true".into()]), Targets::All)
                        .name(name)
                        .deps(deps)
                        .build(),
                )
                .unwrap();
        }

        assert_eq!(finish_member_deps(&infra, &[x, y]), vec![x, y]);
    }

    #[test]
    fn sync_finish_depends_on_leaf_member() {
        use crate::node::{NodeBuilder, Targets};
        use infrazeug_shell::ShellOp;

        let mut infra = Infra::new();
        let begin = nid(1);
        let a = nid(2);
        let b = nid(3);
        for (id, name, deps) in [
            (begin, "begin", vec![]),
            (a, "a", vec![begin]),
            (b, "b", vec![a]),
        ] {
            infra = infra
                .add_node(
                    NodeBuilder::shell(id, ShellOp::run(vec!["true".into()]), Targets::All)
                        .name(name)
                        .deps(deps)
                        .build(),
                )
                .unwrap();
        }

        assert_eq!(finish_member_deps(&infra, &[a, b]), vec![b]);
    }
}
