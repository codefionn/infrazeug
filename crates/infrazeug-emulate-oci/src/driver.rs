//! Podman-backed `EmulatedHost`.

use crate::container::ContainerCli;
use crate::containerfile::render;
use crate::podman::{build_context_dir, write_containerfile};
use async_trait::async_trait;
use infrazeug_emulate::error::{EmulateError, Result};
use infrazeug_emulate::graph::BuildGraph;
use infrazeug_emulate::host::{specs_for_container, BuiltImage, EmulatedHost, RunningContainer};
use infrazeug_emulate::llb::lower_spec;
use infrazeug_emulate::spec::{ContainerRef, SpecId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub struct PodmanHost {
    podman: ContainerCli,
    workspace: PathBuf,
    built: tokio::sync::Mutex<HashMap<String, BuiltImage>>,
}

impl PodmanHost {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            podman: ContainerCli::default(),
            workspace,
            built: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn with_cli(workspace: PathBuf, cli: ContainerCli) -> Self {
        Self {
            podman: cli,
            workspace,
            built: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn ensure_available(&self) -> Result<()> {
        if self.podman.available().await {
            Ok(())
        } else {
            Err(EmulateError::other(format!(
                "{} not available (set INFRZEUG_CONTAINER_RUNTIME or install podman/docker)",
                self.podman.runtime_name()
            )))
        }
    }

    pub fn tag_for_spec(&self, spec_id: &SpecId, run_id: Uuid) -> String {
        format!(
            "localhost/infrazeug/{}:{}",
            spec_id.0,
            &run_id.to_string()[..8]
        )
    }

    async fn build_inner(
        &self,
        run_id: Uuid,
        spec: &Arc<infrazeug_emulate::ContainerSpec>,
    ) -> Result<BuiltImage> {
        let spec_id = spec.id();
        let tag = self.tag_for_spec(&spec_id, run_id);
        if self.podman.image_exists(&tag).await {
            let (_def, digest) = lower_spec(spec)?;
            return Ok(BuiltImage {
                spec_id: spec_id.clone(),
                image_ref: tag,
                digest,
            });
        }

        let ctx = build_context_dir(&self.workspace, &run_id.to_string(), &spec_id.0);
        let from_image = if let infrazeug_emulate::spec::ContainerBase::From(inner) = &spec.base {
            self.built
                .lock()
                .await
                .get(&inner.id().0)
                .map(|b| b.image_ref.clone())
        } else {
            None
        };
        let dockerfile = render(spec, from_image.as_deref());
        let cf = write_containerfile(&ctx, &dockerfile)
            .await
            .map_err(EmulateError::other)?;
        self.podman
            .build(&ctx, &cf, &tag)
            .await
            .map_err(EmulateError::other)?;

        let (_def, digest) = lower_spec(spec)?;
        Ok(BuiltImage {
            spec_id,
            image_ref: tag,
            digest,
        })
    }
}

#[async_trait]
impl EmulatedHost for PodmanHost {
    async fn build_spec(
        &self,
        _workspace: &Path,
        _run_id: Uuid,
        spec_id: &SpecId,
        _image_id: &str,
    ) -> Result<BuiltImage> {
        self.built
            .lock()
            .await
            .get(&spec_id.0)
            .cloned()
            .ok_or_else(|| EmulateError::UnknownSpec(spec_id.0.clone()))
    }

    async fn run_container(
        &self,
        run_id: Uuid,
        image_ref: &str,
        name: &str,
    ) -> Result<RunningContainer> {
        let _ = self.podman.rm_force(name).await;
        self.podman
            .run_detached(image_ref, name, &run_id.to_string())
            .await
            .map_err(EmulateError::other)?;
        Ok(RunningContainer {
            name: name.to_string(),
            image_ref: image_ref.to_string(),
        })
    }

    async fn stop_container(&self, name: &str) -> Result<()> {
        self.podman
            .rm_force(name)
            .await
            .map_err(EmulateError::other)
    }

    fn collect_build_specs(
        &self,
        container: &ContainerRef,
    ) -> Vec<Arc<infrazeug_emulate::ContainerSpec>> {
        specs_for_container(container)
    }
}

pub async fn build_graph(
    host: &PodmanHost,
    run_id: Uuid,
    graph: &BuildGraph,
) -> Result<HashMap<SpecId, BuiltImage>> {
    host.ensure_available().await?;
    let mut out = HashMap::new();
    for level in graph.levels() {
        for spec_id in level {
            let spec = graph
                .specs
                .iter()
                .find(|s| s.id() == *spec_id)
                .ok_or_else(|| EmulateError::UnknownSpec(spec_id.0.clone()))?;
            let built = host.build_inner(run_id, spec).await?;
            host.built
                .lock()
                .await
                .insert(spec_id.0.clone(), built.clone());
            out.insert(spec_id.clone(), built);
        }
    }
    Ok(out)
}

pub fn container_name(run_id: Uuid, machine_name: &str) -> String {
    format!("iz-{}-{}", &run_id.to_string()[..8], machine_name)
}
