mod fs;

pub use fs::FsBackend;

use crate::error::{Result, SecretsError};
use async_trait::async_trait;
use bytes::Bytes;
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Etag(pub String);

#[derive(Clone, Debug)]
pub struct ObjectMeta {
    pub key: String,
    pub etag: Option<Etag>,
    pub mtime: Option<SystemTime>,
    pub size: u64,
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>>;
    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta>;
    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>>;
    async fn delete(&self, key: &str) -> Result<()>;
}

/// Backend keys are slash-separated store-relative paths. They must never be
/// interpreted as filesystem paths or URL paths that can escape the store root.
pub fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(SecretsError::Backend("empty backend key".into()));
    }
    if key.starts_with('/') || key.starts_with('\\') {
        return Err(SecretsError::Backend(format!(
            "backend key {key:?} must be relative"
        )));
    }
    if key.contains('\0') {
        return Err(SecretsError::Backend(format!(
            "backend key {key:?} contains NUL"
        )));
    }
    if key.contains(['%', '?', '#']) {
        return Err(SecretsError::Backend(format!(
            "backend key {key:?} contains unsafe URL metacharacter"
        )));
    }
    for part in key.split(['/', '\\']) {
        if part.is_empty() || part == "." || part == ".." {
            return Err(SecretsError::Backend(format!(
                "backend key {key:?} contains unsafe path component"
            )));
        }
    }
    Ok(())
}
