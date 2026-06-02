use anyhow::{Context, Result};
use dialoguer::MultiSelect;
use std::path::Path;

use crate::CodeAgent;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const ALL_AGENTS: [CodeAgent; 8] = [
    CodeAgent::Claude,
    CodeAgent::OpenCode,
    CodeAgent::Cursor,
    CodeAgent::Vscode,
    CodeAgent::Cline,
    CodeAgent::ContinueDev,
    CodeAgent::Roo,
    CodeAgent::Zed,
];

fn agent_label(a: &CodeAgent) -> &'static str {
    match a {
        CodeAgent::Claude => "Claude Code (.mcp.json)",
        CodeAgent::OpenCode => "OpenCode (opencode.json)",
        CodeAgent::Cursor => "Cursor (.cursor/mcp.json)",
        CodeAgent::Vscode => "VS Code / Copilot (.vscode/mcp.json)",
        CodeAgent::Cline => "Cline (.cline/mcp.json)",
        CodeAgent::ContinueDev => "Continue.dev (.continue/mcpServers/mcp.json)",
        CodeAgent::Roo => "Roo Code (.roo/mcp.json)",
        CodeAgent::Zed => "Zed (.zed/settings.json)",
    }
}

fn resolve_agents(cli_agents: &[CodeAgent]) -> Vec<CodeAgent> {
    if !cli_agents.is_empty() {
        return cli_agents.to_vec();
    }
    let items: Vec<&str> = ALL_AGENTS.iter().map(agent_label).collect();
    let defaults: Vec<bool> = items.iter().map(|_| true).collect();
    let selections = MultiSelect::new()
        .with_prompt("Select LLM code agents to configure")
        .items(&items)
        .defaults(&defaults)
        .interact()
        .unwrap_or_default();
    if selections.is_empty() {
        return ALL_AGENTS.to_vec();
    }
    selections.into_iter().map(|i| ALL_AGENTS[i]).collect()
}

pub fn init_project(name: &str, cli_agents: &[CodeAgent]) -> Result<()> {
    let dir = Path::new(name);
    if dir.exists() {
        anyhow::bail!("directory {} already exists", dir.display());
    }

    let project_name = dir
        .file_name()
        .context("need a directory name")?
        .to_string_lossy();

    let agents = resolve_agents(cli_agents);

    std::fs::create_dir_all(dir.join("src"))?;

    write_cargo_toml(dir, &project_name)?;
    write_main_rs(dir)?;
    write_agents_md(dir, &project_name)?;

    for agent in &agents {
        write_agent_context(dir, &project_name, agent)?;
        write_agent_mcp(dir, agent)?;
    }

    println!("initialized infrazeug project in {}/", dir.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Scaffolding (Cargo.toml, main.rs, AGENTS.md)
// ---------------------------------------------------------------------------

fn crate_dep(crate_name: &str) -> String {
    format!(r#"{crate_name} = "{VERSION}""#)
}

fn write_cargo_toml(dir: &Path, name: &str) -> Result<()> {
    let content = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
anyhow = "1"
tokio = {{ version = "1", features = ["full"] }}
uuid = {{ version = "1", features = ["v4"] }}
{api}
{core}
{shell}
"#,
        name = name,
        api = crate_dep("infrazeug-api"),
        core = crate_dep("infrazeug-core"),
        shell = crate_dep("infrazeug-shell"),
    );
    std::fs::write(dir.join("Cargo.toml"), content)?;
    Ok(())
}

fn write_main_rs(dir: &Path) -> Result<()> {
    let content = r#"use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, RunBuildContext, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::RuntimeConfig;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const LOCAL_MACHINE: &str = "00000000-0000-4000-8000-000000000001";
const HELLO_NODE: &str = "00000000-0000-4000-8000-000000000002";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new(env!("CARGO_PKG_NAME")).about("infrazeug playbook"),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

fn build_infra() -> anyhow::Result<infrazeug_core::Infra> {
    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let node_id = NodeId(Uuid::parse_str(HELLO_NODE)?);

    let infra = InfraBuilder::new()
        .machine(builder::local(machine_id, "localhost"))?
        .shell_on_local(
            node_id,
            "hello",
            machine_id,
            ShellOp::run(argv!["echo", "hello from infrazeug"]),
        )?
        .build();

    Ok(infra.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join(env!("CARGO_PKG_NAME")),
        vault_store: None,
    }))
}
"#;
    std::fs::write(dir.join("src").join("main.rs"), content)?;
    Ok(())
}

fn agent_context_body(name: &str) -> String {
    format!(
        r#"# {name}

Infrazeug playbook project. Uses `infrazeug_api::run` for plan/apply/test/lint.

## Key types

- `InfraBuilder` — fluent builder for machines and shell nodes
- `builder::local(machine_id, hostname)` — create a local machine
- `ShellOp::run(argv![...])` — define a shell operation
- `RunConfig::new(name)` — CLI surface configuration

## Commands

```sh
cargo run -- plan       # compute execution plan
cargo run -- apply      # execute the plan
cargo run -- test       # dry-run / test mode
cargo run -- lint       # validate infra definition
```

## Structure

- `src/main.rs` — playbook entry point, defines machines and nodes via the builder API
- `Cargo.toml` — depends on infrazeug-api, infrazeug-core, infrazeug-shell from crates.io

## References

- [infrazeug API docs](https://docs.rs/infrazeug-api)
- [infrazeug repo](https://github.com/infrazeug/infrazeug)
"#,
        name = name,
    )
}

fn write_agents_md(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join("AGENTS.md"), agent_context_body(name))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent-specific context files
// ---------------------------------------------------------------------------

fn write_agent_context(dir: &Path, name: &str, agent: &CodeAgent) -> Result<()> {
    match agent {
        CodeAgent::Claude => write_claude_md(dir, name),
        CodeAgent::OpenCode => write_opencode_context(dir, name),
        CodeAgent::Cursor => write_cursor_rules(dir, name),
        CodeAgent::Vscode => write_copilot_instructions(dir, name),
        CodeAgent::Cline => write_clinerules(dir, name),
        CodeAgent::ContinueDev => write_continue_rules(dir, name),
        CodeAgent::Roo => write_roo_rules(dir, name),
        CodeAgent::Zed => Ok(()),
    }
}

fn write_claude_md(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join("CLAUDE.md"), agent_context_body(name))?;
    Ok(())
}

fn write_opencode_context(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join("opencode.md"), agent_context_body(name))?;
    Ok(())
}

fn write_cursor_rules(dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(dir.join(".cursor").join("rules"))?;
    std::fs::write(
        dir.join(".cursor").join("rules").join("infrazeug.mdc"),
        format!(
            "---\ndescription: infrazeug playbook context\nglobs:\nalwaysApply: true\n---\n{}",
            agent_context_body(name)
        ),
    )?;
    Ok(())
}

fn write_copilot_instructions(dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(dir.join(".github"))?;
    std::fs::write(
        dir.join(".github").join("copilot-instructions.md"),
        agent_context_body(name),
    )?;
    Ok(())
}

fn write_clinerules(dir: &Path, name: &str) -> Result<()> {
    std::fs::write(dir.join(".clinerules"), agent_context_body(name))?;
    Ok(())
}

fn write_continue_rules(dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(dir.join(".continue"))?;
    std::fs::write(
        dir.join(".continue").join("rules.md"),
        agent_context_body(name),
    )?;
    Ok(())
}

fn write_roo_rules(dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(dir.join(".roo").join("rules"))?;
    std::fs::write(
        dir.join(".roo").join("rules").join("infrazeug.md"),
        agent_context_body(name),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent MCP config writers
// ---------------------------------------------------------------------------

fn mcp_stdio_entry() -> &'static str {
    r#"{
      "type": "stdio",
      "command": "infrazeug",
      "args": ["mcp", "serve"]
    }"#
}

fn mcp_stdio_entry_no_type() -> &'static str {
    r#"{
      "command": "infrazeug",
      "args": ["mcp", "serve"]
    }"#
}

fn write_agent_mcp(dir: &Path, agent: &CodeAgent) -> Result<()> {
    match agent {
        CodeAgent::Claude => write_mcp_json(dir),
        CodeAgent::OpenCode => write_opencode_json(dir),
        CodeAgent::Cursor => write_cursor_mcp_json(dir),
        CodeAgent::Vscode => write_vscode_mcp_json(dir),
        CodeAgent::Cline => write_cline_mcp_json(dir),
        CodeAgent::ContinueDev => write_continue_mcp_json(dir),
        CodeAgent::Roo => write_roo_mcp_json(dir),
        CodeAgent::Zed => write_zed_settings_json(dir),
    }
}

fn write_mcp_json(dir: &Path) -> Result<()> {
    let content = format!(
        r#"{{
  "mcpServers": {{
    "infrazeug": {entry}
  }}
}}
"#,
        entry = mcp_stdio_entry_no_type().trim()
    );
    std::fs::write(dir.join(".mcp.json"), content)?;
    Ok(())
}

fn write_opencode_json(dir: &Path) -> Result<()> {
    let content = r#"{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "infrazeug": {
      "type": "local",
      "command": ["infrazeug", "mcp", "serve"],
      "enabled": true
    }
  },
  "contextPaths": [
    "opencode.md",
    "AGENTS.md"
  ]
}
"#;
    std::fs::write(dir.join("opencode.json"), content)?;
    Ok(())
}

fn write_cursor_mcp_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".cursor"))?;
    let content = format!(
        r#"{{
  "mcpServers": {{
    "infrazeug": {entry}
  }}
}}
"#,
        entry = mcp_stdio_entry().trim()
    );
    std::fs::write(dir.join(".cursor").join("mcp.json"), content)?;
    Ok(())
}

fn write_vscode_mcp_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".vscode"))?;
    let content = r#"{
  "servers": {
    "infrazeug": {
      "type": "stdio",
      "command": "infrazeug",
      "args": ["mcp", "serve"]
    }
  }
}
"#;
    std::fs::write(dir.join(".vscode").join("mcp.json"), content)?;
    Ok(())
}

fn write_cline_mcp_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".cline"))?;
    let content = format!(
        r#"{{
  "mcpServers": {{
    "infrazeug": {entry}
  }}
}}
"#,
        entry = mcp_stdio_entry_no_type().trim()
    );
    std::fs::write(dir.join(".cline").join("mcp.json"), content)?;
    Ok(())
}

fn write_continue_mcp_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".continue").join("mcpServers"))?;
    let content = format!(
        r#"{{
  "mcpServers": {{
    "infrazeug": {entry}
  }}
}}
"#,
        entry = mcp_stdio_entry_no_type().trim()
    );
    std::fs::write(
        dir.join(".continue").join("mcpServers").join("mcp.json"),
        content,
    )?;
    Ok(())
}

fn write_roo_mcp_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".roo"))?;
    let content = format!(
        r#"{{
  "mcpServers": {{
    "infrazeug": {entry}
  }}
}}
"#,
        entry = mcp_stdio_entry_no_type().trim()
    );
    std::fs::write(dir.join(".roo").join("mcp.json"), content)?;
    Ok(())
}

fn write_zed_settings_json(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join(".zed"))?;
    let content = r#"{
  "context_servers": {
    "infrazeug": {
      "command": "infrazeug",
      "args": ["mcp", "serve"]
    }
  }
}
"#;
    std::fs::write(dir.join(".zed").join("settings.json"), content)?;
    Ok(())
}
