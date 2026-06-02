use crate::id::RunId;
use crate::interactor::{Interaction, InteractionResp, Interactor, ProviderKind};
use crate::{CaptureStore, MachineId, NodeId};
use async_trait::async_trait;
use infrazeug_native::{NativeError, SecretSource};
use infrazeug_secrets::{PassphraseProvider, VaultRef, VaultStore};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    pub run_root: PathBuf,
    /// Vault store directory (keys/, files/). None = no vault.
    pub vault_store: Option<PathBuf>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            run_root: PathBuf::from(
                std::env::var("INFRZEUG_RUN_ROOT").unwrap_or_else(|_| "/tmp/infrazeug/runs".into()),
            ),
            vault_store: std::env::var("INFRZEUG_VAULT_STORE")
                .ok()
                .map(PathBuf::from),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Apply,
    Test,
}

/// Vault session: DataKeys unlocked at run start; file bodies lazy (SOUL §6.3).
pub struct VaultSession {
    store: Option<Arc<Mutex<VaultStore>>>,
    /// Data-key ids to unlock interactively when not already unlocked.
    pub pending_keys: Vec<String>,
    pub unlocked: bool,
}

impl Default for VaultSession {
    fn default() -> Self {
        Self {
            store: None,
            pending_keys: Vec::new(),
            unlocked: true,
        }
    }
}

impl VaultSession {
    pub fn from_store(store: VaultStore, pending_keys: Vec<String>) -> Self {
        let unlocked = pending_keys.is_empty();
        Self {
            store: Some(Arc::new(Mutex::new(store))),
            pending_keys,
            unlocked,
        }
    }

    pub fn store(&self) -> Option<Arc<Mutex<VaultStore>>> {
        self.store.clone()
    }

    /// A [`SecretSource`] over the unlocked store, for `Local` native methods.
    ///
    /// Returns `None` when there is no vault store; data keys must already be
    /// unlocked (see [`unlock_if_needed`](Self::unlock_if_needed)) for reads to
    /// succeed.
    pub fn secret_source(&self) -> Option<Arc<dyn SecretSource>> {
        self.secret_source_with_captures(None)
    }

    /// A [`SecretSource`] over the unlocked store plus optional upstream captures.
    pub fn secret_source_with_captures(
        &self,
        captures: Option<CaptureStore>,
    ) -> Option<Arc<dyn SecretSource>> {
        if self.store.is_none() && captures.is_none() {
            return None;
        }
        Some(Arc::new(VaultSecretSource {
            store: self.store.clone(),
            captures,
        }) as Arc<dyn SecretSource>)
    }

    pub async fn unlock_if_needed(
        &mut self,
        interact: Arc<dyn Interactor>,
    ) -> crate::error::Result<()> {
        if self.unlocked {
            return Ok(());
        }
        let Some(store) = self.store.clone() else {
            self.unlocked = true;
            return Ok(());
        };
        for name in self.pending_keys.clone() {
            let recipient = {
                let store = store.lock().await;
                let envelope = store.load_envelope(&name).await?;
                envelope.recipients.into_iter().next().ok_or_else(|| {
                    crate::error::CoreError::other(format!("data key {name} has no recipients"))
                })?
            };
            if recipient.kind != ProviderKind::Passphrase {
                return Err(crate::error::CoreError::other(format!(
                    "data key {name} default recipient {} is {:?}; runtime unlock currently supports passphrase recipients only",
                    recipient.label, recipient.kind
                )));
            }
            let hint = format!("passphrase recipient label: {}", recipient.label);
            let resp = interact
                .ask(Interaction::UnlockDataKey {
                    name: name.clone(),
                    provider: recipient.kind,
                    hint: Some(hint),
                })
                .await?;
            match resp {
                InteractionResp::Passphrase(pass) => {
                    let provider = PassphraseProvider::new(pass);
                    store
                        .lock()
                        .await
                        .unlock_with_provider(&name, &provider, &recipient.label)
                        .await?;
                }
                InteractionResp::Cancel => {
                    return Err(crate::error::CoreError::InteractionCancelled);
                }
                _ => {
                    return Err(crate::error::CoreError::InteractionDenied(format!(
                        "unlock data key {name}"
                    )));
                }
            }
        }
        self.unlocked = true;
        Ok(())
    }
}

/// [`SecretSource`] backed by an unlocked [`VaultSession`] store.
struct VaultSecretSource {
    store: Option<Arc<Mutex<VaultStore>>>,
    captures: Option<CaptureStore>,
}

#[async_trait]
impl SecretSource for VaultSecretSource {
    fn has_vault(&self) -> bool {
        self.store.is_some()
    }

    fn has_mutable_vault(&self) -> bool {
        self.store.is_some()
    }

    fn has_node_captures(&self) -> bool {
        self.captures.is_some()
    }

    async fn read_field(&self, file: &str, field: &str) -> infrazeug_native::Result<Vec<u8>> {
        let store = self
            .store
            .as_ref()
            .ok_or_else(|| NativeError::other("vault source unavailable on this context"))?;
        let reference = VaultRef::field(file, field);
        let value = store
            .lock()
            .await
            .resolve_field(&reference)
            .await
            .map_err(|e| NativeError::other(e.to_string()))?;
        crate::vault_resolve::vault_value_to_bytes(value)
            .map_err(|e| NativeError::other(e.to_string()))
    }

    async fn read_mutable_field(
        &self,
        file: &str,
        field: &str,
    ) -> infrazeug_native::Result<Vec<u8>> {
        let store = self.store.as_ref().ok_or_else(|| {
            NativeError::other("mutable vault source unavailable on this context")
        })?;
        let value = store
            .lock()
            .await
            .resolve_mutable_field(file, field)
            .await
            .map_err(|e| NativeError::other(e.to_string()))?;
        crate::vault_resolve::vault_value_to_bytes(value)
            .map_err(|e| NativeError::other(e.to_string()))
    }

    async fn read_node_capture(
        &self,
        node: uuid::Uuid,
        machine: uuid::Uuid,
    ) -> infrazeug_native::Result<Vec<u8>> {
        let captures = self
            .captures
            .as_ref()
            .ok_or_else(|| NativeError::other("node capture source unavailable on this context"))?;
        captures
            .get(NodeId(node), MachineId(machine))
            .await
            .map_err(|e| NativeError::other(e.to_string()))
    }
}

pub struct RunGuard {
    pub run_id: RunId,
    pub run_dir: PathBuf,
}

/// Short directory name for `run_root/<name>/` (OpenSSH mux sockets need a compact path).
pub fn run_dir_name(run_id: RunId) -> String {
    run_id.0.as_simple().to_string().chars().take(12).collect()
}

impl RunGuard {
    pub fn new(config: &RuntimeConfig, run_id: RunId) -> std::io::Result<Self> {
        std::fs::create_dir_all(&config.run_root)?;
        let run_dir = config.run_root.join(run_dir_name(run_id));
        std::fs::create_dir_all(&run_dir)?;
        // OpenSSH ControlPath (`…/m/%C`) does not create parent directories.
        std::fs::create_dir_all(run_dir.join("m"))?;
        Ok(Self { run_id, run_dir })
    }

    pub fn path(&self) -> &Path {
        &self.run_dir
    }

    pub fn control_socket(&self) -> PathBuf {
        self.run_dir.join("control.sock")
    }

    pub async fn install_signals(cancel: CancellationToken) {
        let token = cancel.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
                let mut int = signal(SignalKind::interrupt()).expect("SIGINT handler");
                tokio::select! {
                    _ = term.recv() => token.cancel(),
                    _ = int.recv() => token.cancel(),
                }
            }
            #[cfg(not(unix))]
            {
                let _ = token;
            }
        });
    }

    pub fn teardown(&self) -> std::io::Result<()> {
        let sock = self.control_socket();
        if sock.exists() {
            let _ = std::fs::remove_file(sock);
        }
        if self.run_dir.exists() {
            std::fs::remove_dir_all(&self.run_dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::FsBackend;
    use serde_cbor::Value;
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn secret_source_reads_unlocked_vault_field() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "pw", "recovery")
            .await
            .unwrap();
        let mut fields = BTreeMap::new();
        fields.insert("application_key".to_string(), Value::Text("ak".into()));
        store
            .put_vault_fields("prod-runtime", "cloud/ovh.vault", &fields)
            .await
            .unwrap();

        // The store already holds the unlocked DataKey, so the session needs no
        // pending unlock; the secret source reads through to the field.
        let session = VaultSession::from_store(store, Vec::new());
        let source = session.secret_source().expect("secret source present");
        let bytes = source
            .read_field("cloud/ovh.vault", "application_key")
            .await
            .unwrap();
        assert_eq!(bytes, b"ak");
    }

    #[tokio::test]
    async fn secret_source_absent_without_store() {
        assert!(VaultSession::default().secret_source().is_none());
    }

    #[tokio::test]
    async fn secret_source_missing_field_errors() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "pw", "recovery")
            .await
            .unwrap();
        let mut fields = BTreeMap::new();
        fields.insert("application_key".to_string(), Value::Text("ak".into()));
        store
            .put_vault_fields("prod-runtime", "cloud/ovh.vault", &fields)
            .await
            .unwrap();

        let session = VaultSession::from_store(store, Vec::new());
        let source = session.secret_source().unwrap();
        assert!(source
            .read_field("cloud/ovh.vault", "consumer_key")
            .await
            .is_err());
    }
}
