use crate::error::{Result, ShellError};
use crate::op::{EnvVarSource, ShellOp};
use crate::source::FileSource;
use std::path::Path;
use std::pin::Pin;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct OutputChunk {
    pub stream: OutputStream,
    pub data: Vec<u8>,
}

pub struct LocalShellExecutor;

impl Default for LocalShellExecutor {
    fn default() -> Self {
        Self
    }
}

impl LocalShellExecutor {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(&self, op: &ShellOp) -> Result<ExecOutput> {
        self.execute_streaming(op, None).await
    }

    pub async fn execute_streaming(
        &self,
        op: &ShellOp,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        self.execute_streaming_inner(op, output.as_ref()).await
    }

    fn execute_streaming_inner<'a>(
        &'a self,
        op: &'a ShellOp,
        output: Option<&'a mpsc::UnboundedSender<OutputChunk>>,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<ExecOutput>> + Send + 'a>> {
        Box::pin(async move {
            match op {
                ShellOp::Run { argv, cwd, env } => {
                    self.run(argv, cwd.as_deref(), env, output).await
                }
                ShellOp::Seq { steps } => self.execute_seq(steps, output).await,
                ShellOp::ReadFile { path } => {
                    let data = fs::read(path).await?;
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: data,
                        stderr: Vec::new(),
                    })
                }
                ShellOp::WriteFile {
                    path,
                    content,
                    mode,
                } => {
                    let content = crate::resolve::resolve_literal_file_source(content)?;
                    let crate::source::FileSource::Bytes(content) = content else {
                        return Err(ShellError::Other(
                            "WriteFile capture/vault refs must be resolved before local execution"
                                .into(),
                        ));
                    };
                    if let Some(parent) = path.parent() {
                        if !parent.as_os_str().is_empty() {
                            fs::create_dir_all(parent).await?;
                        }
                    }
                    fs::write(path, &content).await?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = fs::metadata(path).await?.permissions();
                        perms.set_mode(*mode);
                        fs::set_permissions(path, perms).await?;
                    }
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })
                }
                ShellOp::VaultWrite { .. } => Err(ShellError::Other(
                    "VaultWrite must be handled by the controller before local execution".into(),
                )),
                ShellOp::VaultEnsurePasswordHash { .. } => Err(ShellError::Other(
                    "VaultEnsurePasswordHash must be handled by the controller before local execution"
                        .into(),
                )),
                ShellOp::EnsureDir { path, mode } => {
                    fs::create_dir_all(path).await?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mut perms = fs::metadata(path).await?.permissions();
                        perms.set_mode(*mode);
                        fs::set_permissions(path, perms).await?;
                    }
                    let _ = mode;
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })
                }
                ShellOp::SyncDir { src, dest, options } => {
                    let count = crate::sync_dir::sync_dir_to_local(src, dest, options)?;
                    Ok(ExecOutput {
                        exit_code: 0,
                        stdout: format!("synced {count} entries\n").into_bytes(),
                        stderr: Vec::new(),
                    })
                }
                ShellOp::Poll {
                    check_argv,
                    every,
                    timeout: poll_timeout,
                } => self.poll(check_argv, *every, *poll_timeout, output).await,
            }
        })
    }

    async fn execute_seq(
        &self,
        steps: &[ShellOp],
        output: Option<&mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        let mut last = ExecOutput {
            exit_code: 0,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        for step in steps {
            last = self.execute_streaming_inner(step, output).await?;
            if last.exit_code != 0 {
                return Ok(last);
            }
        }
        Ok(last)
    }

    async fn run(
        &self,
        argv: &[String],
        cwd: Option<&Path>,
        env: &[EnvVarSource],
        output: Option<&mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        if argv.is_empty() {
            return Err(ShellError::Other("empty argv".into()));
        }
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        for entry in env {
            let value = env_source_to_string(entry)?;
            cmd.env(&entry.name, value);
        }
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        cmd.kill_on_drop(true);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel();
        let mut stdout_task = stdout.map(|stream| {
            tokio::spawn(read_stream(stream, OutputStream::Stdout, chunk_tx.clone()))
        });
        let mut stderr_task =
            stderr.map(|stream| tokio::spawn(read_stream(stream, OutputStream::Stderr, chunk_tx)));
        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        loop {
            tokio::select! {
                maybe = chunk_rx.recv() => {
                    match maybe {
                        Some(chunk) => {
                            match chunk.stream {
                                OutputStream::Stdout => stdout_buf.extend_from_slice(&chunk.data),
                                OutputStream::Stderr => stderr_buf.extend_from_slice(&chunk.data),
                            }
                            if let Some(tx) = output {
                                let _ = tx.send(chunk);
                            }
                        }
                        None => break,
                    }
                }
                status = child.wait() => {
                    let status = status?;
                    join_reader(stdout_task.take()).await?;
                    join_reader(stderr_task.take()).await?;
                    while let Ok(chunk) = chunk_rx.try_recv() {
                        match chunk.stream {
                            OutputStream::Stdout => stdout_buf.extend_from_slice(&chunk.data),
                            OutputStream::Stderr => stderr_buf.extend_from_slice(&chunk.data),
                        }
                        if let Some(tx) = output {
                            let _ = tx.send(chunk);
                        }
                    }
                    return Ok(ExecOutput {
                        exit_code: status.code().unwrap_or(-1),
                        stdout: stdout_buf,
                        stderr: stderr_buf,
                    });
                }
            }
        }
        let status = child.wait().await?;
        join_reader(stdout_task.take()).await?;
        join_reader(stderr_task.take()).await?;
        Ok(ExecOutput {
            exit_code: status.code().unwrap_or(-1),
            stdout: stdout_buf,
            stderr: stderr_buf,
        })
    }

    async fn poll(
        &self,
        check_argv: &[String],
        every: Duration,
        poll_timeout: Duration,
        output: Option<&mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        let deadline = tokio::time::Instant::now() + poll_timeout;
        loop {
            let result = self.run(check_argv, None, &[], output).await?;
            if result.exit_code == 0 {
                return Ok(ExecOutput {
                    exit_code: 0,
                    stdout: result.stdout,
                    stderr: result.stderr,
                });
            }
            if tokio::time::Instant::now() + every > deadline {
                let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                return Ok(ExecOutput {
                    exit_code: result.exit_code,
                    stdout: result.stdout,
                    stderr: format!("poll timed out after {:?}: {stderr}", poll_timeout)
                        .into_bytes(),
                });
            }
            tokio::time::sleep(every).await;
        }
    }
}

fn env_source_to_string(entry: &EnvVarSource) -> Result<String> {
    validate_env_name(&entry.name)?;
    let content = crate::resolve::resolve_literal_file_source(&entry.value)?;
    let FileSource::Bytes(bytes) = content else {
        return Err(ShellError::Other(
            "Run env capture/vault refs must be resolved before local execution".into(),
        ));
    };
    let value = String::from_utf8(bytes)
        .map_err(|e| ShellError::Other(format!("env `{}` value is not utf-8: {e}", entry.name)))?;
    if value.contains('\0') {
        return Err(ShellError::Other(format!(
            "env `{}` value contains NUL byte",
            entry.name
        )));
    }
    Ok(value)
}

fn validate_env_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('=') || name.contains('\0') {
        return Err(ShellError::Other(format!("invalid env name `{name}`")));
    }
    Ok(())
}

/// How long to wait for a stream reader to drain after the direct child has
/// already exited. A well-behaved command's pipes reach EOF immediately; this
/// only fires when a descendant process inherited the pipe and is keeping it
/// open (e.g. a pacman/apt hook that spawns or restarts a daemon). In that case
/// the direct child is already gone, so we abort the reader rather than block
/// the scheduler forever (which would hold global locks and freeze the run).
const READER_DRAIN_GRACE: Duration = Duration::from_secs(2);

async fn join_reader(task: Option<tokio::task::JoinHandle<Result<()>>>) -> Result<()> {
    let Some(task) = task else { return Ok(()) };
    let abort = task.abort_handle();
    match tokio::time::timeout(READER_DRAIN_GRACE, task).await {
        Ok(joined) => joined.map_err(|e| ShellError::Other(e.to_string()))?,
        Err(_) => {
            // Pipe never reached EOF because a lingering descendant holds the
            // write end. The child has exited; detach the reader and move on.
            abort.abort();
            Ok(())
        }
    }
}

async fn read_stream<R>(
    mut stream: R,
    kind: OutputStream,
    tx: mpsc::UnboundedSender<OutputChunk>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        let _ = tx.send(OutputChunk {
            stream: kind,
            data: buf[..n].to_vec(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::argv;

    const STDIN_HELPER_ENV: &str = "INFRAZEUG_SHELL_STDIN_HELPER";

    #[tokio::test]
    async fn run_streams_stdout_and_stderr_chunks() {
        let executor = LocalShellExecutor::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let out = executor
            .execute_streaming(
                &ShellOp::run(argv![
                    "sh",
                    "-c",
                    "printf stdout-line; printf stderr-line >&2"
                ]),
                Some(tx),
            )
            .await
            .unwrap();

        let mut chunks = Vec::new();
        while let Ok(chunk) = rx.try_recv() {
            chunks.push(chunk);
        }

        assert_eq!(out.exit_code, 0);
        assert_eq!(out.stdout, b"stdout-line");
        assert_eq!(out.stderr, b"stderr-line");
        assert!(chunks
            .iter()
            .any(|chunk| { chunk.stream == OutputStream::Stdout && chunk.data == b"stdout-line" }));
        assert!(chunks
            .iter()
            .any(|chunk| { chunk.stream == OutputStream::Stderr && chunk.data == b"stderr-line" }));
    }

    #[tokio::test]
    async fn run_returns_when_descendant_keeps_pipe_open() {
        // A command that exits promptly but leaves a detached descendant holding
        // the inherited stdout pipe must not wedge run() forever. Previously the
        // post-exit `task.await` blocked until the descendant's EOF, freezing the
        // scheduler while it held global locks.
        let executor = LocalShellExecutor::new();
        let out = tokio::time::timeout(
            Duration::from_secs(10),
            executor.execute(&ShellOp::run(argv![
                "sh",
                "-c",
                "printf done; setsid sh -c 'sleep 30' & exit 0"
            ])),
        )
        .await
        .expect("run hung on a lingering descendant pipe")
        .unwrap();

        assert_eq!(out.exit_code, 0);
        assert_eq!(out.stdout, b"done");
    }

    #[tokio::test]
    async fn run_does_not_inherit_process_stdin() {
        if std::env::var_os(STDIN_HELPER_ENV).is_some() {
            return;
        }

        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("local::tests::stdin_helper_runs_shell_stdin_probe")
            .arg("--nocapture")
            .env(STDIN_HELPER_ENV, "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let _stdin_guard = child.stdin.take().unwrap();

        match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
            Ok(Ok(status)) => assert!(status.success(), "helper test failed: {status}"),
            Ok(Err(err)) => panic!("helper test wait failed: {err}"),
            Err(_) => {
                let _ = child.kill().await;
                panic!("shell command inherited process stdin and blocked");
            }
        }
    }

    #[tokio::test]
    async fn stdin_helper_runs_shell_stdin_probe() {
        if std::env::var_os(STDIN_HELPER_ENV).is_none() {
            return;
        }

        let executor = LocalShellExecutor::new();
        let out = executor
            .execute(&ShellOp::run(argv![
                "sh",
                "-c",
                "if IFS= read -r _; then printf inherited; else printf eof; fi"
            ]))
            .await
            .unwrap();

        assert_eq!(out.exit_code, 0);
        assert_eq!(out.stdout, b"eof");
    }
}
