//! Application-key credentials for B2 Native API authentication.

/// B2 application key credentials.
///
/// Use the master application key from the Backblaze web UI, or a scoped
/// application key created via [`b2_create_key`](crate::application_key).
#[derive(Clone)]
pub struct Credentials {
    /// Application key ID (master key ID or a key created via `b2_create_key`).
    pub application_key_id: String,
    /// Secret application key.
    pub application_key: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("application_key_id", &self.application_key_id)
            .field("application_key", &"***")
            .finish()
    }
}

impl Credentials {
    /// Create credentials from an application key ID and secret.
    pub fn new(application_key_id: impl Into<String>, application_key: impl Into<String>) -> Self {
        Self {
            application_key_id: application_key_id.into(),
            application_key: application_key.into(),
        }
    }

    /// Build the HTTP Basic `Authorization` header value.
    pub fn basic_auth_header(&self) -> String {
        use base64::Engine;
        let raw = format!("{}:{}", self.application_key_id, self.application_key);
        format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let creds = Credentials::new("key-id-123", "super-secret");
        let rendered = format!("{creds:?}");
        assert!(rendered.contains("key-id-123"));
        assert!(rendered.contains("***"));
        assert!(!rendered.contains("super-secret"));
    }
}
