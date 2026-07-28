//! Decrypt a vault file to YAML, run `$EDITOR`, and save changes.

use crate::unlock::{unlock_data_key, UnlockOpts};
use anyhow::Context;
use infrazeug_migrate::{vault_map_to_yaml, yaml_str_to_vault_map, MigrateError};
use infrazeug_secrets::{FsBackend, VaultStore};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

const EDIT_HEADER: &str = "# Infrazeug vault file (YAML). Save and exit the editor to write back.\n# Root must be a mapping of field names to values.\n\n";

pub async fn vault_edit_file(
    store: PathBuf,
    data_key: &str,
    vault_file: &Path,
    unlock: &UnlockOpts,
) -> anyhow::Result<()> {
    let file = resolve_vault_file_path(&store, vault_file)?;
    let backend = Arc::new(FsBackend::new(&store));
    let mut vault = VaultStore::new(backend, store.clone());
    unlock_data_key(&mut vault, data_key, unlock, "Data key passphrase: ").await?;

    let map = vault.read_vault_map(&file).await?;
    let yaml_body = vault_map_to_yaml(&map).map_err(map_migrate_err)?;
    let before = format!("{EDIT_HEADER}{yaml_body}");

    // Decrypted YAML lives in a private 0700 temp dir so no other local user
    // can read it — including editor swap/backup files, which editors create
    // next to the edited file and which are removed when the dir is dropped.
    let tmp_dir = tempfile::Builder::new()
        .prefix("infrazeug-vault-edit-")
        .tempdir()
        .context("create temp dir for vault edit")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp_dir.path(), std::fs::Permissions::from_mode(0o700))
            .context("chmod vault edit temp dir")?;
    }
    let tmp_path = tmp_dir.path().join("vault.yml");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp_path)
            .context("create temp file for vault edit")?;
        f.write_all(before.as_bytes())?;
    }

    run_editor(&tmp_path).context("editor failed")?;

    let edited = std::fs::read_to_string(&tmp_path).context("read edited vault yaml")?;
    let new_map = yaml_str_to_vault_map(&edited).map_err(map_migrate_err)?;
    if new_map == map {
        println!("vault file unchanged");
        return Ok(());
    }
    vault
        .put_vault_file(data_key, &file, &new_map)
        .await
        .context("save vault file")?;
    println!("updated files/{}", file);
    Ok(())
}

/// Resolve a vault file id: vault-relative (`db/foo.vault`) or a cwd/absolute path under
/// `<store>/files/`.
pub(crate) fn resolve_vault_file_path(store: &Path, file: &Path) -> anyhow::Result<String> {
    if let Some(id) = resolve_vault_file_via_filesystem(store, file)? {
        return Ok(id);
    }
    if looks_like_filesystem_path(file) {
        anyhow::bail!(
            "{} is not a vault file under {}/files/",
            file.display(),
            store.display()
        );
    }
    Ok(normalize_vault_relative_path(file))
}

fn looks_like_filesystem_path(file: &Path) -> bool {
    file.is_absolute()
        || file.components().any(|c| {
            matches!(
                c,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
}

fn resolve_vault_file_via_filesystem(store: &Path, file: &Path) -> anyhow::Result<Option<String>> {
    let candidate = if file.is_absolute() {
        file.to_path_buf()
    } else {
        std::env::current_dir()
            .context("current directory")?
            .join(file)
    };
    if !candidate.exists() {
        return Ok(None);
    }
    let store = store
        .canonicalize()
        .with_context(|| format!("vault store {}", store.display()))?;
    let abs = candidate
        .canonicalize()
        .with_context(|| format!("vault file {}", candidate.display()))?;
    Ok(vault_file_id_under_store(&store, &abs))
}

fn vault_file_id_under_store(store: &Path, abs: &Path) -> Option<String> {
    let files_root = store.join("files");
    let files_root = files_root.canonicalize().ok()?;
    if !abs.starts_with(&files_root) {
        return None;
    }
    let rel = abs.strip_prefix(&files_root).ok()?;
    path_to_vault_file_id(rel)
}

fn path_to_vault_file_id(rel: &Path) -> Option<String> {
    if rel.as_os_str().is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

fn normalize_vault_relative_path(file: &Path) -> String {
    let mut s = file.to_string_lossy().replace('\\', "/");
    while s.starts_with("./") {
        s = s[2..].to_string();
    }
    if let Some(stripped) = s.strip_prefix("files/") {
        s = stripped.to_string();
    }
    s
}

fn map_migrate_err(e: MigrateError) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

fn run_editor(path: &Path) -> std::io::Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let path_str = path.to_string_lossy();
    let status = if editor.contains(' ') {
        Command::new("sh")
            .arg("-c")
            .arg(format!("{editor} \"$1\""))
            .arg("--")
            .arg(path_str.as_ref())
            .status()?
    } else {
        Command::new(&editor).arg(path).status()?
    };
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "editor exited with {status}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    // The cwd is process-global; tests that set it must not run concurrently.
    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, b"placeholder").unwrap();
    }

    #[test]
    fn vault_relative_strips_files_prefix() {
        assert_eq!(
            normalize_vault_relative_path(Path::new("files/db/x.vault")),
            "db/x.vault"
        );
        assert_eq!(
            normalize_vault_relative_path(Path::new("db/x.vault")),
            "db/x.vault"
        );
    }

    #[test]
    fn cwd_path_under_store_files() {
        let _cwd = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = tempdir().unwrap();
        let store = root.path().join("vault");
        let blob = store.join("files/db/x.vault");
        touch(&blob);

        let work = root.path().join("work");
        fs::create_dir_all(&work).unwrap();
        std::env::set_current_dir(&work).unwrap();

        let rel = Path::new("../vault/files/db/x.vault");
        assert_eq!(resolve_vault_file_path(&store, rel).unwrap(), "db/x.vault");
    }

    #[test]
    fn cwd_file_inside_files_subdir() {
        let _cwd = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = tempdir().unwrap();
        let store = root.path().join("vault");
        let blob = store.join("files/db/x.vault");
        touch(&blob);

        let work = store.join("files/db");
        fs::create_dir_all(&work).unwrap();
        std::env::set_current_dir(&work).unwrap();

        assert_eq!(
            resolve_vault_file_path(&store, Path::new("x.vault")).unwrap(),
            "db/x.vault"
        );
    }

    #[test]
    fn explicit_path_outside_store_fails() {
        let _cwd = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = tempdir().unwrap();
        let store = root.path().join("vault");
        fs::create_dir_all(store.join("files")).unwrap();
        let other = root.path().join("other.vault");
        touch(&other);

        std::env::set_current_dir(root.path()).unwrap();
        let err = resolve_vault_file_path(&store, Path::new("../other.vault"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("not a vault file"));
    }

    #[test]
    fn unrelated_cwd_file_falls_back_to_vault_relative() {
        let _cwd = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = tempdir().unwrap();
        let store = root.path().join("vault");
        fs::create_dir_all(store.join("files")).unwrap();
        touch(&root.path().join("db/x.vault"));

        std::env::set_current_dir(root.path()).unwrap();
        assert_eq!(
            resolve_vault_file_path(&store, Path::new("db/x.vault")).unwrap(),
            "db/x.vault"
        );
    }
}
