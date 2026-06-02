//! Built-in MCP documentation resources and overview markdown.

use rmcp::model::{AnnotateAble, RawResource, Resource, ResourceContents};

use crate::api_docs;
use crate::builder::ToolDef;
use crate::builtins::{GRAPH, LIST_MACHINES, PING, SEARCH_API_DOCS};

/// URI for the MCP server overview (tools, security, usage).
pub const DOCS_URI: &str = "infrazeug://docs";

/// URI for the searchable rustdoc index (summaries only, JSON).
pub const DOCS_API_INDEX_URI: &str = "infrazeug://docs/api-index";

/// Prefix for a single API item (`{DOCS_API_ITEM_PREFIX}{path}`).
pub const DOCS_API_ITEM_PREFIX: &str = "infrazeug://docs/api-item#";

/// Resource name shown in `resources/list` for [`DOCS_URI`].
pub const DOCS_NAME: &str = "documentation";

/// Resource name for [`DOCS_API_INDEX_URI`].
pub const DOCS_API_INDEX_NAME: &str = "api-documentation-index";

/// All built-in documentation resources.
pub fn doc_resources() -> Vec<Resource> {
    vec![docs_resource(), api_index_resource()]
}

/// MCP resource descriptor for [`DOCS_URI`].
pub fn docs_resource() -> Resource {
    RawResource::new(DOCS_URI, DOCS_NAME)
        .with_title("infrazeug MCP documentation")
        .with_description("Tool catalog, security rules, and usage for this MCP server")
        .with_mime_type("text/markdown")
        .no_annotation()
}

pub fn api_index_resource() -> Resource {
    RawResource::new(DOCS_API_INDEX_URI, DOCS_API_INDEX_NAME)
        .with_title("infrazeug API documentation index")
        .with_description(
            "Searchable rustdoc index (infrazeug-api and extensions). Use search_api_docs or read api-item URIs.",
        )
        .with_mime_type("application/json")
        .no_annotation()
}

pub fn api_item_uri(path: &str) -> String {
    format!("{DOCS_API_ITEM_PREFIX}{path}")
}

pub fn parse_api_item_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix(DOCS_API_ITEM_PREFIX)
}

/// Markdown body for [`DOCS_URI`], including registered custom tools.
pub fn docs_markdown(server_name: &str, custom_tools: &[ToolDef]) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {server_name} MCP server\n\n"));
    out.push_str(&format!(
        "infrazeug-mcp {} — `mcp serve` (JSON-RPC stdio by default, or `--http ADDR`).\n\n",
        env!("CARGO_PKG_VERSION")
    ));

    out.push_str("## Security\n\n");
    out.push_str(
        "- **Secrets are never exposed** (SOUL locked): no vault read tools, not configurable.\n",
    );
    out.push_str("- Builtin and custom tools run through the real apply pipeline; they must not return secret plaintext.\n\n");

    out.push_str("## Builtin tools\n\n");
    out.push_str(&format!(
        "| Tool | Description |\n|------|-------------|\n| `{LIST_MACHINES}` | List machines on this infra (metadata only) |\n| `{PING}` | Reachability check (`uname -n` capture) |\n| `{GRAPH}` | Planning DAG view (optional machine/start/tags filters) |\n| `{SEARCH_API_DOCS}` | Search embedded rustdoc for infrazeug-api and extension crates |\n\n"
    ));

    out.push_str("## Custom tools\n\n");
    if custom_tools.is_empty() {
        out.push_str("_None registered on this server._\n\n");
    } else {
        out.push_str("| Tool | Description |\n|------|-------------|\n");
        for t in custom_tools {
            out.push_str(&format!("| `{}` | {} |\n", t.name, t.description));
        }
        out.push('\n');
    }

    out.push_str("## API documentation (rustdoc)\n\n");
    if api_docs::index_available() {
        let idx = api_docs::index();
        out.push_str(&format!(
            "- **Index:** `{DOCS_API_INDEX_URI}` — {} items from crates: {}\n",
            idx.items.len(),
            idx.crates.join(", ")
        ));
        out.push_str(&format!(
            "- **Item:** `{DOCS_API_ITEM_PREFIX}<rust_path>` — full docs for one symbol (e.g. `{DOCS_API_ITEM_PREFIX}infrazeug_api::cli::RunConfig`)\n"
        ));
        out.push_str(&format!(
            "- **Search:** call tool `{SEARCH_API_DOCS}` with `{{ \"query\": \"RunConfig mcp\", \"limit\": 5 }}` — returns ranked JSON hits (LLM-readable)\n\n"
        ));
    } else {
        out.push_str(
            "_API index not embedded. Regenerate with `cargo run -p infrazeug-doc-index` and rebuild `infrazeug-mcp`._\n\n",
        );
    }

    out.push_str("## Resources\n\n");
    out.push_str(&format!(
        "| URI | Description |\n|-----|-------------|\n| `{DOCS_URI}` | This document |\n| `{DOCS_API_INDEX_URI}` | Rust API index (JSON summaries) |\n| `{DOCS_API_ITEM_PREFIX}<path>` | One API symbol (JSON) |\n\n"
    ));

    out.push_str("## Usage\n\n");
    out.push_str("1. Register tools with `RunConfig::mcp()` on your playbook binary.\n");
    out.push_str(
        "2. Run `your-playbook mcp serve` (or `infrazeug mcp serve` from the project root).\n",
    );
    out.push_str(
        "3. Point your MCP client at stdio (default) or `http://ADDR/mcp` when using `--http`.\n",
    );
    out.push_str(&format!(
        "4. Read `{DOCS_URI}` or search API docs before calling tools.\n"
    ));

    out
}

/// Contents returned from `resources/read` for [`DOCS_URI`].
pub fn docs_contents(server_name: &str, custom_tools: &[ToolDef]) -> ResourceContents {
    ResourceContents::text(docs_markdown(server_name, custom_tools), DOCS_URI)
        .with_mime_type("text/markdown")
}

pub fn api_index_contents() -> ResourceContents {
    let json =
        serde_json::to_string_pretty(&api_docs::index_json()).unwrap_or_else(|_| "{}".to_string());
    ResourceContents::text(json, DOCS_API_INDEX_URI).with_mime_type("application/json")
}

pub fn api_item_contents(path: &str) -> Option<ResourceContents> {
    let json = serde_json::to_string_pretty(&api_docs::item_json(path)?).ok()?;
    Some(ResourceContents::text(json, api_item_uri(path)).with_mime_type("application/json"))
}
