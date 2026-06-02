//! Embedded rustdoc index (`cargo doc` → `infrazeug-doc-index`) for MCP search.

use std::sync::{LazyLock, Once};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// JSON index written by `infrazeug-doc-index` (see `generated/api-docs.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocIndex {
    pub version: String,
    pub rustdoc_version: Option<String>,
    pub crates: Vec<String>,
    pub items: Vec<DocItem>,
}

/// One public API item from rustdoc HTML.
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

/// Compact row for browse/search listing (no full `doc` body).
#[derive(Debug, Clone, Serialize)]
pub struct DocItemSummary {
    pub id: String,
    pub crate_name: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocSearchHit {
    pub score: u32,
    pub id: String,
    pub crate_name: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub summary: String,
    pub doc: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocSearchResult {
    pub query: String,
    pub limit: usize,
    pub total_matches: usize,
    pub hits: Vec<DocSearchHit>,
}

static INDEX: LazyLock<DocIndex> = LazyLock::new(|| {
    let raw = include_str!(concat!(env!("OUT_DIR"), "/api-docs.json"));
    serde_json::from_str(raw).unwrap_or_else(|e| {
        panic!("invalid embedded api-docs.json: {e}");
    })
});

struct SearchIndex {
    entries: Vec<SearchEntry>,
}

struct SearchEntry {
    item_idx: usize,
    path_l: String,
    name_l: String,
    summary_l: String,
    doc_l: String,
}

static SEARCH_INDEX: LazyLock<SearchIndex> = LazyLock::new(|| SearchIndex {
    entries: index()
        .items
        .iter()
        .enumerate()
        .map(|(item_idx, item)| SearchEntry {
            item_idx,
            path_l: item.path.to_ascii_lowercase(),
            name_l: item.name.to_ascii_lowercase(),
            summary_l: item.summary.to_ascii_lowercase(),
            doc_l: item.doc.to_ascii_lowercase(),
        })
        .collect(),
});

static WARMUP: Once = Once::new();

/// Loaded embedded API documentation index.
pub fn index() -> &'static DocIndex {
    &INDEX
}

/// Force-load the embedded docs and precomputed search cache.
pub fn warm() {
    LazyLock::force(&INDEX);
    LazyLock::force(&SEARCH_INDEX);
}

/// Start loading the docs/search cache without delaying MCP startup.
pub fn warm_in_background() {
    WARMUP.call_once(|| {
        if let Err(e) = std::thread::Builder::new()
            .name("infrazeug-mcp-doc-warmup".to_string())
            .spawn(warm)
        {
            tracing::debug!(%e, "failed to spawn MCP doc warmup thread");
        }
    });
}

pub fn index_available() -> bool {
    !index().items.is_empty()
}

pub fn item_by_path(path: &str) -> Option<&'static DocItem> {
    index()
        .items
        .iter()
        .find(|i| i.path == path || i.id == path)
}

pub fn summaries() -> Vec<DocItemSummary> {
    index()
        .items
        .iter()
        .map(|i| DocItemSummary {
            id: i.id.clone(),
            crate_name: i.crate_name.clone(),
            kind: i.kind.clone(),
            name: i.name.clone(),
            path: i.path.clone(),
            summary: i.summary.clone(),
        })
        .collect()
}

/// Token-based search over path, name, summary, and doc body.
pub fn search(query: &str, limit: usize, crate_filter: Option<&str>) -> DocSearchResult {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| t.len() >= 2)
        .collect();

    let limit = limit.clamp(1, 50);
    let doc_index = index();
    let mut scored: Vec<(u32, &DocItem)> = SEARCH_INDEX
        .entries
        .iter()
        .filter_map(|entry| {
            let item = &doc_index.items[entry.item_idx];
            if crate_filter.is_some_and(|c| !item.crate_name.eq_ignore_ascii_case(c)) {
                return None;
            }
            let score = score_item(entry, &tokens, query);
            (score > 0).then_some((score, item))
        })
        .collect();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let total_matches = scored.len();
    let hits = scored
        .into_iter()
        .take(limit)
        .map(|(score, item)| DocSearchHit {
            score,
            id: item.id.clone(),
            crate_name: item.crate_name.clone(),
            kind: item.kind.clone(),
            name: item.name.clone(),
            path: item.path.clone(),
            summary: item.summary.clone(),
            doc: truncate_doc(&item.doc, 4000),
        })
        .collect();

    DocSearchResult {
        query: query.to_string(),
        limit,
        total_matches,
        hits,
    }
}

fn score_item(entry: &SearchEntry, tokens: &[String], raw_query: &str) -> u32 {
    let q = raw_query.trim().to_ascii_lowercase();

    let mut score = 0u32;
    if !q.is_empty() {
        if entry.path_l == q {
            score += 200;
        } else if entry.path_l.contains(&q) {
            score += 80;
        }
        if entry.name_l == q {
            score += 120;
        }
    }

    for t in tokens {
        if entry.path_l.contains(t) {
            score += 40;
        }
        if entry.name_l.contains(t) {
            score += 30;
        }
        if entry.summary_l.contains(t) {
            score += 15;
        }
        if entry.doc_l.contains(t) {
            score += 5;
        }
    }
    score
}

fn truncate_doc(doc: &str, max: usize) -> String {
    if doc.len() <= max {
        return doc.to_string();
    }
    let mut end = max;
    while end > 0 && !doc.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &doc[..end])
}

/// LLM-oriented JSON for `search_api_docs` tool results.
pub fn search_json(query: &str, limit: usize, crate_filter: Option<&str>) -> Value {
    let result = search(query, limit, crate_filter);
    serde_json::to_value(&result).unwrap_or_else(|_| json!({ "error": "serialize failed" }))
}

/// Full item JSON for `resources/read` on an API item URI.
pub fn item_json(path: &str) -> Option<Value> {
    item_by_path(path).map(|item| {
        json!({
            "id": item.id,
            "crate": item.crate_name,
            "kind": item.kind,
            "name": item.name,
            "path": item.path,
            "summary": item.summary,
            "doc": item.doc,
        })
    })
}

/// Browse index (summaries only) as JSON.
pub fn index_json() -> Value {
    json!({
        "version": index().version,
        "rustdoc_version": index().rustdoc_version,
        "crates": index().crates,
        "item_count": index().items.len(),
        "items": summaries(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_run_config_when_index_populated() {
        if !index_available() {
            return;
        }
        warm();
        let r = search("RunConfig mcp", 5, Some("infrazeug-api"));
        assert!(!r.hits.is_empty(), "expected hits in embedded index");
    }

    #[test]
    fn warm_is_idempotent() {
        warm();
        warm();
        assert_eq!(SEARCH_INDEX.entries.len(), index().items.len());
    }
}
