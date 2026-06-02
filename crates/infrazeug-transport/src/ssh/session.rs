use crate::error::{Result, TransportError};
use infrazeug_core::machine::{SshAuth, SshConfig};
use infrazeug_shell::local::ExecOutput;
use infrazeug_shell::lower::{argv_to_remote_command, shell_escape};
use infrazeug_shell::{pack_sync_plan, plan_sync_dir, SyncDirEntry, SyncDirOptions};
use infrazeug_shell::{OutputChunk, OutputStream};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing::info;

const MIN_OPENSSH: &str = "8.0";

/// Linux `AF_UNIX` paths are limited to 108 bytes; OpenSSH expands `ControlPath` `%C` to ~60 chars.
const MUX_EXPANDED_LEN: usize = 64;
const MUX_PATH_LIMIT: usize = 107;

pub struct SshSession {
    pub ssh_config: SshConfig,
    pub mux_socket: PathBuf,
    pub run_dir: PathBuf,
    /// `0600` file holding the SSH secret for interactive auth, fed to OpenSSH
    /// via `SSH_ASKPASS`. `None` for non-interactive (`BatchMode`) connections.
    pub askpass_file: Option<PathBuf>,
}

impl SshSession {
    pub fn new(ssh: SshConfig, run_dir: impl AsRef<Path>) -> Self {
        let run_dir = run_dir.as_ref().to_path_buf();
        let _ = ensure_ssh_mux_dir(&run_dir);
        let mux_socket = ssh_mux_control_path(&run_dir);
        if let Some(msg) = mux_path_too_long(&mux_socket) {
            tracing::warn!("{msg}");
        }
        Self {
            ssh_config: ssh,
            mux_socket,
            run_dir,
            askpass_file: None,
        }
    }

    /// Attach the resolved askpass secret file for interactive auth.
    pub fn with_askpass_file(mut self, askpass_file: Option<PathBuf>) -> Self {
        self.askpass_file = askpass_file;
        self
    }

    /// Set `SSH_ASKPASS`/`DISPLAY` and point OpenSSH at the secret file so it can
    /// authenticate without a TTY. No-op for non-interactive sessions. Also sets
    /// stdin to `/dev/null`; callers that pipe stdin (sftp) re-set it afterwards.
    fn apply_askpass_env(&self, cmd: &mut Command) {
        set_askpass_env(cmd, self.askpass_file.as_deref());
    }

    pub async fn check_openssh() -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            return Err(TransportError::Other(
                "infrazeug SSH controller requires Linux (v1)".into(),
            ));
        }
        let output = Command::new("ssh")
            .arg("-V")
            .output()
            .await
            .map_err(|e| TransportError::Other(format!("ssh not found: {e}")))?;
        let ver = String::from_utf8_lossy(&output.stderr);
        let major = parse_openssh_major(&ver).unwrap_or(0);
        if major < 8 {
            return Err(TransportError::Other(format!(
                "OpenSSH {MIN_OPENSSH}+ required, got: {}",
                ver.trim()
            )));
        }
        Ok(())
    }

    pub fn base_ssh_args(&self) -> Vec<String> {
        let (_, port) = ssh_destination_and_port(&self.ssh_config);
        let mut args = Vec::new();
        push_auth_mode_args(&mut args, &self.ssh_config.auth);
        args.extend([
            "-o".into(),
            "StrictHostKeyChecking=accept-new".into(),
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            format!("ControlPath={}", self.mux_socket.display()),
            "-o".into(),
            "ControlPersist=300".into(),
            "-o".into(),
            "LogLevel=ERROR".into(),
        ]);
        for opt in &self.ssh_config.extra_opts {
            args.push("-o".into());
            args.push(opt.clone());
        }
        // Enforced after extras so a stray playbook AddressFamily cannot relax it.
        if let Some(opt) = self.ssh_config.address_family_opt() {
            args.push("-o".into());
            args.push(opt);
        }
        if let Some(port) = port {
            args.push("-o".into());
            args.push(format!("Port={port}"));
        }
        // Non-interactive transport (overrides playbook extras like RequestTTY=force).
        for opt in [
            "RequestTTY=no",
            "ConnectTimeout=30",
            "ServerAliveInterval=15",
            "ServerAliveCountMax=3",
        ] {
            args.push("-o".into());
            args.push(opt.into());
        }
        args
    }

    pub fn destination(&self) -> String {
        ssh_destination_and_port(&self.ssh_config).0
    }

    pub async fn exec_remote(&self, remote_argv: &[String]) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        self.exec_remote_streaming(remote_argv, None).await
    }

    pub async fn exec_remote_streaming(
        &self,
        remote_argv: &[String],
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<(i32, Vec<u8>, Vec<u8>)> {
        let dest = self.destination();
        let mut cmd = Command::new("ssh");
        for a in self.base_ssh_args() {
            cmd.arg(a);
        }
        cmd.arg(&dest);
        cmd.arg("--");
        // OpenSSH concatenates multiple remote argv with spaces; `sh -c` must receive one script arg.
        let remote_cmd = if remote_argv.len() == 1 {
            remote_argv[0].clone()
        } else if remote_argv.len() == 3 && remote_argv[0] == "sh" && remote_argv[1] == "-c" {
            remote_argv[2].clone()
        } else {
            argv_to_remote_command(remote_argv)
        };
        cmd.arg(remote_cmd);
        cmd.kill_on_drop(true);
        self.apply_askpass_env(&mut cmd);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| TransportError::Other(e.to_string()))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (chunk_tx, mut chunk_rx) = mpsc::unbounded_channel();
        let mut stdout_task = stdout.map(|stream| {
            tokio::spawn(read_child_stream(
                stream,
                OutputStream::Stdout,
                chunk_tx.clone(),
            ))
        });
        let mut stderr_task = stderr
            .map(|stream| tokio::spawn(read_child_stream(stream, OutputStream::Stderr, chunk_tx)));
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
                            if let Some(tx) = output.as_ref() {
                                let _ = tx.send(chunk);
                            }
                        }
                        None => break,
                    }
                }
                status = child.wait() => {
                    let status = status.map_err(|e| TransportError::Other(e.to_string()))?;
                    if let Some(task) = stdout_task.take() {
                        task.await
                            .map_err(|e| TransportError::Other(e.to_string()))?
                            .map_err(|e| TransportError::Other(e.to_string()))?;
                    }
                    if let Some(task) = stderr_task.take() {
                        task.await
                            .map_err(|e| TransportError::Other(e.to_string()))?
                            .map_err(|e| TransportError::Other(e.to_string()))?;
                    }
                    while let Ok(chunk) = chunk_rx.try_recv() {
                        match chunk.stream {
                            OutputStream::Stdout => stdout_buf.extend_from_slice(&chunk.data),
                            OutputStream::Stderr => stderr_buf.extend_from_slice(&chunk.data),
                        }
                        if let Some(tx) = output.as_ref() {
                            let _ = tx.send(chunk);
                        }
                    }
                    return Ok((status.code().unwrap_or(-1), stdout_buf, stderr_buf));
                }
            }
        }

        let status = child
            .wait()
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        if let Some(task) = stdout_task.take() {
            task.await
                .map_err(|e| TransportError::Other(e.to_string()))?
                .map_err(|e| TransportError::Other(e.to_string()))?;
        }
        if let Some(task) = stderr_task.take() {
            task.await
                .map_err(|e| TransportError::Other(e.to_string()))?
                .map_err(|e| TransportError::Other(e.to_string()))?;
        }
        Ok((status.code().unwrap_or(-1), stdout_buf, stderr_buf))
    }

    pub async fn sftp_batch(&self, commands: &str) -> Result<()> {
        let dest = self.destination();
        let mut cmd = Command::new("sftp");
        for a in self.base_ssh_args() {
            cmd.arg(a);
        }
        cmd.arg("-b").arg("-").arg(&dest);
        self.apply_askpass_env(&mut cmd);
        // sftp reads its batch from stdin, so re-pipe it (over askpass's null).
        cmd.stdin(std::process::Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| TransportError::Other(e.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(commands.as_bytes())
                .await
                .map_err(|e| TransportError::Other(e.to_string()))?;
        }
        let status = child
            .wait()
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        if !status.success() {
            return Err(TransportError::Other(format!(
                "sftp failed (exit {:?})",
                status.code()
            )));
        }
        Ok(())
    }

    /// Remote login home (`$HOME`), for expanding `~/…` paths consistently over SSH and SFTP.
    pub async fn remote_home(&self) -> Result<String> {
        let (code, stdout, stderr) = self
            .exec_remote(&["printf '%s' \"$HOME\"".to_string()])
            .await?;
        if code != 0 {
            return Err(TransportError::Other(format!(
                "remote $HOME failed (exit {code}): {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        let home = String::from_utf8_lossy(&stdout).trim().to_string();
        if home.is_empty() {
            return Err(TransportError::Other("remote $HOME is empty".into()));
        }
        Ok(home)
    }

    /// Expand `~/path` (and `~`) using the remote `$HOME`; other paths are unchanged.
    pub async fn expand_remote_path(&self, path: &str) -> Result<String> {
        if let Some(rest) = path.strip_prefix("~/") {
            let home = self.remote_home().await?;
            Ok(format!("{home}/{rest}"))
        } else if path == "~" {
            self.remote_home().await
        } else {
            Ok(path.to_string())
        }
    }

    /// Push bytes with `scp` (one file, works with mux; no TTY required).
    pub async fn upload_bytes(&self, remote_path: &str, data: &[u8], mode: u32) -> Result<()> {
        let remote_abs = self.expand_remote_path(remote_path).await?;
        let dest = self.destination();
        info!(
            dest = %dest,
            path = %remote_abs,
            bytes = data.len(),
            "uploading file via scp"
        );

        if let Some(parent) = Path::new(&remote_abs).parent() {
            let parent = parent.to_string_lossy();
            if !parent.is_empty() {
                let (code, _, stderr) = self
                    .exec_remote(&[format!("mkdir -p {}", shell_escape(&parent))])
                    .await?;
                if code != 0 {
                    return Err(TransportError::Other(format!(
                        "remote mkdir failed (exit {code}): {}",
                        String::from_utf8_lossy(&stderr)
                    )));
                }
            }
        }

        let tmp = format!(
            "{}/.infrazeug-upload-{}",
            self.run_dir.display(),
            uuid::Uuid::new_v4()
        );
        tokio::fs::write(&tmp, data)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;

        // Stage beside the final path, then rename into place. Direct scp onto an
        // in-use executable fails with "dest open … Failure" (ETXTBSY) when a prior
        // agent is still serving from that path.
        let staging = format!("{}.infrazeug-staging-{}", remote_abs, uuid::Uuid::new_v4());
        let scp_target = format!("{dest}:{staging}");
        let upload = async {
            let mut cmd = Command::new("scp");
            for a in self.base_ssh_args() {
                cmd.arg(a);
            }
            cmd.arg("-q").arg(&tmp).arg(&scp_target);
            self.apply_askpass_env(&mut cmd);
            cmd.output()
                .await
                .map_err(|e| TransportError::Other(format!("scp: {e}")))
        };

        let output = timeout(Duration::from_secs(300), upload)
            .await
            .map_err(|_| {
                TransportError::Other(format!("scp upload to {remote_abs} timed out"))
            })??;

        let _ = tokio::fs::remove_file(&tmp).await;

        if !output.status.success() {
            return Err(TransportError::Other(format!(
                "scp to {remote_abs} failed (exit {:?}): {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let (code, _, stderr) = self
            .exec_remote(&[format!(
                "chmod {mode:o} {staging} && mv -f {staging} {final_path}",
                staging = shell_escape(&staging),
                final_path = shell_escape(&remote_abs)
            )])
            .await?;
        if code != 0 {
            let _ = self
                .exec_remote(&[format!("rm -f {}", shell_escape(&staging))])
                .await;
            return Err(TransportError::Other(format!(
                "remote chmod+mv into place failed (exit {code}): {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        Ok(())
    }

    /// Sync a controller-local directory to a remote directory over the existing
    /// SSH session primitives. This does not invoke the external `rsync` tool.
    ///
    /// Fast path: pack the plan into one tar archive, upload it with a single
    /// scp, and extract it with a single remote `tar -xpf`. Hosts without a
    /// remote `tar` fall back to one scp/exec per entry.
    pub async fn sync_dir(
        &self,
        local_src: &Path,
        remote_dest: &Path,
        options: &SyncDirOptions,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        let plan =
            plan_sync_dir(local_src, options).map_err(|e| TransportError::Other(e.to_string()))?;
        let remote_abs = self
            .expand_remote_path(&remote_dest.display().to_string())
            .await?;

        if options.delete {
            let out = self
                .exec_remote_streaming(
                    &[format!("rm -rf {}", shell_escape(&remote_abs))],
                    output.clone(),
                )
                .await?;
            if out.0 != 0 {
                return Ok(exec_output(out));
            }
        }
        let root_out = self
            .exec_remote_streaming(
                &[format!("mkdir -p {}", shell_escape(&remote_abs))],
                output.clone(),
            )
            .await?;
        if root_out.0 != 0 {
            return Ok(exec_output(root_out));
        }

        let (tar_probe, _, _) = self
            .exec_remote(&["command -v tar >/dev/null 2>&1".to_string()])
            .await?;
        if tar_probe == 0 {
            let archive = pack_sync_plan(local_src, &plan)
                .map_err(|e| TransportError::Other(e.to_string()))?;
            let archive_path = format!("{remote_abs}/.infrazeug-sync-{}.tar", uuid::Uuid::new_v4());
            self.upload_bytes(&archive_path, &archive, 0o600).await?;
            let out = self
                .exec_remote_streaming(
                    &[format!(
                        "tar -xpf {archive} -C {dest}; rc=$?; rm -f {archive}; exit $rc",
                        archive = shell_escape(&archive_path),
                        dest = shell_escape(&remote_abs)
                    )],
                    output,
                )
                .await?;
            if out.0 != 0 {
                return Ok(exec_output(out));
            }
        } else {
            let out = self
                .sync_dir_entries(local_src, &remote_abs, &plan, output)
                .await?;
            if out.exit_code != 0 {
                return Ok(out);
            }
        }
        Ok(ExecOutput {
            exit_code: 0,
            stdout: format!("synced {} entries\n", plan.entries.len()).into_bytes(),
            stderr: Vec::new(),
        })
    }

    /// Per-entry fallback for hosts without a remote `tar`: one scp/exec per
    /// dir, file, and link. Returns the first failing remote output, or a
    /// zero-exit summary when everything applied.
    async fn sync_dir_entries(
        &self,
        local_src: &Path,
        remote_abs: &str,
        plan: &infrazeug_shell::SyncDirPlan,
        output: Option<mpsc::UnboundedSender<OutputChunk>>,
    ) -> Result<ExecOutput> {
        for entry in &plan.entries {
            match entry {
                SyncDirEntry::Dir { rel, mode } => {
                    let path = remote_join(remote_abs, rel);
                    let out = self
                        .exec_remote_streaming(
                            &[format!(
                                "mkdir -p {} && chmod {:o} {}",
                                shell_escape(&path),
                                mode,
                                shell_escape(&path)
                            )],
                            output.clone(),
                        )
                        .await?;
                    if out.0 != 0 {
                        return Ok(exec_output(out));
                    }
                }
                SyncDirEntry::File {
                    rel,
                    mode,
                    hard_link_to,
                } => {
                    let path = remote_join(remote_abs, rel);
                    if let Some(link_to) = hard_link_to {
                        let link_src = remote_join(remote_abs, link_to);
                        let out = self
                            .exec_remote_streaming(
                                &[format!(
                                    "rm -rf {} && ln {} {}",
                                    shell_escape(&path),
                                    shell_escape(&link_src),
                                    shell_escape(&path)
                                )],
                                output.clone(),
                            )
                            .await?;
                        if out.0 != 0 {
                            return Ok(exec_output(out));
                        }
                    } else {
                        let data = tokio::fs::read(local_src.join(rel))
                            .await
                            .map_err(|e| TransportError::Other(e.to_string()))?;
                        self.upload_bytes(&path, &data, *mode).await?;
                    }
                }
                SyncDirEntry::Symlink { rel, target } => {
                    let path = remote_join(remote_abs, rel);
                    let out = self
                        .exec_remote_streaming(
                            &[format!(
                                "rm -rf {} && ln -s {} {}",
                                shell_escape(&path),
                                shell_escape(&target.display().to_string()),
                                shell_escape(&path)
                            )],
                            output.clone(),
                        )
                        .await?;
                    if out.0 != 0 {
                        return Ok(exec_output(out));
                    }
                }
            }
        }
        Ok(ExecOutput {
            exit_code: 0,
            stdout: format!("synced {} entries\n", plan.entries.len()).into_bytes(),
            stderr: Vec::new(),
        })
    }

    pub async fn download_bytes(&self, remote_path: &str) -> Result<Vec<u8>> {
        let tmp = format!(
            "{}/.infrazeug-download-{}",
            self.run_dir.display(),
            uuid::Uuid::new_v4()
        );
        let batch = format!(
            "get {remote_path} {tmp}\n",
            tmp = tmp,
            remote_path = remote_path
        );
        self.sftp_batch(&batch).await?;
        let data = tokio::fs::read(&tmp)
            .await
            .map_err(|e| TransportError::Other(e.to_string()))?;
        let _ = tokio::fs::remove_file(&tmp).await;
        Ok(data)
    }
}

/// Directory holding SSH mux sockets for a run.
pub fn ssh_mux_dir(run_dir: &Path) -> PathBuf {
    run_dir.join("m")
}

/// Create the mux directory (OpenSSH will not create it for `ControlPath`).
pub fn ensure_ssh_mux_dir(run_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(ssh_mux_dir(run_dir))
}

/// `ControlPath` under `run_dir/m/%C` (keeps mux sockets in a subdir; path must stay short).
pub fn ssh_mux_control_path(run_dir: &Path) -> PathBuf {
    run_dir.join("m/%C")
}

fn mux_path_too_long(template: &Path) -> Option<String> {
    let parent = template.parent().unwrap_or(template);
    let len = parent.as_os_str().len();
    if len + 1 + MUX_EXPANDED_LEN > MUX_PATH_LIMIT {
        Some(format!(
            "SSH ControlPath parent {:?} is {} bytes; OpenSSH needs <={} (set INFRZEUG_RUN_ROOT to a shorter path, e.g. /tmp/iz)",
            parent,
            len,
            MUX_PATH_LIMIT.saturating_sub(1 + MUX_EXPANDED_LEN)
        ))
    } else {
        None
    }
}

/// Point OpenSSH at the askpass secret `file` (env + `/dev/null` stdin) so it can
/// authenticate without a TTY. No-op when `file` is `None`.
pub(crate) fn set_askpass_env(cmd: &mut Command, file: Option<&Path>) {
    let Some(file) = file else {
        return;
    };
    if let Ok(exe) = std::env::current_exe() {
        cmd.env("SSH_ASKPASS", exe);
    }
    cmd.env("SSH_ASKPASS_REQUIRE", "force");
    // OpenSSH <8.4 only consults SSH_ASKPASS when DISPLAY is set and there is no
    // controlling tty; set a dummy so the helper is used on those versions too.
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    cmd.env("DISPLAY", display);
    cmd.env(infrazeug_core::ssh_askpass::SECRET_FILE_ENV, file);
    cmd.stdin(std::process::Stdio::null());
}

/// Push the authentication-mode `-o` options for `auth`.
///
/// `NonInteractive` keeps the hardened `BatchMode=yes`. The interactive modes
/// drop `BatchMode` (so an `SSH_ASKPASS` helper can supply the secret), cap
/// retries at one prompt, and steer OpenSSH at the right method.
pub(crate) fn push_auth_mode_args(args: &mut Vec<String>, auth: &SshAuth) {
    let mut add = |s: &str| {
        args.push("-o".into());
        args.push(s.into());
    };
    match auth {
        SshAuth::NonInteractive => add("BatchMode=yes"),
        SshAuth::Password(_) => {
            add("NumberOfPasswordPrompts=1");
            add("PreferredAuthentications=keyboard-interactive,password");
            add("PubkeyAuthentication=no");
        }
        SshAuth::KeyPassphrase(_) => {
            add("NumberOfPasswordPrompts=1");
            add("PreferredAuthentications=publickey");
            add("PubkeyAuthentication=yes");
        }
    }
}

fn parse_openssh_major(ver: &str) -> Option<u32> {
    // `ssh -V` writes to stderr, e.g. `OpenSSH_10.3p1, OpenSSL 3.6.2 …`
    if let Some(token) = ver.split_whitespace().next() {
        let rest = token.strip_prefix("OpenSSH_")?;
        let num = rest.trim_end_matches(',');
        let major = num.split('.').next()?.split('p').next()?;
        return major.parse().ok();
    }
    None
}

pub(crate) fn ssh_destination_and_port(ssh: &SshConfig) -> (String, Option<u16>) {
    let dest = ssh.destination();
    if let Some((host, port)) = dest.rsplit_once(':') {
        if !host.is_empty() {
            if let Ok(port) = port.parse::<u16>() {
                return (host.to_string(), Some(port));
            }
        }
    }
    (dest, None)
}

fn remote_join(root: &str, rel: &Path) -> String {
    let rel = rel.to_string_lossy();
    if rel.is_empty() {
        root.to_string()
    } else if root.ends_with('/') {
        format!("{root}{rel}")
    } else {
        format!("{root}/{rel}")
    }
}

fn exec_output((exit_code, stdout, stderr): (i32, Vec<u8>, Vec<u8>)) -> ExecOutput {
    ExecOutput {
        exit_code,
        stdout,
        stderr,
    }
}

async fn read_child_stream<R>(
    mut stream: R,
    kind: OutputStream,
    tx: mpsc::UnboundedSender<OutputChunk>,
) -> std::io::Result<()>
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

    #[test]
    fn parses_openssh_10() {
        assert_eq!(
            parse_openssh_major("OpenSSH_10.3p1, OpenSSL 3.6.2 7 Apr 2026"),
            Some(10)
        );
    }

    #[test]
    fn parses_openssh_8() {
        assert_eq!(
            parse_openssh_major("OpenSSH_8.9p1 Ubuntu-3ubuntu0.13, OpenSSL 3.0.13"),
            Some(8)
        );
    }

    #[test]
    fn default_run_layout_fits_mux_socket() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        assert!(mux_path_too_long(&ssh_mux_control_path(&run_dir)).is_none());
    }

    #[test]
    fn long_run_root_warns_for_mux() {
        let run_dir = PathBuf::from("/tmp/infra-infrazeug/382ac066-e32c-4777-96b8-cb285523c8ba");
        assert!(mux_path_too_long(&ssh_mux_control_path(&run_dir)).is_some());
    }

    #[test]
    fn ssh_destination_splits_host_port_after_user_prefix() {
        let ssh = SshConfig::new("127.0.0.1:3890").with_user("debian");

        assert_eq!(
            ssh_destination_and_port(&ssh),
            ("debian@127.0.0.1".to_string(), Some(3890))
        );
    }

    #[test]
    fn base_ssh_args_enforces_ipv4_only() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let session = SshSession::new(SshConfig::new("example.com").ipv4_only(), &run_dir);

        assert!(session
            .base_ssh_args()
            .windows(2)
            .any(|pair| pair == ["-o", "AddressFamily=inet"]));
    }

    #[test]
    fn base_ssh_args_enforces_ipv6_only() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let session = SshSession::new(SshConfig::new("example.com").ipv6_only(), &run_dir);

        assert!(session
            .base_ssh_args()
            .windows(2)
            .any(|pair| pair == ["-o", "AddressFamily=inet6"]));
    }

    #[test]
    fn base_ssh_args_omits_address_family_when_unrestricted() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let session = SshSession::new(SshConfig::new("example.com"), &run_dir);

        assert!(!session
            .base_ssh_args()
            .iter()
            .any(|a| a.starts_with("AddressFamily=")));
    }

    #[test]
    fn base_ssh_args_password_auth_drops_batchmode() {
        use infrazeug_core::machine::{SshAuth, SshSecret};
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let ssh = SshConfig::new("example.com")
            .with_auth(SshAuth::Password(SshSecret::Prompt { hint: None }));
        let args = SshSession::new(ssh, &run_dir).base_ssh_args();

        assert!(
            !args.iter().any(|a| a == "BatchMode=yes"),
            "interactive auth must not force BatchMode"
        );
        assert!(args.iter().any(|a| a == "NumberOfPasswordPrompts=1"));
        assert!(args
            .iter()
            .any(|a| a == "PreferredAuthentications=keyboard-interactive,password"));
        assert!(args.iter().any(|a| a == "PubkeyAuthentication=no"));
    }

    #[test]
    fn base_ssh_args_key_passphrase_prefers_publickey() {
        use infrazeug_core::machine::{SshAuth, SshSecret};
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let ssh =
            SshConfig::new("example.com").with_auth(SshAuth::KeyPassphrase(SshSecret::Vault {
                file: "keys".into(),
                field: "passphrase".into(),
            }));
        let args = SshSession::new(ssh, &run_dir).base_ssh_args();

        assert!(!args.iter().any(|a| a == "BatchMode=yes"));
        assert!(args
            .iter()
            .any(|a| a == "PreferredAuthentications=publickey"));
    }

    #[test]
    fn base_ssh_args_default_keeps_batchmode() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let args = SshSession::new(SshConfig::new("example.com"), &run_dir).base_ssh_args();
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        assert!(!args.iter().any(|a| a == "NumberOfPasswordPrompts=1"));
    }

    #[test]
    fn base_ssh_args_adds_port_option_for_host_port() {
        let run_dir = PathBuf::from("/tmp/iz/382ac066e32c");
        let session = SshSession::new(
            SshConfig::new("127.0.0.1:3890").with_user("debian"),
            &run_dir,
        );

        assert_eq!(session.destination(), "debian@127.0.0.1");
        assert!(session
            .base_ssh_args()
            .windows(2)
            .any(|pair| pair == ["-o", "Port=3890"]));
    }
}
