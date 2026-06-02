//! Automatic vault store migrations on open (idempotent).

use crate::backend::Backend;
use crate::envelope::{
    deduplicate_recipient_labels, encode_dkey_blob, parse_dkey_blob, seal_envelope_auth,
    DataKeyEnvelope,
};
use crate::error::Result;
use crate::format::MAGIC as VAULT_MAGIC;
use crate::store_format::{
    decode_store_meta, encode_store_meta, is_wrapped_dkey_blob, StoreMeta, META_KEY,
    VAULT_FILE_VERSION,
};
use std::sync::Arc;

/// Ensure `meta/store.cbor` exists and on-disk objects match [`STORE_FORMAT_VERSION`].
pub async fn ensure_store_format(backend: &Arc<dyn Backend>) -> Result<()> {
    let current = load_meta(backend)
        .await?
        .map(|m| m.format_version)
        .unwrap_or(0);
    if current < 1 {
        migrate_store_v0_to_v1(backend).await?;
    }
    if current < 2 {
        migrate_store_deduplicate_recipient_labels(backend).await?;
    }
    if current < 3 {
        migrate_vault_files_to_current_version(backend).await?;
    }
    write_meta(backend, &StoreMeta::current()).await
}

/// After a successful unlock, upgrade envelope semantics (e.g. add auth tag).
pub fn migrate_envelope_after_unlock(
    envelope: &mut DataKeyEnvelope,
    dek: &[u8; 32],
) -> Result<bool> {
    if !envelope.auth.is_empty() {
        return Ok(false);
    }
    seal_envelope_auth(envelope, dek)?;
    Ok(true)
}

async fn load_meta(backend: &Arc<dyn Backend>) -> Result<Option<StoreMeta>> {
    let Some((bytes, _)) = backend.get(META_KEY).await? else {
        return Ok(None);
    };
    Ok(Some(decode_store_meta(&bytes)?))
}

async fn write_meta(backend: &Arc<dyn Backend>, meta: &StoreMeta) -> Result<()> {
    let bytes = encode_store_meta(meta)?;
    backend
        .put(META_KEY, bytes::Bytes::from(bytes), None)
        .await?;
    Ok(())
}

/// v0: no store meta, bare CBOR `.dkey` files. v1: meta + `INFRZDKEY` wire wrapper.
async fn migrate_store_v0_to_v1(backend: &Arc<dyn Backend>) -> Result<()> {
    let objects = backend.list("keys/").await?;
    for obj in objects {
        if !obj.key.ends_with(".dkey") {
            continue;
        }
        let Some((bytes, meta)) = backend.get(&obj.key).await? else {
            continue;
        };
        if is_wrapped_dkey_blob(&bytes) {
            continue;
        }
        let mut envelope = parse_dkey_blob(&bytes)?;
        normalize_envelope_recipients(&mut envelope);
        let wrapped = encode_dkey_blob(&envelope)?;
        backend
            .put(&obj.key, bytes::Bytes::from(wrapped), meta.etag.as_ref())
            .await?;
    }
    Ok(())
}

/// v1→v2: remove duplicate recipient labels left over from older stores.
async fn migrate_store_deduplicate_recipient_labels(backend: &Arc<dyn Backend>) -> Result<()> {
    let objects = backend.list("keys/").await?;
    for obj in objects {
        if !obj.key.ends_with(".dkey") {
            continue;
        }
        let Some((bytes, meta)) = backend.get(&obj.key).await? else {
            continue;
        };
        let mut envelope = parse_dkey_blob(&bytes)?;
        if !normalize_envelope_recipients(&mut envelope) {
            continue;
        }
        let wrapped = encode_dkey_blob(&envelope)?;
        backend
            .put(&obj.key, bytes::Bytes::from(wrapped), meta.etag.as_ref())
            .await?;
        tracing::info!(
            data_key = %envelope.id,
            key = %obj.key,
            "removed duplicate recipient labels during store migration"
        );
    }
    Ok(())
}

/// v2→v3: rewrite legacy vault-file version bytes to the current file version.
///
/// Vault file v2 keeps the same encrypted header/body layout as v1; only the
/// outer version byte changes, so this does not require unlocking DataKeys.
async fn migrate_vault_files_to_current_version(backend: &Arc<dyn Backend>) -> Result<()> {
    let objects = backend.list("files/").await?;
    for obj in objects {
        if !obj.key.ends_with(".vault") {
            continue;
        }
        let Some((bytes, meta)) = backend.get(&obj.key).await? else {
            continue;
        };
        let mut bytes = bytes.to_vec();
        if bytes.len() < 9 || &bytes[0..8] != VAULT_MAGIC {
            continue;
        }
        let old = bytes[8];
        if old == VAULT_FILE_VERSION {
            continue;
        }
        crate::format::vault_header_from_blob(&bytes)?;
        bytes[8] = VAULT_FILE_VERSION;
        backend
            .put(&obj.key, bytes::Bytes::from(bytes), meta.etag.as_ref())
            .await?;
        tracing::info!(
            key = %obj.key,
            old_version = old,
            new_version = VAULT_FILE_VERSION,
            "migrated vault file to current version"
        );
    }
    Ok(())
}

/// Deduplicate labels; clear `auth` when the recipient set changes (re-sealed on unlock).
fn normalize_envelope_recipients(envelope: &mut DataKeyEnvelope) -> bool {
    if !deduplicate_recipient_labels(envelope) {
        return false;
    }
    envelope.auth.clear();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FsBackend;
    use crate::envelope::{create_envelope, DataKeyFile};
    use crate::provider::PassphraseProvider;
    use crate::store_format::STORE_FORMAT_VERSION;

    #[tokio::test]
    async fn v0_dkey_wrapped_on_store_migrate() {
        let dir = tempfile::tempdir().unwrap();
        let backend: Arc<dyn crate::backend::Backend> = Arc::new(FsBackend::new(dir.path()));
        let dek = crate::envelope::generate_dek();
        let provider = PassphraseProvider::new("secret");
        let envelope = create_envelope("prod", &dek, &provider, "recovery")
            .await
            .unwrap();
        // Legacy on-disk shape (bare CBOR, no INFRZDKEY wrapper).
        let legacy = serde_cbor::to_vec(&DataKeyFile { envelope }).unwrap();
        backend
            .put("keys/prod.dkey", bytes::Bytes::from(legacy), None)
            .await
            .unwrap();

        ensure_store_format(&backend).await.unwrap();

        let (got, _) = backend.get("keys/prod.dkey").await.unwrap().unwrap();
        assert!(is_wrapped_dkey_blob(&got));
        let parsed = parse_dkey_blob(&got).unwrap();
        assert_eq!(parsed.id, "prod");

        let meta = backend.get(META_KEY).await.unwrap().unwrap();
        let m = decode_store_meta(&meta.0).unwrap();
        assert_eq!(m.format_version, STORE_FORMAT_VERSION);
    }

    #[tokio::test]
    async fn legacy_envelope_without_auth_migrates_on_unlock() {
        use crate::envelope::{generate_dek, unlock_envelope, verify_envelope_auth};
        use crate::provider::Provider;

        let dek = generate_dek();
        let mut envelope = DataKeyEnvelope {
            id: "prod".into(),
            file_salt: [1u8; 32],
            recipients: vec![],
            auth: Vec::new(),
        };
        let provider = PassphraseProvider::new("hunter2");
        let entry = provider
            .wrap(
                &dek,
                &crate::provider::WrapCtx {
                    data_key_id: "prod".into(),
                    file_salt: envelope.file_salt,
                },
                "recovery",
            )
            .await
            .unwrap();
        envelope.recipients.push(entry);

        verify_envelope_auth(&envelope, &dek).unwrap();
        assert!(envelope.auth.is_empty());

        let _ = unlock_envelope(&envelope, &provider, &envelope.recipients[0])
            .await
            .unwrap();
        assert!(migrate_envelope_after_unlock(&mut envelope, &dek).unwrap());
        assert!(!envelope.auth.is_empty());
        verify_envelope_auth(&envelope, &dek).unwrap();
    }

    #[tokio::test]
    async fn duplicate_recipient_labels_deduped_on_store_migrate() {
        use crate::envelope::{create_envelope, DataKeyFile};
        use crate::provider::PassphraseProvider;

        let dir = tempfile::tempdir().unwrap();
        let backend: Arc<dyn crate::backend::Backend> = Arc::new(FsBackend::new(dir.path()));
        let dek = crate::envelope::generate_dek();
        let provider = PassphraseProvider::new("secret");
        let mut envelope = create_envelope("prod", &dek, &provider, "yubikey-5")
            .await
            .unwrap();
        let dup = envelope.recipients[0].clone();
        envelope.recipients.push(dup);
        let legacy = serde_cbor::to_vec(&DataKeyFile { envelope }).unwrap();
        backend
            .put("keys/prod.dkey", bytes::Bytes::from(legacy), None)
            .await
            .unwrap();

        ensure_store_format(&backend).await.unwrap();

        let (got, _) = backend.get("keys/prod.dkey").await.unwrap().unwrap();
        let parsed = crate::decode_dkey_blob(&got).unwrap();
        assert_eq!(parsed.recipients.len(), 1);
        assert_eq!(parsed.recipients[0].label, "yubikey-5");
        assert!(parsed.auth.is_empty());
    }

    #[tokio::test]
    async fn v2_store_migrates_vault_files_to_current_version() {
        use crate::format::encrypt_map;
        use crate::store_format::{decode_store_meta, encode_store_meta, META_KEY};
        use serde_cbor::Value;
        use std::collections::BTreeMap;

        let dir = tempfile::tempdir().unwrap();
        let backend: Arc<dyn crate::backend::Backend> = Arc::new(FsBackend::new(dir.path()));
        let mut map = BTreeMap::new();
        map.insert("password".to_string(), Value::Text("secret".to_string()));
        let mut blob = encrypt_map(&[3u8; 32], "prod", &map).unwrap();
        blob[8] = 1;
        backend
            .put("files/db/postgres.vault", bytes::Bytes::from(blob), None)
            .await
            .unwrap();
        backend
            .put(
                META_KEY,
                bytes::Bytes::from(encode_store_meta(&StoreMeta { format_version: 2 }).unwrap()),
                None,
            )
            .await
            .unwrap();

        ensure_store_format(&backend).await.unwrap();

        let (migrated, _) = backend
            .get("files/db/postgres.vault")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(migrated[8], crate::store_format::VAULT_FILE_VERSION);
        let (meta, _) = backend.get(META_KEY).await.unwrap().unwrap();
        assert_eq!(
            decode_store_meta(&meta).unwrap().format_version,
            STORE_FORMAT_VERSION
        );
    }
}
