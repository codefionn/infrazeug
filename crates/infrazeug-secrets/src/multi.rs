use crate::backend::{Backend, Etag, ObjectMeta};
use crate::error::{Result, SecretsError};
use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
pub enum WritePolicy {
    #[default]
    WriteAll,
    PrimaryOnly,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ReadPolicy {
    #[default]
    FirstSuccess,
    LatestByMtime,
}

pub struct MultiBackend {
    pub primary: Arc<dyn Backend>,
    pub mirrors: Vec<Arc<dyn Backend>>,
    pub write: WritePolicy,
    pub read: ReadPolicy,
}

impl MultiBackend {
    pub fn new(primary: Arc<dyn Backend>) -> Self {
        Self {
            primary,
            mirrors: Vec::new(),
            write: WritePolicy::default(),
            read: ReadPolicy::default(),
        }
    }

    pub fn with_mirror(mut self, mirror: Arc<dyn Backend>) -> Self {
        self.mirrors.push(mirror);
        self
    }

    pub fn with_read(mut self, read: ReadPolicy) -> Self {
        self.read = read;
        self
    }

    fn all_write_targets(&self) -> Vec<Arc<dyn Backend>> {
        match self.write {
            WritePolicy::PrimaryOnly => vec![Arc::clone(&self.primary)],
            WritePolicy::WriteAll => {
                let mut v = vec![Arc::clone(&self.primary)];
                v.extend(self.mirrors.iter().cloned());
                v
            }
        }
    }

    fn all_read_targets(&self) -> Vec<Arc<dyn Backend>> {
        let mut v = vec![Arc::clone(&self.primary)];
        v.extend(self.mirrors.iter().cloned());
        v
    }
}

#[async_trait]
impl Backend for MultiBackend {
    async fn get(&self, key: &str) -> Result<Option<(Bytes, ObjectMeta)>> {
        match self.read {
            ReadPolicy::FirstSuccess => {
                for b in self.all_read_targets() {
                    if let Some(v) = b.get(key).await? {
                        return Ok(Some(v));
                    }
                }
                Ok(None)
            }
            ReadPolicy::LatestByMtime => {
                let mut best: Option<(Bytes, ObjectMeta)> = None;
                for b in self.all_read_targets() {
                    if let Some((data, meta)) = b.get(key).await? {
                        let replace =
                            best.as_ref()
                                .is_none_or(|(_, m)| match (m.mtime, meta.mtime) {
                                    (Some(a), Some(b)) => b > a,
                                    (None, Some(_)) => true,
                                    _ => false,
                                });
                        if replace {
                            best = Some((data, meta));
                        }
                    }
                }
                Ok(best)
            }
        }
    }

    async fn put(&self, key: &str, v: Bytes, prev: Option<&Etag>) -> Result<ObjectMeta> {
        let mut last_meta = None;
        for b in self.all_write_targets() {
            let meta = b.put(key, v.clone(), prev).await?;
            last_meta = Some(meta);
        }
        last_meta.ok_or_else(|| SecretsError::Backend("no backends".into()))
    }

    async fn list(&self, prefix: &str) -> Result<Vec<ObjectMeta>> {
        self.primary.list(prefix).await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        for b in self.all_write_targets() {
            b.delete(key).await?;
        }
        Ok(())
    }
}
