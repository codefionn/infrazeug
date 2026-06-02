use crate::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use infrazeug_shell::{local::LocalShellExecutor, FileSource, ShellOp};
use std::path::{Path, PathBuf};

pub struct LocalTransport {
    exec: LocalShellExecutor,
}

impl Default for LocalTransport {
    fn default() -> Self {
        Self {
            exec: LocalShellExecutor::new(),
        }
    }
}

impl LocalTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn exec_shell(&self, op: &ShellOp) -> Result<infrazeug_shell::local::ExecOutput> {
        self.exec
            .execute(op)
            .await
            .map_err(|e| crate::error::TransportError::Other(e.to_string()))
    }

    pub async fn read_file(&self, path: &Path) -> Result<Bytes> {
        let out = self
            .exec
            .execute(&ShellOp::ReadFile {
                path: path.to_path_buf(),
            })
            .await
            .map_err(|e| crate::error::TransportError::Other(e.to_string()))?;
        Ok(Bytes::from(out.stdout))
    }

    pub async fn write_file(&self, path: &Path, data: Bytes, mode: u32) -> Result<()> {
        self.exec
            .execute(&ShellOp::WriteFile {
                path: path.to_path_buf(),
                content: FileSource::bytes(data.to_vec()),
                mode,
            })
            .await
            .map_err(|e| crate::error::TransportError::Other(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn read_file(&self, path: &Path) -> Result<Bytes>;
    async fn write_file(&self, path: &Path, data: Bytes, mode: u32) -> Result<()>;
}

#[async_trait]
impl Transport for LocalTransport {
    async fn read_file(&self, path: &Path) -> Result<Bytes> {
        LocalTransport::read_file(self, path).await
    }

    async fn write_file(&self, path: &Path, data: Bytes, mode: u32) -> Result<()> {
        LocalTransport::write_file(self, path, data, mode).await
    }
}

#[allow(dead_code)]
pub struct Command {
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
}
