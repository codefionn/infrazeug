//! Canonical LLB-style definition + lowering (SOUL §5.1.8 spike).

use crate::digest::ContentDigest;
use crate::error::Result;
use crate::spec::{ContainerBase, ContainerSpec, ImageRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Definition {
    pub spec_id: String,
    pub base: BaseNode,
    pub steps: Vec<StepNode>,
    pub platforms: Vec<String>,
    pub builder: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BaseNode {
    Scratch,
    Image {
        reference: String,
        digest: Option<String>,
    },
    From {
        spec_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepNode {
    Run { argv: Vec<String> },
    Copy { from: String, dest: String },
    Env { kv: Vec<(String, String)> },
    Meta { kind: String, value: String },
}

pub fn lower_spec(spec: &ContainerSpec) -> Result<(Definition, ContentDigest)> {
    let spec_id = spec.id().0.clone();
    let base = match &spec.base {
        ContainerBase::Scratch => BaseNode::Scratch,
        ContainerBase::Image(img) => BaseNode::Image {
            reference: img.reference(),
            digest: img.digest.map(|d| d.to_string()),
        },
        ContainerBase::From(inner) => BaseNode::From {
            spec_id: inner.id().0,
        },
    };
    let mut steps = Vec::new();
    for step in &spec.steps {
        use crate::spec::BuildStep;
        match step {
            BuildStep::Run { argv, .. } => steps.push(StepNode::Run { argv: argv.clone() }),
            BuildStep::Copy { dest, .. } => steps.push(StepNode::Copy {
                from: "context".into(),
                dest: dest.display().to_string(),
            }),
            BuildStep::Env { kv } => steps.push(StepNode::Env { kv: kv.clone() }),
            BuildStep::Workdir(p) => steps.push(StepNode::Meta {
                kind: "workdir".into(),
                value: p.display().to_string(),
            }),
            BuildStep::User(u) => steps.push(StepNode::Meta {
                kind: "user".into(),
                value: u.clone(),
            }),
            BuildStep::Cmd(argv) => steps.push(StepNode::Meta {
                kind: "cmd".into(),
                value: argv.join(" "),
            }),
            BuildStep::Entrypoint(argv) => steps.push(StepNode::Meta {
                kind: "entrypoint".into(),
                value: argv.join(" "),
            }),
        }
    }
    let def = Definition {
        spec_id,
        base,
        steps,
        platforms: spec
            .build
            .platforms
            .iter()
            .map(|p| format!("{}/{}", p.os, p.arch))
            .collect(),
        builder: format!("{:?}", spec.build.builder),
    };
    let digest = ContentDigest::hash_json(&def)?;
    Ok((def, digest))
}

pub fn graph_digest(spec_digests: &[ContentDigest]) -> ContentDigest {
    let mut sorted: Vec<String> = spec_digests.iter().map(|d| d.to_string()).collect();
    sorted.sort();
    ContentDigest::hash_bytes(sorted.join("\n").as_bytes())
}

pub fn resolve_image_digest(img: &ImageRef) -> ContentDigest {
    if let Some(d) = img.digest {
        return d;
    }
    ContentDigest::hash_bytes(img.reference().as_bytes())
}
