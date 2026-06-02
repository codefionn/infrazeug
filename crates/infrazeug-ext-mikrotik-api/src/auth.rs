//! RouterOS API authentication material.

/// Username + password for `/login` (RouterOS 6.43+).
#[derive(Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    /// RouterOS user credentials.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_password() {
        let rendered = format!("{:?}", Credentials::new("admin", "secret"));
        assert!(rendered.contains("admin"));
        assert!(!rendered.contains("secret"));
    }
}
