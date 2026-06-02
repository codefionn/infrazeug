//! List field names inside each vault file (requires DataKey unlock).

use crate::unlock::{unlock_data_key, UnlockOpts};
use anyhow::Context;
use infrazeug_secrets::{FsBackend, VaultStore};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

pub async fn vault_show_keys(
    store: PathBuf,
    data_key: Option<String>,
    unlock: &UnlockOpts,
) -> anyhow::Result<()> {
    let backend = Arc::new(FsBackend::new(&store));
    let mut vault = VaultStore::new(backend, store);
    let files = vault
        .list_vault_files_with_data_keys()
        .await
        .context("list vault files")?;

    if files.is_empty() {
        println!("no vault files under files/");
        return Ok(());
    }

    let keys_to_unlock: Vec<String> = if let Some(ref dk) = data_key {
        vec![dk.clone()]
    } else {
        files
            .iter()
            .map(|f| f.data_key_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };

    for dk in keys_to_unlock {
        unlock_data_key(
            &mut vault,
            &dk,
            unlock,
            &format!("Data key {dk} passphrase: "),
        )
        .await
        .with_context(|| format!("unlock data key {dk}"))?;
    }

    let filter_key = data_key.as_deref();
    let mut any = false;
    for entry in &files {
        if filter_key.is_some_and(|want| want != entry.data_key_id) {
            continue;
        }
        any = true;
        println!("{}:", entry.file);
        if !vault.is_unlocked(&entry.data_key_id) {
            println!(
                "  (locked — unlock data key {} to list fields)",
                entry.data_key_id
            );
            continue;
        }
        let paths = vault
            .list_vault_field_paths(&entry.file)
            .await
            .with_context(|| format!("read {}", entry.file))?;
        if paths.is_empty() {
            println!("  (empty)");
        } else {
            for path in paths {
                println!("  {path}");
            }
        }
    }

    if !any {
        anyhow::bail!("no vault files for data key {}", filter_key.unwrap_or(""));
    }
    Ok(())
}
