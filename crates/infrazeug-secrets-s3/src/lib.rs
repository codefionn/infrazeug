//! S3 storage backends for the vault (SOUL §6.5).
//!
//! - [`S3HttpBackend`] — production S3 / S3-compatible over HTTP with AWS
//!   Signature V4.
//! - [`S3CompatBackend`] — local mirror directory used by tests and offline
//!   development (M4 test path).

mod http;
pub mod sigv4;

pub use http::{S3Config, S3HttpBackend};
pub use sigv4::Credentials;

use async_trait::async_trait;
use bytes::Bytes;
use infrazeug_secrets::backend::{validate_key, Backend, Etag, FsBackend, ObjectMeta};
use infrazeug_secrets::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Prefix objects under `s3://bucket/` as `bucket/` keys in the backing FS layout.
pub struct S3CompatBackend {
    inner: Arc<FsBackend>,
    bucket: String,
}

impl S3CompatBackend {
    pub fn local_mirror(root: impl Into<PathBuf>, bucket: impl Into<String>) -> Self {
        let root = root.into();
        let bucket = bucket.into();
        let inner = Arc::new(FsBackend::new(root.join(&bucket)));
        Self { inner, bucket }
    }

    fn key(&self, key: &str) -> String {
        if key.starts_with(&format!("{}/", self.bucket)) {
            key.to_string()
        } else {
            format!("{}/{}", self.bucket, key.trim_start_matches('/'))
        }
    }
}

#[async_trait]
impl Backend for S3CompatBackend {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>> {
        validate_key(key)?;
        self.inner.get(&self.key(key)).await
    }

    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta> {
        validate_key(key)?;
        self.inner.put(&self.key(key), v, prev).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        validate_key(prefix.trim_end_matches('/'))?;
        self.inner.list(&self.key(prefix)).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        validate_key(key)?;
        self.inner.delete(&self.key(key)).await
    }
}
