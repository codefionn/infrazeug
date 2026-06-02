//! kubectl helpers that produce [`ShellOp`] nodes (SOUL tier 2).
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_kubectl::{ApplyOptions, Kubectl};
//! use infrazeug_shell::ShellOp;
//!
//! let kubectl = Kubectl::k3s().with_namespace("hermes");
//! let op: ShellOp = kubectl.ensure_namespace("hermes");
//! ```

use infrazeug_k8s::{join_argv, shell_escape, staging_path, KubeContext};
use infrazeug_shell::ShellOp;
use std::path::PathBuf;

/// kubectl CLI wrapper lowering to serializable [`ShellOp`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Kubectl {
    ctx: KubeContext,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApplyOptions {
    pub dry_run: bool,
    pub server_side: bool,
    pub force: bool,
    pub wait: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeleteOptions {
    pub ignore_not_found: bool,
    pub wait: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RolloutOptions {
    pub timeout_secs: Option<u64>,
}

impl Kubectl {
    pub fn new(ctx: KubeContext) -> Self {
        Self { ctx }
    }

    #[allow(clippy::self_named_constructors)]
    pub fn kubectl() -> Self {
        Self::new(KubeContext::kubectl())
    }

    pub fn k3s() -> Self {
        Self::new(KubeContext::k3s_kubectl())
    }

    pub fn context(&self) -> &KubeContext {
        &self.ctx
    }

    pub fn with_kubeconfig(mut self, path: impl Into<PathBuf>) -> Self {
        self.ctx = self.ctx.clone().with_kubeconfig(path);
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.ctx = self.ctx.clone().with_context(context);
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.ctx = self.ctx.clone().with_namespace(namespace);
        self
    }

    fn argv(&self, args: &[&str]) -> Vec<String> {
        join_argv(&self.ctx.prefix_argv(), args.iter().copied())
    }

    /// Idempotent namespace create (`create --dry-run=client | apply -f -`).
    pub fn ensure_namespace(&self, name: &str) -> ShellOp {
        let cmd = self.ctx.shell_command();
        let ns = shell_escape(name);
        let script =
            format!("{cmd} create namespace {ns} --dry-run=client -o yaml | {cmd} apply -f -");
        ShellOp::run(vec!["sh".into(), "-ec".into(), script])
    }

    /// Apply manifest bytes from a staged file under `/tmp/infrazeug-k8s/…`.
    pub fn apply_manifest(&self, staging_id: &str, yaml: &str, opts: ApplyOptions) -> ShellOp {
        let path = staging_path(staging_id, "manifest.yaml");
        let path_str = path.display().to_string();
        let mut args = vec!["apply".to_string(), "-f".to_string(), path_str];
        if opts.dry_run {
            args.push("--dry-run=client".into());
        }
        if opts.server_side {
            args.push("--server-side".into());
        }
        if opts.force {
            args.push("--force".into());
        }
        if opts.wait {
            args.push("--wait".into());
        }
        let apply_argv: Vec<&str> = args.iter().map(String::as_str).collect();
        ShellOp::Seq {
            steps: vec![
                ShellOp::EnsureDir {
                    path: path.parent().expect("parent").to_path_buf(),
                    mode: 0o700,
                },
                ShellOp::WriteFile {
                    path: path.clone(),
                    content: infrazeug_shell::FileSource::bytes(yaml.as_bytes()),
                    mode: 0o600,
                },
                ShellOp::run(self.argv(&apply_argv)),
            ],
        }
    }

    /// Apply an on-disk manifest path (must exist on the target host).
    pub fn apply_file(&self, path: impl Into<PathBuf>, opts: ApplyOptions) -> ShellOp {
        let path = path.into();
        let path_str = path.display().to_string();
        let mut args = vec!["apply".to_string(), "-f".to_string(), path_str];
        if opts.dry_run {
            args.push("--dry-run=client".into());
        }
        if opts.server_side {
            args.push("--server-side".into());
        }
        if opts.force {
            args.push("--force".into());
        }
        if opts.wait {
            args.push("--wait".into());
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        ShellOp::run(self.argv(&argv))
    }

    /// `kubectl delete -f …`.
    pub fn delete_file(&self, path: impl Into<PathBuf>, opts: DeleteOptions) -> ShellOp {
        let path = path.into();
        let path_str = path.display().to_string();
        let mut args = vec!["delete".to_string(), "-f".to_string(), path_str];
        if opts.ignore_not_found {
            args.push("--ignore-not-found".into());
        }
        if opts.wait {
            args.push("--wait".into());
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        ShellOp::run(self.argv(&argv))
    }

    /// `kubectl rollout status`.
    pub fn rollout_status(&self, resource: &str, opts: RolloutOptions) -> ShellOp {
        let mut args = vec![
            "rollout".to_string(),
            "status".to_string(),
            resource.to_string(),
        ];
        if let Some(secs) = opts.timeout_secs {
            args.push("--timeout".to_string());
            args.push(format!("{secs}s"));
        }
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        ShellOp::run(self.argv(&argv))
    }

    /// `kubectl wait --for=condition=…` (namespace via [`Self::with_namespace`]).
    ///
    /// Pass `condition=Ready` (or another `--for=` value); do not embed `-n` in `resource`.
    pub fn wait_condition(&self, resource: &str, condition: &str, timeout_secs: u64) -> ShellOp {
        let timeout = format!("{timeout_secs}s");
        let for_flag = if condition.starts_with("--for=") {
            condition.to_string()
        } else if condition.starts_with("condition=") || condition.starts_with("jsonpath=") {
            format!("--for={condition}")
        } else {
            format!("--for=condition={condition}")
        };
        let args = [
            "wait".to_string(),
            resource.to_string(),
            for_flag,
            "--timeout".to_string(),
            timeout,
        ];
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        ShellOp::run(self.argv(&argv))
    }

    /// Run an arbitrary kubectl subcommand with explicit argv pieces.
    pub fn run(&self, args: &[&str]) -> ShellOp {
        ShellOp::run(self.argv(args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_shell::lower::{lower, Lowered};

    fn run_argv(op: &ShellOp) -> Vec<String> {
        let Lowered::Exec { argv } = lower(op).unwrap() else {
            panic!("expected exec lowered op");
        };
        argv
    }

    #[test]
    fn ensure_namespace_uses_pipeline() {
        let op = Kubectl::k3s().ensure_namespace("hermes");
        let ShellOp::Run { argv, .. } = &op else {
            panic!("expected Run");
        };
        assert_eq!(argv[0], "sh");
        assert_eq!(argv[1], "-ec");
        assert!(argv[2].contains("create namespace hermes"));
        assert!(argv[2].contains("k3s kubectl apply"));
    }

    #[test]
    fn ensure_namespace_escapes_unsafe_name() {
        let op = Kubectl::kubectl().ensure_namespace("bad name");
        let ShellOp::Run { argv, .. } = op else {
            panic!("expected Run");
        };
        assert!(argv[2].contains("'bad name'"));
    }

    #[test]
    fn apply_manifest_serializes() {
        let op = Kubectl::kubectl().apply_manifest(
            "test-run",
            "apiVersion: v1\nkind: Namespace\n",
            ApplyOptions::default(),
        );
        let bytes = serde_cbor::to_vec(&op).unwrap();
        let back: ShellOp = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn apply_manifest_stages_yaml_and_applies() {
        let yaml = "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: hermes\n";
        let op = Kubectl::kubectl().apply_manifest("hermes", yaml, ApplyOptions::default());
        let Lowered::Seq { steps } = lower(&op).unwrap() else {
            panic!("expected seq");
        };
        assert_eq!(steps.len(), 3);
        assert!(matches!(
            &steps[0],
            Lowered::Mkdir {
                path,
                mode: 0o700
            } if path.ends_with("hermes")
        ));
        let Lowered::SftpWrite {
            content,
            mode,
            path,
        } = &steps[1]
        else {
            panic!("expected write");
        };
        assert_eq!(content, yaml.as_bytes());
        assert_eq!(*mode, 0o600);
        assert!(path.ends_with("manifest.yaml"));
        let Lowered::Exec { argv } = &steps[2] else {
            panic!("expected exec apply");
        };
        let joined = argv.join(" ");
        assert!(joined.contains("kubectl"));
        assert!(joined.contains("apply"));
        assert!(joined.contains("manifest.yaml"));
    }

    #[test]
    fn apply_manifest_honors_apply_options() {
        let op = Kubectl::kubectl().apply_manifest(
            "dry-run",
            "kind: Pod",
            ApplyOptions {
                dry_run: true,
                server_side: true,
                force: true,
                wait: true,
            },
        );
        let Lowered::Seq { steps } = lower(&op).unwrap() else {
            panic!("expected seq");
        };
        let Lowered::Exec { argv } = &steps[2] else {
            panic!("expected exec");
        };
        let joined = argv.join(" ");
        assert!(joined.contains("--dry-run=client"));
        assert!(joined.contains("--server-side"));
        assert!(joined.contains("--force"));
        assert!(joined.contains("--wait"));
    }

    #[test]
    fn apply_file_includes_path_and_flags() {
        let op = Kubectl::k3s().with_namespace("apps").apply_file(
            "/opt/manifests/app.yaml",
            ApplyOptions {
                wait: true,
                ..Default::default()
            },
        );
        let argv = run_argv(&op);
        let joined = argv.join(" ");
        assert!(joined.contains("k3s"));
        assert!(joined.contains("kubectl"));
        assert!(joined.contains("-n"));
        assert!(joined.contains("apps"));
        assert!(joined.contains("/opt/manifests/app.yaml"));
        assert!(joined.contains("--wait"));
    }

    #[test]
    fn delete_file_honors_options() {
        let op = Kubectl::kubectl().delete_file(
            "/tmp/gone.yaml",
            DeleteOptions {
                ignore_not_found: true,
                wait: true,
            },
        );
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("delete"));
        assert!(joined.contains("--ignore-not-found"));
        assert!(joined.contains("--wait"));
    }

    #[test]
    fn rollout_status_includes_timeout() {
        let op = Kubectl::kubectl().rollout_status(
            "deployment/hermes",
            RolloutOptions {
                timeout_secs: Some(120),
            },
        );
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("rollout"));
        assert!(joined.contains("status"));
        assert!(joined.contains("deployment/hermes"));
        assert!(joined.contains("120s"));
    }

    #[test]
    fn wait_condition_builds_for_flag() {
        let op = Kubectl::kubectl().wait_condition("pod/hermes-0", "condition=Ready", 300);
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("wait"));
        assert!(joined.contains("pod/hermes-0"));
        assert!(joined.contains("--for=condition=Ready"));
        assert!(joined.contains("300s"));
    }

    #[test]
    fn wait_condition_namespace_not_in_resource_name() {
        let op = Kubectl::kubectl().with_namespace("nebula").wait_condition(
            "certificate/knot-resolver-tls",
            "condition=Ready",
            120,
        );
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("certificate/knot-resolver-tls"));
        assert!(joined.contains("-n nebula"));
        assert!(!joined.contains("knot-resolver-tls -n"));
    }

    #[test]
    fn builder_setters_update_context() {
        let k = Kubectl::kubectl()
            .with_kubeconfig("/kc")
            .with_context("staging")
            .with_namespace("dev");
        assert_eq!(
            k.context().prefix_argv(),
            vec![
                "kubectl",
                "--kubeconfig",
                "/kc",
                "--context",
                "staging",
                "-n",
                "dev"
            ]
        );
    }

    #[test]
    fn kubectl_struct_cbor_roundtrip() {
        let k = Kubectl::k3s().with_namespace("hermes");
        let bytes = serde_cbor::to_vec(&k).unwrap();
        let back: Kubectl = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(k, back);
    }
}
