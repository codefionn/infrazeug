//! AWS credential configuration.

/// Long-term or temporary AWS credentials for SigV4 signing.
#[derive(Clone)]
pub struct AwsCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

impl std::fmt::Debug for AwsCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsCredentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"***")
            .field("session_token", &self.session_token.as_ref().map(|_| "***"))
            .finish()
    }
}

impl AwsCredentials {
    pub fn new(access_key_id: impl Into<String>, secret_access_key: impl Into<String>) -> Self {
        Self {
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
            session_token: None,
        }
    }

    pub fn with_session_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let creds = AwsCredentials::new("AKIAEXAMPLE", "super-secret")
            .with_session_token("temp-session-token");
        let rendered = format!("{creds:?}");
        assert!(rendered.contains("AKIAEXAMPLE"));
        assert!(rendered.contains("***"));
        assert!(!rendered.contains("super-secret"));
        assert!(!rendered.contains("temp-session-token"));
    }
}
