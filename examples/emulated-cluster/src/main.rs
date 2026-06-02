//! M5 deferred slice: a three-machine cluster (one postgres + two nginx) whose
//! production twins emulate as QEMU microVMs (`like = MicroVm`).
//!
//! The graph builds offline so `plan`/`lint` run without QEMU; `test`/`apply`
//! with `--emulate-first` boot the microVMs and need `qemu-system-*` plus
//! `INFRZEUG_QEMU_IMAGE` (a cloud-init friendly qcow2) and
//! `INFRZEUG_QEMU_SSH_PUBKEY`.
//!
//! Topology:
//!
//! ```text
//!   db (postgres)  <--  web-1 (nginx) -.
//!                  <--  web-2 (nginx) -'
//! ```
//!
//! Each web node depends on the db node, so the scheduler brings postgres up
//! before the nginx front ends.

use infrazeug_api::builder::{self, write_rendered, InfraBuilder};
use infrazeug_api::{init_tracing, run, ExtraSubcommand, RunBuildContext, RunCommands, RunConfig};
use infrazeug_core::id::{MachineId, NodeId};
use infrazeug_core::infra::shell_node;
use infrazeug_core::{Machine, RuntimeConfig, Targets};
use infrazeug_emulate::spec::{EmulatedKind, LikeConfig, QemuConfig, VmImage};
use infrazeug_emulate_qemu::qemu_available;
use infrazeug_shell::{argv, ShellOp};
use uuid::Uuid;

const DB_MACHINE: &str = "11111111-1111-4111-8111-111111111111";
const WEB1_MACHINE: &str = "22222222-2222-4222-8222-222222222222";
const WEB2_MACHINE: &str = "33333333-3333-4333-8333-333333333333";

const PG_NODE: &str = "aaaaaaaa-1111-4aaa-8aaa-aaaaaaaaaaaa";
const WEB1_NODE: &str = "bbbbbbbb-2222-4bbb-8bbb-bbbbbbbbbbbb";
const WEB2_NODE: &str = "cccccccc-3333-4ccc-8ccc-cccccccccccc";

static EXTRAS: [ExtraSubcommand; 1] = [ExtraSubcommand {
    name: "probe",
    about: "Report QEMU availability and required env for emulate-first runs",
    run: || Box::pin(async { probe().await }),
}];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    run(
        std::env::args(),
        RunConfig::new("emulated-cluster")
            .about("M5 three-microVM nginx/postgres cluster")
            .commands(RunCommands::ALL)
            .extras(&EXTRAS),
        |ctx| match ctx {
            RunBuildContext::Playbook(_) => build_infra(),
            RunBuildContext::Pull(_) => unreachable!(),
        },
    )
    .await
}

async fn probe() -> anyhow::Result<()> {
    let image = std::env::var("INFRZEUG_QEMU_IMAGE").unwrap_or_else(|_| "<unset>".into());
    let pubkey = std::env::var("INFRZEUG_QEMU_SSH_PUBKEY")
        .map(|_| "set")
        .unwrap_or("<unset>");
    println!("qemu-system-* on PATH : {}", qemu_available());
    println!("INFRZEUG_QEMU_IMAGE   : {image}");
    println!("INFRZEUG_QEMU_SSH_PUBKEY: {pubkey}");
    println!("`plan`/`lint` run offline; `test`/`apply --emulate-first` need all three.");
    Ok(())
}

/// MicroVm `like` config from the shared qcow2 image (placeholder until set, so
/// the graph still builds for offline `plan`/`lint`).
fn microvm_like() -> LikeConfig {
    let image = std::env::var("INFRZEUG_QEMU_IMAGE")
        .unwrap_or_else(|_| "INFRZEUG_QEMU_IMAGE-unset.qcow2".into());
    let memory_mb = std::env::var("INFRZEUG_QEMU_MEM_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024);
    LikeConfig {
        kind: EmulatedKind::MicroVm {
            image: VmImage::RemoteQcow2(image),
            qemu: QemuConfig { memory_mb },
        },
        vars: Default::default(),
    }
}

/// Build a machine as remote (when `INFRZEUG_SSH_HOST_<suffix>` is set) or a
/// local stand-in, with a MicroVm emulated twin attached.
fn cluster_machine(id: MachineId, name: &str, ssh_env: &str) -> Machine {
    let mut m = match std::env::var(ssh_env) {
        Ok(host) => builder::remote(id, name, infrazeug_core::SshConfig::new(host)),
        Err(_) => builder::local(id, name),
    };
    m.like = Some(microvm_like());
    m
}

const NGINX_CONF: &str = "\
events {}
http {
    upstream app_db { server db.internal:5432; }
    server {
        listen 80;
        location /health { return 200 'ok\\n'; }
        location / { proxy_pass http://app_db; }
    }
}
";

fn build_infra() -> anyhow::Result<infrazeug_api::PlaybookBundle> {
    let db_mid = MachineId(Uuid::parse_str(DB_MACHINE)?);
    let web1_mid = MachineId(Uuid::parse_str(WEB1_MACHINE)?);
    let web2_mid = MachineId(Uuid::parse_str(WEB2_MACHINE)?);

    let pg_node_id = NodeId(Uuid::parse_str(PG_NODE)?);

    // Postgres bring-up on the db machine.
    let pg_op = ShellOp::Seq {
        steps: vec![
            ShellOp::run(argv![
                "apk",
                "add",
                "--no-cache",
                "postgresql",
                "postgresql-contrib"
            ]),
            ShellOp::run(argv![
                "sh",
                "-c",
                "[ -d /var/lib/postgresql/data/base ] || \
                 su postgres -c 'initdb -D /var/lib/postgresql/data'"
            ]),
            ShellOp::run(argv![
                "sh",
                "-c",
                "su postgres -c 'pg_ctl -D /var/lib/postgresql/data -w start' || true"
            ]),
            ShellOp::run(argv![
                "sh",
                "-c",
                "su postgres -c \"psql -tc \\\"SELECT 1 FROM pg_database WHERE datname='app'\\\" \
                 | grep -q 1 || createdb app\""
            ]),
        ],
    };
    let pg_node = shell_node(pg_node_id, "postgres-up", pg_op, Targets::Machine(db_mid));

    // nginx front-end bring-up shared by both web machines.
    let web_op = || ShellOp::Seq {
        steps: vec![
            ShellOp::run(argv!["apk", "add", "--no-cache", "nginx"]),
            write_rendered("/etc/nginx/nginx.conf", 0o644, NGINX_CONF.to_string()),
            ShellOp::run(argv!["nginx", "-t"]),
            ShellOp::run(argv!["sh", "-c", "nginx -s reload 2>/dev/null || nginx"]),
        ],
    };

    let mut web1_node = shell_node(
        NodeId(Uuid::parse_str(WEB1_NODE)?),
        "nginx-up@web-1",
        web_op(),
        Targets::Machine(web1_mid),
    );
    web1_node.deps = vec![pg_node_id];

    let mut web2_node = shell_node(
        NodeId(Uuid::parse_str(WEB2_NODE)?),
        "nginx-up@web-2",
        web_op(),
        Targets::Machine(web2_mid),
    );
    web2_node.deps = vec![pg_node_id];

    let bundle = InfraBuilder::new()
        .machine(cluster_machine(db_mid, "db", "INFRZEUG_SSH_HOST_DB"))?
        .machine(cluster_machine(web1_mid, "web-1", "INFRZEUG_SSH_HOST_WEB1"))?
        .machine(cluster_machine(web2_mid, "web-2", "INFRZEUG_SSH_HOST_WEB2"))?
        .node(pg_node)?
        .node(web1_node)?
        .node(web2_node)?
        .build();

    Ok(bundle.with_runtime(RuntimeConfig {
        run_root: std::env::temp_dir().join("infrazeug-emulated-cluster"),
        vault_store: None,
    }))
}
