use crate::error::{Result, ShellError};
use crate::op::SyncDirOptions;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncDirPlan {
    pub entries: Vec<SyncDirEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncDirEntry {
    Dir {
        rel: PathBuf,
        mode: u32,
    },
    File {
        rel: PathBuf,
        mode: u32,
        hard_link_to: Option<PathBuf>,
    },
    Symlink {
        rel: PathBuf,
        target: PathBuf,
    },
}

pub fn plan_sync_dir(src: &Path, options: &SyncDirOptions) -> Result<SyncDirPlan> {
    validate_sync_source(src)?;
    let mut entries = Vec::new();
    let mut hard_links = HardLinkTracker::default();
    visit_dir(src, src, options, &mut hard_links, &mut entries)?;
    entries.sort_by_key(entry_sort_key);
    Ok(SyncDirPlan { entries })
}

pub fn sync_dir_to_local(src: &Path, dest: &Path, options: &SyncDirOptions) -> Result<usize> {
    let plan = plan_sync_dir(src, options)?;
    if options.delete {
        remove_path_if_exists(dest)?;
    }
    std::fs::create_dir_all(dest)?;
    for entry in &plan.entries {
        match entry {
            SyncDirEntry::Dir { rel, mode } => {
                let path = dest.join(rel);
                std::fs::create_dir_all(&path)?;
                set_mode(&path, *mode)?;
            }
            SyncDirEntry::File {
                rel,
                mode,
                hard_link_to,
            } => {
                let path = dest.join(rel);
                ensure_parent(&path)?;
                if let Some(link_to) = hard_link_to {
                    remove_path_if_exists(&path)?;
                    std::fs::hard_link(dest.join(link_to), &path)?;
                } else {
                    std::fs::copy(src.join(rel), &path)?;
                    set_mode(&path, *mode)?;
                }
            }
            SyncDirEntry::Symlink { rel, target } => {
                let path = dest.join(rel);
                ensure_parent(&path)?;
                remove_path_if_exists(&path)?;
                create_symlink(target, &path)?;
            }
        }
    }
    Ok(plan.entries.len())
}

/// Pack a sync plan into a tar archive so transports can transfer the whole
/// tree in one upload and unpack it with a single remote `tar -xpf` (or a
/// single agent op) instead of one roundtrip per entry. Entry order follows
/// the plan (dirs first, then files/symlinks sorted by path), so hard-link
/// targets always precede their links.
pub fn pack_sync_plan(src: &Path, plan: &SyncDirPlan) -> Result<Vec<u8>> {
    let mtime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut builder = tar::Builder::new(Vec::new());
    for entry in &plan.entries {
        let mut header = tar::Header::new_gnu();
        header.set_mtime(mtime);
        header.set_uid(0);
        header.set_gid(0);
        match entry {
            SyncDirEntry::Dir { rel, mode } => {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(*mode);
                header.set_size(0);
                builder.append_data(&mut header, rel, std::io::empty())?;
            }
            SyncDirEntry::File {
                rel,
                mode,
                hard_link_to,
            } => {
                header.set_mode(*mode);
                if let Some(link_to) = hard_link_to {
                    header.set_entry_type(tar::EntryType::Link);
                    header.set_size(0);
                    builder.append_link(&mut header, rel, link_to)?;
                } else {
                    let file = std::fs::File::open(src.join(rel))?;
                    header.set_size(file.metadata()?.len());
                    builder.append_data(&mut header, rel, file)?;
                }
            }
            SyncDirEntry::Symlink { rel, target } => {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_mode(0o777);
                header.set_size(0);
                builder.append_link(&mut header, rel, target)?;
            }
        }
    }
    Ok(builder.into_inner()?)
}

pub fn validate_sync_source(src: &Path) -> Result<()> {
    let meta = std::fs::metadata(src)?;
    if !meta.is_dir() {
        return Err(ShellError::Other(format!(
            "sync source is not a directory: {}",
            src.display()
        )));
    }
    Ok(())
}

fn visit_dir(
    src: &Path,
    dir: &Path,
    options: &SyncDirOptions,
    hard_links: &mut HardLinkTracker,
    entries: &mut Vec<SyncDirEntry>,
) -> Result<()> {
    let mut children = std::fs::read_dir(dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.path());
    for child in children {
        let path = child.path();
        let rel = path.strip_prefix(src).map_err(|e| {
            ShellError::Other(format!(
                "sync source path {} is not under {}: {e}",
                path.display(),
                src.display()
            ))
        })?;
        let rel = rel.to_path_buf();
        let meta = std::fs::symlink_metadata(&path)?;
        let file_type = meta.file_type();
        if file_type.is_dir() {
            entries.push(SyncDirEntry::Dir {
                rel: rel.clone(),
                mode: mode(&meta, 0o755),
            });
            visit_dir(src, &path, options, hard_links, entries)?;
        } else if file_type.is_symlink() {
            entries.push(SyncDirEntry::Symlink {
                rel,
                target: std::fs::read_link(&path)?,
            });
        } else if file_type.is_file() {
            let hard_link_to = if options.hard_links {
                hard_links.link_target(&meta, &rel)
            } else {
                None
            };
            entries.push(SyncDirEntry::File {
                rel,
                mode: mode(&meta, 0o644),
                hard_link_to,
            });
        }
    }
    Ok(())
}

fn entry_sort_key(entry: &SyncDirEntry) -> (u8, PathBuf) {
    match entry {
        SyncDirEntry::Dir { rel, .. } => (0, rel.clone()),
        SyncDirEntry::File { rel, .. } | SyncDirEntry::Symlink { rel, .. } => (1, rel.clone()),
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => std::fs::remove_dir_all(path)?,
        Ok(_) => std::fs::remove_file(path)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

#[cfg(unix)]
fn mode(meta: &std::fs::Metadata, _default: u32) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode(_meta: &std::fs::Metadata, default: u32) -> u32 {
    default
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, link: &Path) -> Result<()> {
    Err(ShellError::Other(format!(
        "symlink sync is not supported on this platform: {}",
        link.display()
    )))
}

#[cfg(unix)]
#[derive(Default)]
struct HardLinkTracker {
    seen: HashMap<(u64, u64), PathBuf>,
}

#[cfg(unix)]
impl HardLinkTracker {
    fn link_target(&mut self, meta: &std::fs::Metadata, rel: &Path) -> Option<PathBuf> {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() <= 1 {
            return None;
        }
        let key = (meta.dev(), meta.ino());
        if let Some(first) = self.seen.get(&key) {
            return Some(first.clone());
        }
        self.seen.insert(key, rel.to_path_buf());
        None
    }
}

#[cfg(not(unix))]
#[derive(Default)]
struct HardLinkTracker;

#[cfg(not(unix))]
impl HardLinkTracker {
    fn link_target(&mut self, _meta: &std::fs::Metadata, _rel: &Path) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_directory_contents() {
        let root =
            std::env::temp_dir().join(format!("infrazeug-sync-src-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/file.txt"), b"hello").unwrap();

        let plan = plan_sync_dir(&root, &SyncDirOptions::default()).unwrap();

        assert!(plan.entries.iter().any(|entry| {
            matches!(entry, SyncDirEntry::Dir { rel, .. } if rel == Path::new("sub"))
        }));
        assert!(plan.entries.iter().any(|entry| {
            matches!(entry, SyncDirEntry::File { rel, .. } if rel == Path::new("sub/file.txt"))
        }));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn packs_plan_into_extractable_tar() {
        let src = std::env::temp_dir().join(format!("infrazeug-pack-src-{}", uuid::Uuid::new_v4()));
        let dest =
            std::env::temp_dir().join(format!("infrazeug-pack-dest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub/file.txt"), b"hello").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("sub/file.txt", src.join("link")).unwrap();

        let plan = plan_sync_dir(&src, &SyncDirOptions::default()).unwrap();
        let data = pack_sync_plan(&src, &plan).unwrap();

        std::fs::create_dir_all(&dest).unwrap();
        tar::Archive::new(data.as_slice()).unpack(&dest).unwrap();
        assert_eq!(std::fs::read(dest.join("sub/file.txt")).unwrap(), b"hello");
        #[cfg(unix)]
        assert_eq!(std::fs::read(dest.join("link")).unwrap(), b"hello");
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(dest);
    }

    /// The agentless fast path extracts with the remote host's `tar -xpf`;
    /// make sure the archive we build is accepted by the system tar, not just
    /// the `tar` crate reader.
    #[test]
    #[cfg(unix)]
    fn packed_tar_extracts_with_system_tar() {
        if !std::process::Command::new("tar")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
        {
            return;
        }
        let src = std::env::temp_dir().join(format!("infrazeug-tar-src-{}", uuid::Uuid::new_v4()));
        let dest =
            std::env::temp_dir().join(format!("infrazeug-tar-dest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub/file.txt"), b"hello").unwrap();
        std::os::unix::fs::symlink("sub/file.txt", src.join("link")).unwrap();
        std::fs::hard_link(src.join("sub/file.txt"), src.join("hard.txt")).unwrap();

        let options = SyncDirOptions {
            delete: false,
            hard_links: true,
        };
        let plan = plan_sync_dir(&src, &options).unwrap();
        let archive = dest.join("plan.tar");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(&archive, pack_sync_plan(&src, &plan).unwrap()).unwrap();

        let status = std::process::Command::new("tar")
            .arg("-xpf")
            .arg(&archive)
            .arg("-C")
            .arg(&dest)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(std::fs::read(dest.join("sub/file.txt")).unwrap(), b"hello");
        assert_eq!(std::fs::read(dest.join("link")).unwrap(), b"hello");
        use std::os::unix::fs::MetadataExt;
        let original = std::fs::metadata(dest.join("sub/file.txt")).unwrap();
        let hard = std::fs::metadata(dest.join("hard.txt")).unwrap();
        assert_eq!(original.ino(), hard.ino());
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(dest);
    }

    #[test]
    fn syncs_directory_to_local_destination() {
        let src = std::env::temp_dir().join(format!("infrazeug-sync-src-{}", uuid::Uuid::new_v4()));
        let dest =
            std::env::temp_dir().join(format!("infrazeug-sync-dest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub/file.txt"), b"hello").unwrap();

        let count = sync_dir_to_local(&src, &dest, &SyncDirOptions::default()).unwrap();

        assert_eq!(std::fs::read(dest.join("sub/file.txt")).unwrap(), b"hello");
        assert!(count >= 2);
        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(dest);
    }
}
