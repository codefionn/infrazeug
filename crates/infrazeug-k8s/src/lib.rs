//! Shared Kubernetes CLI context for [`infrazeug-kubectl`] and [`infrazeug-helm`].
//!
//! [`KubeContext`] holds kubeconfig path, context name, and namespace defaults.
//! Extension crates lower to [`ShellOp`](infrazeug_shell::ShellOp) (tier 2) so
//! kubectl/helm nodes work in agentless SSH mode as well as agent push.
//!
//! Embed only this crate if you need shared flags; depend on `infrazeug-kubectl`
//! or `infrazeug-helm` for concrete resource helpers.

use std::path::{Path, PathBuf};

/// Global flags shared by kubectl and helm CLIs.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KubeContext {
    /// Executable prefix, e.g. `["kubectl"]` or `["k3s", "kubectl"]`.
    pub command: Vec<String>,
    pub kubeconfig: Option<PathBuf>,
    pub context: Option<String>,
    pub namespace: Option<String>,
    #[serde(default)]
    pub extra_flags: Vec<String>,
}

impl KubeContext {
    pub fn kubectl() -> Self {
        Self {
            command: vec!["kubectl".into()],
            ..Default::default()
        }
    }

    /// Prefix argv for k3s clusters (`k3s kubectl …`).
    pub fn k3s_kubectl() -> Self {
        Self {
            command: vec!["k3s".into(), "kubectl".into()],
            ..Default::default()
        }
    }

    pub fn helm() -> Self {
        Self {
            command: vec!["helm".into()],
            ..Default::default()
        }
    }

    pub fn with_kubeconfig(mut self, path: impl Into<PathBuf>) -> Self {
        self.kubeconfig = Some(path.into());
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    pub fn with_extra_flag(mut self, flag: impl Into<String>) -> Self {
        self.extra_flags.push(flag.into());
        self
    }

    /// Build argv prefix: command + `--kubeconfig` + `--context` + `-n`.
    pub fn prefix_argv(&self) -> Vec<String> {
        let mut argv = self.command.clone();
        if let Some(path) = &self.kubeconfig {
            argv.push("--kubeconfig".into());
            argv.push(path.display().to_string());
        }
        if let Some(ctx) = &self.context {
            argv.push("--context".into());
            argv.push(ctx.clone());
        }
        if let Some(ns) = &self.namespace {
            argv.push("-n".into());
            argv.push(ns.clone());
        }
        argv.extend(self.extra_flags.iter().cloned());
        argv
    }

    /// Space-joined command for embedding in `sh -c` pipelines.
    pub fn shell_command(&self) -> String {
        self.prefix_argv()
            .iter()
            .map(|s| shell_escape(s))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn join_argv(
    prefix: &[String],
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    let mut argv = prefix.to_vec();
    argv.extend(args.into_iter().map(|a| a.as_ref().to_string()));
    argv
}

/// Quote a string for safe use inside double-quoted POSIX `sh -c` scripts.
pub fn shell_escape(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '%' | '+' | '-' | '_' | '.' | '/'))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Path under `run_root`-style temp dirs; callers should pass a unique suffix.
pub fn staging_path(run_segment: &str, file_name: &str) -> PathBuf {
    Path::new("/tmp")
        .join("infrazeug-k8s")
        .join(run_segment)
        .join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_argv_includes_flags() {
        let ctx = KubeContext::kubectl()
            .with_kubeconfig("/etc/kube/config")
            .with_context("prod")
            .with_namespace("apps");
        assert_eq!(
            ctx.prefix_argv(),
            vec![
                "kubectl",
                "--kubeconfig",
                "/etc/kube/config",
                "--context",
                "prod",
                "-n",
                "apps"
            ]
        );
    }

    #[test]
    fn k3s_prefix_argv() {
        let ctx = KubeContext::k3s_kubectl().with_namespace("kube-system");
        assert_eq!(
            ctx.prefix_argv(),
            vec!["k3s", "kubectl", "-n", "kube-system"]
        );
    }

    #[test]
    fn helm_prefix_argv() {
        assert_eq!(KubeContext::helm().prefix_argv(), vec!["helm"]);
    }

    #[test]
    fn extra_flags_appended() {
        let ctx = KubeContext::kubectl()
            .with_extra_flag("--insecure-skip-tls-verify")
            .with_extra_flag("--request-timeout=30s");
        assert_eq!(
            ctx.prefix_argv(),
            vec![
                "kubectl",
                "--insecure-skip-tls-verify",
                "--request-timeout=30s"
            ]
        );
    }

    #[test]
    fn shell_command_quotes_kubeconfig() {
        let ctx = KubeContext::kubectl().with_kubeconfig("/path/with spaces/config");
        assert_eq!(
            ctx.shell_command(),
            "kubectl --kubeconfig '/path/with spaces/config'"
        );
    }

    #[test]
    fn join_argv_extends_prefix() {
        assert_eq!(
            join_argv(
                &["kubectl".into(), "-n".into(), "apps".into()],
                ["get", "pods"]
            ),
            vec!["kubectl", "-n", "apps", "get", "pods"]
        );
    }

    #[test]
    fn shell_escape_simple_tokens() {
        assert_eq!(shell_escape("hermes"), "hermes");
        assert_eq!(shell_escape("prod-cluster-1"), "prod-cluster-1");
    }

    #[test]
    fn shell_escape_empty_and_special() {
        assert_eq!(shell_escape(""), "''");
        assert_eq!(shell_escape("has space"), "'has space'");
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn staging_path_is_namespaced() {
        assert_eq!(
            staging_path("hermes-deploy", "manifest.yaml"),
            PathBuf::from("/tmp/infrazeug-k8s/hermes-deploy/manifest.yaml")
        );
    }

    #[test]
    fn kube_context_cbor_roundtrip() {
        let ctx = KubeContext::k3s_kubectl()
            .with_kubeconfig("/etc/k3s/kubeconfig")
            .with_namespace("hermes");
        let bytes = serde_cbor::to_vec(&ctx).unwrap();
        let back: KubeContext = serde_cbor::from_slice(&bytes).unwrap();
        assert_eq!(ctx, back);
    }
}
