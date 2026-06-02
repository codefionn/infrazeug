//! Container build types (SOUL §5.1).

use crate::digest::ContentDigest;
use infrazeug_shell::op::Argv;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SpecId(pub String);

impl SpecId {
    pub fn from_spec(spec: &ContainerSpec) -> Self {
        let digest = ContentDigest::hash_json(spec).expect("spec serializes");
        Self(digest.short())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContainerRef {
    Prebuilt(ImageRef),
    Spec(Arc<ContainerSpec>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub base: ContainerBase,
    pub steps: Vec<BuildStep>,
    pub runtime: ContainerRuntime,
    pub build: BuildConfig,
    pub outputs: Vec<BuildOutput>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ContainerBase {
    Scratch,
    Image(ImageRef),
    From(Arc<ContainerSpec>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ContainerRuntime {
    #[default]
    Podman,
    Containerd,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildConfig {
    pub builder: Builder,
    pub platforms: Vec<Platform>,
    pub cross: CrossPolicy,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            builder: Builder::Local,
            platforms: vec![Platform::linux_amd64()],
            cross: CrossPolicy::Auto,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Builder {
    Local,
    Buildkit { addr: String },
    OnMachine(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Platform {
    pub os: String,
    pub arch: String,
}

impl Platform {
    pub fn linux_amd64() -> Self {
        Self {
            os: "linux".into(),
            arch: "amd64".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CrossPolicy {
    #[default]
    Auto,
    PreferCross,
    EmulateOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BuildStep {
    Run {
        argv: Argv,
        env: Vec<(String, String)>,
        mounts: Vec<Mount>,
        network: NetMode,
        cache_id: Option<String>,
    },
    Copy {
        from: CopySource,
        src: Vec<PathBuf>,
        dest: PathBuf,
        chmod: Option<u32>,
    },
    Env {
        kv: Vec<(String, String)>,
    },
    Workdir(PathBuf),
    User(String),
    Cmd(Argv),
    Entrypoint(Argv),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CopySource {
    Context(BuildContext),
    Stage(Arc<ContainerSpec>),
    Image(ImageRef),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BuildContext {
    LocalDir {
        path: PathBuf,
        include: Vec<String>,
        exclude: Vec<String>,
    },
    Whole(PathBuf),
    InlineFiles(BTreeMap<PathBuf, Vec<u8>>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Mount {
    Cache {
        id: String,
        target: PathBuf,
        sharing: CacheSharing,
    },
    Secret {
        id: String,
        target: PathBuf,
        source: SecretSource,
    },
    Bind {
        source: PathBuf,
        target: PathBuf,
        readonly: bool,
    },
    TmpFs {
        target: PathBuf,
        size: Option<u64>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CacheSharing {
    #[default]
    Shared,
    Private,
    Locked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SecretSource {
    Vault { path: String },
    EnvVar(String),
    File(PathBuf),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum NetMode {
    #[default]
    Default,
    None,
    Host,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BuildOutput {
    LocalStore {
        runtime: ContainerRuntime,
        namespace: String,
    },
    OciImage {
        image: ImageRef,
        push: bool,
    },
    OciTarball {
        path: PathBuf,
    },
    Rootfs {
        path: PathBuf,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageRef {
    pub registry: String,
    pub repo: String,
    pub tag: Option<String>,
    pub digest: Option<ContentDigest>,
}

impl ImageRef {
    pub fn docker_io(repo: &str, tag: &str) -> Self {
        Self {
            registry: "docker.io".into(),
            repo: repo.into(),
            tag: Some(tag.into()),
            digest: None,
        }
    }

    pub fn reference(&self) -> String {
        let name = if self.registry == "docker.io" {
            if self.repo.contains('/') {
                format!("docker.io/{}", self.repo)
            } else {
                format!("docker.io/library/{}", self.repo)
            }
        } else {
            format!("{}/{}", self.registry, self.repo)
        };
        if let Some(d) = &self.digest {
            format!("{name}@{}", d)
        } else if let Some(tag) = &self.tag {
            format!("{name}:{tag}")
        } else {
            name
        }
    }
}

/// Placeholder for M5 QEMU microVM.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MicroVmConfig {
    pub image: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VmImage {
    RemoteQcow2(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QemuConfig {
    pub memory_mb: u32,
}

impl Default for QemuConfig {
    fn default() -> Self {
        Self { memory_mb: 1024 }
    }
}

/// Emulated twin configuration on a production machine (SOUL §3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LikeConfig {
    pub kind: EmulatedKind,
    #[serde(default)]
    pub vars: super::like::LikeVars,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmulatedKind {
    Local,
    Container(ContainerRef),
    MicroVm { image: VmImage, qemu: QemuConfig },
}

pub fn is_emulated_kind(kind: &EmulatedKind) -> bool {
    matches!(
        kind,
        EmulatedKind::Local | EmulatedKind::Container(_) | EmulatedKind::MicroVm { .. }
    )
}

impl ContainerSpec {
    pub fn id(&self) -> SpecId {
        SpecId::from_spec(self)
    }

    pub fn validate_mounts(&self) -> crate::error::Result<()> {
        for step in &self.steps {
            if let BuildStep::Run { mounts, .. } = step {
                for m in mounts {
                    if let Mount::Secret {
                        source: SecretSource::Vault { .. },
                        ..
                    } = m
                    {
                        return Err(crate::error::EmulateError::VaultSecretMount);
                    }
                }
            }
        }
        if matches!(self.build.builder, Builder::OnMachine(_)) {
            return Err(crate::error::EmulateError::OnMachineBuilder);
        }
        Ok(())
    }
}
