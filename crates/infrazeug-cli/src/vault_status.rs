//! Summarize vault store layout without unlocking DataKeys.

use anyhow::Context;
use infrazeug_secrets::{FsBackend, VaultStore};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn vault_status(store: PathBuf, data_key: Option<String>) -> anyhow::Result<()> {
    let backend = Arc::new(FsBackend::new(&store));
    let vault = VaultStore::new(backend, store.clone());

    println!("store: {}", store.display());
    let format = vault
        .store_format_version()
        .await
        .context("read store metadata")?;
    match format {
        Some(v) => println!("format: {v}"),
        None => println!("format: (no meta/store.cbor)"),
    }

    let mut keys = vault.list_data_keys().await.context("list data keys")?;
    if let Some(ref want) = data_key {
        if !keys.iter().any(|id| id == want) {
            anyhow::bail!("data key {want} not found in {}", store.display());
        }
        keys.retain(|id| id == want);
    }

    let files = vault
        .list_vault_files_with_data_keys()
        .await
        .context("list vault files")?;
    let mut files_by_key: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for entry in &files {
        if data_key
            .as_ref()
            .is_some_and(|want| want != &entry.data_key_id)
        {
            continue;
        }
        files_by_key
            .entry(entry.data_key_id.as_str())
            .or_default()
            .push(entry.file.as_str());
    }

    if keys.is_empty() {
        println!("data keys: (none)");
        if files.is_empty() {
            println!("vault files: (none)");
        } else {
            println!("vault files: {}", files.len());
            for entry in &files {
                println!("  {} [{}]", entry.file, entry.data_key_id);
            }
        }
        return Ok(());
    }

    println!("data keys: {}", keys.len());
    for id in &keys {
        let state = if vault.is_unlocked(id) {
            "unlocked"
        } else {
            "locked"
        };
        println!();
        println!("{id} ({state})");
        let recipients = vault
            .list_recipients(id)
            .await
            .with_context(|| format!("list recipients for data key {id}"))?;
        if recipients.is_empty() {
            println!("  recipients: (none)");
        } else {
            println!("  recipients (* = default):");
            for (i, (kind, label)) in recipients.iter().enumerate() {
                let marker = if i == 0 { '*' } else { ' ' };
                println!("    {marker} {label} [{kind:?}]");
            }
        }
        if files_by_key
            .get(id.as_str())
            .is_none_or(|files| files.is_empty())
        {
            println!("  vault files: (none)");
        } else if let Some(files) = files_by_key.get(id.as_str()) {
            println!("  vault files ({}):", files.len());
            for file in files {
                println!("    {file}");
            }
        }
    }

    let orphan_files: Vec<_> = files
        .iter()
        .filter(|e| !keys.iter().any(|id| id == &e.data_key_id))
        .filter(|e| data_key.as_ref().is_none_or(|want| want == &e.data_key_id))
        .collect();
    if !orphan_files.is_empty() {
        println!();
        println!("vault files with unknown data key:");
        for entry in orphan_files {
            println!("  {} [{}]", entry.file, entry.data_key_id);
        }
    }

    Ok(())
}
