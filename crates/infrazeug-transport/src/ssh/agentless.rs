use super::session::SshSession;
use infrazeug_shell::local::{ExecOutput, OutputChunk};
use infrazeug_shell::lower::{lower, lowered_exec_argv, Lowered};
use infrazeug_shell::{Result as ShellResult, ShellError, ShellOp};
use tokio::sync::mpsc;

pub struct AgentlessBackend {
    session: SshSession,
}

impl AgentlessBackend {
    pub fn new(session: SshSession) -> Self {
        Self { session }
    }

    pub async fn execute(&self, op: &ShellOp) -> ShellResult<ExecOutput> {
        self.execute_streaming(op, None).await
    }

    pub async fn execute_streaming(
        &self,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        if let ShellOp::Seq { steps } = op {
            if steps.iter().any(contains_sync_dir) {
                return self.execute_seq(steps, output).await;
            }
        }
        if let ShellOp::SyncDir { src, dest, options } = op {
            return self
                .session
                .sync_dir(src, dest, options, output)
                .await
                .map_err(|e| ShellError::Other(e.to_string()));
        }
        let lowered = lower(op).map_err(ShellError::Other)?;
        self.execute_lowered(&lowered, output.as_ref()).await
    }

    async fn execute_seq(
        &self,
        steps: &[ShellOp],
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        let mut last = ExecOutput {
            exit_code: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        for step in steps {
            last = Box::pin(self.execute_streaming(step, output.clone())).await?;
            if last.exit_code != 0 {
                return Ok(last);
            }
        }
        Ok(last)
    }

    async fn execute_lowered(
        &self,
        lowered: &Lowered,
        output: Option<&mpsc::UnboundedSender<OutputChunk>>,
    ) -> ShellResult<ExecOutput> {
        match lowered {
            Lowered::Exec { .. } => {
                let argv = lowered_exec_argv(lowered).map_err(ShellError::Other)?;
                let (code, stdout, stderr) = self
                    .session
                    .exec_remote_streaming(&argv, output.cloned())
                    .await
                    .map_err(|e| ShellError::Other(e.to_string()))?;
                Ok(ExecOutput {
                    exit_code: code,
                    stdout,
                    stderr,
                })
            }
            Lowered::Seq { steps } => {
                let mut last = ExecOutput {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                };
                for step in steps {
                    last = Box::pin(self.execute_lowered(step, output)).await?;
                    if last.exit_code != 0 {
                        return Ok(last);
                    }
                }
                Ok(last)
            }
            Lowered::SftpRead { path } => {
                let data = self
                    .session
                    .download_bytes(&path.display().to_string())
                    .await
                    .map_err(|e| ShellError::Other(e.to_string()))?;
                Ok(ExecOutput {
                    exit_code: 0,
                    stdout: data,
                    stderr: Vec::new(),
                })
            }
            Lowered::SftpWrite {
                path,
                content,
                mode,
            } => {
                self.session
                    .upload_bytes(&path.display().to_string(), content, *mode)
                    .await
                    .map_err(|e| ShellError::Other(e.to_string()))?;
                Ok(ExecOutput {
                    exit_code: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
            Lowered::Mkdir { path, mode } => {
                let argv = vec![
                    "mkdir".into(),
                    "-p".into(),
                    "-m".into(),
                    format!("{mode:o}"),
                    path.display().to_string(),
                ];
                let (code, stdout, stderr) = self
                    .session
                    .exec_remote_streaming(&argv, output.cloned())
                    .await
                    .map_err(|e| ShellError::Other(e.to_string()))?;
                Ok(ExecOutput {
                    exit_code: code,
                    stdout,
                    stderr,
                })
            }
        }
    }
}

fn contains_sync_dir(op: &ShellOp) -> bool {
    match op {
        ShellOp::SyncDir { .. } => true,
        ShellOp::Seq { steps } => steps.iter().any(contains_sync_dir),
        _ => false,
    }
}
