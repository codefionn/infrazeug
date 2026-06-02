//! Proves the new `template!` output flows into a real container: render a
//! config, `WriteFile` it through the container exec backend, then read it back
//! and check the bytes + mode. Skips cleanly when no container runtime exists.

use infrazeug_emulate_oci::PodmanExec;
use infrazeug_shell::{FileSource, ShellOp};
use infrazeug_templates::template;
use std::path::PathBuf;
use tokio::process::Command;

fn runtime() -> Option<String> {
    for r in ["podman", "docker"] {
        if std::process::Command::new(r)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(r.to_string());
        }
    }
    None
}

async fn run(rt: &str, args: &[&str]) -> std::process::Output {
    Command::new(rt)
        .args(args)
        .output()
        .await
        .expect("spawn runtime")
}

#[tokio::test]
async fn template_renders_into_container() {
    let Some(rt) = runtime() else {
        eprintln!("skipping: no podman/docker runtime available");
        return;
    };

    let name = format!("izg-tpl-test-{}", uuid::Uuid::new_v4());
    let image = "docker.io/library/alpine:3.19";

    let start = run(
        &rt,
        &["run", "-d", "--rm", "--name", &name, image, "sleep", "120"],
    )
    .await;
    if !start.status.success() {
        eprintln!(
            "skipping: could not start container ({}): {}",
            start.status,
            String::from_utf8_lossy(&start.stderr)
        );
        return;
    }

    let result = exercise(&rt, &name).await;

    // Always tear down, then surface any assertion failure.
    let _ = run(&rt, &["rm", "-f", &name]).await;
    result.unwrap();
}

async fn exercise(rt: &str, container: &str) -> Result<(), String> {
    let exec = PodmanExec {
        runtime: rt.to_string(),
        container: container.to_string(),
    };

    // Render a config with the macro — typed, in-scope bindings.
    let port = 8443u16;
    let upstreams = ["10.0.0.1", "10.0.0.2"];
    let rendered = template!(
        "listen {{ port }};\n@for u in &upstreams {server {{ u }};\n}",
        port = port,
        upstreams = upstreams
    );

    let path = PathBuf::from("/etc/demo/app.conf");
    let write = ShellOp::write_file(
        &path,
        FileSource::bytes(rendered.clone().into_bytes()),
        0o640,
    );
    let out = exec.execute(&write).await.map_err(|e| e.to_string())?;
    if out.exit_code != 0 {
        return Err(format!(
            "WriteFile failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    // Read it back through the same backend; bytes must match the render.
    let read = ShellOp::read_file(&path);
    let got = exec.execute(&read).await.map_err(|e| e.to_string())?;
    if got.stdout != rendered.as_bytes() {
        return Err(format!(
            "round-trip mismatch:\n--- wrote ---\n{rendered}\n--- read ---\n{}",
            String::from_utf8_lossy(&got.stdout)
        ));
    }

    // Mode must be applied.
    let stat = exec
        .execute(&ShellOp::run(vec![
            "stat".into(),
            "-c".into(),
            "%a".into(),
            path.to_string_lossy().into_owned(),
        ]))
        .await
        .map_err(|e| e.to_string())?;
    let mode = String::from_utf8_lossy(&stat.stdout).trim().to_string();
    if mode != "640" {
        return Err(format!("expected mode 640, got {mode}"));
    }

    Ok(())
}
