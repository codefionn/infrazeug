# Runnable binaries and CLI surface

All playbook commands (`plan`, `apply`, `test`, `lint`) are defined once in `infrazeug-api::cli` (`PlaybookCommands`, `PLAYBOOK_SUBCOMMANDS`). User binaries and examples call [`infrazeug_api::run`] with a `RunConfig` and an infra factory instead of declaring their own `clap` parsers.

For graph execution semantics, especially when composing idempotent shell nodes
with dependent workflow phases, see [Run policy and change policy](./run-policy.md).

## Playbook embed API

```rust
infrazeug_api::init_tracing();
infrazeug_api::run(
    std::env::args(),
    RunConfig::new("my-stack").about("…").default_playbook("main"),
    |ctx| match ctx {
        infrazeug_api::RunBuildContext::Playbook(ctx) => match ctx.playbook_name("main") {
            "main" => build_main(&ctx),
            "machines" => build_machines(&ctx),
            other => anyhow::bail!("unknown playbook {other}"),
        },
        infrazeug_api::RunBuildContext::Pull(_) => build_main_default(),
    },
)
.await?;
```

Several playbooks can be registered statically and selected with `--playbook` on `plan`, `apply`, `test`, `lint`, and `graph`:

```rust
static PLAYBOOKS: PlaybookRegistry = PlaybookRegistry {
    default: "main",
    entries: &[
        PlaybookEntry { name: "main", build: build_main },
        PlaybookEntry { name: "machines", build: build_machines },
    ],
};
infrazeug_api::run(
    std::env::args(),
    RunConfig::new("my-stack"),
    |ctx| infrazeug_api::build_from_registry(&PLAYBOOKS, ctx),
)
.await?;
```

The stock `infrazeug` CLI forwards `--playbook` to a discovered project binary. From a playbook directory it also builds and runs the project binary for `plan`/`apply`/… and for any top-level subcommand not on the stock CLI (e.g. `hello-vault init`, `hello-pull demo` via `RunConfig::extras`).

Pull-mode commands (`machine`, `plan-op`, `serve-pull`, `bootstrap`) live in `infrazeug-api::pull_cli` (`PULL_SUBCOMMANDS`). Enable with `RunConfig::pull(PullCommandSet::…)`.

## Stock `infrazeug` binary

`infrazeug-cli` flattens `PlaybookCommands` and adds operational subcommands (`vault`, `agent`, `gc`, `mcp`, …). See `infrazeug_cli::all_subcommands()` for the full list.

`infrazeug mcp serve` (stock CLI only) builds the project playbook, starts its `mcp serve`, and **watches** `src/` and `Cargo.toml` for changes—on each change it rebuilds (probe + agents) and restarts MCP. The user's playbook binary still needs `RunConfig::mcp()`. Default transport is **JSON-RPC over stdio** (the stock CLI proxies stdio to the playbook child); pass `--http ADDR` (e.g. `127.0.0.1:7777`) for a Streamable HTTP server instead. Direct `your-playbook mcp serve` (embedder CLI) is one-shot with no watch.

The MCP server exposes documentation resources and a search tool backed by embedded `cargo doc` output:

| URI / tool | Format | Purpose |
|------------|--------|---------|
| `infrazeug://docs` | markdown | MCP tool catalog, security rules, usage |
| `infrazeug://docs/api-index` | JSON | Browse all indexed public API items (summaries) |
| `infrazeug://docs/api-item#<rust_path>` | JSON | Full rustdoc text for one symbol |
| `search_api_docs` | JSON (tool) | Ranked search over the embedded index |

Regenerate the index after API doc changes:

```bash
cargo run -p infrazeug-doc-index   # runs cargo doc, writes crates/infrazeug-mcp/generated/api-docs.json
cargo build -p infrazeug-mcp       # re-embeds the index
```

Indexed crates by default: `infrazeug-api`, `infrazeug-core`, `infrazeug-mcp`, `infrazeug-shell`, `infrazeug-pull`, `infrazeug-secrets`, `infrazeug-tui`, `infrazeug-templates`, `infrazeug-emulate`.

## Workspace binaries

| Binary / example   | Entry                         | Subcommands |
|--------------------|-------------------------------|-------------|
| `infrazeug`        | `infrazeug-cli`               | playbook + operational |
| `hello-local`      | `infrazeug_api::run`          | playbook (default set) |
| `hello-multi-playbook` | `infrazeug_api::run`      | playbook — `--playbook main\|machines` |
| `hello-ssh`        | `infrazeug_api::run`          | playbook |
| `hello-emulate`    | `infrazeug_api::run`          | playbook |
| `hello-vault`      | `infrazeug_api::run`          | `init` + `apply` |
| `hello-qemu`       | `infrazeug_api::run`          | `probe` + `test` |
| `hello-pull`       | `infrazeug_api::run` + pull   | `demo` + `machine` / `plan-op` / `serve-pull` / `bootstrap` |
| `hello-template`   | `infrazeug_api::run`          | playbook — typed-group `template!` rendering |
| `infrazeug-agent`  | `infrazeug_api::run` + pull   | `serve-rpc` (extra) + `serve-pull` |
| `infrazeug-bootstrap` | `infrazeug_api::run` + pull | `bootstrap` (delegates to agent) |

[`infrazeug_api::run`]: ../crates/infrazeug-api/src/cli.rs
