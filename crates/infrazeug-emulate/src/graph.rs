//! BuildGraph DAG (SOUL §5.1.7).

use crate::error::{EmulateError, Result};
use crate::spec::{ContainerBase, ContainerSpec, CopySource, SpecId};
use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct BuildGraph {
    pub specs: Vec<Arc<ContainerSpec>>,
    levels: Vec<Vec<SpecId>>,
}

impl BuildGraph {
    pub fn from_specs(specs: Vec<Arc<ContainerSpec>>) -> Result<Self> {
        let mut ids: HashMap<SpecId, Arc<ContainerSpec>> = HashMap::new();
        for s in &specs {
            let id = s.id();
            if ids.contains_key(&id) {
                continue;
            }
            s.validate_mounts()?;
            ids.insert(id, Arc::clone(s));
        }

        let mut graph = DiGraph::<SpecId, ()>::new();
        let mut index: HashMap<SpecId, NodeIndex> = HashMap::new();
        for id in ids.keys() {
            index.insert(id.clone(), graph.add_node(id.clone()));
        }

        for (id, spec) in &ids {
            for dep_id in spec_dependencies(spec).into_iter().flatten() {
                if !index.contains_key(&dep_id) {
                    return Err(EmulateError::other(format!(
                        "spec {} depends on unregistered {}",
                        id.0, dep_id.0
                    )));
                }
                let from = index[&dep_id];
                let to = index[id];
                graph.add_edge(from, to, ());
            }
        }

        if is_cyclic_directed(&graph) {
            return Err(EmulateError::Cycle("container build graph".into()));
        }

        let order = petgraph::algo::toposort(&graph, None)
            .map_err(|_| EmulateError::Cycle("container build graph".into()))?;

        let mut level_map: HashMap<SpecId, usize> = HashMap::new();
        for idx in order {
            let id = graph[idx].clone();
            let mut lvl = 0usize;
            for edge in graph.edges_directed(idx, petgraph::Direction::Incoming) {
                let pred = graph[edge.source()].clone();
                lvl = lvl.max(level_map.get(&pred).copied().unwrap_or(0) + 1);
            }
            level_map.insert(id, lvl);
        }

        let max_level = level_map.values().copied().max().unwrap_or(0);
        let mut levels = vec![Vec::new(); max_level + 1];
        for (id, lvl) in level_map {
            levels[lvl].push(id);
        }

        Ok(Self { specs, levels })
    }

    pub fn levels(&self) -> &[Vec<SpecId>] {
        &self.levels
    }

    pub fn all_spec_ids(&self) -> Vec<SpecId> {
        self.specs.iter().map(|s| s.id()).collect()
    }
}

fn spec_dependencies(spec: &ContainerSpec) -> Vec<Option<SpecId>> {
    let mut deps = Vec::new();
    if let ContainerBase::From(inner) = &spec.base {
        deps.push(Some(inner.id()));
    }
    for step in &spec.steps {
        if let crate::spec::BuildStep::Copy {
            from: CopySource::Stage(inner),
            ..
        } = step
        {
            deps.push(Some(inner.id()));
        }
    }
    deps
}

pub fn collect_specs_from_ref(
    seen: &mut HashSet<SpecId>,
    out: &mut Vec<Arc<ContainerSpec>>,
    spec: &Arc<ContainerSpec>,
) {
    let id = spec.id();
    if !seen.insert(id) {
        return;
    }
    if let ContainerBase::From(inner) = &spec.base {
        collect_specs_from_ref(seen, out, inner);
    }
    for step in &spec.steps {
        if let crate::spec::BuildStep::Copy {
            from: CopySource::Stage(inner),
            ..
        } = step
        {
            collect_specs_from_ref(seen, out, inner);
        }
    }
    out.push(Arc::clone(spec));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{BuildConfig, ContainerBase, ContainerRuntime};

    fn empty_spec(base: ContainerBase) -> Arc<ContainerSpec> {
        Arc::new(ContainerSpec {
            base,
            steps: vec![],
            runtime: ContainerRuntime::Podman,
            build: BuildConfig::default(),
            outputs: vec![],
        })
    }

    #[test]
    fn orders_multi_stage() {
        let base = empty_spec(ContainerBase::Image(crate::spec::ImageRef::docker_io(
            "library/alpine",
            "3.19",
        )));
        let app = empty_spec(ContainerBase::From(Arc::clone(&base)));
        let graph = BuildGraph::from_specs(vec![base, app]).expect("acyclic");
        assert_eq!(graph.levels().len(), 2);
    }

    #[test]
    fn rejects_unregistered_from_dependency() {
        let inner = empty_spec(ContainerBase::Image(crate::spec::ImageRef::docker_io(
            "library/alpine",
            "3.19",
        )));
        let outer = empty_spec(ContainerBase::From(inner));
        let err = BuildGraph::from_specs(vec![outer]).unwrap_err();
        assert!(err.to_string().contains("unregistered"));
    }
}
