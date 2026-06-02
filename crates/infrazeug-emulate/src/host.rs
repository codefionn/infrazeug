//! Emulated host factory trait (SOUL §5).

use crate::digest::ContentDigest;
use crate::error::Result;
use crate::spec::{ContainerRef, SpecId};
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct BuiltImage {
    pub spec_id: SpecId,
    pub image_ref: String,
    pub digest: ContentDigest,
}

#[derive(Clone, Debug)]
pub struct RunningContainer {
    pub name: String,
    pub image_ref: String,
}

#[async_trait]
pub trait EmulatedHost: Send + Sync {
    async fn build_spec(
        &self,
        workspace: &Path,
        run_id: Uuid,
        spec_id: &SpecId,
        image_id: &str,
    ) -> Result<BuiltImage>;

    async fn run_container(
        &self,
        run_id: Uuid,
        image_ref: &str,
        name: &str,
    ) -> Result<RunningContainer>;

    async fn stop_container(&self, name: &str) -> Result<()>;

    fn collect_build_specs(&self, container: &ContainerRef)
        -> Vec<Arc<crate::spec::ContainerSpec>>;
}

pub fn specs_for_container(container: &ContainerRef) -> Vec<Arc<crate::spec::ContainerSpec>> {
    use crate::graph::collect_specs_from_ref;
    use crate::spec::ContainerRef;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    match container {
        ContainerRef::Prebuilt(_) => {}
        ContainerRef::Spec(spec) => collect_specs_from_ref(&mut seen, &mut out, spec),
    }
    out
}
