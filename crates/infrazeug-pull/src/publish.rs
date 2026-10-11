use crate::error::{PullError, Result};
use crate::store::PlanStore;
use infrazeug_core::id::MachineId;
use infrazeug_core::slice::{PlanSlice, SliceMode};
use infrazeug_core::Infra;
use infrazeug_secrets::{
    seal_bytes, sign_digest, signing_key_from_seed, verify_signature, PlanSignature,
};
use uuid::Uuid;

pub struct PublishOptions {
    pub agent_digest: Option<String>,
    pub signing_seed: Option<[u8; 32]>,
    pub signer_id: String,
}

/// Register a machine's X25519 public key in the plan store.
///
/// Part of the pull-mode microarchitecture: the controller generates a
/// keypair per machine (`machine keygen`), and the host later uses the
/// private key to unseal its plan slice.
pub async fn register_machine_pubkey(
    store: &PlanStore,
    machine: Uuid,
    pubkey: [u8; 32],
) -> Result<()> {
    store.put_machine_pubkey(machine, &pubkey).await
}

/// Plan → pull-mode slice → sign → seal → store.
///
/// Core of the pull-mode publishing microarchitecture:
/// 1. `infra.plan()` — compute the full plan.
/// 2. `plan.slice_for_machine(Pull)` — per-machine slice (rejects cross-machine deps).
/// 3. Optional Ed25519 signature on the slice digest.
/// 4. `seal_bytes()` — X25519 + XChaCha20-Poly1305 AEAD encryption to the machine's public key.
/// 5. `store.put_sealed_plan()` — persist the sealed blob.
pub async fn publish_slice(
    infra: &Infra,
    store: &PlanStore,
    machine: Uuid,
    opts: PublishOptions,
) -> Result<PlanSlice> {
    let mid = MachineId(machine);
    let plan = infra.plan()?;
    let mut slice = plan.slice_for_machine(infra, mid, SliceMode::Pull)?;
    slice.agent_digest = opts.agent_digest;
    // Recompute the digest over the final slice contents (agent_digest included)
    // before signing; `slice_digest` excludes signatures so it stays stable as
    // they are appended, and the apply side rejects any digest/content mismatch.
    let mut slice = slice.finalize();

    if let Some(seed) = opts.signing_seed {
        let key = signing_key_from_seed(&seed);
        let sig = sign_digest(&slice.digest.0, &key, &opts.signer_id);
        slice = sign_slice(&slice, sig)?;
    }

    let pubkey = store.get_machine_pubkey(machine).await?;
    let body = slice
        .to_cbor()
        .map_err(|e| PullError::Sealed(e.to_string()))?;
    let sealed = seal_bytes(&body, &pubkey)?;
    store.put_sealed_plan(machine, &sealed).await?;
    Ok(slice)
}

fn sign_slice(slice: &PlanSlice, sig: PlanSignature) -> Result<PlanSlice> {
    verify_signature(&slice.digest.0, &sig).map_err(|e| PullError::Signature(e.to_string()))?;
    let mut copy = slice.clone();
    copy.signatures.push(sig);
    Ok(copy)
}

pub async fn revoke_machine(
    store: &PlanStore,
    machine: Uuid,
    with_teardown: bool,
    signing_key: Option<&ed25519_dalek::SigningKey>,
    signer_id: &str,
) -> Result<()> {
    let action = if with_teardown {
        "revoke-with-teardown"
    } else {
        "revoked"
    };
    let body = tombstone_body(machine, action, signing_key, signer_id)?;
    store.put_tombstone(machine, &body).await
}

/// Canonical bytes a tombstone signature covers: domain-separated machine id
/// and revocation action, so a `revoke` cannot be replayed as a
/// `revoke-with-teardown` (or vice versa) or moved between machines.
fn tombstone_digest(machine: Uuid, action: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"infrazeug-tombstone-v1\0");
    h.update(machine.as_bytes());
    h.update(action.as_bytes());
    h.finalize().into()
}

/// Serialize a revocation tombstone. Signed tombstones carry an Ed25519
/// signature over [`tombstone_digest`]; without a signing key the tombstone is
/// written in the legacy plain-text form, which daemons with a trusted-signer
/// set reject (a hostile store could forge it — including a forged
/// `revoke-with-teardown` to trigger cleanup actions on a healthy machine).
pub(crate) fn tombstone_body(
    machine: Uuid,
    action: &str,
    signing_key: Option<&ed25519_dalek::SigningKey>,
    signer_id: &str,
) -> Result<Vec<u8>> {
    match signing_key {
        Some(key) => {
            let digest = tombstone_digest(machine, action);
            let sig = infrazeug_secrets::sign_digest(&digest, key, signer_id);
            serde_cbor::to_vec(&SignedTombstone {
                action: action.to_string(),
                signatures: vec![sig],
            })
            .map_err(|e| PullError::Sealed(e.to_string()))
        }
        None => {
            tracing::warn!(
                "writing unsigned legacy tombstone; daemons with trusted signers will ignore it"
            );
            Ok(action.as_bytes().to_vec())
        }
    }
}

/// A signed revocation tombstone body.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SignedTombstone {
    pub action: String,
    pub signatures: Vec<infrazeug_secrets::PlanSignature>,
}

/// Check a tombstone body against the trusted signer set.
///
/// Returns `true` only when the tombstone is authentic:
/// - signed tombstones require a valid signature from a trusted signer;
/// - legacy plain-text tombstones are honored (fail-closed on revocation) only
///   when the caller has no trusted signers configured, and rejected with a
///   warning otherwise — an unsigned body is forgeable by anyone with store
///   write access.
pub fn tombstone_is_authentic(
    machine: Uuid,
    body: &[u8],
    trusted_signers: &[[u8; 32]],
) -> Result<bool> {
    if let Ok(tomb) = serde_cbor::from_slice::<SignedTombstone>(body) {
        if !matches!(
            tomb.action.as_str(),
            "revoked" | "revoke-with-teardown"
        ) {
            return Ok(false);
        }
        let digest = tombstone_digest(machine, &tomb.action);
        let ok = tomb.signatures.iter().any(|sig| {
            trusted_signers.contains(&sig.public_key)
                && infrazeug_secrets::verify_signature(&digest, sig).is_ok()
        });
        return Ok(ok);
    }
    let text = String::from_utf8_lossy(body);
    match text.as_ref() {
        "revoked" | "revoke-with-teardown" => {
            if trusted_signers.is_empty() {
                Ok(true)
            } else {
                tracing::warn!(
                    %machine,
                    "ignoring unsigned tombstone from store (signed revocations required)"
                );
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}

pub fn machine_keygen(machine: Uuid, out: &std::path::Path) -> Result<[u8; 32]> {
    let pair = infrazeug_secrets::MachineKeyPair::generate();
    pair.write_private_file(out)
        .map_err(|e| PullError::Other(e.to_string()))?;
    let bootstrap = format!(
        "# register with: infrazeug machine register --machine {machine} --pubkey {}\n",
        hex::encode(pair.public)
    );
    std::fs::write(out.with_extension("pub.txt"), bootstrap)
        .map_err(|e| PullError::Other(e.to_string()))?;
    Ok(pair.public)
}
