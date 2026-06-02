//! Resolve how a DataKey is unlocked from an explicit recipient or the default recipient.

use anyhow::Context;
use clap::Args;
use infrazeug_secrets::{PassphraseProvider, Provider, ProviderKind, RecipientEntry, VaultStore};
#[cfg(feature = "fido2-device")]
use infrazeug_secrets_hw::{Fido2Device, Fido2DeviceConfig};
use infrazeug_secrets_hw::{Fido2Provider, Pkcs11Provider};
use infrazeug_secrets_kms::{AgeProvider, EnvKmsProvider, KmsConfig};
use infrazeug_secrets_ssh::SshAgentProvider;
use std::path::PathBuf;

/// Default relying-party id for FIDO2 credentials; must match between enroll and unlock.
pub const DEFAULT_FIDO2_RP_ID: &str = "infrazeug.local";

/// Shared clap flags for choosing how a DataKey is unlocked.
#[derive(Args, Default)]
pub struct UnlockArgs {
    /// Unlock passphrase (otherwise password file, stdin, or prompt).
    #[arg(long)]
    pub passphrase: Option<String>,
    /// File whose first line is the unlock passphrase.
    #[arg(long)]
    pub password_file: Option<PathBuf>,
    /// Recipient label to unlock with (defaults to the DataKey's default recipient).
    #[arg(long)]
    pub unlock_label: Option<String>,
    /// Unlock with a real FIDO2 authenticator (credential id comes from the recipient entry).
    #[arg(long)]
    pub fido2_device: bool,
    /// Relying-party id for `--fido2-device`.
    #[arg(long, default_value = DEFAULT_FIDO2_RP_ID)]
    pub fido2_rp_id: String,
    #[arg(long)]
    pub fido2_credential: Option<String>,
    #[arg(long)]
    pub fido2_pin: Option<String>,
    #[arg(long)]
    pub pkcs11_slot: Option<String>,
    #[arg(long)]
    pub pkcs11_pin: Option<String>,
    #[arg(long)]
    pub age_identity: Option<String>,
    #[arg(long)]
    pub kms_key_id: Option<String>,
}

impl UnlockArgs {
    pub fn to_opts(&self) -> UnlockOpts {
        UnlockOpts {
            passphrase: self.passphrase.clone(),
            password_file: self.password_file.clone(),
            label: self.unlock_label.clone(),
            fido2_device: self.fido2_device,
            fido2_rp_id: self.fido2_rp_id.clone(),
            fido2_credential: self.fido2_credential.clone(),
            fido2_pin: self.fido2_pin.clone(),
            pkcs11_slot: self.pkcs11_slot.clone(),
            pkcs11_pin: self.pkcs11_pin.clone(),
            age_identity: self.age_identity.clone(),
            kms_key_id: self.kms_key_id.clone(),
        }
    }
}

/// Unlock inputs gathered from CLI flags.
#[derive(Default)]
pub struct UnlockOpts {
    pub passphrase: Option<String>,
    pub password_file: Option<PathBuf>,
    /// Recipient label to unlock with (defaults to the DataKey's default recipient).
    pub label: Option<String>,
    pub fido2_device: bool,
    pub fido2_rp_id: String,
    pub fido2_credential: Option<String>,
    pub fido2_pin: Option<String>,
    pub pkcs11_slot: Option<String>,
    pub pkcs11_pin: Option<String>,
    pub age_identity: Option<String>,
    pub kms_key_id: Option<String>,
}

impl UnlockOpts {
    fn explicit_kind(&self) -> Option<ProviderKind> {
        if self.passphrase.is_some() || self.password_file.is_some() {
            Some(ProviderKind::Passphrase)
        } else if self.fido2_device || self.fido2_credential.is_some() {
            Some(ProviderKind::Fido2)
        } else if self.pkcs11_slot.is_some() {
            Some(ProviderKind::Pkcs11)
        } else if self.age_identity.is_some() {
            Some(ProviderKind::Age)
        } else if self.kms_key_id.is_some() {
            Some(ProviderKind::Kms)
        } else {
            None
        }
    }

    fn provider_for(
        &self,
        recipient: &RecipientEntry,
        prompt: &str,
    ) -> anyhow::Result<Box<dyn Provider>> {
        match recipient.kind {
            ProviderKind::Passphrase => {
                let pp = crate::passphrase::resolve_passphrase(
                    self.passphrase.clone(),
                    self.password_file.as_deref(),
                    None,
                    prompt,
                )?;
                Ok(Box::new(PassphraseProvider::new(pp)))
            }
            ProviderKind::SshAgent => {
                let comment = param_str(recipient, "comment")
                    .context("ssh-agent recipient is missing key comment")?;
                Ok(Box::new(SshAgentProvider::new(comment)))
            }
            ProviderKind::Fido2 => self.fido2_provider_for(recipient),
            ProviderKind::Pkcs11 => self.pkcs11_provider_for(recipient),
            ProviderKind::Age => {
                let identity = self
                    .age_identity
                    .as_deref()
                    .context("age unlock requires --age-identity")?;
                Ok(Box::new(AgeProvider::from_identity_str(identity)?))
            }
            ProviderKind::Kms => {
                let key_id = self
                    .kms_key_id
                    .as_deref()
                    .or_else(|| param_str(recipient, "key_id"))
                    .context("KMS recipient is missing key_id")?;
                Ok(Box::new(EnvKmsProvider::from_env(KmsConfig {
                    key_id: key_id.to_string(),
                })?))
            }
        }
    }

    fn fido2_provider_for(&self, recipient: &RecipientEntry) -> anyhow::Result<Box<dyn Provider>> {
        if self.fido2_device {
            return self.fido2_device_provider_for(recipient);
        }
        if param_str(recipient, "device") == Some("fido2") {
            return self.fido2_device_provider_for(recipient);
        }
        let cred = self
            .fido2_credential
            .as_deref()
            .or_else(|| param_str(recipient, "credential_id"))
            .context("FIDO2 recipient is missing credential_id")?;
        let pin = match self.fido2_pin.clone() {
            Some(pin) => pin,
            None => crate::passphrase::resolve_passphrase(None, None, None, "FIDO2 PIN: ")?,
        };
        Ok(Box::new(Fido2Provider::new(cred.to_string(), pin)))
    }

    fn fido2_device_provider_for(
        &self,
        recipient: &RecipientEntry,
    ) -> anyhow::Result<Box<dyn Provider>> {
        #[cfg(feature = "fido2-device")]
        {
            let pin = crate::passphrase::resolve_optional_secret(
                self.fido2_pin.clone(),
                "FIDO2 PIN (blank for built-in UV): ",
            )?;
            let rp_id = param_str(recipient, "rp_id").unwrap_or(&self.fido2_rp_id);
            let mut cfg = Fido2DeviceConfig::new(rp_id.to_string());
            if let Some(pin) = pin {
                cfg = cfg.with_pin(pin);
            }
            println!("touch your authenticator to unlock…");
            Ok(Box::new(Fido2Device::new(cfg)))
        }
        #[cfg(not(feature = "fido2-device"))]
        {
            let _ = recipient;
            anyhow::bail!(
                "FIDO2 device unlock requires building the CLI with --features fido2-device"
            );
        }
    }

    fn pkcs11_provider_for(&self, recipient: &RecipientEntry) -> anyhow::Result<Box<dyn Provider>> {
        let slot = self
            .pkcs11_slot
            .as_deref()
            .or_else(|| param_str(recipient, "slot"))
            .context("PKCS#11 recipient is missing slot")?;
        let pin = match self.pkcs11_pin.clone() {
            Some(pin) => pin,
            None => crate::passphrase::resolve_passphrase(None, None, None, "PKCS#11 PIN: ")?,
        };
        Ok(Box::new(Pkcs11Provider::new(slot.to_string(), pin)))
    }
}

fn param_str<'a>(recipient: &'a RecipientEntry, key: &str) -> Option<&'a str> {
    recipient.params.get(key).and_then(|v| v.as_str())
}

async fn select_recipient(
    vault: &VaultStore,
    data_key: &str,
    opts: &UnlockOpts,
) -> anyhow::Result<RecipientEntry> {
    let envelope = vault.load_envelope(data_key).await?;
    if let Some(label) = opts.label.as_deref() {
        return envelope
            .recipients
            .into_iter()
            .find(|r| r.label == label)
            .with_context(|| format!("recipient {label} not found in data key {data_key}"));
    }
    if let Some(kind) = opts.explicit_kind() {
        if let Some(recipient) = envelope.recipients.iter().find(|r| r.kind == kind) {
            return Ok(recipient.clone());
        }
    }
    envelope
        .recipients
        .into_iter()
        .next()
        .with_context(|| format!("data key {data_key} has no recipients"))
}

/// Unlock `data_key` in `vault` if still locked, using the method selected by `opts`.
pub async fn unlock_data_key(
    vault: &mut VaultStore,
    data_key: &str,
    opts: &UnlockOpts,
    prompt: &str,
) -> anyhow::Result<()> {
    if vault.is_unlocked(data_key) {
        return Ok(());
    }
    let recipient = select_recipient(vault, data_key, opts).await?;
    let provider = opts.provider_for(&recipient, prompt)?;
    vault
        .unlock_with_provider_label(data_key, provider.as_ref(), &recipient.label)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::{FsBackend, PassphraseProvider};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn default_unlock_uses_default_recipient_label() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut vault = VaultStore::new(backend, dir.path().to_path_buf());
        vault
            .keygen_passphrase("prod", "recovery-pass", "recovery")
            .await
            .unwrap();
        vault
            .add_recipient(
                "prod",
                &PassphraseProvider::new("operator-pass"),
                "operator",
            )
            .await
            .unwrap();
        vault
            .set_default_recipient("prod", "operator")
            .await
            .unwrap();
        vault.lock_all();

        unlock_data_key(
            &mut vault,
            "prod",
            &UnlockOpts {
                passphrase: Some("operator-pass".into()),
                ..UnlockOpts::default()
            },
            "Data key passphrase: ",
        )
        .await
        .unwrap();

        assert!(vault.is_unlocked("prod"));
    }

    #[tokio::test]
    async fn explicit_label_overrides_default_recipient() {
        let dir = tempdir().unwrap();
        let backend = Arc::new(FsBackend::new(dir.path()));
        let mut vault = VaultStore::new(backend, dir.path().to_path_buf());
        vault
            .keygen_passphrase("prod", "recovery-pass", "recovery")
            .await
            .unwrap();
        vault
            .add_recipient(
                "prod",
                &PassphraseProvider::new("operator-pass"),
                "operator",
            )
            .await
            .unwrap();
        vault
            .set_default_recipient("prod", "operator")
            .await
            .unwrap();
        vault.lock_all();

        unlock_data_key(
            &mut vault,
            "prod",
            &UnlockOpts {
                passphrase: Some("recovery-pass".into()),
                label: Some("recovery".into()),
                ..UnlockOpts::default()
            },
            "Data key passphrase: ",
        )
        .await
        .unwrap();

        assert!(vault.is_unlocked("prod"));
    }
}
