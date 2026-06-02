use crate::bootstrap::Bootstrap;
use crate::error::{PullError, Result};
use crate::mode::PullMode;
use crate::serve::apply_sealed_slice;
use crate::store::PlanStore;
use infrazeug_core::Infra;
use infrazeug_secrets::backend::FsBackend;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub async fn run_oneshot(
    infra: &Infra,
    store: Arc<PlanStore>,
    machine: Uuid,
    key_path: PathBuf,
    trusted_signers: &[[u8; 32]],
) -> Result<()> {
    apply_sealed_slice(infra, store.as_ref(), machine, &key_path, trusted_signers).await
}

pub async fn run_daemon(
    infra: &Infra,
    store: Arc<PlanStore>,
    machine: Uuid,
    key_path: PathBuf,
    interval: Duration,
    jitter: Duration,
    trusted_signers: &[[u8; 32]],
) -> Result<()> {
    let mut last_digest: Option<[u8; 32]> = None;
    loop {
        if store.is_revoked(machine).await? {
            return Err(PullError::Revoked);
        }
        if let Some(sealed) = store.get_sealed_plan(machine).await? {
            use sha2::{Digest, Sha256};
            let digest: [u8; 32] = Sha256::digest(&sealed).into();
            if last_digest != Some(digest) {
                apply_sealed_slice(infra, store.as_ref(), machine, &key_path, trusted_signers)
                    .await?;
                last_digest = Some(digest);
            }
        }
        let sleep = interval.saturating_add(jitter / 2);
        tokio::time::sleep(sleep).await;
    }
}

pub async fn run_from_bootstrap(infra: &Infra, bootstrap: &Bootstrap) -> Result<()> {
    let store_path = bootstrap.plan_url.trim_end_matches('/');
    let backend = Arc::new(FsBackend::new(store_path));
    let store = Arc::new(PlanStore::new(backend));
    let machine = bootstrap.machine_id;
    let key = bootstrap.machine_key.clone();
    let trusted = parse_trusted_signers(&bootstrap.plan_signer)?;
    match bootstrap.pull_mode() {
        PullMode::OneShot => run_oneshot(infra, store, machine, key, &trusted).await,
        PullMode::Daemon { interval, jitter } => {
            run_daemon(infra, store, machine, key, interval, jitter, &trusted).await
        }
    }
}

/// Parse `Bootstrap::plan_signer` into trusted Ed25519 public keys. Accepts one
/// or more hex-encoded 32-byte keys separated by commas or whitespace. An empty
/// or malformed value is rejected so the host never applies an untrusted plan.
pub fn parse_trusted_signers(plan_signer: &str) -> Result<Vec<[u8; 32]>> {
    let mut out = Vec::new();
    for tok in plan_signer.split([',', ' ', '\t', '\n', '\r']) {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let bytes = hex::decode(tok)
            .map_err(|e| PullError::Bootstrap(format!("invalid plan_signer hex {tok:?}: {e}")))?;
        if bytes.len() != 32 {
            return Err(PullError::Bootstrap(format!(
                "plan_signer {tok:?} must be a 32-byte Ed25519 key"
            )));
        }
        let mut k = [0u8; 32];
        k.copy_from_slice(&bytes);
        out.push(k);
    }
    if out.is_empty() {
        return Err(PullError::Bootstrap(
            "bootstrap plan_signer is empty; refusing to apply untrusted plans".into(),
        ));
    }
    Ok(out)
}

pub fn open_fs_store(plan_url: &str) -> PlanStore {
    PlanStore::new(Arc::new(FsBackend::new(plan_url.trim_end_matches('/'))))
}
