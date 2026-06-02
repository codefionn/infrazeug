use crate::error::{PullError, Result};
use crate::store::PlanStore;
use infrazeug_core::exec::LocalExecutor;
use infrazeug_core::id::MachineId;
use infrazeug_core::interactor::AutoDenyInteractor;
use infrazeug_core::slice::{slice_digest, PlanSlice};
use infrazeug_core::{Infra, MachineKind};
use infrazeug_secrets::{unseal_bytes, verify_signature, MachineKeyPair};
use std::sync::Arc;
use uuid::Uuid;

/// Reconstruct an [`Infra`] graph from a pull-mode slice's embedded nodes
/// and machine definition. In the pull microarchitecture the target host
/// has no access to the controller's full graph — everything it needs is
/// embedded in the slice.
fn infra_from_slice(slice: &PlanSlice) -> Infra {
    let mut infra = Infra::new();
    if let Some(m) = &slice.embedded_machine {
        infra.machines.push(m.clone());
    } else {
        infra.machines.push(infrazeug_core::Machine {
            id: slice.machine_id,
            name: slice.machine_id.0.to_string(),
            kind: MachineKind::Local,
            vars: Default::default(),
            groups: vec![],
            tags: vec![],
            max_parallel_nodes: None,
            lifecycle: infrazeug_core::machine::Lifecycle::Persistent,
            like: None,
            lazy: false,
        });
    }
    infra.nodes = slice.embedded_nodes.clone();
    infra
}

/// Unseal and apply a pull-mode plan slice.
///
/// Core of the pull-mode apply microarchitecture:
/// 1. Check tombstone (revocation).
/// 2. Fetch sealed blob from `PlanStore`.
/// 3. Unseal with the machine's X25519 private key.
/// 4. Recompute the slice digest from its contents and require a valid Ed25519
///    signature from a key in `trusted_signers`.
/// 5. Build `Infra` from embedded nodes/machine.
/// 6. Apply with `AutoDenyInteractor` (no interactive prompts).
///
/// `trusted_signers` are the Ed25519 public keys the host is bootstrapped to
/// trust (from `Bootstrap::plan_signer`). It must be non-empty: a sealed blob
/// only provides confidentiality, not sender authentication (anyone holding the
/// machine's public X25519 key can seal to it), and a [`PlanSignature`] embeds
/// its own verifying key, so without an independent trust anchor signatures are
/// forgeable. An empty trust set is therefore rejected (fail-closed).
pub async fn apply_sealed_slice(
    infra: &Infra,
    store: &PlanStore,
    machine: Uuid,
    machine_key_path: &std::path::Path,
    trusted_signers: &[[u8; 32]],
) -> Result<()> {
    if store.is_revoked(machine).await? {
        return Err(PullError::Revoked);
    }
    let sealed = store
        .get_sealed_plan(machine)
        .await?
        .ok_or_else(|| PullError::Sealed("no plan published".into()))?;
    let pair = MachineKeyPair::read_private_file(machine_key_path)
        .map_err(|e| PullError::Sealed(e.to_string()))?;
    let plain = unseal_bytes(&sealed, pair.secret_bytes())?;
    let slice: PlanSlice =
        PlanSlice::from_cbor(&plain).map_err(|e| PullError::Sealed(e.to_string()))?;
    if slice.machine_id != MachineId(machine) {
        return Err(PullError::Sealed("slice machine mismatch".into()));
    }

    // Bind the signed digest to the slice contents. Signatures only cover
    // `slice.digest`; without recomputing it from the actual steps/nodes an
    // attacker could keep a valid signature while swapping in a different plan.
    if slice_digest(&slice) != slice.digest {
        return Err(PullError::Signature(
            "sealed slice digest does not match its contents".into(),
        ));
    }

    // Fail-closed: require an explicit trust anchor for signers.
    if trusted_signers.is_empty() {
        return Err(PullError::Signature(
            "no trusted plan signer configured; refusing to apply sealed plan".into(),
        ));
    }
    let trusted_ok = slice.signatures.iter().any(|sig| {
        trusted_signers.contains(&sig.public_key) && verify_signature(&slice.digest.0, sig).is_ok()
    });
    if !trusted_ok {
        return Err(PullError::Signature(
            "sealed plan has no valid signature from a trusted signer".into(),
        ));
    }
    let apply_infra = if infra.nodes.is_empty() && !slice.embedded_nodes.is_empty() {
        infra_from_slice(&slice)
    } else {
        infra.clone()
    };
    let (events, _) = tokio::sync::broadcast::channel(64);
    let (_tx, cmd_rx) = tokio::sync::mpsc::channel(4);
    let cancel = tokio_util::sync::CancellationToken::new();
    let interact = Arc::new(AutoDenyInteractor);
    let executor = Arc::new(LocalExecutor);
    let native_executor = infrazeug_core::empty_native_executor();
    let relay = Arc::new(infrazeug_core::HashRelay::new());
    let _report = apply_infra
        .apply_slice(
            slice,
            interact,
            events,
            cancel,
            cmd_rx,
            executor,
            native_executor,
            relay,
        )
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub fn verify_detached_agent_sig(
    agent_bytes: &[u8],
    sig_bytes: &[u8],
    trusted_pubkey: &[u8; 32],
) -> Result<()> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    use sha2::{Digest, Sha256};
    let vk = VerifyingKey::from_bytes(trusted_pubkey)
        .map_err(|_| PullError::Signature("bad pubkey".into()))?;
    let sig = Signature::from_slice(sig_bytes)
        .map_err(|_| PullError::Signature("bad signature bytes".into()))?;
    let digest = Sha256::digest(agent_bytes);
    vk.verify(&digest, &sig)
        .map_err(|_| PullError::Signature("agent signature mismatch".into()))?;
    Ok(())
}
