//! Migration helpers (Ansible Vault → infrazeug vault store).
//!
//! [`migrate_ansible_vault`] decrypts Ansible Vault blobs with a passphrase,
//! maps YAML keys into infrazeug vault files, and writes through a configured
//! [`VaultStore`](infrazeug_secrets::VaultStore). [`yaml_str_to_vault_map`] /
//! [`vault_map_to_yaml`] support round-tripping for `vault edit` workflows.
//!
//! Invoked from `infrazeug migrate ansible-vault` in [`infrazeug-cli`].

mod ansible;
mod error;
mod yaml;

pub use ansible::{
    migrate_ansible_vault, read_passphrase_file, AnsibleVaultMigrateOptions, MigrateReport,
    MigratedFile,
};
pub use error::MigrateError;
pub use yaml::{vault_map_to_yaml, yaml_str_to_vault_map};
