//! Generate a Containerfile from `ContainerSpec` (M3 podman path).

use infrazeug_emulate::spec::{BuildStep, ContainerBase, ContainerSpec, CopySource, ImageRef};
use std::fmt::Write;

pub fn render(spec: &ContainerSpec, from_image: Option<&str>) -> String {
    let mut out = String::new();
    match &spec.base {
        ContainerBase::Scratch => {
            out.push_str("FROM scratch\n");
        }
        ContainerBase::Image(img) => {
            let _ = writeln!(out, "FROM {}", img.reference());
        }
        ContainerBase::From(_) => {
            let base = from_image.unwrap_or("scratch");
            let _ = writeln!(out, "FROM {base}");
        }
    }
    for step in &spec.steps {
        match step {
            BuildStep::Run { argv, .. } => {
                let cmd = shell_join(argv);
                let _ = writeln!(out, "RUN {cmd}");
            }
            BuildStep::Copy { from, dest, .. } => {
                let src = match from {
                    CopySource::Context(_) => ".".to_string(),
                    CopySource::Image(img) => img.reference(),
                    CopySource::Stage(_) => continue,
                };
                let _ = writeln!(out, "COPY {src} {}", dest.display());
            }
            BuildStep::Env { kv } => {
                for (k, v) in kv {
                    let _ = writeln!(out, "ENV {k}={v}");
                }
            }
            BuildStep::Workdir(p) => {
                let _ = writeln!(out, "WORKDIR {}", p.display());
            }
            BuildStep::User(u) => {
                let _ = writeln!(out, "USER {u}");
            }
            BuildStep::Cmd(argv) => {
                let _ = writeln!(out, "CMD {}", json_argv(argv));
            }
            BuildStep::Entrypoint(argv) => {
                let _ = writeln!(out, "ENTRYPOINT {}", json_argv(argv));
            }
        }
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|s| {
            if s.chars()
                .any(|c| c.is_whitespace() || c == '"' || c == '\'')
            {
                format!("{s:?}")
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn json_argv(argv: &[String]) -> String {
    serde_json::to_string(argv).unwrap_or_else(|_| shell_join(argv))
}

pub fn base_image_ref(spec: &ContainerSpec) -> Option<&ImageRef> {
    if let ContainerBase::Image(img) = &spec.base {
        Some(img)
    } else {
        None
    }
}
