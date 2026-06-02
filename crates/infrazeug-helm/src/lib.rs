//! Helm helpers that produce [`ShellOp`] nodes (SOUL tier 2).
//!
//! # Example
//!
//! ```ignore
//! use infrazeug_helm::{Helm, HelmChart, UpgradeOptions};
//!
//! let helm = Helm::new().with_namespace("apps");
//! let op = helm.upgrade_install(
//!     "ntfy",
//!     HelmChart::Path("./charts/ntfy".into()),
//!     UpgradeOptions::default(),
//!     "stage-01",
//! );
//! ```

use infrazeug_k8s::{join_argv, staging_path, KubeContext};
use infrazeug_shell::{FileSource, ShellOp};
use std::path::PathBuf;

/// Helm CLI wrapper lowering to serializable [`ShellOp`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Helm {
    ctx: KubeContext,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HelmChart {
    Path(PathBuf),
    /// Remote chart reference, e.g. `bitnami/nginx` or `oci://registry/chart`.
    Reference(String),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpgradeOptions {
    pub create_namespace: bool,
    pub wait: bool,
    /// e.g. `15m` when `wait` is true (`helm upgrade --timeout`).
    pub timeout: Option<String>,
    pub atomic: bool,
    pub values_yaml: Option<String>,
    pub values_files: Vec<PathBuf>,
    pub set: Vec<(String, String)>,
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UninstallOptions {
    pub wait: bool,
    pub ignore_not_found: bool,
}

impl Helm {
    pub fn new() -> Self {
        Self {
            ctx: KubeContext::helm(),
        }
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

    fn argv(&self, args: Vec<String>) -> Vec<String> {
        join_argv(&self.ctx.prefix_argv(), args)
    }

    /// `helm repo add` (idempotent: ignores "already exists" exit 1).
    pub fn repo_add(&self, name: &str, url: &str) -> ShellOp {
        let helm = self.ctx.shell_command();
        let name_esc = infrazeug_k8s::shell_escape(name);
        let url_esc = infrazeug_k8s::shell_escape(url);
        let script = format!(
            "set -e; {helm} repo add {name_esc} {url_esc} || {{ rc=$?; \
             {helm} repo list 2>/dev/null | grep -q '^{name_esc}[[:space:]]' || exit $rc; }}"
        );
        ShellOp::run(vec!["sh".into(), "-ec".into(), script])
    }

    /// `helm repo update`.
    pub fn repo_update(&self) -> ShellOp {
        ShellOp::run(self.argv(vec!["repo".into(), "update".into()]))
    }

    /// `helm dependency update` in a chart directory.
    pub fn dependency_update(&self, chart_dir: impl Into<PathBuf>) -> ShellOp {
        let dir = chart_dir.into();
        ShellOp::run(self.argv(vec![
            "dependency".into(),
            "update".into(),
            dir.display().to_string(),
        ]))
    }

    /// `helm upgrade --install` with optional staged values YAML.
    pub fn upgrade_install(
        &self,
        release: &str,
        chart: HelmChart,
        opts: UpgradeOptions,
        staging_id: &str,
    ) -> ShellOp {
        let mut args = vec!["upgrade".into(), "--install".into(), release.into()];
        match &chart {
            HelmChart::Path(p) => args.push(p.display().to_string()),
            HelmChart::Reference(r) => args.push(r.clone()),
        }
        if opts.create_namespace {
            args.push("--create-namespace".into());
        }
        if opts.wait {
            args.push("--wait".into());
            if let Some(t) = &opts.timeout {
                args.push("--timeout".into());
                args.push(t.clone());
            }
        }
        if opts.atomic {
            args.push("--atomic".into());
        }
        if let Some(ver) = &opts.version {
            args.push("--version".into());
            args.push(ver.clone());
        }
        for path in &opts.values_files {
            args.push("-f".into());
            args.push(path.display().to_string());
        }
        for (k, v) in &opts.set {
            args.push("--set".into());
            args.push(format!("{k}={v}"));
        }

        let mut steps = Vec::new();
        if let Some(yaml) = &opts.values_yaml {
            let path = staging_path(staging_id, "values.yaml");
            steps.push(ShellOp::EnsureDir {
                path: path.parent().expect("parent").to_path_buf(),
                mode: 0o700,
            });
            steps.push(ShellOp::WriteFile {
                path: path.clone(),
                content: FileSource::bytes(yaml.as_bytes()),
                mode: 0o600,
            });
            args.push("-f".into());
            args.push(path.display().to_string());
        }
        steps.push(ShellOp::run(self.argv(args)));
        if steps.len() == 1 {
            steps.into_iter().next().expect("one step")
        } else {
            ShellOp::Seq { steps }
        }
    }

    /// `helm uninstall`.
    pub fn uninstall(&self, release: &str, opts: UninstallOptions) -> ShellOp {
        let mut args = vec!["uninstall".into(), release.into()];
        if opts.wait {
            args.push("--wait".into());
        }
        if opts.ignore_not_found {
            let helm = self.ctx.shell_command();
            let rel = infrazeug_k8s::shell_escape(release);
            let script = format!(
                "{helm} uninstall {rel} || {{ rc=$?; \
                 {helm} list -q 2>/dev/null | grep -qx {rel} || exit $rc; }}"
            );
            return ShellOp::run(vec!["sh".into(), "-ec".into(), script]);
        }
        ShellOp::run(self.argv(args))
    }

    /// Run an arbitrary helm subcommand with explicit argv pieces.
    pub fn run(&self, args: &[&str]) -> ShellOp {
        ShellOp::run(self.argv(args.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()))
    }
}

impl Default for Helm {
    fn default() -> Self {
        Self::new()
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
    fn upgrade_install_with_values_stages_file() {
        let op = Helm::new().upgrade_install(
            "ntfy",
            HelmChart::Path("./charts/ntfy".into()),
            UpgradeOptions {
                values_yaml: Some("replicaCount: 1\n".into()),
                ..Default::default()
            },
            "ntfy-release",
        );
        let ShellOp::Seq { steps } = &op else {
            panic!("expected Seq");
        };
        assert_eq!(steps.len(), 3);
        let Lowered::Seq { steps: lowered } = lower(&op).unwrap() else {
            panic!("expected lowered seq");
        };
        assert_eq!(lowered.len(), 3);
        let Lowered::SftpWrite { content, .. } = &lowered[1] else {
            panic!("expected values write");
        };
        assert_eq!(content, b"replicaCount: 1\n");
    }

    #[test]
    fn upgrade_install_without_values_is_single_run() {
        let op = Helm::new().upgrade_install(
            "ntfy",
            HelmChart::Path("./charts/ntfy".into()),
            UpgradeOptions::default(),
            "ntfy",
        );
        assert!(matches!(op, ShellOp::Run { .. }));
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("helm"));
        assert!(joined.contains("upgrade"));
        assert!(joined.contains("--install"));
        assert!(joined.contains("ntfy"));
    }

    #[test]
    fn upgrade_install_reference_chart_and_options() {
        let op = Helm::new().with_namespace("apps").upgrade_install(
            "nginx",
            HelmChart::Reference("bitnami/nginx".into()),
            UpgradeOptions {
                create_namespace: true,
                wait: true,
                timeout: Some("15m".into()),
                atomic: true,
                version: Some("15.0.0".into()),
                values_files: vec!["./extra.yaml".into()],
                set: vec![("replicaCount".into(), "2".into())],
                values_yaml: None,
            },
            "nginx",
        );
        let joined = run_argv(&op).join(" ");
        assert!(joined.contains("-n"));
        assert!(joined.contains("apps"));
        assert!(joined.contains("bitnami/nginx"));
        assert!(joined.contains("--create-namespace"));
        assert!(joined.contains("--wait"));
        assert!(joined.contains("--atomic"));
        assert!(joined.contains("--version"));
        assert!(joined.contains("15.0.0"));
        assert!(joined.contains("./extra.yaml"));
        assert!(joined.contains("replicaCount=2"));
    }

    #[test]
    fn repo_add_wraps_idempotent_shell() {
        let op = Helm::new().repo_add("bitnami", "https://charts.bitnami.com/bitnami");
        let ShellOp::Run { argv, .. } = op else {
            panic!("expected Run");
        };
        assert_eq!(argv[0], "sh");
        assert!(argv[2].contains("helm repo add"));
        assert!(argv[2].contains("bitnami"));
        assert!(argv[2].contains("charts.bitnami.com"));
        assert!(argv[2].contains("repo list"));
    }

    #[test]
    fn repo_update_argv() {
        let joined = run_argv(&Helm::new().repo_update()).join(" ");
        assert_eq!(joined.matches("helm").count(), 1);
        assert!(joined.contains("repo"));
        assert!(joined.contains("update"));
    }

    #[test]
    fn dependency_update_includes_chart_dir() {
        let joined = run_argv(&Helm::new().dependency_update("/charts/ntfy")).join(" ");
        assert!(joined.contains("dependency"));
        assert!(joined.contains("update"));
        assert!(joined.contains("/charts/ntfy"));
    }

    #[test]
    fn uninstall_ignore_not_found_uses_shell_wrapper() {
        let op = Helm::new().uninstall(
            "ntfy",
            UninstallOptions {
                ignore_not_found: true,
                wait: false,
            },
        );
        let ShellOp::Run { argv, .. } = op else {
            panic!("expected Run");
        };
        assert!(argv[2].contains("helm uninstall"));
        assert!(argv[2].contains("ntfy"));
        assert!(argv[2].contains("helm list"));
    }

    #[test]
    fn uninstall_wait_flag() {
        let joined = run_argv(&Helm::new().uninstall(
            "ntfy",
            UninstallOptions {
                wait: true,
                ignore_not_found: false,
            },
        ))
        .join(" ");
        assert!(joined.contains("uninstall"));
        assert!(joined.contains("--wait"));
    }

    #[test]
    fn helm_op_cbor_roundtrip() {
        let op = Helm::new().repo_update();
        let bytes = serde_cbor::to_vec(&op).unwrap();
        let back: ShellOp = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(op, back);
    }

    #[test]
    fn helm_struct_cbor_roundtrip() {
        let h = Helm::new().with_namespace("apps").with_context("prod");
        let bytes = serde_cbor::to_vec(&h).unwrap();
        let back: Helm = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(h, back);
    }
}
