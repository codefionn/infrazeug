//! SSH helpers for QEMU lab guests (shared by stack integration tests).

use infrazeug_core::machine::SshConfig;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

fn ssh_base(ssh: &SshConfig, identity: Option<&Path>) -> Result<Command, String> {
    let (host, port) = parse_host_port(&ssh.host);
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o",
        "StrictHostKeyChecking=no",
        "-o",
        "UserKnownHostsFile=/dev/null",
        "-o",
        "LogLevel=ERROR",
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=30",
    ]);
    if let Some(key) = identity {
        cmd.args(["-i", key.to_str().ok_or("identity path")?]);
    }
    if let Some(p) = port {
        cmd.args(["-p", &p.to_string()]);
    }
    let dest = if let Some(user) = &ssh.user {
        format!("{user}@{host}")
    } else {
        host
    };
    cmd.arg(dest);
    Ok(cmd)
}

/// Single-quote `arg` so the remote login shell receives it as one literal token.
///
/// `ssh host a b c` concatenates the trailing args with spaces and re-parses the
/// result in the guest's login shell, so an unquoted multi-word arg (e.g. the
/// script in `sh -c <script>`) is split apart on the far side. Quoting each token
/// and joining them ourselves makes the remote command match `argv` exactly.
fn shell_quote(arg: &str) -> String {
    const SAFE: fn(u8) -> bool = |b| {
        matches!(b,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9'
            | b'-' | b'_' | b'/' | b'.' | b':' | b'@' | b'=' | b',' | b'+' | b'%')
    };
    if !arg.is_empty() && arg.bytes().all(SAFE) {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn remote_command(argv: &[&str]) -> String {
    argv.iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

pub async fn ssh_run(
    ssh: &SshConfig,
    identity: Option<&Path>,
    argv: &[&str],
) -> Result<i32, String> {
    let mut cmd = ssh_base(ssh, identity)?;
    cmd.arg(remote_command(argv));
    cmd.stdin(Stdio::null());
    let out = cmd.output().await.map_err(|e| e.to_string())?;
    Ok(out.status.code().unwrap_or(-1))
}

/// Run a remote shell command and return `(exit_code, combined_stdout_stderr)`.
pub async fn ssh_capture(
    ssh: &SshConfig,
    identity: Option<&Path>,
    script: &str,
) -> Result<(i32, String), String> {
    let mut cmd = ssh_base(ssh, identity)?;
    cmd.arg(remote_command(&["sh", "-c", script]));
    cmd.stdin(Stdio::null());
    let out = cmd.output().await.map_err(|e| e.to_string())?;
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    Ok((out.status.code().unwrap_or(-1), combined))
}

/// Upload a file via `ssh` + `cat` on stdin (avoids ARG_MAX limits on huge heredocs).
pub async fn ssh_upload(
    ssh: &SshConfig,
    identity: Option<&Path>,
    remote_path: &str,
    content: &[u8],
    mode: &str,
) -> Result<(), String> {
    let mut cmd = ssh_base(ssh, identity)?;
    cmd.arg(remote_command(&[
        "sh",
        "-c",
        &format!("cat > {remote_path} && chmod {mode} {remote_path}"),
    ]));
    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("ssh stdin")?;
    stdin.write_all(content).await.map_err(|e| e.to_string())?;
    drop(stdin);
    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ssh upload to {remote_path} failed"))
    }
}

pub async fn wait_cloud_init(
    ssh: &SshConfig,
    identity: Option<&Path>,
    timeout_secs: u64,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    while tokio::time::Instant::now() < deadline {
        if ssh_run(ssh, identity, &["true"]).await != Ok(0) {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            continue;
        }
        let (_, status) = ssh_capture(ssh, identity, "cloud-init status 2>&1 || true")
            .await
            .unwrap_or((-1, String::new()));
        if status.contains("status: done") {
            return Ok(());
        }
        if status.contains("status: error") {
            return Err(cloud_init_diagnostics(ssh, identity).await);
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
    Err(cloud_init_diagnostics(ssh, identity).await)
}

async fn cloud_init_diagnostics(ssh: &SshConfig, identity: Option<&Path>) -> String {
    let (_, detail) = ssh_capture(
        ssh,
        identity,
        "echo '=== cloud-init status ==='; cloud-init status --long 2>&1; \
         echo '=== cloud-init.log ==='; tail -50 /var/log/cloud-init.log 2>/dev/null || true",
    )
    .await
    .unwrap_or((-1, String::new()));
    format!("cloud-init did not reach done state:\n{detail}")
}

fn parse_host_port(host: &str) -> (String, Option<u16>) {
    if let Some((h, p)) = host.rsplit_once(':') {
        if let Ok(port) = p.parse::<u16>() {
            return (h.to_string(), Some(port));
        }
    }
    (host.to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_command_keeps_sh_c_script_as_one_token() {
        // Regression: ssh re-parses the joined command in the guest's login shell,
        // so the script must stay a single quoted arg or `cloud-init` runs bare.
        let got = remote_command(&["sh", "-c", "cloud-init status 2>&1 || true"]);
        assert_eq!(got, "sh -c 'cloud-init status 2>&1 || true'");
    }

    #[test]
    fn remote_command_escapes_embedded_single_quotes() {
        // jsonpath args in verify() contain single quotes; each must become '\''.
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
    }

    #[test]
    fn remote_command_leaves_safe_tokens_bare() {
        assert_eq!(
            remote_command(&["grep", "-qi", "debian", "/etc/os-release"]),
            "grep -qi debian /etc/os-release"
        );
        assert_eq!(remote_command(&["true"]), "true");
    }
}
