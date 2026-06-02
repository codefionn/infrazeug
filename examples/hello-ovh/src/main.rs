//! OVH backup stack: native ensure nodes + standard VaultWrite into mutable vault.

use infrazeug_api::builder::{self, InfraBuilder};
use infrazeug_api::{init_tracing, run, ExtraSubcommand, RunBuildContext, RunCommands, RunConfig};
use infrazeug_core::id::MachineId;
use infrazeug_core::RuntimeConfig;
use infrazeug_ovh::{BackupStack, OvhInfraExt};
use infrazeug_secrets::{FsBackend, VaultStore};
use serde_cbor::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

const LOCAL_MACHINE: &str = "d4e5f6a7-b8c9-4012-d345-6789abcdef01";
const VAULT_DATA_KEY: &str = "prod-runtime";
/// Vault file (under `files/`) holding the OVH API credentials.
const OVH_VAULT_FILE: &str = "cloud/ovh.vault";

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "init",
    about:
        "Create vault store under `./vault-store` and store OVH credentials (passphrase: `demo`)",
    run: || Box::pin(init_vault()),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("hello-ovh")
            .about("OVH backup bucket + S3 user + mutable vault")
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
    let mut vault = VaultStore::new(backend, store.clone());

    // Create the DataKey on first run; otherwise unlock it so credentials can be
    // (re)stored without recreating the store.
    if store.join(format!("keys/{VAULT_DATA_KEY}.dkey")).exists() {
        vault
            .unlock_passphrase(VAULT_DATA_KEY, "demo", "recovery")
            .await?;
    } else {
        vault
            .keygen_passphrase(VAULT_DATA_KEY, "demo", "recovery")
            .await?;
    }

    // Seed the OVH API credentials into the vault so `apply` reads them from the
    // unlocked controller vault — no OVH_* environment variables at apply time.
    match ovh_credentials_from_env() {
        Ok(fields) => {
            vault
                .put_vault_fields(VAULT_DATA_KEY, OVH_VAULT_FILE, &fields)
                .await?;
            println!(
                "vault-store ready (data key {VAULT_DATA_KEY}, passphrase demo); stored OVH credentials in files/{OVH_VAULT_FILE}"
            );
        }
        Err(missing) => {
            println!("vault-store ready (data key {VAULT_DATA_KEY}, passphrase demo)");
            println!(
                "set {missing} (with OVH_APPLICATION_SECRET, OVH_CONSUMER_KEY) and re-run `init` to store credentials"
            );
        }
    }
    Ok(())
}

/// Collect OVH API credentials from the environment for one-time vault seeding.
///
/// On a missing required variable, returns its name so `init` can prompt for it.
fn ovh_credentials_from_env() -> Result<BTreeMap<String, Value>, String> {
    let required = |key: &str| std::env::var(key).map_err(|_| key.to_string());
    let mut fields = BTreeMap::new();
    fields.insert(
        "application_key".to_string(),
        Value::Text(required("OVH_APPLICATION_KEY")?),
    );
    fields.insert(
        "application_secret".to_string(),
        Value::Text(required("OVH_APPLICATION_SECRET")?),
    );
    fields.insert(
        "consumer_key".to_string(),
        Value::Text(required("OVH_CONSUMER_KEY")?),
    );
    if let Ok(endpoint) = std::env::var("OVH_ENDPOINT") {
        fields.insert("endpoint".to_string(), Value::Text(endpoint));
    }
    Ok(fields)
}

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let store = PathBuf::from("vault-store");
    if !store.join(format!("keys/{VAULT_DATA_KEY}.dkey")).exists() {
        anyhow::bail!("run `hello-ovh init` first");
    }

    let machine_id = MachineId(Uuid::parse_str(LOCAL_MACHINE)?);
    let project_id = std::env::var("OVH_PROJECT_ID")
        .map_err(|_| anyhow::anyhow!("OVH_PROJECT_ID is not set"))?;
    let container_name =
        std::env::var("OVH_CONTAINER_NAME").unwrap_or_else(|_| "infrazeug-backups".into());

    Ok(InfraBuilder::new()
        .vault_data_keys(vec![VAULT_DATA_KEY.into()])
        .machine(builder::controller(machine_id))?
        .ovh_vault(OVH_VAULT_FILE, machine_id)
        .ensure_backup_stack(
            BackupStack::new(
                project_id,
                container_name.clone(),
                "GRA",
                "infrazeug-backup-user",
            )
            .with_mutable_vault(VAULT_DATA_KEY, format!("cloud/{container_name}.vault")),
        )?
        .finish()
        .with_runtime(RuntimeConfig {
            run_root: std::env::temp_dir().join("infrazeug-hello-ovh"),
            vault_store: Some(store),
        }))
}
