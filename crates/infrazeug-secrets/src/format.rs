use crate::error::{Result, SecretsError};
use crate::store_format::VAULT_FILE_VERSION;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_cbor::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MAGIC: &[u8; 8] = b"INFRZVLT";
/// Current vault-file version byte; alias of [`crate::store_format::VAULT_FILE_VERSION`].
pub const VERSION: u8 = VAULT_FILE_VERSION;
const MIN_SUPPORTED_VERSION: u8 = 0x01;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VaultHeader {
    pub data_key_id: String,
    pub content_type: String,
    pub nonce: [u8; 24],
    pub aad_hash: [u8; 32],
    pub file_salt: [u8; 32],
}

pub fn canonical_header_bytes(header: &VaultHeader) -> Result<Vec<u8>> {
    Ok(serde_cbor::to_vec(header)?)
}

pub fn header_aad_hash(header: &VaultHeader) -> [u8; 32] {
    let mut h = header.clone();
    h.aad_hash = [0; 32];
    let bytes = serde_cbor::to_vec(&h).expect("header cbor");
    Sha256::digest(&bytes).into()
}

pub fn encrypt_map(
    dek: &[u8],
    data_key_id: &str,
    plaintext: &BTreeMap<String, Value>,
) -> Result<Vec<u8>> {
    if dek.len() != 32 {
        return Err(SecretsError::Format("dek must be 32 bytes".into()));
    }
    let mut file_salt = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut file_salt);
    let mut nonce = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce);

    let mut header = VaultHeader {
        data_key_id: data_key_id.to_string(),
        content_type: "application/cbor".into(),
        nonce,
        aad_hash: [0; 32],
        file_salt,
    };
    header.aad_hash = header_aad_hash(&header);

    let body_plain = serde_cbor::to_vec(plaintext)?;
    let key = Key::from_slice(dek);
    let cipher = XChaCha20Poly1305::new(key);
    let aad_final = canonical_header_bytes(&header)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&header.nonce),
            chacha20poly1305::aead::Payload {
                msg: &body_plain,
                aad: &aad_final,
            },
        )
        .map_err(|_| SecretsError::Decrypt)?;

    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    let header_cbor = serde_cbor::to_vec(&header)?;
    out.extend_from_slice(&(header_cbor.len() as u32).to_be_bytes());
    out.extend_from_slice(&header_cbor);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn parse_vault_header_cbor(blob: &[u8]) -> Result<VaultHeader> {
    if blob.len() < 4 {
        return Err(SecretsError::Format("truncated header".into()));
    }
    let hlen = u32::from_be_bytes(blob[0..4].try_into().unwrap()) as usize;
    if blob.len() < 4 + hlen {
        return Err(SecretsError::Format("truncated header".into()));
    }
    serde_cbor::from_slice(&blob[4..4 + hlen]).map_err(|e| SecretsError::Format(e.to_string()))
}

/// Parse header from a full vault file blob (magic + version + header + ciphertext).
pub fn vault_header_from_blob(blob: &[u8]) -> Result<VaultHeader> {
    if blob.len() < 8 + 1 + 4 {
        return Err(SecretsError::Format("too short".into()));
    }
    if &blob[0..8] != MAGIC {
        return Err(SecretsError::Format("bad magic".into()));
    }
    let ver = blob[8];
    if !(MIN_SUPPORTED_VERSION..=VERSION).contains(&ver) {
        return Err(SecretsError::Format(format!(
            "unsupported vault file version {ver} (latest {VERSION})"
        )));
    }
    parse_vault_header_cbor(&blob[9..])
}

pub fn decrypt_map(dek: &[u8], blob: &[u8]) -> Result<(VaultHeader, BTreeMap<String, Value>)> {
    if dek.len() != 32 {
        return Err(SecretsError::Format("dek must be 32 bytes".into()));
    }
    if blob.len() < 8 + 1 + 4 {
        return Err(SecretsError::Format("too short".into()));
    }
    if &blob[0..8] != MAGIC {
        return Err(SecretsError::Format("bad magic".into()));
    }
    let ver = blob[8];
    if !(MIN_SUPPORTED_VERSION..=VERSION).contains(&ver) {
        return Err(SecretsError::Format(format!(
            "unsupported vault file version {ver} (latest {VERSION})"
        )));
    }
    let header = parse_vault_header_cbor(&blob[9..])?;
    let hlen = u32::from_be_bytes(blob[9..13].try_into().unwrap()) as usize;
    let expected = header_aad_hash(&header);
    if expected != header.aad_hash {
        return Err(SecretsError::Format("aad tamper".into()));
    }
    let ct = &blob[13 + hlen..];
    let key = Key::from_slice(dek);
    let cipher = XChaCha20Poly1305::new(key);
    let aad = canonical_header_bytes(&header)?;
    let plain = cipher
        .decrypt(
            XNonce::from_slice(&header.nonce),
            chacha20poly1305::aead::Payload { msg: ct, aad: &aad },
        )
        .map_err(|_| SecretsError::Decrypt)?;
    let map: BTreeMap<String, Value> = serde_cbor::from_slice(&plain)?;
    Ok((header, map))
}

/// Dotted field paths for every leaf value in a decrypted vault map (e.g. `password`, `db.host`).
pub fn collect_vault_field_paths(map: &BTreeMap<String, Value>) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in map {
        collect_vault_field_paths_value(v, k, &mut out);
    }
    out.sort();
    out
}

fn collect_vault_field_paths_value(value: &Value, prefix: &str, out: &mut Vec<String>) {
    match value {
        Value::Map(m) => {
            for (k, v) in m {
                if let Value::Text(key) = k {
                    let path = format!("{prefix}.{key}");
                    collect_vault_field_paths_value(v, &path, out);
                }
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let path = format!("{prefix}.{i}");
                collect_vault_field_paths_value(item, &path, out);
            }
        }
        _ => out.push(prefix.to_string()),
    }
}

pub fn field_from_map(map: &BTreeMap<String, Value>, field: &str) -> Result<Value> {
    let mut cur = Value::Map(
        map.iter()
            .map(|(k, v)| (Value::Text(k.clone()), v.clone()))
            .collect(),
    );
    for part in field.split('.') {
        cur = match cur {
            Value::Map(ref m) => m
                .iter()
                .find(|(k, _)| matches!(k, Value::Text(s) if s == part))
                .map(|(_, v)| v.clone())
                .ok_or_else(|| SecretsError::Format(format!("missing field {part}")))?,
            _ => return Err(SecretsError::Format("not a map".into())),
        };
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_field_paths_nested() {
        let mut nested = BTreeMap::new();
        nested.insert(Value::Text("host".into()), Value::Text("db".into()));
        let mut m = BTreeMap::new();
        m.insert("password".into(), Value::Text("secret".into()));
        m.insert(
            "db".into(),
            Value::Map(
                nested
                    .into_iter()
                    .collect::<std::collections::BTreeMap<_, _>>(),
            ),
        );
        assert_eq!(
            collect_vault_field_paths(&m),
            vec!["db.host".to_string(), "password".to_string()]
        );
    }

    #[test]
    fn roundtrip_encrypt() {
        let dek = [7u8; 32];
        let mut m = BTreeMap::new();
        m.insert("password".into(), Value::Text("secret".into()));
        let blob = encrypt_map(&dek, "prod", &m).unwrap();
        let (_, out) = decrypt_map(&dek, &blob).unwrap();
        assert_eq!(out.get("password"), Some(&Value::Text("secret".into())));
    }
}
