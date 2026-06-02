//! Locate an infrazeug playbook crate from `Cargo.toml`.

use anyhow::Context;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct PlaybookProject {
    pub manifest_dir: PathBuf,
    pub package_name: String,
    pub bin_name: String,
}

/// Find a playbook crate in `cwd` (or ancestors up to 3 levels).
pub fn discover_playbook(cwd: impl AsRef<Path>) -> anyhow::Result<Option<PlaybookProject>> {
    let mut dir = cwd.as_ref().to_path_buf();
    for _ in 0..4 {
        if let Some(project) = try_discover_in(&dir)? {
            return Ok(Some(project));
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(None)
}

fn try_discover_in(manifest_dir: &Path) -> anyhow::Result<Option<PlaybookProject>> {
    let manifest = manifest_dir.join("Cargo.toml");
    if !manifest.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(&manifest).with_context(|| manifest.display().to_string())?;
    let doc: toml::Table = toml::from_str(&text).context("parse Cargo.toml")?;
    if !depends_on_infrazeug(&doc) {
        return Ok(None);
    }
    let package_name = doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .context("Cargo.toml missing [package].name")?
        .to_string();
    let bin_name = pick_bin_name(&doc, &package_name)?;
    Ok(Some(PlaybookProject {
        manifest_dir: manifest_dir.to_path_buf(),
        package_name,
        bin_name,
    }))
}

fn depends_on_infrazeug(doc: &toml::Table) -> bool {
    let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) else {
        return false;
    };
    deps.contains_key("infrazeug-api")
        || deps.contains_key("infrazeug_core")
        || deps.contains_key("infrazeug-core")
}

fn pick_bin_name(doc: &toml::Table, package_name: &str) -> anyhow::Result<String> {
    if let Some(bins) = doc.get("bin").and_then(|b| b.as_array()) {
        for entry in bins {
            let Some(table) = entry.as_table() else {
                continue;
            };
            if let Some(name) = table.get("name").and_then(|n| n.as_str()) {
                return Ok(name.to_string());
            }
        }
    }
    if let Some(default_bin) = doc
        .get("package")
        .and_then(|p| p.get("default-run"))
        .and_then(|n| n.as_str())
    {
        return Ok(default_bin.to_string());
    }
    Ok(package_name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_infra_infrazeug() {
        let infra_infrazeug =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../infra-infrazeug");
        if !infra_infrazeug.join("Cargo.toml").is_file() {
            return;
        }
        let found = discover_playbook(&infra_infrazeug)
            .unwrap()
            .expect("playbook");
        assert_eq!(found.bin_name, "infra");
    }
}
