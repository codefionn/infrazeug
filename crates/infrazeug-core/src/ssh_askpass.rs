//! Controller-side `SSH_ASKPASS` helper plumbing.
//!
//! When a machine's [`SshAuth`](crate::machine::SshAuth) is interactive, the
//! controller resolves the secret once (prompt or vault), writes it to a `0600`
//! file under the run dir, and spawns `ssh`/`scp`/`sftp` with
//! [`SECRET_FILE_ENV`] pointing at that file plus `SSH_ASKPASS` set to this same
//! executable. OpenSSH then re-execs us as its askpass program; [`is_askpass_invocation`]
//! detects that and [`emit_secret`] prints the file's bytes to stdout for OpenSSH
//! to read. The secret never travels through an environment variable — only the
//! file path does.

use crate::error::{CoreError, Result};
use std::io::Write;
use std::path::Path;

/// Env var holding the path to the `0600` file with the SSH secret bytes.
pub const SECRET_FILE_ENV: &str = "INFRZEUG_SSH_ASKPASS_SECRET_FILE";

/// True when the current process was launched by OpenSSH as the askpass helper,
/// i.e. [`SECRET_FILE_ENV`] is set. Check this before normal CLI arg parsing.
pub fn is_askpass_invocation() -> bool {
    std::env::var_os(SECRET_FILE_ENV).is_some()
}

/// Print the secret named by [`SECRET_FILE_ENV`] to stdout for OpenSSH to read.
///
/// Output is the raw file bytes followed by a single newline (OpenSSH reads one
/// line and strips the trailing newline), so the secret file itself must be
/// stored without a trailing newline.
pub fn emit_secret() -> Result<()> {
    let path = std::env::var_os(SECRET_FILE_ENV)
        .ok_or_else(|| CoreError::other("SSH askpass invoked without a secret file"))?;
    let bytes = std::fs::read(&path).map_err(CoreError::from)?;
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(&bytes).map_err(CoreError::from)?;
    lock.write_all(b"\n").map_err(CoreError::from)?;
    lock.flush().map_err(CoreError::from)?;
    Ok(())
}

/// Write a secret to `path` with `0600` permissions and no trailing newline.
pub fn write_secret_file(path: &Path, secret: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(CoreError::from)?;
    f.write_all(secret).map_err(CoreError::from)?;
    f.flush().map_err(CoreError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_secret_file_is_0600_and_exact_bytes() {
        let dir = std::env::temp_dir().join(format!("iz-askpass-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("secret");
        write_secret_file(&path, b"hunter2").unwrap();

        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        // Stored without a trailing newline; the helper adds it on emit.
        assert_eq!(std::fs::read(&path).unwrap(), b"hunter2");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn not_an_askpass_invocation_by_default() {
        // SECRET_FILE_ENV is only set by the controller when spawning ssh.
        if std::env::var_os(SECRET_FILE_ENV).is_none() {
            assert!(!is_askpass_invocation());
        }
    }
}
