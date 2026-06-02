use crate::id::{GroupId, MachineId, RunId, Tag};
use crate::varset::VarSet;
use infrazeug_emulate::{ContainerRef, LikeConfig};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Machine {
    pub id: MachineId,
    pub name: String,
    pub kind: MachineKind,
    pub vars: VarSet,
    pub groups: Vec<GroupId>,
    pub tags: Vec<Tag>,
    pub max_parallel_nodes: Option<usize>,
    pub lifecycle: Lifecycle,
    /// Emulated twin for test / emulate-first (SOUL §5.4).
    #[serde(default)]
    pub like: Option<LikeConfig>,
    /// When true, transport (SSH, agent push) is set up on first use by a node
    /// instead of eagerly during prepare. Useful for machines provisioned by
    /// preceding nodes in the same run.
    #[serde(default)]
    pub lazy: bool,
}

/// Which IP protocol the transport may use when connecting (OpenSSH `AddressFamily`).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum AddressFamily {
    /// IPv4 or IPv6, let the resolver decide (`AddressFamily any`).
    #[default]
    Any,
    /// Force IPv4 only (`AddressFamily inet`, i.e. `ssh -4`).
    V4,
    /// Force IPv6 only (`AddressFamily inet6`, i.e. `ssh -6`).
    V6,
}

impl AddressFamily {
    /// OpenSSH `AddressFamily` value, or `None` when unrestricted.
    pub fn ssh_value(self) -> Option<&'static str> {
        match self {
            AddressFamily::Any => None,
            AddressFamily::V4 => Some("inet"),
            AddressFamily::V6 => Some("inet6"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshConfig {
    /// `user@host` or `host` (uses ssh config) or `host:port`.
    pub host: String,
    pub user: Option<String>,
    /// Extra `-o key=value` options passed to ssh/sftp.
    pub extra_opts: Vec<String>,
    /// Restrict the connection to IPv4 or IPv6 only.
    #[serde(default)]
    pub address_family: AddressFamily,
    /// How the transport authenticates. Defaults to non-interactive (SSH agent
    /// or unencrypted key only); set to prompt or read the vault for a login
    /// password or an encrypted private-key passphrase.
    #[serde(default)]
    pub auth: SshAuth,
}

/// How the SSH transport authenticates a connection.
///
/// `NonInteractive` is the hardened default (`BatchMode=yes`): only an SSH agent
/// or an unencrypted key works, and a connection that would otherwise prompt
/// fails fast. The other variants relax that for a single connection, feeding the
/// secret to OpenSSH through a controller-side askpass helper so the rest of the
/// transport stays non-interactive (the persistent control-master authenticates
/// once, later commands reuse the socket).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SshAuth {
    /// SSH agent / unencrypted key only; never prompts (`BatchMode=yes`).
    #[default]
    NonInteractive,
    /// Authenticate with a login password (password / keyboard-interactive).
    Password(SshSecret),
    /// Decrypt an encrypted private key with a passphrase.
    KeyPassphrase(SshSecret),
}

/// Where an interactive SSH secret (login password or key passphrase) comes from.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SshSecret {
    /// Prompt the operator once at connect time with hidden input.
    Prompt {
        /// Optional hint shown in the prompt (e.g. which key/account).
        #[serde(default)]
        hint: Option<String>,
    },
    /// Read from the controller vault: `file` and a dot-path `field`.
    Vault { file: String, field: String },
}

impl SshAuth {
    /// The configured secret source, if this auth method needs one.
    pub fn secret(&self) -> Option<&SshSecret> {
        match self {
            SshAuth::NonInteractive => None,
            SshAuth::Password(s) | SshAuth::KeyPassphrase(s) => Some(s),
        }
    }

    /// Whether this auth method requires interactive/secret-backed authentication.
    pub fn is_interactive(&self) -> bool {
        !matches!(self, SshAuth::NonInteractive)
    }

    /// Whether the secret unlocks an encrypted private key (vs a login password).
    pub fn is_key_passphrase(&self) -> bool {
        matches!(self, SshAuth::KeyPassphrase(_))
    }
}

impl SshConfig {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            user: None,
            extra_opts: Vec::new(),
            address_family: AddressFamily::Any,
            auth: SshAuth::NonInteractive,
        }
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    /// Set the SSH authentication method (default [`SshAuth::NonInteractive`]).
    pub fn with_auth(mut self, auth: SshAuth) -> Self {
        self.auth = auth;
        self
    }

    /// Prompt the operator for a login password at connect time.
    pub fn ask_password(self) -> Self {
        self.with_auth(SshAuth::Password(SshSecret::Prompt { hint: None }))
    }

    /// Prompt the operator for an encrypted private-key passphrase at connect time.
    pub fn ask_key_passphrase(self) -> Self {
        self.with_auth(SshAuth::KeyPassphrase(SshSecret::Prompt { hint: None }))
    }

    /// Read the login password from a vault `file`/`field`.
    pub fn password_from_vault(self, file: impl Into<String>, field: impl Into<String>) -> Self {
        self.with_auth(SshAuth::Password(SshSecret::Vault {
            file: file.into(),
            field: field.into(),
        }))
    }

    /// Read the private-key passphrase from a vault `file`/`field`.
    pub fn key_passphrase_from_vault(
        self,
        file: impl Into<String>,
        field: impl Into<String>,
    ) -> Self {
        self.with_auth(SshAuth::KeyPassphrase(SshSecret::Vault {
            file: file.into(),
            field: field.into(),
        }))
    }

    /// Restrict the connection to a single IP protocol.
    pub fn with_address_family(mut self, family: AddressFamily) -> Self {
        self.address_family = family;
        self
    }

    /// Force the connection over IPv4 only (`AddressFamily inet`).
    pub fn ipv4_only(self) -> Self {
        self.with_address_family(AddressFamily::V4)
    }

    /// Force the connection over IPv6 only (`AddressFamily inet6`).
    pub fn ipv6_only(self) -> Self {
        self.with_address_family(AddressFamily::V6)
    }

    /// `AddressFamily=…` ssh `-o` option value when a family is enforced.
    pub fn address_family_opt(&self) -> Option<String> {
        self.address_family
            .ssh_value()
            .map(|v| format!("AddressFamily={v}"))
    }

    /// SSH destination string (`user@host` or host).
    pub fn destination(&self) -> String {
        if let Some(user) = &self.user {
            if self.host.contains('@') {
                self.host.clone()
            } else {
                format!("{user}@{}", self.host)
            }
        } else {
            self.host.clone()
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum OsFamily {
    Linux,
    Freebsd,
    Windows,
    Macos,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OsHint {
    pub family: OsFamily,
    pub distro: Option<String>,
    pub version: Option<String>,
    /// Machine hardware name (`uname -m`), when known at playbook definition time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MachineKind {
    Local,
    Remote { ssh: SshConfig, os: Option<OsHint> },
    Container(ContainerRef),
}

/// A machine produced at apply time by a dynamic-group discovery method.
///
/// The discovery [`NodeMethod`](infrazeug_native::NodeMethod) emits a JSON array
/// of these as its node capture; the scheduler turns each into a lazy push
/// [`Machine`] and fans the dynamic group's template out over it. The connection
/// data (`ssh`) is expected to be fully prepared by the discovery method and any
/// upstream prep nodes it reads from.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredMachine {
    /// Stable, unique-within-group machine name (drives the deterministic id).
    pub name: String,
    /// Fully-resolved SSH connection for the machine.
    pub ssh: SshConfig,
    /// Machine-level vars surfaced to the template's nodes.
    #[serde(default)]
    pub vars: VarSet,
    /// Tags applied to the machine (selection / grouping).
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// Optional OS / arch hint to skip the runtime `uname` probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<OsHint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Lifecycle {
    Persistent,
    Ephemeral { owner: RunId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub vars: VarSet,
}

/// Static machine facts for controller UIs (TUI machine grid, §6ter.4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSummary {
    pub name: String,
    /// Reachable host: SSH destination, image ref, or `localhost`.
    pub endpoint: String,
    pub kind: String,
    /// OS family / arch when declared on the machine.
    pub os_hint: Option<String>,
}

/// Framework-supplied resolved machine view (SOUL §3.9).
#[derive(Clone, Debug)]
pub struct MachineSpec {
    pub id: MachineId,
    pub name: String,
    pub tags: Vec<Tag>,
    pub groups: Vec<GroupId>,
}

impl Machine {
    pub fn lazy(mut self) -> Self {
        self.lazy = true;
        self
    }

    pub fn spec(&self) -> MachineSpec {
        MachineSpec {
            id: self.id,
            name: self.name.clone(),
            tags: self.tags.clone(),
            groups: self.groups.clone(),
        }
    }

    /// Human-readable identity for dashboards (name, host, kind, OS).
    pub fn summary(&self) -> MachineSummary {
        let (kind, endpoint, os_hint) = match &self.kind {
            MachineKind::Local => ("local".into(), "localhost".into(), None),
            MachineKind::Remote { ssh, os } => (
                "remote".into(),
                ssh.destination(),
                os.as_ref().map(format_os_hint),
            ),
            MachineKind::Container(c) => ("container".into(), container_endpoint(c), None),
        };
        MachineSummary {
            name: self.name.clone(),
            endpoint,
            kind,
            os_hint,
        }
    }
}

fn container_endpoint(c: &ContainerRef) -> String {
    match c {
        ContainerRef::Prebuilt(img) => img.reference(),
        ContainerRef::Spec(_) => "container spec".into(),
    }
}

fn format_os_hint(h: &OsHint) -> String {
    let family = match h.family {
        OsFamily::Linux => "linux",
        OsFamily::Freebsd => "freebsd",
        OsFamily::Windows => "windows",
        OsFamily::Macos => "macos",
        OsFamily::Other => "other",
    };
    let mut parts = vec![family.to_string()];
    if let Some(d) = &h.distro {
        parts.push(d.clone());
    }
    if let Some(v) = &h.version {
        parts.push(v.clone());
    }
    if let Some(a) = &h.arch {
        parts.push(a.clone());
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::uuid as parse_uuid;
    use crate::id::MachineId;

    #[test]
    fn remote_summary_uses_ssh_destination_and_os() {
        let m = Machine {
            id: MachineId(parse_uuid("00000000-0000-4000-8000-000000000001")),
            name: "web-01".into(),
            kind: MachineKind::Remote {
                ssh: SshConfig::new("10.0.0.5").with_user("deploy"),
                os: Some(OsHint {
                    family: OsFamily::Linux,
                    distro: Some("debian".into()),
                    version: None,
                    arch: Some("x86_64".into()),
                }),
            },
            vars: VarSet::new(),
            groups: Vec::new(),
            tags: Vec::new(),
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: false,
        };
        let s = m.summary();
        assert_eq!(s.name, "web-01");
        assert_eq!(s.endpoint, "deploy@10.0.0.5");
        assert_eq!(s.kind, "remote");
        assert_eq!(s.os_hint.as_deref(), Some("linux · debian · x86_64"));
    }

    #[test]
    fn ssh_auth_defaults_to_non_interactive() {
        let ssh = SshConfig::new("example.com");
        assert_eq!(ssh.auth, SshAuth::NonInteractive);
        assert!(!ssh.auth.is_interactive());
        assert!(ssh.auth.secret().is_none());
    }

    #[test]
    fn ssh_auth_builders_set_method_and_source() {
        let pw = SshConfig::new("h").ask_password();
        assert!(matches!(
            pw.auth,
            SshAuth::Password(SshSecret::Prompt { .. })
        ));
        assert!(pw.auth.is_interactive());
        assert!(!pw.auth.is_key_passphrase());

        let key = SshConfig::new("h").key_passphrase_from_vault("keys", "passphrase");
        assert!(key.auth.is_key_passphrase());
        assert_eq!(
            key.auth.secret(),
            Some(&SshSecret::Vault {
                file: "keys".into(),
                field: "passphrase".into(),
            })
        );
    }

    #[test]
    fn ssh_config_auth_json_roundtrips_and_defaults_when_absent() {
        let ssh = SshConfig::new("h").ask_key_passphrase();
        let json = serde_json::to_string(&ssh).unwrap();
        let back: SshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(ssh, back);

        // Older payloads without `auth` deserialize to the non-interactive default.
        let legacy = r#"{"host":"h","user":null,"extra_opts":[],"address_family":"Any"}"#;
        let parsed: SshConfig = serde_json::from_str(legacy).unwrap();
        assert_eq!(parsed.auth, SshAuth::NonInteractive);
    }

    #[test]
    fn container_summary_uses_image_ref() {
        let m = Machine {
            id: MachineId(parse_uuid("00000000-0000-4000-8000-000000000002")),
            name: "svc".into(),
            kind: MachineKind::Container(ContainerRef::Prebuilt(
                infrazeug_emulate::ImageRef::docker_io("nginx", "latest"),
            )),
            vars: VarSet::new(),
            groups: Vec::new(),
            tags: Vec::new(),
            max_parallel_nodes: None,
            lifecycle: Lifecycle::Persistent,
            like: None,
            lazy: false,
        };
        let s = m.summary();
        assert_eq!(s.kind, "container");
        assert!(s.endpoint.contains("nginx"));
    }
}
