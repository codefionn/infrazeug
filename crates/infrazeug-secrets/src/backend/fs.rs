use super::{validate_key, Backend, Etag, ObjectMeta};
use crate::error::{Result, SecretsError};
use async_trait::async_trait;
use bytes::Bytes;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

/// Max wait before giving up on a contended compare-and-swap lock.
const LOCK_MAX_ATTEMPTS: u32 = 250;
const LOCK_RETRY: Duration = Duration::from_millis(20);
/// A lock file older than this is assumed orphaned by a crashed writer.
const LOCK_STALE: Duration = Duration::from_secs(30);

pub struct FsBackend {
    root: PathBuf,
}

impl FsBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path_for(&self, key: &str) -> Result<PathBuf> {
        validate_key(key)?;
        Ok(self.root.join(key))
    }

    async fn create_private_dir_all(path: &std::path::Path) -> Result<()> {
        tokio::fs::create_dir_all(path)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    async fn write_private_atomic(path: &std::path::Path, data: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| SecretsError::Backend("backend key has no parent".into()))?;
        Self::create_private_dir_all(parent).await?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| SecretsError::Backend("backend key has no file name".into()))?;
        let mut suffix = [0u8; 8];
        rand::thread_rng().fill_bytes(&mut suffix);
        let tmp = parent.join(format!(
            ".{name}.tmp-{}-{}",
            std::process::id(),
            hex::encode(suffix)
        ));
        // `create_new` (O_EXCL) refuses to open an existing path, so a pre-placed
        // file or symlink at the temp name cannot be written through.
        #[cfg(unix)]
        let mut file = {
            tokio::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&tmp)
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?
        };
        #[cfg(not(unix))]
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        file.write_all(data)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        // fsync the payload before rename so a crash cannot leave the target
        // pointing at data that never reached stable storage.
        file.sync_all()
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        drop(file);
        tokio::fs::rename(&tmp, path)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        // fsync the parent directory so the rename itself is durable.
        #[cfg(unix)]
        {
            std::fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
        }
        Ok(())
    }

    /// Compare-and-swap body: if `prev` is set, write only when the on-disk
    /// content still hashes to it; then write atomically and report the new meta.
    async fn put_inner(
        path: &Path,
        key: &str,
        v: &[u8],
        prev: Option<&Etag>,
    ) -> Result<ObjectMeta> {
        if let Some(p) = prev {
            if path.exists() {
                let existing = tokio::fs::read(path)
                    .await
                    .map_err(|e| SecretsError::Backend(e.to_string()))?;
                let etag = Etag(hex::encode(Sha256::digest(&existing)));
                if &etag != p {
                    return Err(SecretsError::Conflict {
                        key: key.to_string(),
                    });
                }
            }
        }
        Self::write_private_atomic(path, v).await?;
        let meta = tokio::fs::metadata(path)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        let etag = Etag(hex::encode(Sha256::digest(v)));
        Ok(ObjectMeta {
            key: key.to_string(),
            etag: Some(etag),
            mtime: meta.modified().ok(),
            size: meta.len(),
        })
    }
}

/// Per-target lock file path (`.<name>.lock`, hidden alongside the target).
fn lock_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| SecretsError::Backend("backend key has no parent".into()))?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| SecretsError::Backend("backend key has no file name".into()))?;
    Ok(parent.join(format!(".{name}.lock")))
}

/// Acquire an exclusive lock by atomically creating `lock` (O_EXCL). Retries
/// while another writer holds it, and breaks a lock left orphaned by a crash.
async fn acquire_lock(lock: &Path) -> Result<()> {
    for _ in 0..LOCK_MAX_ATTEMPTS {
        match tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(lock)
            .await
        {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Ok(meta) = tokio::fs::metadata(lock).await {
                    let age = meta
                        .modified()
                        .ok()
                        .and_then(|m| m.elapsed().ok())
                        .unwrap_or_default();
                    if age > LOCK_STALE {
                        let _ = tokio::fs::remove_file(lock).await;
                        continue;
                    }
                }
                tokio::time::sleep(LOCK_RETRY).await;
            }
            Err(e) => return Err(SecretsError::Backend(e.to_string())),
        }
    }
    Err(SecretsError::Backend(format!(
        "timed out acquiring write lock for {lock:?}"
    )))
}

#[async_trait]
impl Backend for FsBackend {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>> {
        let path = self.path_for(key)?;
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        let meta = tokio::fs::metadata(&path)
            .await
            .map_err(|e| SecretsError::Backend(e.to_string()))?;
        let mtime = meta.modified().ok();
        let etag = Etag(hex::encode(Sha256::digest(&data)));
        Ok(Some((
            Bytes::from(data),
            ObjectMeta {
                key: key.to_string(),
                etag: Some(etag),
                mtime,
                size: meta.len(),
            },
        )))
    }

    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta> {
        let path = self.path_for(key)?;
        // Without an expected etag the write is unconditional (last-writer-wins),
        // so no lock is needed. With one, serialize the read-compare-rename under
        // a per-key lock so two writers that both observe the expected etag cannot
        // both commit (lost update) — the loser gets a `Conflict`.
        let Some(expected) = prev else {
            return Self::put_inner(&path, key, &v, None).await;
        };
        if let Some(parent) = path.parent() {
            Self::create_private_dir_all(parent).await?;
        }
        let lock = lock_path(&path)?;
        acquire_lock(&lock).await?;
        let out = Self::put_inner(&path, key, &v, Some(expected)).await;
        let _ = tokio::fs::remove_file(&lock).await;
        out
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        validate_key(prefix.trim_end_matches('/'))?;
        let mut out = Vec::new();
        let start = self.root.join(prefix);
        if !start.exists() {
            return Ok(out);
        }
        let mut stack = vec![start];
        while let Some(dir) = stack.pop() {
            let mut rd = tokio::fs::read_dir(&dir)
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
            while let Some(ent) = rd
                .next_entry()
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?
            {
                let path = ent.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    // Skip our transient writer files (`.<name>.tmp-*`, `.<name>.lock`).
                    if path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n.starts_with('.'))
                    {
                        continue;
                    }
                    let rel = path.strip_prefix(&self.root).unwrap_or(path.as_path());
                    let key = rel.to_string_lossy().replace('\\', "/");
                    if !key.starts_with(prefix) {
                        continue;
                    }
                    let meta = ent
                        .metadata()
                        .await
                        .map_err(|e| SecretsError::Backend(e.to_string()))?;
                    let data = tokio::fs::read(&path)
                        .await
                        .map_err(|e| SecretsError::Backend(e.to_string()))?;
                    out.push(ObjectMeta {
                        key,
                        etag: Some(Etag(hex::encode(Sha256::digest(&data)))),
                        mtime: meta.modified().ok(),
                        size: meta.len(),
                    });
                }
            }
        }
        Ok(out)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.path_for(key)?;
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| SecretsError::Backend(e.to_string()))?;
        }
        Ok(())
    }
}
