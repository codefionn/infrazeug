#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::backend::{Backend, FsBackend};
    use crate::multi::{MultiBackend, ReadPolicy};
    use serde_cbor::Value;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn passphrase_vault_roundtrip() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();

        let mut m = BTreeMap::new();
        m.insert("greeting".into(), Value::Text("hello vault".into()));
        store
            .put_vault_file("prod", "demo/greeting.vault", &m)
            .await
            .unwrap();

        store.lock_all();
        store
            .unlock_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();
        let v = store
            .resolve_field(&VaultRef::field("demo/greeting.vault", "greeting"))
            .await
            .unwrap();
        assert!(matches!(v, Value::Text(s) if s == "hello vault"));
    }

    #[tokio::test]
    async fn fs_backend_rejects_path_traversal_keys() {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path());

        assert!(backend
            .put("../outside", bytes::Bytes::from_static(b"x"), None)
            .await
            .is_err());
        assert!(backend
            .put(
                "files/%2e%2e/outside",
                bytes::Bytes::from_static(b"x"),
                None
            )
            .await
            .is_err());
        assert!(backend.get("../outside").await.is_err());
        assert!(backend.delete("../outside").await.is_err());
        assert!(!dir.path().join("../outside").exists());
    }

    #[tokio::test]
    async fn vault_store_rejects_unsafe_ids_and_file_paths() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());

        assert!(store
            .keygen_passphrase("../prod", "hunter2", "recovery")
            .await
            .is_err());

        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();
        let mut m = BTreeMap::new();
        m.insert("x".into(), Value::Text("1".into()));
        assert!(store
            .put_vault_file("prod", "../escape.vault", &m)
            .await
            .is_err());
        assert!(!dir.path().join("escape.vault").exists());
    }

    #[tokio::test]
    async fn new_vault_files_use_current_version_and_v1_still_reads() {
        let dek = [7u8; 32];
        let mut m = BTreeMap::new();
        m.insert("x".into(), Value::Text("1".into()));

        let blob = crate::format::encrypt_map(&dek, "prod", &m).unwrap();
        assert_eq!(blob[8], crate::store_format::VAULT_FILE_VERSION);
        assert_eq!(crate::store_format::VAULT_FILE_VERSION, 2);

        let mut legacy = blob.clone();
        legacy[8] = 1;
        let (_, out) = crate::format::decrypt_map(&dek, &legacy).unwrap();
        assert_eq!(out.get("x"), Some(&Value::Text("1".into())));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fs_backend_writes_private_files() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path());
        backend
            .put("keys/prod.dkey", bytes::Bytes::from_static(b"x"), None)
            .await
            .unwrap();

        let file_mode = std::fs::metadata(dir.path().join("keys/prod.dkey"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        let dir_mode = std::fs::metadata(dir.path().join("keys"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600);
        assert_eq!(dir_mode, 0o700);
    }

    #[tokio::test]
    async fn list_files_with_data_keys() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();
        store
            .keygen_passphrase("ops", "hunter2", "recovery")
            .await
            .unwrap();

        let mut m = BTreeMap::new();
        m.insert("x".into(), Value::Text("1".into()));
        store.put_vault_file("prod", "a.vault", &m).await.unwrap();
        store
            .put_vault_file("ops", "b/nested.vault", &m)
            .await
            .unwrap();

        let listed = store.list_vault_files_with_data_keys().await.unwrap();
        assert_eq!(
            listed,
            vec![
                VaultFileKey {
                    file: "a.vault".into(),
                    data_key_id: "prod".into(),
                },
                VaultFileKey {
                    file: "b/nested.vault".into(),
                    data_key_id: "ops".into(),
                },
            ]
        );

        let paths = store.list_vault_field_paths("a.vault").await.unwrap();
        assert_eq!(paths, vec!["x".to_string()]);

        let keys = store.list_data_keys().await.unwrap();
        assert_eq!(keys, vec!["ops".to_string(), "prod".to_string()]);
        let format = store.store_format_version().await.unwrap();
        assert!(format.is_some());
    }

    #[tokio::test]
    async fn mutable_vault_fields_patch_generated_secret() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod-runtime", "hunter2", "recovery")
            .await
            .unwrap();

        let mut first = BTreeMap::new();
        first.insert("bucket.name".into(), Value::Text("images-prod".into()));
        first.insert(
            "credentials.access_key".into(),
            Value::Text("AKIA...".into()),
        );
        first.insert(
            "credentials.secret_key".into(),
            Value::Text("secret".into()),
        );
        store
            .put_mutable_vault_fields("prod-runtime", "cloud/images.vault", &first)
            .await
            .unwrap();

        let name = store
            .resolve_field(&VaultRef::mutable_field(
                "cloud/images.vault",
                "bucket.name",
            ))
            .await
            .unwrap();
        assert_eq!(name, Value::Text("images-prod".into()));
        let access_key = store
            .resolve_mutable_field("cloud/images.vault", "credentials.access_key")
            .await
            .unwrap();
        assert_eq!(access_key, Value::Text("AKIA...".into()));

        let mut second = BTreeMap::new();
        second.insert("credentials.access_key".into(), Value::Text("AKIA2".into()));
        store
            .put_mutable_vault_fields("prod-runtime", "cloud/images.vault", &second)
            .await
            .unwrap();

        let map = store
            .read_vault_map("mutable/cloud/images.vault")
            .await
            .unwrap();
        let paths = crate::collect_vault_field_paths(&map);
        assert_eq!(
            paths,
            vec![
                "bucket.name".to_string(),
                "credentials.access_key".to_string(),
                "credentials.secret_key".to_string(),
            ]
        );
        assert_eq!(
            store
                .resolve_mutable_field("cloud/images.vault", "credentials.access_key")
                .await
                .unwrap(),
            Value::Text("AKIA2".into())
        );
    }

    #[tokio::test]
    async fn mutable_path_constructor_is_idempotent() {
        assert_eq!(
            mutable_vault_path("cloud/bucket.vault"),
            "mutable/cloud/bucket.vault"
        );
        assert_eq!(
            mutable_vault_path("mutable/cloud/bucket.vault"),
            "mutable/cloud/bucket.vault"
        );
        assert_eq!(
            VaultRef::mutable_field("cloud/bucket.vault", "api.key").file,
            "mutable/cloud/bucket.vault"
        );
    }

    #[tokio::test]
    async fn set_default_recipient_reorders() {
        use crate::provider::PassphraseProvider;

        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();
        store
            .add_recipient("prod", &PassphraseProvider::new("op-pass"), "operator")
            .await
            .unwrap();

        let labels: Vec<_> = store
            .list_recipients("prod")
            .await
            .unwrap()
            .into_iter()
            .map(|(_, l)| l)
            .collect();
        assert_eq!(labels, vec!["recovery", "operator"]);

        store
            .set_default_recipient("prod", "operator")
            .await
            .unwrap();
        let labels: Vec<_> = store
            .list_recipients("prod")
            .await
            .unwrap()
            .into_iter()
            .map(|(_, l)| l)
            .collect();
        assert_eq!(labels, vec!["operator", "recovery"]);

        // Unlock still works via the now-second recovery recipient (label-only lookup).
        store.lock_all();
        store
            .unlock_with_provider_label("prod", &PassphraseProvider::new("hunter2"), "recovery")
            .await
            .unwrap();
        assert!(store.is_unlocked("prod"));
    }

    #[tokio::test]
    async fn store_migrates_on_keygen_and_writes_meta() {
        use crate::store_format::{decode_store_meta, is_wrapped_dkey_blob, META_KEY};

        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend.clone(), dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();

        let (dkey, _) = backend.get("keys/prod.dkey").await.unwrap().unwrap();
        assert!(is_wrapped_dkey_blob(&dkey));

        let (meta, _) = backend.get(META_KEY).await.unwrap().unwrap();
        let m = decode_store_meta(&meta).unwrap();
        assert_eq!(m.format_version, crate::store_format::STORE_FORMAT_VERSION);
    }

    #[tokio::test]
    async fn legacy_envelope_auth_migrated_on_unlock() {
        use crate::envelope::DataKeyFile;
        use crate::provider::PassphraseProvider;
        use crate::store_format::is_wrapped_dkey_blob;

        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend.clone(), dir.path().to_path_buf());

        let dek = crate::envelope::generate_dek();
        let provider = PassphraseProvider::new("hunter2");
        let mut envelope = crate::envelope::DataKeyEnvelope {
            id: "prod".into(),
            file_salt: [2u8; 32],
            recipients: vec![],
            auth: Vec::new(),
        };
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
        let legacy = serde_cbor::to_vec(&DataKeyFile { envelope }).unwrap();
        backend
            .put("keys/prod.dkey", bytes::Bytes::from(legacy), None)
            .await
            .unwrap();

        store
            .unlock_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();

        let (dkey, _) = backend.get("keys/prod.dkey").await.unwrap().unwrap();
        assert!(is_wrapped_dkey_blob(&dkey));
        let parsed = crate::decode_dkey_blob(&dkey).unwrap();
        assert!(!parsed.auth.is_empty());
    }

    #[tokio::test]
    async fn duplicate_recipient_label_rejected() {
        use crate::provider::PassphraseProvider;

        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();

        let err = store
            .add_recipient("prod", &PassphraseProvider::new("other"), "recovery")
            .await;
        assert!(matches!(err, Err(SecretsError::Provider(msg)) if msg.contains("recovery")));
    }

    #[tokio::test]
    async fn set_default_unknown_label_errors() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();
        assert!(store.set_default_recipient("prod", "nope").await.is_err());
    }

    #[tokio::test]
    async fn envelope_tamper_rejected_on_unlock() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut store = VaultStore::new(backend, dir.path().to_path_buf());
        store
            .keygen_passphrase("prod", "hunter2", "recovery")
            .await
            .unwrap();

        // Append a bogus recipient without re-sealing the auth tag. The original
        // "recovery" recipient still unwraps, so without envelope authentication
        // this injected recipient would go unnoticed.
        let mut envelope = store.load_envelope("prod").await.unwrap();
        let mut intruder = envelope.recipients[0].clone();
        intruder.label = "intruder".into();
        envelope.recipients.push(intruder);
        store.save_envelope(&envelope).await.unwrap();

        store.lock_all();
        let err = store.unlock_passphrase("prod", "hunter2", "recovery").await;
        assert!(
            matches!(err, Err(SecretsError::BadSignature)),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn multi_backend_latest_mtime() {
        let primary_dir = tempdir().unwrap();
        let mirror_dir = tempdir().unwrap();
        let primary = Arc::new(FsBackend::new(primary_dir.path()));
        let mirror: Arc<dyn Backend> = Arc::new(FsBackend::new(mirror_dir.path()));
        let multi = MultiBackend::new(primary)
            .with_mirror(Arc::clone(&mirror))
            .with_read(ReadPolicy::LatestByMtime);

        multi
            .put("files/x.vault", bytes::Bytes::from_static(b"old"), None)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        mirror
            .put("files/x.vault", bytes::Bytes::from_static(b"new"), None)
            .await
            .unwrap();

        let (got, _) = multi.get("files/x.vault").await.unwrap().unwrap();
        assert_eq!(&got[..], b"new");
    }
}
