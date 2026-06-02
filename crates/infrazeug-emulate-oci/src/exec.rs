//! Execute ShellOps inside a running container via `podman exec` / `docker exec`.

use infrazeug_shell::local::ExecOutput;
use infrazeug_shell::op::EnvVarSource;
use infrazeug_shell::source::FileSource;
use infrazeug_shell::{Result as ShellResult, ShellError, ShellOp};
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Clone)]
pub struct ContainerExec {
    pub runtime: String,
    pub container: String,
}

/// Backward-compatible alias.
pub type PodmanExec = ContainerExec;

impl ContainerExec {
    pub async fn execute(&self, op: &ShellOp) -> ShellResult<ExecOutput> {
        match op {
            ShellOp::Run { argv, cwd: _, env } => {
                self.exec(argv.iter().map(String::as_str), None, env).await
            }
            ShellOp::Seq { steps } => {
                let mut last = ok_empty();
                for step in steps {
                    last = Box::pin(self.execute(step)).await?;
                    if last.exit_code != 0 {
                        return Ok(last);
                    }
                }
                Ok(last)
            }
            ShellOp::ReadFile { path } => {
                // File bytes come back on stdout, mirroring the local executor.
                self.exec(["cat", &path_str(path)], None, &[]).await
            }
            ShellOp::EnsureDir { path, mode } => {
                let p = path_str(path);
                let out = self.exec(["mkdir", "-p", &p], None, &[]).await?;
                if out.exit_code != 0 {
                    return Ok(out);
                }
                self.exec(["chmod", &octal(*mode), &p], None, &[]).await
            }
            ShellOp::WriteFile {
                path,
                content,
                mode,
            } => {
                let FileSource::Bytes(bytes) = content else {
                    return Err(ShellError::Other(
                        "WriteFile capture refs must be resolved before container execution".into(),
                    ));
                };
                let p = path_str(path);
                if let Some(parent) = path.parent() {
                    let parent = parent.to_string_lossy();
                    if !parent.is_empty() {
                        let out = self.exec(["mkdir", "-p", &parent], None, &[]).await?;
                        if out.exit_code != 0 {
                            return Ok(out);
                        }
                    }
                }
                let out = self
                    .exec(["sh", "-c", "cat > \"$1\"", "sh", &p], Some(bytes), &[])
                    .await?;
                if out.exit_code != 0 {
                    return Ok(out);
                }
                self.exec(["chmod", &octal(*mode), &p], None, &[]).await
            }
            ShellOp::Poll {
                check_argv,
                every,
                timeout: poll_timeout,
            } => {
                let deadline = tokio::time::Instant::now() + *poll_timeout;
                loop {
                    let result = self
                        .exec(check_argv.iter().map(String::as_str), None, &[])
                        .await?;
                    if result.exit_code == 0 {
                        return Ok(result);
                    }
                    if tokio::time::Instant::now() + *every > deadline {
                        return Ok(ExecOutput {
                            exit_code: result.exit_code,
                            stdout: result.stdout,
                            stderr: format!("poll timed out after {:?}", poll_timeout).into_bytes(),
                        });
                    }
                    tokio::time::sleep(*every).await;
                }
            }
            ShellOp::VaultWrite { .. } => Err(ShellError::Other(
                "VaultWrite is a controller-side operation and cannot run inside a container"
                    .into(),
            )),
            ShellOp::VaultEnsurePasswordHash { .. } => Err(ShellError::Other(
                "VaultEnsurePasswordHash is a controller-side operation and cannot run inside a container"
                    .into(),
            )),
            ShellOp::SyncDir { .. } => Err(ShellError::Other(
                "SyncDir syncs from the controller filesystem and cannot run inside a container"
                    .into(),
            )),
        }
    }

    /// Run `runtime exec [-i] <container> <argv...>`, optionally feeding `stdin`.
    async fn exec<'a, I>(
        &self,
        argv: I,
        stdin: Option<&[u8]>,
        env: &[EnvVarSource],
    ) -> ShellResult<ExecOutput>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut cmd = Command::new(&self.runtime);
        cmd.arg("exec");
        if stdin.is_some() {
            cmd.arg("-i");
        }
        for entry in env {
            cmd.arg("-e");
            cmd.arg(format!("{}={}", entry.name, env_source_to_string(entry)?));
        }
        cmd.arg(&self.container);
        for a in argv {
            cmd.arg(a);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().map_err(|e| ShellError::Other(e.to_string()))?;
        if let Some(bytes) = stdin {
            let mut sink = child
                .stdin
                .take()
                .ok_or_else(|| ShellError::Other("failed to open container stdin".into()))?;
            sink.write_all(bytes)
                .await
                .map_err(|e| ShellError::Other(e.to_string()))?;
            sink.shutdown()
                .await
                .map_err(|e| ShellError::Other(e.to_string()))?;
            drop(sink);
        }
        let out = child
            .wait_with_output()
            .await
            .map_err(|e| ShellError::Other(e.to_string()))?;
        Ok(ExecOutput {
            exit_code: out.status.code().unwrap_or(-1),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }
}

fn ok_empty() -> ExecOutput {
    ExecOutput {
        exit_code: 0,
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Format a unix mode as the octal string `chmod` expects (`0o640` -> "640").
fn octal(mode: u32) -> String {
    format!("{:o}", mode & 0o7777)
}

fn env_source_to_string(entry: &EnvVarSource) -> ShellResult<String> {
    if entry.name.is_empty() || entry.name.contains('=') || entry.name.contains('\0') {
        return Err(ShellError::Other(format!(
            "invalid env name `{}`",
            entry.name
        )));
    }
    let FileSource::Bytes(bytes) = &entry.value else {
        return Err(ShellError::Other(
            "Run env capture/vault refs must be resolved before container execution".into(),
        ));
    };
    let value = String::from_utf8(bytes.clone())
        .map_err(|e| ShellError::Other(format!("env `{}` value is not utf-8: {e}", entry.name)))?;
    if value.contains('\0') {
        return Err(ShellError::Other(format!(
            "env `{}` value contains NUL byte",
            entry.name
        )));
    }
    Ok(value)
}
