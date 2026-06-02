//! Read-only inspection of the planning DAG (no fact gathering, no vault).
//!
//! [`Infra::graph_view`](crate::Infra::graph_view) resolves every node's target
//! machines and emits a serializable [`GraphView`]; [`GraphView::select`] then
//! filters it by machine, tag, and/or a start node (the start plus everything
//! that transitively depends on it). Shared by the `graph` CLI subcommand and
//! the `graph` MCP tool.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// One node in the inspected graph.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `"shell"` or `"native"`.
    pub kind: String,
    /// Resolved target machine names.
    pub machines: Vec<String>,
    /// Tags as `"key=value"`.
    pub tags: Vec<String>,
    /// Predecessor node ids (this node's `deps`).
    pub deps: Vec<String>,
}

/// A dependency edge: `from` (a predecessor) must finish before `to` runs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

/// A serializable slice of the planning DAG.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GraphView {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Filter applied to a [`GraphView`]. Empty fields impose no constraint; set
/// fields combine with AND.
#[derive(Clone, Debug, Default)]
pub struct GraphSelect {
    /// Keep nodes targeting any of these machine names.
    pub machines: Vec<String>,
    /// Keep the named node (name, full uuid, or uuid prefix) and its transitive
    /// dependents.
    pub start: Option<String>,
    /// Keep nodes carrying any of these tags (`"key=value"`, key, or value).
    pub tags: Vec<String>,
}

impl GraphView {
    /// Return a filtered copy per `select`. Edges are kept only when both
    /// endpoints survive.
    pub fn select(&self, select: &GraphSelect) -> GraphView {
        let reachable = select
            .start
            .as_ref()
            .and_then(|s| self.resolve_node(s))
            .map(|id| self.descendants(&id));

        let keep: HashSet<&str> = self
            .nodes
            .iter()
            .filter(|n| {
                let machine_ok = select.machines.is_empty()
                    || n.machines.iter().any(|m| select.machines.contains(m));
                let tag_ok =
                    select.tags.is_empty() || select.tags.iter().any(|t| node_has_tag(n, t));
                let reach_ok = reachable.as_ref().is_none_or(|r| r.contains(&n.id));
                machine_ok && tag_ok && reach_ok
            })
            .map(|n| n.id.as_str())
            .collect();

        GraphView {
            nodes: self
                .nodes
                .iter()
                .filter(|n| keep.contains(n.id.as_str()))
                .cloned()
                .collect(),
            edges: self
                .edges
                .iter()
                .filter(|e| keep.contains(e.from.as_str()) && keep.contains(e.to.as_str()))
                .cloned()
                .collect(),
        }
    }

    /// Resolve a node reference (exact name, exact id, or id prefix) to its id.
    fn resolve_node(&self, needle: &str) -> Option<String> {
        if let Some(n) = self
            .nodes
            .iter()
            .find(|n| n.name == needle || n.id == needle)
        {
            return Some(n.id.clone());
        }
        self.nodes
            .iter()
            .find(|n| n.id.starts_with(needle))
            .map(|n| n.id.clone())
    }

    /// `start` plus every node transitively reachable by following edges
    /// (i.e. everything that depends on `start`).
    fn descendants(&self, start: &str) -> HashSet<String> {
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &self.edges {
            adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
        }
        let mut seen = HashSet::new();
        let mut stack = vec![start.to_string()];
        while let Some(cur) = stack.pop() {
            if !seen.insert(cur.clone()) {
                continue;
            }
            if let Some(succ) = adj.get(cur.as_str()) {
                for s in succ {
                    stack.push(s.to_string());
                }
            }
        }
        seen
    }

    /// Render as Graphviz DOT, labelling nodes by name.
    pub fn to_dot(&self) -> String {
        self.to_dot_with_rankdir("TB")
    }

    /// Like [`to_dot`](Self::to_dot) with an explicit Graphviz `rankdir` (`TB` or `LR`).
    ///
    /// Node ids (not display names) are used as Graphviz identities so duplicate
    /// names cannot collapse distinct nodes. Begin/finish bookends are pinned to
    /// `rank=source` / `rank=sink` so they stay at the layout edge.
    pub fn to_dot_with_rankdir(&self, rankdir: &str) -> String {
        let mut out = format!(
            "digraph infrazeug {{\n  rankdir={rankdir};\n  bgcolor=transparent;\n  node [fontname=Helvetica];\n"
        );
        for n in &self.nodes {
            let label = if n.machines.is_empty() {
                escape_dot_label(&n.name)
            } else {
                format!(
                    "{}\\n[{}]",
                    escape_dot_label(&n.name),
                    escape_dot_label(&n.machines.join(", "))
                )
            };
            let attrs = dot_node_attrs(&n.kind);
            out.push_str(&format!(
                "  \"{}\" [label=\"{label}\"{attrs}];\n",
                escape_dot_id(&n.id),
            ));
        }
        for e in &self.edges {
            out.push_str(&format!(
                "  \"{}\" -> \"{}\";\n",
                escape_dot_id(&e.from),
                escape_dot_id(&e.to)
            ));
        }
        let begins: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| n.kind == "begin")
            .map(|n| n.id.as_str())
            .collect();
        if !begins.is_empty() {
            out.push_str("  { rank=source; ");
            for id in begins {
                out.push_str(&format!("\"{}\"; ", escape_dot_id(id)));
            }
            out.push_str("}\n");
        }
        let finishes: Vec<&str> = self
            .nodes
            .iter()
            .filter(|n| n.kind == "finish")
            .map(|n| n.id.as_str())
            .collect();
        if !finishes.is_empty() {
            out.push_str("  { rank=sink; ");
            for id in finishes {
                out.push_str(&format!("\"{}\"; ", escape_dot_id(id)));
            }
            out.push_str("}\n");
        }
        out.push_str("}\n");
        out
    }

    /// Render a human-readable summary (one line per node, then edges).
    pub fn to_text(&self) -> String {
        let name_by_id: HashMap<&str, &str> = self
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.name.as_str()))
            .collect();
        let mut out = String::new();
        out.push_str(&format!(
            "{} node(s), {} edge(s)\n",
            self.nodes.len(),
            self.edges.len()
        ));
        for n in &self.nodes {
            let machines = if n.machines.is_empty() {
                "-".to_string()
            } else {
                n.machines.join(",")
            };
            let tags = if n.tags.is_empty() {
                String::new()
            } else {
                format!(" tags=[{}]", n.tags.join(","))
            };
            out.push_str(&format!(
                "- {} ({}) machines=[{}]{}\n",
                n.name, n.kind, machines, tags
            ));
        }
        if !self.edges.is_empty() {
            out.push_str("edges:\n");
            for e in &self.edges {
                let from = name_by_id
                    .get(e.from.as_str())
                    .copied()
                    .unwrap_or(e.from.as_str());
                let to = name_by_id
                    .get(e.to.as_str())
                    .copied()
                    .unwrap_or(e.to.as_str());
                out.push_str(&format!("  {from} -> {to}\n"));
            }
        }
        out
    }

    pub fn to_yaml(&self) -> Result<String, crate::error::CoreError> {
        Ok(serde_yaml::to_string(self)?)
    }

    pub fn to_toml(&self) -> Result<String, crate::error::CoreError> {
        let val = serde_json::to_value(self)?;
        Ok(toml::to_string_pretty(&val)?)
    }

    /// Render a self-contained interactive HTML page (top-to-bottom layout).
    pub fn to_html(&self) -> Result<String, crate::error::CoreError> {
        self.to_html_with_rankdir("TB")
    }

    /// Like [`to_html`](Self::to_html) with an explicit layout direction
    /// (`TB` or `LR`).
    ///
    /// The page is a single offline file: all CSS/JS is inlined and the
    /// [`GraphView`] is embedded as a JSON object literal inside one
    /// `<script>`. `<`, `>`, and `&` in that JSON are rewritten to their
    /// `\uXXXX` escapes (still valid JSON) so a node name or description
    /// containing `</script>` cannot break out of the script element.
    pub fn to_html_with_rankdir(&self, rankdir: &str) -> Result<String, crate::error::CoreError> {
        let data = html_safe_json(&serde_json::to_string(self)?);
        let rankdir = if rankdir == "LR" { "LR" } else { "TB" };
        let html = include_str!("graph_view.html")
            .replace("__INFRAZEUG_GRAPH_DATA__", &data)
            .replace("__INFRAZEUG_RANKDIR__", rankdir);
        Ok(html)
    }
}

/// Escape `<`, `>`, and `&` to their `\uXXXX` JSON escapes. The result is still
/// valid JSON but is safe to embed verbatim inside an HTML `<script>` element.
fn html_safe_json(json: &str) -> String {
    json.replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

fn escape_dot_id(id: &str) -> String {
    id.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_dot_label(label: &str) -> String {
    label.replace('\\', "\\\\").replace('"', "\\\"")
}

fn dot_node_attrs(kind: &str) -> String {
    match kind {
        "begin" => ", shape=invhouse, style=\"filled,bold\", fillcolor=\"#b6f2b6\", fontname=Helvetica, fontsize=16".into(),
        "finish" => ", shape=doublecircle, style=\"filled,bold\", fillcolor=\"#f2b6b6\", fontname=Helvetica, fontsize=16".into(),
        "barrier" => ", shape=diamond, style=\"filled\", fillcolor=\"#fff3b0\", fontname=Helvetica".into(),
        _ => String::new(),
    }
}

fn node_has_tag(node: &GraphNode, needle: &str) -> bool {
    node.tags.iter().any(|t| {
        t == needle
            || t.split_once('=')
                .is_some_and(|(key, value)| key == needle || value == needle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, machines: &[&str], tags: &[&str], deps: &[&str]) -> GraphNode {
        GraphNode {
            id: id.into(),
            name: id.into(),
            description: None,
            kind: "shell".into(),
            machines: machines.iter().map(|s| s.to_string()).collect(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn sample() -> GraphView {
        // base -> nginx (web), base -> pg (db)
        GraphView {
            nodes: vec![
                node("base", &["web", "db"], &["tier=base"], &[]),
                node("nginx", &["web"], &["app=web"], &["base"]),
                node("pg", &["db"], &["app=db"], &["base"]),
            ],
            edges: vec![
                GraphEdge {
                    from: "base".into(),
                    to: "nginx".into(),
                },
                GraphEdge {
                    from: "base".into(),
                    to: "pg".into(),
                },
            ],
        }
    }

    fn names(v: &GraphView) -> Vec<&str> {
        let mut n: Vec<&str> = v.nodes.iter().map(|n| n.name.as_str()).collect();
        n.sort_unstable();
        n
    }

    #[test]
    fn empty_select_keeps_everything() {
        let v = sample().select(&GraphSelect::default());
        assert_eq!(names(&v), vec!["base", "nginx", "pg"]);
        assert_eq!(v.edges.len(), 2);
    }

    #[test]
    fn machine_filter_keeps_targeting_nodes_and_prunes_edges() {
        let sel = GraphSelect {
            machines: vec!["db".into()],
            ..Default::default()
        };
        let v = sample().select(&sel);
        assert_eq!(names(&v), vec!["base", "pg"]);
        assert_eq!(v.edges.len(), 1);
        assert_eq!(v.edges[0].to, "pg");
    }

    #[test]
    fn tag_filter_matches_key_value_or_key_value() {
        let by_pair = sample().select(&GraphSelect {
            tags: vec!["app=web".into()],
            ..Default::default()
        });
        assert_eq!(names(&by_pair), vec!["nginx"]);
        let by_key = sample().select(&GraphSelect {
            tags: vec!["app".into()],
            ..Default::default()
        });
        assert_eq!(names(&by_key), vec!["nginx", "pg"]);
        let by_value = sample().select(&GraphSelect {
            tags: vec!["web".into()],
            ..Default::default()
        });
        assert_eq!(names(&by_value), vec!["nginx"]);
    }

    #[test]
    fn start_keeps_node_and_transitive_dependents() {
        let from_base = sample().select(&GraphSelect {
            start: Some("base".into()),
            ..Default::default()
        });
        assert_eq!(names(&from_base), vec!["base", "nginx", "pg"]);
        // A leaf has no dependents: only itself.
        let from_nginx = sample().select(&GraphSelect {
            start: Some("nginx".into()),
            ..Default::default()
        });
        assert_eq!(names(&from_nginx), vec!["nginx"]);
        assert!(from_nginx.edges.is_empty());
    }

    #[test]
    fn filters_compose_with_and() {
        let sel = GraphSelect {
            machines: vec!["web".into()],
            start: Some("base".into()),
            tags: vec!["app".into()],
        };
        // app-tagged AND on web AND dependent of base => nginx only (pg is db, base has no app tag).
        assert_eq!(names(&sample().select(&sel)), vec!["nginx"]);
    }

    #[test]
    fn yaml_output_contains_nodes() {
        let yaml = sample().to_yaml().unwrap();
        assert!(yaml.contains("nginx"));
        assert!(yaml.contains("base"));
    }

    #[test]
    fn toml_output_contains_nodes() {
        let toml = sample().to_toml().unwrap();
        assert!(toml.contains("nginx"));
        assert!(toml.contains("base"));
    }

    #[test]
    fn html_output_is_self_contained_with_canvas() {
        let html = sample().to_html().unwrap();
        assert!(html.contains("<canvas"));
        assert!(html.contains("nginx"));
        assert!(html.contains("base"));
        // Both injection tokens must be substituted.
        assert!(!html.contains("__INFRAZEUG_"));
        // No external assets: no src=/href= to a remote scheme.
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn html_escapes_script_break_out() {
        let mut v = sample();
        v.nodes[0].name = "</script><script>alert(1)</script>".into();
        let html = v.to_html().unwrap();
        // The injected closing tag must not appear verbatim in the document.
        assert!(!html.contains("</script><script>alert(1)"));
        // It survives only as an escaped unicode sequence.
        assert!(html.contains("\\u003c/script"));
    }

    #[test]
    fn html_rankdir_is_injected() {
        assert!(sample()
            .to_html_with_rankdir("LR")
            .unwrap()
            .contains("const RANKDIR = \"LR\""));
        assert!(sample()
            .to_html_with_rankdir("TB")
            .unwrap()
            .contains("const RANKDIR = \"TB\""));
    }

    #[test]
    fn dot_highlights_bookend_nodes() {
        let view = GraphView {
            nodes: vec![
                GraphNode {
                    id: "b".into(),
                    name: "group/begin".into(),
                    description: None,
                    kind: "begin".into(),
                    machines: vec!["host".into()],
                    tags: vec![],
                    deps: vec![],
                },
                GraphNode {
                    id: "f".into(),
                    name: "group/finish".into(),
                    description: None,
                    kind: "finish".into(),
                    machines: vec!["host".into()],
                    tags: vec![],
                    deps: vec!["b".into()],
                },
            ],
            edges: vec![GraphEdge {
                from: "b".into(),
                to: "f".into(),
            }],
        };
        let dot = view.to_dot();
        assert!(dot.contains("rankdir=TB"));
        assert!(dot.contains("group/begin"));
        assert!(dot.contains("fillcolor=\"#b6f2b6\""));
        assert!(dot.contains("fillcolor=\"#f2b6b6\""));
        assert!(dot.contains("{ rank=source; \"b\";"));
        assert!(dot.contains("{ rank=sink; \"f\";"));
    }

    #[test]
    fn dot_uses_ids_so_duplicate_names_stay_distinct() {
        let view = GraphView {
            nodes: vec![
                GraphNode {
                    id: "id-a".into(),
                    name: "same".into(),
                    description: None,
                    kind: "shell".into(),
                    machines: vec![],
                    tags: vec![],
                    deps: vec![],
                },
                GraphNode {
                    id: "id-b".into(),
                    name: "same".into(),
                    description: None,
                    kind: "begin".into(),
                    machines: vec![],
                    tags: vec![],
                    deps: vec![],
                },
            ],
            edges: vec![],
        };
        let dot = view.to_dot();
        assert!(dot.contains("\"id-a\""));
        assert!(dot.contains("\"id-b\""));
    }

    #[test]
    fn json_output_is_valid_json() {
        let json = serde_json::to_string_pretty(&sample()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 3);
    }
}
