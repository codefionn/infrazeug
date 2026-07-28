//! age X25519 recipient.

use async_trait::async_trait;
use infrazeug_secrets::provider::{Provider, ProviderKind, RecipientEntry, WrapCtx};
use infrazeug_secrets::{Result, SecretsError};

pub struct AgeProvider {
    identity: age::x25519::Identity,
    recipient: age::x25519::Recipient,
}

impl AgeProvider {
    pub fn from_identity_str(identity: &str) -> Result<Self> {
        let id: age::x25519::Identity = identity
            .parse()
            .map_err(|e| SecretsError::Provider(format!("invalid age identity: {e}")))?;
        let recipient = id.to_public();
        Ok(Self {
            identity: id,
            recipient,
        })
    }

    pub fn generate() -> Result<(Self, String)> {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public();
        let recipient_str = recipient.to_string();
        Ok((
            Self {
                identity,
                recipient,
            },
            recipient_str,
        ))
    }

    pub fn recipient(&self) -> String {
        self.recipient.to_string()
    }

    fn encrypt_dek(&self, dek: &[u8; 32], _ctx: &WrapCtx) -> Result<Vec<u8>> {
        let recipient: &dyn age::Recipient = &self.recipient;
        let encryptor = age::Encryptor::with_recipients(std::iter::once(recipient))
            .map_err(|e| SecretsError::Provider(e.to_string()))?;
        let mut wrapped = Vec::new();
        {
            let mut writer = encryptor
                .wrap_output(&mut wrapped)
                .map_err(|e| SecretsError::Provider(e.to_string()))?;
            use std::io::Write;
            writer
                .write_all(dek)
                .map_err(|e| SecretsError::Provider(e.to_string()))?;
            writer
                .finish()
                .map_err(|e| SecretsError::Provider(e.to_string()))?;
        }
        Ok(wrapped)
    }

    fn decrypt_dek(&self, blob: &[u8]) -> Result<Vec<u8>> {
        let decryptor = age::Decryptor::new_buffered(blob).map_err(|_| SecretsError::Decrypt)?;
        let mut reader = decryptor
            .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
            .map_err(|_| SecretsError::Decrypt)?;
        let mut out = Vec::new();
        use std::io::Read;
        reader
            .read_to_end(&mut out)
            .map_err(|_| SecretsError::Decrypt)?;
        Ok(out)
    }
}

#[async_trait]
impl Provider for AgeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Age
    }

    async fn wrap(&self, dek: &[u8; 32], ctx: &WrapCtx, label: &str) -> Result<RecipientEntry> {
        let wrapped_key = self.encrypt_dek(dek, ctx)?;
        Ok(RecipientEntry {
            kind: ProviderKind::Age,
            label: label.to_string(),
            wrapped_key,
            params: serde_json::json!({
                "recipient": self.recipient.to_string(),
            }),
        })
    }

    async fn unwrap(&self, entry: &RecipientEntry, _ctx: &WrapCtx) -> Result<Vec<u8>> {
        self.decrypt_dek(&entry.wrapped_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infrazeug_secrets::{create_envelope, generate_dek, unlock_envelope};

    #[tokio::test]
    async fn age_roundtrip() {
        let (provider, _recipient) = AgeProvider::generate().unwrap();
        let dek = generate_dek();
        let envelope = create_envelope("prod", &dek, &provider, "age-key")
            .await
            .unwrap();
        let entry = &envelope.recipients[0];
        let unlocked = unlock_envelope(&envelope, &provider, entry).await.unwrap();
        assert_eq!(&*unlocked, &dek);
    }
}
