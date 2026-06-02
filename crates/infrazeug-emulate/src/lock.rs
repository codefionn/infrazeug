//! `infrazeug.lock` (SOUL §5.1.9).

use crate::digest::ContentDigest;
use crate::error::{EmulateError, Result};
use crate::llb::{graph_digest, lower_spec, resolve_image_digest};
use crate::spec::{ContainerSpec, ImageRef, SpecId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    #[serde(default)]
    pub images: BTreeMap<String, String>,
    #[serde(default)]
    pub contexts: BTreeMap<String, String>,
    #[serde(default)]
    pub specs: BTreeMap<String, String>,
    #[serde(default)]
    pub graph_digest: Option<String>,
}

impl LockFile {
    pub const VERSION: u32 = 1;
    pub const FILENAME: &'static str = "infrazeug.lock";

    pub fn load(path: impl AsRef<Path>) -> Result<Option<Self>> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(path)?;
        let lock: Self = toml::from_str(&text)?;
        Ok(Some(lock))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    pub fn refresh_from_graph(
        &mut self,
        specs: &[Arc<ContainerSpec>],
        context_digests: &BTreeMap<String, ContentDigest>,
    ) -> Result<()> {
        self.version = Self::VERSION;
        let mut spec_digests = Vec::new();
        for spec in specs {
            spec.validate_mounts()?;
            let (_def, digest) = lower_spec(spec)?;
            let id = spec.id().0.clone();
            self.specs.insert(id.clone(), digest.to_string());
            spec_digests.push(digest);
            if let crate::spec::ContainerBase::Image(img) = &spec.base {
                self.images
                    .insert(img.reference(), resolve_image_digest(img).to_string());
            }
        }
        for (k, d) in context_digests {
            self.contexts.insert(k.clone(), d.to_string());
        }
        self.graph_digest = Some(graph_digest(&spec_digests).to_string());
        Ok(())
    }

    pub fn enforce_spec(&self, spec: &ContainerSpec, unpinned: bool) -> Result<ContentDigest> {
        let (_def, fresh) = lower_spec(spec)?;
        if unpinned {
            return Ok(fresh);
        }
        let id = spec.id().0.clone();
        if let Some(pinned) = self.specs.get(&id) {
            if pinned != &fresh.to_string() {
                return Err(EmulateError::LockDrift(format!(
                    "spec {id}: lock has {pinned}, computed {fresh}"
                )));
            }
        }
        Ok(fresh)
    }

    pub fn enforce_image(&self, img: &ImageRef, unpinned: bool) -> Result<()> {
        if unpinned || img.digest.is_some() {
            return Ok(());
        }
        let key = img.reference();
        let fresh = resolve_image_digest(img);
        if let Some(pinned) = self.images.get(&key) {
            if pinned != &fresh.to_string() {
                return Err(EmulateError::LockDrift(format!(
                    "image {key}: lock has {pinned}, computed {fresh}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct LockContext {
    pub path: std::path::PathBuf,
    pub lock: LockFile,
    pub unpinned: bool,
}

impl LockContext {
    pub fn open(workspace: impl AsRef<Path>, unpinned: bool) -> Result<Self> {
        let path = workspace.as_ref().join(LockFile::FILENAME);
        let lock = LockFile::load(&path)?.unwrap_or_default();
        Ok(Self {
            path,
            lock,
            unpinned,
        })
    }

    pub fn save(&self) -> Result<()> {
        self.lock.save(&self.path)
    }
}

pub type SpecLockMap = BTreeMap<SpecId, ContentDigest>;
