//! Example internal-network stack: PostgreSQL, Keycloak, Open WebUI, RustFS.

use crate::container::ContainerCli;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

const STACK_LABEL: &str = "infrazeug.example_stack";
const PG_PASSWORD: &str = "iz-test-secret";

const IMG_POSTGRES: &str = "docker.io/library/postgres:16-alpine";
const IMG_KEYCLOAK: &str = "quay.io/keycloak/keycloak:26.0";
const IMG_OPENWEBUI: &str = "ghcr.io/open-webui/open-webui:main";
const IMG_RUSTFS: &str = "docker.io/rustfs/rustfs:latest";

/// Lab stack on a private bridge network (for integration tests and demos).
#[allow(dead_code)]
pub struct ExampleStack {
    pub cli: ContainerCli,
    pub run_id: String,
    pub network: String,
    pub postgres: String,
    pub keycloak: String,
    pub openwebui: String,
    pub rustfs: String,
    init_dir: PathBuf,
}

impl ExampleStack {
    pub async fn up(cli: ContainerCli) -> Result<Self, String> {
        let run_id = Uuid::new_v4().to_string();
        let short = &run_id[..8];
        let network = format!("iz-stack-{short}");
        let postgres = format!("iz-pg-{short}");
        let keycloak = format!("iz-keycloak-{short}");
        let openwebui = format!("iz-openwebui-{short}");
        let rustfs = format!("iz-rustfs-{short}");

        let init_dir = std::env::temp_dir().join("infrazeug-stack").join(short);
        std::fs::create_dir_all(&init_dir).map_err(|e| e.to_string())?;
        let init_sql = init_dir.join("01-init.sql");
        std::fs::write(&init_sql, INIT_SQL).map_err(|e| e.to_string())?;

        cli.network_create(&network).await?;

        let net = format!("--network={network}");
        let label = format!("infrazeug.run_id={run_id}");
        let stack_label = format!("{STACK_LABEL}=1");

        let pg_mount = format!("{}:/docker-entrypoint-initdb.d:ro", init_dir.display());

        run_labeled(
            &cli,
            &postgres,
            &label,
            &stack_label,
            &[
                &net,
                "-e",
                &format!("POSTGRES_PASSWORD={PG_PASSWORD}"),
                "-v",
                &pg_mount,
            ],
            IMG_POSTGRES,
            None,
        )
        .await?;

        wait_until(
            || async {
                cli.exec(
                    &postgres,
                    &["pg_isready", "-U", "keycloak", "-d", "keycloak"],
                )
                .await
                .map(|c| c == 0)
                .unwrap_or(false)
            },
            Duration::from_secs(90),
            "postgres",
        )
        .await?;

        run_labeled(
            &cli,
            &keycloak,
            &label,
            &stack_label,
            &[
                &net,
                "-e",
                "KC_DB=postgres",
                "-e",
                &format!("KC_DB_URL=jdbc:postgresql://{postgres}:5432/keycloak"),
                "-e",
                "KC_DB_USERNAME=keycloak",
                "-e",
                &format!("KC_DB_PASSWORD={PG_PASSWORD}"),
                "-e",
                "KC_BOOTSTRAP_ADMIN_USERNAME=admin",
                "-e",
                &format!("KC_BOOTSTRAP_ADMIN_PASSWORD={PG_PASSWORD}"),
                "-e",
                "KC_HOSTNAME_STRICT=false",
                "-e",
                "KC_HTTP_ENABLED=true",
            ],
            IMG_KEYCLOAK,
            Some(&["start-dev"]),
        )
        .await?;

        run_labeled(
            &cli,
            &openwebui,
            &label,
            &stack_label,
            &[
                &net,
                "-e",
                &format!(
                    "DATABASE_URL=postgresql://openwebui:{PG_PASSWORD}@{postgres}:5432/openwebui"
                ),
                "-e",
                "WEBUI_SECRET_KEY=iz-test-webui-secret",
            ],
            IMG_OPENWEBUI,
            None,
        )
        .await?;

        run_labeled(
            &cli,
            &rustfs,
            &label,
            &stack_label,
            &[
                &net,
                "-e",
                "RUSTFS_ACCESS_KEY=iz-rustfs-access",
                "-e",
                "RUSTFS_SECRET_KEY=iz-rustfs-secret",
            ],
            IMG_RUSTFS,
            None,
        )
        .await?;

        wait_until(
            || async { cli.container_running(&keycloak).await },
            Duration::from_secs(120),
            "keycloak container",
        )
        .await?;

        wait_until(
            || async { cli.container_running(&openwebui).await },
            Duration::from_secs(120),
            "openwebui container",
        )
        .await?;

        wait_until(
            || async { cli.container_running(&rustfs).await },
            Duration::from_secs(60),
            "rustfs container",
        )
        .await?;

        Ok(Self {
            cli,
            run_id,
            network,
            postgres,
            keycloak,
            openwebui,
            rustfs,
            init_dir,
        })
    }

    /// Containers on the stack network resolve each other by name.
    pub async fn verify_internal_dns(&self) -> Result<(), String> {
        let code = self
            .cli
            .exec(&self.postgres, &["ping", "-c", "1", &self.keycloak])
            .await?;
        if code == 0 {
            Ok(())
        } else {
            Err(format!(
                "postgres -> keycloak DNS check failed (exit {code})"
            ))
        }
    }

    /// RustFS S3 API is reachable from another container on the same network.
    pub async fn verify_rustfs_reachable(&self) -> Result<(), String> {
        let host = &self.rustfs;
        let script = format!(
            "for i in $(seq 1 20); do \
               if curl -s --connect-timeout 3 http://{host}:9000/ >/dev/null 2>&1; then exit 0; fi; \
               sleep 3; \
             done; \
             exit 1"
        );
        let code = self
            .cli
            .exec(&self.openwebui, &["sh", "-c", &script])
            .await?;
        if code == 0 {
            Ok(())
        } else {
            Err(format!(
                "openwebui -> rustfs HTTP check failed (exit {code})"
            ))
        }
    }

    pub async fn down(self) -> Result<(), String> {
        for name in [
            &self.openwebui,
            &self.keycloak,
            &self.rustfs,
            &self.postgres,
        ] {
            let _ = self.cli.rm_force(name).await;
        }
        let _ = self.cli.network_rm(&self.network).await;
        let _ = std::fs::remove_dir_all(&self.init_dir);
        Ok(())
    }
}

const INIT_SQL: &str = r"
CREATE USER keycloak WITH PASSWORD 'iz-test-secret';
CREATE DATABASE keycloak OWNER keycloak;
CREATE USER openwebui WITH PASSWORD 'iz-test-secret';
CREATE DATABASE openwebui OWNER openwebui;
";

async fn run_labeled(
    cli: &ContainerCli,
    name: &str,
    run_label: &str,
    stack_label: &str,
    extra_args: &[&str],
    image: &str,
    command: Option<&[&str]>,
) -> Result<(), String> {
    let _ = cli.rm_force(name).await;
    let mut cmd = tokio::process::Command::new(&cli.bin);
    cmd.args([
        "run",
        "-d",
        "--name",
        name,
        "--label",
        run_label,
        "--label",
        stack_label,
    ]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.arg(image);
    if let Some(argv) = command {
        cmd.args(argv);
    }
    let status = cmd.status().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} run failed for {name} ({image})",
            cli.runtime_name()
        ))
    }
}

async fn wait_until<F, Fut>(mut check: F, timeout: Duration, what: &str) -> Result<(), String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if check().await {
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(format!("timed out waiting for {what}"))
}

#[cfg(test)]
mod tests {
    use super::ExampleStack;
    use crate::resolve_container_cli;

    fn stack_test_enabled() -> bool {
        std::env::var("INFRZEUG_STACK_TEST")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    /// PostgreSQL + Keycloak + Open WebUI + RustFS on a private bridge network.
    ///
    /// ```no_run
    /// INFRZEUG_STACK_TEST=1 cargo test -p infrazeug-emulate-oci example_stack_internal_network -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "pulls OCI images; run with INFRZEUG_STACK_TEST=1"]
    async fn example_stack_internal_network() {
        if !stack_test_enabled() {
            eprintln!("skip: set INFRZEUG_STACK_TEST=1 to run this test");
            return;
        }

        let cli = resolve_container_cli()
            .await
            .expect("podman or docker must be installed (podman preferred)");

        eprintln!("using {} ({})", cli.bin, cli.runtime_name());

        let stack = ExampleStack::up(cli).await.expect("stack should start");

        let result = async {
            stack.verify_internal_dns().await?;
            stack.verify_rustfs_reachable().await?;
            Ok::<(), String>(())
        }
        .await;

        stack.down().await.expect("stack teardown");

        result.expect("stack health checks");
    }
}
