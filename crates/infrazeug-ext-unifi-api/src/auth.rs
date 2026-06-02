//! UniFi controller authentication material.

/// How the client authenticates to the controller.
///
/// - [`UserPass`](Self::UserPass): the classic local-admin login flow. The client
///   `POST`s to the login endpoint, keeps the returned session cookie, and replays
///   the rotating CSRF token on mutating requests. This is the flow the REST
///   resource endpoints (`wlanconf`, `networkconf`, `portforward`, …) require.
/// - [`ApiKey`](Self::ApiKey): a controller API key sent as `X-API-KEY` on every
///   request with no login step (UniFi Network 9+).
#[derive(Clone)]
pub enum Credentials {
    /// Local controller administrator username + password.
    UserPass { username: String, password: String },
    /// A controller API key (`X-API-KEY`).
    ApiKey(String),
}

impl Credentials {
    /// Username + password session login.
    pub fn user_pass(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::UserPass {
            username: username.into(),
            password: password.into(),
        }
    }

    /// Controller API key (`X-API-KEY`).
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::ApiKey(key.into())
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserPass { username, .. } => f
                .debug_struct("UserPass")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
            Self::ApiKey(_) => f.debug_tuple("ApiKey").field(&"<redacted>").finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let rendered = format!("{:?}", Credentials::user_pass("admin", "super-secret"));
        assert!(rendered.contains("admin"));
        assert!(!rendered.contains("super-secret"));
        let rendered = format!("{:?}", Credentials::api_key("key-material"));
        assert!(!rendered.contains("key-material"));
    }
}
