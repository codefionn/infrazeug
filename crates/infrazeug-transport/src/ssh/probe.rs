//! Lightweight SSH probes (no persistent session).

use crate::error::{Result, TransportError};
use crate::ssh::session::{push_auth_mode_args, set_askpass_env, ssh_destination_and_port};
use infrazeug_core::SshConfig;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Probe `uname -m` over a one-off SSH connection. `askpass_file` carries the
/// interactive auth secret when `ssh.auth` is interactive (otherwise `None`).
pub async fn probe_uname_machine(ssh: &SshConfig, askpass_file: Option<&Path>) -> Result<String> {
    let (dest, port) = ssh_destination_and_port(ssh);
    let mut cmd = Command::new("ssh");
    let mut auth_args = Vec::new();
    push_auth_mode_args(&mut auth_args, &ssh.auth);
    for a in &auth_args {
        cmd.arg(a);
    }
    // Keep the probe's policy uniform with real sessions: same host-key
    // handling, and enforced options BEFORE extras (OpenSSH takes the first
    // obtained value for duplicate `-o` options). BatchMode/NumberOfPasswordPrompts
    // come from `push_auth_mode_args` so interactive auth still works.
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("ConnectTimeout=10");
    if let Some(opt) = ssh.address_family_opt() {
        cmd.arg("-o").arg(opt);
    }
    if let Some(port) = port {
        cmd.arg("-o").arg(format!("Port={port}"));
    }
    for opt in &ssh.extra_opts {
        cmd.arg("-o").arg(opt);
    }
    cmd.arg(&dest).arg("uname").arg("-m");
    set_askpass_env(&mut cmd, askpass_file);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = cmd
        .output()
        .await
        .map_err(|e| TransportError::Other(e.to_string()))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(TransportError::Other(format!(
            "ssh uname -m failed for {dest}: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
