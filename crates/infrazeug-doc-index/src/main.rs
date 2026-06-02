//! Build a searchable JSON API index from `cargo doc` HTML output.
//!
//! ```bash
//! cargo run -p infrazeug-doc-index
//! # writes crates/infrazeug-mcp/generated/api-docs.json
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Crates indexed for MCP (`infrazeug-api` and common extension crates).
pub const DEFAULT_CRATES: &[&str] = &[
    "infrazeug-api",
    "infrazeug-core",
    "infrazeug-mcp",
    "infrazeug-shell",
    "infrazeug-pull",
    "infrazeug-secrets",
    "infrazeug-tui",
    "infrazeug-templates",
    "infrazeug-emulate",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocIndex {
    pub version: String,
    pub rustdoc_version: Option<String>,
    pub crates: Vec<String>,
    pub items: Vec<DocItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocItem {
    pub id: String,
    pub crate_name: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub summary: String,
    pub doc: String,
}

fn main() -> anyhow::Result<()> {
    let workspace = workspace_root()?;
    let out = workspace.join("crates/infrazeug-mcp/generated/api-docs.json");
    let index = build_index(&workspace, DEFAULT_CRATES)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, serde_json::to_vec_pretty(&index)?)?;
    eprintln!(
        "wrote {} ({} items from {} crates)",
        out.display(),
        index.items.len(),
        index.crates.len()
    );
    Ok(())
}

pub fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .context("expected crates/infrazeug-doc-index under workspace root")
}

pub fn build_index(workspace: &Path, crates: &[&str]) -> anyhow::Result<DocIndex> {
    run_cargo_doc(workspace, crates)?;
    let doc_root = workspace.join("target/doc");
    let mut items = Vec::new();
    for pkg in crates {
        let dir = doc_root.join(pkg.replace('-', "_"));
        if !dir.is_dir() {
            bail!("missing rustdoc output at {}", dir.display());
        }
        items.extend(extract_crate(pkg, &dir)?);
    }
    items.sort_by(|a, b| a.path.cmp(&b.path));
    items.dedup_by(|a, b| a.path == b.path);
    Ok(DocIndex {
        version: env!("CARGO_PKG_VERSION").to_string(),
        rustdoc_version: read_rustdoc_version(
            &doc_root
                .join(crates[0].replace('-', "_"))
                .join("index.html"),
        ),
        crates: crates.iter().map(|s| (*s).to_string()).collect(),
        items,
    })
}

fn run_cargo_doc(workspace: &Path, crates: &[&str]) -> anyhow::Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(workspace).arg("doc").arg("--no-deps");
    for pkg in crates {
        cmd.arg("-p").arg(pkg);
    }
    let status = cmd.status().context("cargo doc failed")?;
    if !status.success() {
        bail!("cargo doc exited with {status}");
    }
    Ok(())
}

fn read_rustdoc_version(index_html: &Path) -> Option<String> {
    let html = fs::read_to_string(index_html).ok()?;
    let document = Html::parse_document(&html);
    let sel = Selector::parse("meta[name='rustdoc-vars']").ok()?;
    document
        .select(&sel)
        .next()?
        .value()
        .attr("data-rustdoc-version")
        .map(str::to_string)
}

fn extract_crate(crate_name: &str, dir: &Path) -> anyhow::Result<Vec<DocItem>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|e| e != "html") {
            continue;
        }
        let rel = path
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if should_skip_path(&rel) {
            continue;
        }
        let html = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        if let Some(item) = parse_item_page(crate_name, &html) {
            out.push(item);
        }
    }
    Ok(out)
}

fn should_skip_path(rel: &str) -> bool {
    rel.starts_with("src/")
        || rel.contains("/trait.impl/")
        || rel == "all.html"
        || rel == "help.html"
        || rel == "settings.html"
        || rel.ends_with("/sidebar-items.js")
}

fn parse_item_page(crate_name: &str, html: &str) -> Option<DocItem> {
    let document = Html::parse_document(html);
    let title = text_of(&document, "title")?;
    if title.contains(" for ") {
        return None;
    }

    let body_class = document
        .select(&Selector::parse("body").ok()?)
        .next()?
        .value()
        .attr("class")?;
    let kind = kind_from_body(body_class)?;

    let (name, path) = parse_title(&title, kind)?;
    let summary = document
        .select(&Selector::parse("meta[name='description']").ok()?)
        .next()
        .and_then(|n| n.value().attr("content"))
        .unwrap_or("")
        .trim()
        .to_string();

    let doc = extract_docblock(&document);

    Some(DocItem {
        id: path.clone(),
        crate_name: crate_name.to_string(),
        kind: kind.to_string(),
        name,
        path,
        summary,
        doc,
    })
}

fn kind_from_body(body_class: &str) -> Option<&'static str> {
    if body_class.contains("rustdoc mod crate") {
        return Some("crate");
    }
    for (token, kind) in [
        ("rustdoc struct", "struct"),
        ("rustdoc enum", "enum"),
        ("rustdoc trait", "trait"),
        ("rustdoc fn", "fn"),
        ("rustdoc type", "type"),
        ("rustdoc constant", "constant"),
        ("rustdoc macro", "macro"),
        ("rustdoc mod", "module"),
    ] {
        if body_class.contains(token) {
            return Some(kind);
        }
    }
    None
}

fn parse_title(title: &str, kind: &str) -> Option<(String, String)> {
    let title = title.strip_suffix(" - Rust")?;
    if kind == "crate" {
        let name = title.strip_prefix("Crate ")?.replace('_', "-");
        let path = title.strip_prefix("Crate ")?.to_string();
        return Some((name, path));
    }
    if let Some((name, path)) = title.split_once(" in ") {
        let name = name.trim().to_string();
        let path = path.trim().to_string();
        let full = format!("{path}::{name}");
        return Some((name, full));
    }
    if title.contains("::") {
        let name = title.rsplit("::").next()?.to_string();
        return Some((name, title.to_string()));
    }
    None
}

fn extract_docblock(document: &Html) -> String {
    let sel = match Selector::parse("#main-content .docblock") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let mut parts = Vec::new();
    for node in document.select(&sel).take(6) {
        let text = node.text().collect::<Vec<_>>().join(" ");
        let text = normalize_ws(&text);
        if !text.is_empty() {
            parts.push(text);
        }
    }
    parts.join("\n\n")
}

fn text_of(document: &Html, tag: &str) -> Option<String> {
    let sel = Selector::parse(tag).ok()?;
    let text = document.select(&sel).next()?.text().collect::<String>();
    let text = normalize_ws(&text);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_struct_title() {
        let (name, path) = parse_title("RunConfig in infrazeug_api::cli - Rust", "struct").unwrap();
        assert_eq!(name, "RunConfig");
        assert_eq!(path, "infrazeug_api::cli::RunConfig");
    }

    #[test]
    fn parse_module_title() {
        let (name, path) = parse_title("infrazeug_api::cli - Rust", "module").unwrap();
        assert_eq!(name, "cli");
        assert_eq!(path, "infrazeug_api::cli");
    }
}
