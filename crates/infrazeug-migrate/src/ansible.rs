use crate::error::MigrateError;
use crate::yaml::yaml_mapping_to_vault_map;
use ansible_vault::decrypt_vault;
use infrazeug_secrets::VaultStore;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

const VAULT_MAGIC: &str = "$ANSIBLE_VAULT";

/// Options for [`migrate_ansible_vault`].
#[derive(Clone, Debug)]
pub struct AnsibleVaultMigrateOptions {
    /// Ansible vault password (plaintext).
    pub ansible_passphrase: String,
    /// Infrazeug data key id (must already be unlocked on `store`).
    pub data_key: String,
    /// Prefix for output vault file paths inside the store (`files/<prefix><name>`).
    pub out_prefix: String,
    /// When set, print actions without writing.
    pub dry_run: bool,
}

#[derive(Clone, Debug)]
pub struct MigratedFile {
    pub source: PathBuf,
    pub vault_file: String,
    pub field_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct MigrateReport {
    pub migrated: Vec<MigratedFile>,
    pub skipped: Vec<(PathBuf, String)>,
}

/// Decrypt one Ansible Vault file and write it into an infrazeug vault store.
pub fn decrypt_ansible_vault_file(
    path: &Path,
    ansible_passphrase: &str,
) -> Result<BTreeMap<String, serde_cbor::Value>, MigrateError> {
    let raw = std::fs::read_to_string(path)?;
    let trimmed = raw.trim_start();
    if !trimmed.starts_with(VAULT_MAGIC) {
        return Err(MigrateError::Other(format!(
            "{} is not an ansible vault file (missing {VAULT_MAGIC} header)",
            path.display()
        )));
    }
    let plain = decrypt_vault(raw.as_bytes(), ansible_passphrase)
        .map_err(|e| MigrateError::AnsibleDecrypt(e.to_string()))?;
    let yaml: serde_yaml::Value =
        serde_yaml::from_slice(&plain).map_err(|e| MigrateError::Yaml(e.to_string()))?;
    yaml_mapping_to_vault_map(yaml)
}

#[cfg(test)]
pub fn out_vault_path(source: &Path, out_prefix: &str) -> String {
    out_vault_path_relative(source, out_prefix)
}

fn out_vault_path_for_source(source: &Path, root: &Path, out_prefix: &str) -> String {
    let rel = source.strip_prefix(root).unwrap_or(source);
    out_vault_path_relative(rel, out_prefix)
}

fn out_vault_path_relative(source: &Path, out_prefix: &str) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_os_string().into_string().ok())
        .unwrap_or_else(|| "vault".into());
    let mut rel_parts = source
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .components()
        .filter_map(normal_component_str)
        .collect::<Vec<_>>();
    rel_parts.push(stem);
    let mut rel = rel_parts.join("/");
    rel = rel.replace('\\', "/");
    if rel.ends_with(".vault") {
        rel.truncate(rel.len() - ".vault".len());
    }
    let prefix = out_prefix
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect::<Vec<_>>()
        .join("/");
    if prefix.is_empty() {
        format!("{rel}.vault")
    } else {
        format!("{prefix}/{rel}.vault")
    }
}

fn normal_component_str(component: Component<'_>) -> Option<String> {
    match component {
        Component::Normal(part) => part.to_str().map(|s| s.to_string()),
        Component::Prefix(_) | Component::RootDir | Component::CurDir | Component::ParentDir => {
            None
        }
    }
}

/// Migrate one file or every `*vault*.yml` under a directory tree.
pub async fn migrate_ansible_vault(
    store: &mut VaultStore,
    opts: &AnsibleVaultMigrateOptions,
    from: &Path,
) -> Result<MigrateReport, MigrateError> {
    if !store.is_unlocked(&opts.data_key) {
        return Err(MigrateError::Other(format!(
            "data key `{}` is not unlocked on the vault store",
            opts.data_key
        )));
    }

    let source_root = if from.is_file() {
        from.parent().unwrap_or_else(|| Path::new(""))
    } else {
        from
    };
    let mut report = MigrateReport::default();
    let sources: Vec<PathBuf> = if from.is_file() {
        vec![from.to_path_buf()]
    } else if from.is_dir() {
        WalkDir::new(from)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .filter(|p| is_ansible_vault_candidate(p))
            .collect()
    } else {
        return Err(MigrateError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{} not found", from.display()),
        )));
    };

    if sources.is_empty() {
        return Err(MigrateError::Other(format!(
            "no ansible vault files found under {}",
            from.display()
        )));
    }

    for source in sources {
        let vault_file = out_vault_path_for_source(&source, source_root, &opts.out_prefix);
        match decrypt_ansible_vault_file(&source, &opts.ansible_passphrase) {
            Ok(map) => {
                let field_count = map.len();
                if opts.dry_run {
                    report.migrated.push(MigratedFile {
                        source,
                        vault_file,
                        field_count,
                    });
                    continue;
                }
                store
                    .put_vault_file(&opts.data_key, &vault_file, &map)
                    .await?;
                report.migrated.push(MigratedFile {
                    source,
                    vault_file,
                    field_count,
                });
            }
            Err(e) => report.skipped.push((source, e.to_string())),
        }
    }

    Ok(report)
}

fn is_ansible_vault_candidate(path: &Path) -> bool {
    let ext_ok = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e, "yml" | "yaml"));
    if !ext_ok {
        return false;
    }
    let Ok(raw) = std::fs::read(path) else {
        return false;
    };
    raw.starts_with(b"$ANSIBLE_VAULT")
}

pub fn read_passphrase_file(path: &Path) -> Result<String, MigrateError> {
    let s = std::fs::read_to_string(path)?;
    Ok(s.lines().next().unwrap_or("").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ansible_vault::encrypt_vault;
    use infrazeug_secrets::{FsBackend, VaultStore};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn out_path_preserves_relative_dirs() {
        let p = Path::new("group_vars/global/vault.yml");
        assert_eq!(
            out_vault_path(p, "ansible/"),
            "ansible/group_vars/global/vault.vault"
        );
    }

    #[test]
    fn out_path_strips_migration_root_for_absolute_sources() {
        let root = Path::new("/tmp/in");
        let p = root.join("group_vars/global/vault.yml");
        assert_eq!(
            out_vault_path_for_source(&p, root, "ansible/"),
            "ansible/group_vars/global/vault.vault"
        );
    }

    #[test]
    fn out_path_for_single_file_uses_filename_only() {
        let root = Path::new("/tmp/in");
        let p = root.join("secrets.yml");
        assert_eq!(
            out_vault_path_for_source(&p, root, "ansible/"),
            "ansible/secrets.vault"
        );
    }

    #[test]
    fn out_path_never_emits_absolute_backend_key() {
        assert_eq!(
            out_vault_path(Path::new("/tmp/in/secrets.yml"), "/ansible/"),
            "ansible/tmp/in/secrets.vault"
        );
    }

    #[tokio::test]
    async fn roundtrip_into_store() {
        let dir = tempdir().unwrap();
        let ansible_path = dir.path().join("secrets.yml");
        let yaml = "db:\n  password: secret\n";
        let enc = encrypt_vault(yaml.as_bytes(), "ansible-demo").unwrap();
        std::fs::write(&ansible_path, enc).unwrap();

        let store_dir = dir.path().join("store");
        let backend = Arc::new(FsBackend::new(&store_dir));
        let mut store = VaultStore::new(backend, store_dir);
        store
            .keygen_passphrase("prod", "infra-demo", "recovery")
            .await
            .unwrap();

        let report = migrate_ansible_vault(
            &mut store,
            &AnsibleVaultMigrateOptions {
                ansible_passphrase: "ansible-demo".into(),
                data_key: "prod".into(),
                out_prefix: "ansible/".into(),
                dry_run: false,
            },
            &ansible_path,
        )
        .await
        .unwrap();

        assert_eq!(report.migrated.len(), 1);
        let map = store
            .read_vault_map(&report.migrated[0].vault_file)
            .await
            .unwrap();
        let pw = map
            .get("db")
            .and_then(|v| match v {
                serde_cbor::Value::Map(m) => m
                    .iter()
                    .find(|(k, _)| matches!(k, serde_cbor::Value::Text(s) if s == "password"))
                    .map(|(_, v)| v.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(pw, serde_cbor::Value::Text("secret".into()));
    }
}
