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

pub async fn revoke_machine(store: &PlanStore, machine: Uuid, with_teardown: bool) -> Result<()> {
    let body: &[u8] = if with_teardown {
        b"revoke-with-teardown"
    } else {
        b"revoked"
    };
    store.put_tombstone(machine, body).await
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
