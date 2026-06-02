use crate::backend::{Backend, Etag};
use crate::envelope::{
    create_envelope, decode_dkey_blob, encode_dkey_blob, find_recipient, generate_dek,
    seal_envelope_auth, unlock_envelope, verify_envelope_auth, DataKeyEnvelope,
};
use crate::error::{Result, SecretsError};
use crate::format::{
    collect_vault_field_paths, decrypt_map, encrypt_map, field_from_map, vault_header_from_blob,
};
use crate::migrate::{ensure_store_format, migrate_envelope_after_unlock};
use crate::provider::{PassphraseProvider, Provider, ProviderKind, WrapCtx};
use crate::store_format::{decode_store_meta, META_KEY};
use crate::vault_ref::{mutable_vault_path, VaultRef};
use bytes::Bytes;
use serde_cbor::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zeroize::Zeroizing;

/// Vault file path (under `files/`) and the DataKey id from its header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultFileKey {
    pub file: String,
    pub data_key_id: String,
}

pub struct VaultStore {
    pub backend: Arc<dyn Backend>,
    pub store_root: PathBuf,
    unlocked: HashMap<String, Zeroizing<[u8; 32]>>,
    file_cache: HashMap<String, BTreeMap<String, Value>>,
    store_ready: AtomicBool,
}

impl VaultStore {
    pub fn new(backend: Arc<dyn Backend>, store_root: PathBuf) -> Self {
        Self {
            backend,
            store_root,
            unlocked: HashMap::new(),
            file_cache: HashMap::new(),
            store_ready: AtomicBool::new(false),
        }
    }

    async fn ensure_store_ready(&self) -> Result<()> {
        if self.store_ready.load(Ordering::Acquire) {
            return Ok(());
        }
        ensure_store_format(&self.backend).await?;
        self.store_ready.store(true, Ordering::Release);
        Ok(())
    }

    async fn persist_envelope_if_migrated(
        &self,
        id: &str,
        envelope: &mut DataKeyEnvelope,
        dek: &[u8; 32],
        prev: Option<&Etag>,
    ) -> Result<()> {
        if migrate_envelope_after_unlock(envelope, dek)? {
            self.save_envelope_with_prev(envelope, prev).await?;
            tracing::info!(data_key = %id, "migrated data key envelope to authenticated format");
        }
        Ok(())
    }

    fn key_path(file: &str) -> String {
        format!("files/{file}")
    }

    fn dkey_path(id: &str) -> String {
        format!("keys/{id}.dkey")
    }

    pub async fn load_envelope(&self, id: &str) -> Result<DataKeyEnvelope> {
        let (envelope, _) = self.load_envelope_with_etag(id).await?;
        Ok(envelope)
    }

    async fn load_envelope_with_etag(&self, id: &str) -> Result<(DataKeyEnvelope, Option<Etag>)> {
        self.ensure_store_ready().await?;
        let key = Self::dkey_path(id);
        let (bytes, meta) = self
            .backend
            .get(&key)
            .await?
            .ok_or_else(|| SecretsError::Other(format!("missing data key {id}")))?;
        Ok((decode_dkey_blob(&bytes)?, meta.etag))
    }

    pub async fn save_envelope(&self, envelope: &DataKeyEnvelope) -> Result<()> {
        self.save_envelope_with_prev(envelope, None).await
    }

    async fn save_envelope_with_prev(
        &self,
        envelope: &DataKeyEnvelope,
        prev: Option<&Etag>,
    ) -> Result<()> {
        self.ensure_store_ready().await?;
        let key = Self::dkey_path(&envelope.id);
        let bytes = encode_dkey_blob(envelope)?;
        self.backend.put(&key, Bytes::from(bytes), prev).await?;
        Ok(())
    }

    pub async fn keygen_passphrase(
        &mut self,
        id: &str,
        passphrase: &str,
        label: &str,
    ) -> Result<()> {
        self.ensure_store_ready().await?;
        let key = Self::dkey_path(id);
        if self.backend.get(&key).await?.is_some() {
            return Err(SecretsError::Conflict { key });
        }
        let dek = generate_dek();
        let provider = PassphraseProvider::new(passphrase);
        let envelope = create_envelope(id, &dek, &provider, label).await?;
        self.save_envelope(&envelope).await?;
        self.unlocked.insert(id.to_string(), Zeroizing::new(dek));
        Ok(())
    }

    pub async fn unlock_passphrase(
        &mut self,
        id: &str,
        passphrase: &str,
        label: &str,
    ) -> Result<()> {
        let (envelope, etag) = self.load_envelope_with_etag(id).await?;
        let entry = find_recipient(&envelope, ProviderKind::Passphrase, label)
            .ok_or_else(|| SecretsError::Provider("passphrase recipient not found".into()))?;
        let provider = PassphraseProvider::new(passphrase);
        let dek = unlock_envelope(&envelope, &provider, entry).await?;
        verify_envelope_auth(&envelope, &dek)?;
        let mut envelope = envelope;
        self.persist_envelope_if_migrated(id, &mut envelope, &dek, etag.as_ref())
            .await?;
        self.unlocked.insert(id.to_string(), dek);
        Ok(())
    }

    pub async fn add_recipient(
        &mut self,
        id: &str,
        provider: &dyn Provider,
        label: &str,
    ) -> Result<()> {
        let (envelope, etag) = self.load_envelope_with_etag(id).await?;
        if envelope.recipients.iter().any(|r| r.label == label) {
            return Err(SecretsError::Provider(format!(
                "recipient label {label} already exists in data key {id}"
            )));
        }
        let dek = self.dek(id)?;
        verify_envelope_auth(&envelope, dek)?;
        let ctx = WrapCtx {
            data_key_id: envelope.id.clone(),
            file_salt: envelope.file_salt,
        };
        let entry = provider.wrap(dek, &ctx, label).await?;
        let mut envelope = envelope;
        envelope.recipients.push(entry);
        seal_envelope_auth(&mut envelope, dek)?;
        self.save_envelope_with_prev(&envelope, etag.as_ref()).await
    }

    pub async fn unlock_with_provider(
        &mut self,
        id: &str,
        provider: &dyn Provider,
        label: &str,
    ) -> Result<()> {
        let (envelope, etag) = self.load_envelope_with_etag(id).await?;
        let entry = find_recipient(&envelope, provider.kind(), label)
            .ok_or_else(|| SecretsError::Provider("recipient not found".into()))?;
        let dek = unlock_envelope(&envelope, provider, entry).await?;
        verify_envelope_auth(&envelope, &dek)?;
        let mut envelope = envelope;
        self.persist_envelope_if_migrated(id, &mut envelope, &dek, etag.as_ref())
            .await?;
        self.unlocked.insert(id.to_string(), dek);
        Ok(())
    }

    /// Unlock by recipient `label` alone (kind taken from `provider`).
    pub async fn unlock_with_provider_label(
        &mut self,
        id: &str,
        provider: &dyn Provider,
        label: &str,
    ) -> Result<()> {
        let (envelope, etag) = self.load_envelope_with_etag(id).await?;
        let entry = envelope
            .recipients
            .iter()
            .find(|r| r.label == label)
            .ok_or_else(|| {
                SecretsError::Provider(format!("recipient {label} not found in data key {id}"))
            })?;
        let dek = unlock_envelope(&envelope, provider, entry).await?;
        verify_envelope_auth(&envelope, &dek)?;
        let mut envelope = envelope;
        self.persist_envelope_if_migrated(id, &mut envelope, &dek, etag.as_ref())
            .await?;
        self.unlocked.insert(id.to_string(), dek);
        Ok(())
    }

    /// Data key ids present under `keys/` (sorted).
    pub async fn list_data_keys(&self) -> Result<Vec<String>> {
        self.ensure_store_ready().await?;
        let objects = self.backend.list("keys/").await?;
        let mut ids: Vec<String> = objects
            .iter()
            .filter_map(|o| {
                o.key
                    .strip_prefix("keys/")
                    .and_then(|rest| rest.strip_suffix(".dkey"))
                    .map(str::to_string)
            })
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// Store format version from `meta/store.cbor`, if present.
    pub async fn store_format_version(&self) -> Result<Option<u32>> {
        self.ensure_store_ready().await?;
        let Some((bytes, _)) = self.backend.get(META_KEY).await? else {
            return Ok(None);
        };
        Ok(Some(decode_store_meta(&bytes)?.format_version))
    }

    /// `(kind, label)` of every recipient in stored order (first = default).
    pub async fn list_recipients(&self, id: &str) -> Result<Vec<(ProviderKind, String)>> {
        let envelope = self.load_envelope(id).await?;
        Ok(envelope
            .recipients
            .iter()
            .map(|r| (r.kind, r.label.clone()))
            .collect())
    }

    /// Move the recipient with `label` to the front, making it the default
    /// decryption method. Reordering is authenticated, so the data key must be
    /// unlocked to re-seal the envelope.
    pub async fn set_default_recipient(&self, id: &str, label: &str) -> Result<()> {
        let (mut envelope, etag) = self.load_envelope_with_etag(id).await?;
        let dek = self.dek(id)?;
        verify_envelope_auth(&envelope, dek)?;
        let pos = envelope
            .recipients
            .iter()
            .position(|r| r.label == label)
            .ok_or_else(|| {
                SecretsError::Provider(format!("recipient {label} not found in data key {id}"))
            })?;
        let entry = envelope.recipients.remove(pos);
        envelope.recipients.insert(0, entry);
        seal_envelope_auth(&mut envelope, dek)?;
        self.save_envelope_with_prev(&envelope, etag.as_ref()).await
    }

    pub fn is_unlocked(&self, id: &str) -> bool {
        self.unlocked.contains_key(id)
    }

    pub fn required_data_keys(&self) -> Vec<String> {
        self.unlocked.keys().cloned().collect()
    }

    pub fn lock_all(&mut self) {
        self.unlocked.clear();
        self.file_cache.clear();
    }

    pub async fn put_vault_file(
        &mut self,
        data_key_id: &str,
        file: &str,
        map: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let dek = self.dek(data_key_id)?;
        let blob = encrypt_map(dek.as_ref(), data_key_id, map)?;
        let key = Self::key_path(file);
        let prev = self
            .backend
            .get(&key)
            .await?
            .and_then(|(_, meta)| meta.etag);
        self.backend
            .put(&key, Bytes::from(blob), prev.as_ref())
            .await?;
        self.file_cache.remove(file);
        Ok(())
    }

    /// Store a generated secret file under the reserved mutable namespace
    /// (`files/mutable/...`).
    pub async fn put_mutable_vault_file(
        &mut self,
        data_key_id: &str,
        file: &str,
        map: &BTreeMap<String, Value>,
    ) -> Result<()> {
        self.put_vault_file(data_key_id, &mutable_vault_path(file), map)
            .await
    }

    /// Atomically merge field values into a vault file. Missing files are
    /// created under `data_key_id`; existing files must already be sealed by it.
    /// Returns `true` if any field was actually changed, `false` if all values
    /// were already identical (no write performed).
    pub async fn put_vault_fields(
        &mut self,
        data_key_id: &str,
        file: &str,
        fields: &BTreeMap<String, Value>,
    ) -> Result<bool> {
        let key = Self::key_path(file);
        let existing = self.backend.get(&key).await?;
        let (seal_key, mut map, prev) = match existing {
            Some((bytes, meta)) => {
                let header = vault_header_from_blob(&bytes)?;
                if header.data_key_id != data_key_id {
                    return Err(SecretsError::Other(format!(
                        "vault file {file} is sealed under data key {}, not {data_key_id}",
                        header.data_key_id
                    )));
                }
                let dek = self.dek(&header.data_key_id)?;
                let (_, map) = decrypt_map(dek.as_ref(), &bytes)?;
                (header.data_key_id, map, meta.etag)
            }
            None => {
                self.dek(data_key_id)?;
                (data_key_id.to_string(), BTreeMap::new(), None)
            }
        };

        let mut changed = false;
        for (field, value) in fields {
            let current = field_from_map(&map, field).ok();
            if current.as_ref() != Some(value) {
                changed = true;
                set_field_path(&mut map, field, value.clone())?;
            }
        }
        if !changed {
            return Ok(false);
        }

        let dek = self.dek(&seal_key)?;
        let blob = encrypt_map(dek.as_ref(), &seal_key, &map)?;
        self.backend
            .put(&key, Bytes::from(blob), prev.as_ref())
            .await?;
        self.file_cache.remove(file);
        Ok(true)
    }

    /// Atomically merge only absent field values into a vault file. Missing
    /// files are created under `data_key_id`; existing present fields are never
    /// overwritten. Returns `true` if any field was inserted.
    pub async fn put_vault_fields_if_absent(
        &mut self,
        data_key_id: &str,
        file: &str,
        fields: &BTreeMap<String, Value>,
    ) -> Result<bool> {
        let key = Self::key_path(file);
        let existing = self.backend.get(&key).await?;
        let (seal_key, mut map, prev) = match existing {
            Some((bytes, meta)) => {
                let header = vault_header_from_blob(&bytes)?;
                if header.data_key_id != data_key_id {
                    return Err(SecretsError::Other(format!(
                        "vault file {file} is sealed under data key {}, not {data_key_id}",
                        header.data_key_id
                    )));
                }
                let dek = self.dek(&header.data_key_id)?;
                let (_, map) = decrypt_map(dek.as_ref(), &bytes)?;
                (header.data_key_id, map, meta.etag)
            }
            None => {
                self.dek(data_key_id)?;
                (data_key_id.to_string(), BTreeMap::new(), None)
            }
        };

        let mut changed = false;
        for (field, value) in fields {
            if field_from_map(&map, field).is_err() {
                changed = true;
                set_field_path(&mut map, field, value.clone())?;
            }
        }
        if !changed {
            return Ok(false);
        }

        let dek = self.dek(&seal_key)?;
        let blob = encrypt_map(dek.as_ref(), &seal_key, &map)?;
        self.backend
            .put(&key, Bytes::from(blob), prev.as_ref())
            .await?;
        self.file_cache.remove(file);
        Ok(true)
    }

    /// Atomically merge field values into a generated secret file under
    /// `files/mutable/...`.
    pub async fn put_mutable_vault_fields(
        &mut self,
        data_key_id: &str,
        file: &str,
        fields: &BTreeMap<String, Value>,
    ) -> Result<bool> {
        self.put_vault_fields(data_key_id, &mutable_vault_path(file), fields)
            .await
    }

    pub async fn read_vault_map(&mut self, file: &str) -> Result<BTreeMap<String, Value>> {
        if let Some(cached) = self.file_cache.get(file) {
            return Ok(cached.clone());
        }
        let key = Self::key_path(file);
        let (bytes, _) = self
            .backend
            .get(&key)
            .await?
            .ok_or_else(|| SecretsError::Other(format!("missing vault file {file}")))?;
        let data_key_id = parse_data_key_id(&bytes)?;
        let dek = self.dek(&data_key_id)?;
        let (_header, map) = decrypt_map(dek.as_ref(), &bytes)?;
        self.file_cache.insert(file.to_string(), map.clone());
        Ok(map)
    }

    pub async fn read_mutable_vault_map(&mut self, file: &str) -> Result<BTreeMap<String, Value>> {
        self.read_vault_map(&mutable_vault_path(file)).await
    }

    /// Field paths in a decrypted vault file (requires the sealing DataKey to be unlocked).
    pub async fn list_vault_field_paths(&mut self, file: &str) -> Result<Vec<String>> {
        let map = self.read_vault_map(file).await?;
        Ok(collect_vault_field_paths(&map))
    }

    /// List every object under `files/` with its sealing DataKey (header only; no unlock).
    pub async fn list_vault_files_with_data_keys(&self) -> Result<Vec<VaultFileKey>> {
        self.ensure_store_ready().await?;
        let objects = self.backend.list("files/").await?;
        let mut out = Vec::with_capacity(objects.len());
        for meta in objects {
            let file = meta
                .key
                .strip_prefix("files/")
                .unwrap_or(meta.key.as_str())
                .to_string();
            let (bytes, _) = self
                .backend
                .get(&meta.key)
                .await?
                .ok_or_else(|| SecretsError::Other(format!("missing vault file {file}")))?;
            let header = vault_header_from_blob(&bytes)?;
            out.push(VaultFileKey {
                file,
                data_key_id: header.data_key_id,
            });
        }
        out.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(out)
    }

    pub async fn resolve_field(&mut self, reference: &VaultRef) -> Result<Value> {
        let map = self.read_vault_map(&reference.file).await?;
        match &reference.field {
            Some(f) => field_from_map(&map, f),
            None => Ok(Value::Map(
                map.into_iter().map(|(k, v)| (Value::Text(k), v)).collect(),
            )),
        }
    }

    pub async fn resolve_field_optional(&mut self, reference: &VaultRef) -> Result<Option<Value>> {
        let key = Self::key_path(&reference.file);
        let Some((bytes, _)) = self.backend.get(&key).await? else {
            return Ok(None);
        };
        let data_key_id = parse_data_key_id(&bytes)?;
        let dek = self.dek(&data_key_id)?;
        let (_header, map) = decrypt_map(dek.as_ref(), &bytes)?;
        match &reference.field {
            Some(f) => Ok(field_from_map(&map, f).ok()),
            None => Ok(Some(Value::Map(
                map.into_iter().map(|(k, v)| (Value::Text(k), v)).collect(),
            ))),
        }
    }

    pub async fn resolve_mutable_field(&mut self, file: &str, field: &str) -> Result<Value> {
        self.resolve_field(&VaultRef::mutable_field(file, field))
            .await
    }

    fn dek(&self, id: &str) -> Result<&Zeroizing<[u8; 32]>> {
        self.unlocked
            .get(id)
            .ok_or_else(|| SecretsError::Locked(id.to_string()))
    }
}

fn parse_data_key_id(blob: &[u8]) -> Result<String> {
    Ok(vault_header_from_blob(blob)?.data_key_id)
}

fn set_field_path(map: &mut BTreeMap<String, Value>, field: &str, value: Value) -> Result<()> {
    let mut parts = field.split('.').peekable();
    let Some(first) = parts.next() else {
        return Err(SecretsError::Format("empty vault field path".into()));
    };
    if first.is_empty() {
        return Err(SecretsError::Format(
            "empty vault field path component".into(),
        ));
    }
    if parts.peek().is_none() {
        map.insert(first.to_string(), value);
        return Ok(());
    }

    let mut cur = map
        .entry(first.to_string())
        .or_insert_with(|| Value::Map(BTreeMap::new()));
    while let Some(part) = parts.next() {
        if part.is_empty() {
            return Err(SecretsError::Format(
                "empty vault field path component".into(),
            ));
        }
        let is_last = parts.peek().is_none();
        match cur {
            Value::Map(m) if is_last => {
                m.insert(Value::Text(part.to_string()), value);
                return Ok(());
            }
            Value::Map(m) => {
                cur = m
                    .entry(Value::Text(part.to_string()))
                    .or_insert_with(|| Value::Map(BTreeMap::new()));
            }
            _ => {
                return Err(SecretsError::Format(format!(
                    "vault field path {field} crosses a non-map value"
                )));
            }
        }
    }
    Ok(())
}
