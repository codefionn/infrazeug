//! Controller-side implementation of [`SshAuthResolver`].
//!
//! Resolves a machine's interactive SSH secret (login password or key
//! passphrase) by prompting the operator through the run's [`Interactor`] or
//! reading the controller vault through a [`SecretSource`], then writes it to a
//! `0600` askpass file under the run dir. The transport factory calls this on
//! demand, so statically-declared, lazy, and dynamically-discovered machines all
//! authenticate the same way.

use async_trait::async_trait;
use infrazeug_core::id::MachineId;
use infrazeug_core::interactor::{Interaction, InteractionResp, Interactor};
use infrazeug_core::machine::{SshConfig, SshSecret};
use infrazeug_core::ssh_askpass::write_secret_file;
use infrazeug_native::SecretSource;
use infrazeug_transport::SshAuthResolver;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Resolves interactive SSH secrets into askpass files, caching per machine and
/// serializing resolution so no two prompts are ever outstanding at once.
pub(crate) struct ApiSshAuthResolver {
    interact: Option<Arc<dyn Interactor>>,
    secrets: Option<Arc<dyn SecretSource>>,
    dir: PathBuf,
    /// Cache of resolved askpass files; the lock also serializes prompts.
    resolved: Mutex<HashMap<MachineId, PathBuf>>,
}

impl ApiSshAuthResolver {
    pub fn new(
        interact: Option<Arc<dyn Interactor>>,
        secrets: Option<Arc<dyn SecretSource>>,
        run_dir: &Path,
    ) -> Self {
        Self {
            interact,
            secrets,
            dir: run_dir.join("askpass"),
            resolved: Mutex::new(HashMap::new()),
        }
    }

    async fn fetch_secret(&self, machine: MachineId, ssh: &SshConfig) -> Result<Vec<u8>, String> {
        let Some(secret) = ssh.auth.secret() else {
            return Err(format!("machine {machine} has no SSH secret source"));
        };
        match secret {
            SshSecret::Prompt { hint } => {
                let interact = self.interact.as_ref().ok_or_else(|| {
                    format!(
                        "machine {machine} needs an interactive SSH prompt, but this run has no interactor"
                    )
                })?;
                let resp = interact
                    .ask(Interaction::SshAuthSecret {
                        machine,
                        key_passphrase: ssh.auth.is_key_passphrase(),
                        hint: hint.clone(),
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                match resp {
                    InteractionResp::Passphrase(p) => Ok(p.into_bytes()),
                    _ => Err(format!(
                        "SSH authentication for machine {machine} was cancelled"
                    )),
                }
            }
            SshSecret::Vault { file, field } => {
                let secrets = self.secrets.as_ref().ok_or_else(|| {
                    format!(
                        "machine {machine} reads its SSH secret from the vault, but no vault is available for this run"
                    )
                })?;
                secrets.read_field(file, field).await.map_err(|e| {
                    format!(
                        "reading SSH secret for machine {machine} from vault {file}/{field}: {e}"
                    )
                })
            }
        }
    }
}

#[async_trait]
impl SshAuthResolver for ApiSshAuthResolver {
    async fn askpass_file(
        &self,
        machine: MachineId,
        ssh: &SshConfig,
    ) -> Result<Option<PathBuf>, String> {
        if !ssh.auth.is_interactive() {
            return Ok(None);
        }
        // Holding the lock across the prompt serializes resolution: at most one
        // prompt is outstanding, and the cache check + write are atomic.
        let mut resolved = self.resolved.lock().await;
        if let Some(path) = resolved.get(&machine) {
            return Ok(Some(path.clone()));
        }
        let bytes = self.fetch_secret(machine, ssh).await?;
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| e.to_string())?;
        let path = self.dir.join(machine.0.to_string());
        write_secret_file(&path, &bytes).map_err(|e| e.to_string())?;
        resolved.insert(machine, path.clone());
        Ok(Some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_core::error::Result as CoreResult;
    use infrazeug_core::interactor::Interaction;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn mid(seed: u64) -> MachineId {
        MachineId(infrazeug_core::uuid(&format!(
            "00000000-0000-4000-8000-{seed:012x}"
        )))
    }

    fn run_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("iz-sshauth-{}-{tag}", std::process::id()))
    }

    struct CountingVault {
        value: Vec<u8>,
        reads: AtomicUsize,
    }

    #[async_trait]
    impl SecretSource for CountingVault {
        async fn read_field(&self, _file: &str, _field: &str) -> infrazeug_native::Result<Vec<u8>> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(self.value.clone())
        }
    }

    struct FixedPrompt(String);

    #[async_trait]
    impl Interactor for FixedPrompt {
        async fn ask(&self, req: Interaction) -> CoreResult<InteractionResp> {
            assert!(matches!(req, Interaction::SshAuthSecret { .. }));
            Ok(InteractionResp::Passphrase(self.0.clone()))
        }
    }

    #[tokio::test]
    async fn non_interactive_machine_resolves_to_none() {
        let r = ApiSshAuthResolver::new(None, None, &run_dir("noni"));
        let got = r.askpass_file(mid(1), &SshConfig::new("h")).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn vault_source_writes_0600_and_caches() {
        let dir = run_dir("vault");
        let vault = Arc::new(CountingVault {
            value: b"s3cret".to_vec(),
            reads: AtomicUsize::new(0),
        });
        let r = ApiSshAuthResolver::new(None, Some(vault.clone()), &dir);
        let ssh = SshConfig::new("h").password_from_vault("creds", "ssh_pw");

        let p1 = r.askpass_file(mid(2), &ssh).await.unwrap().unwrap();
        let p2 = r.askpass_file(mid(2), &ssh).await.unwrap().unwrap();
        assert_eq!(p1, p2, "same machine reuses the cached file");
        assert_eq!(
            vault.reads.load(Ordering::SeqCst),
            1,
            "vault read once (cached)"
        );
        assert_eq!(std::fs::read(&p1).unwrap(), b"s3cret");
        assert_eq!(
            std::fs::metadata(&p1).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn prompt_source_uses_interactor_response() {
        let dir = run_dir("prompt");
        let r = ApiSshAuthResolver::new(Some(Arc::new(FixedPrompt("typed-pw".into()))), None, &dir);
        let ssh = SshConfig::new("h").ask_password();
        let p = r.askpass_file(mid(3), &ssh).await.unwrap().unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"typed-pw");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn prompt_without_interactor_errors() {
        let r = ApiSshAuthResolver::new(None, None, &run_dir("noprompt"));
        let ssh = SshConfig::new("h").ask_key_passphrase();
        let err = r.askpass_file(mid(4), &ssh).await.unwrap_err();
        assert!(err.contains("no interactor"), "got: {err}");
    }
}
