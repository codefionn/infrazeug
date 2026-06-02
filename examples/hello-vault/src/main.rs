//! M4: encrypted vault var with `VarAcl::Prompt`, apply via `--tui`.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, ExtraSubcommand, RunBuildContext, RunCommands, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::{RuntimeConfig, VarAcl, VarKey, VarValue};
use infrazeug_secrets::{FsBackend, VaultRef, VaultStore};
use infrazeug_shell::{argv, ShellOp};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

const LOCAL_MACHINE: &str = "c3d4e5f6-a7b8-4901-c234-56789abcdef0";
const CHECK_NODE: &str = "d4e5f6a7-b8c9-4012-d345-6789abcdef01";

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "init",
    about: "Create vault store under `./vault-store` (passphrase: `demo`)",
    run: || Box::pin(init_vault()),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-vault")
            .about("M4 vault var + apply --tui")
            .commands(RunCommands::empty().with(RunCommands::APPLY))
            .extras(&EXTRAS),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

async fn init_vault() -> anyhow::Result<()> {
    let store = PathBuf::from("vault-store");
    let backend = Arc::new(FsBackend::new(&store));
    let mut vault = VaultStore::new(backend, store);
    vault.keygen_passphrase("prod", "demo", "recovery").await?;
    let mut m = BTreeMap::new();
    m.insert(
        "message".into(),
        serde_cbor::Value::Text("from encrypted vault".into()),
    );
    vault.put_vault_file("prod", "demo/msg.vault", &m).await?;
    println!("vault-store ready (data key prod, passphrase demo)");
    Ok(())
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let store = PathBuf::from("vault-store");
    if !store.join("keys/prod.dkey").exists() {
        anyhow::bail!("run `hello-vault init` first");
    }
    let runtime = RuntimeConfig {
        vault_store: Some(store),
        ..Default::default()
    };

    let mut vars = infrazeug_core::VarSet::new();
    vars.insert_with_acl(
        VarKey::new("greeting"),
        VarValue::Vault(VaultRef::field("demo/msg.vault", "message")),
        VarAcl::Prompt,
    );

    let machine_id = MachineId(uuid::Uuid::parse_str(LOCAL_MACHINE)?);
    let node_id = NodeId(uuid::Uuid::parse_str(CHECK_NODE)?);

    let bundle = InfraBuilder::new()
        .vault_data_keys(vec!["prod".into()])
        .global_vars(vars)
        .machine(builder::local(machine_id, "local"))?
        .shell_on_local(
            node_id,
            "check-greeting",
            machine_id,
            ShellOp::Run {
                argv: argv!["sh", "-c", "echo $greeting"],
                cwd: None,
                env: Vec::new(),
            },
        )?
        .build();

    Ok(bundle.with_runtime(runtime))
}
