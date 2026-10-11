use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FetchAuth {
    #[default]
    NoAuth,
    CustomHeader {
        name: String,
        value: String,
    },
    BearerToken {
        token: String,
    },
    InstanceIdentity {
        provider: String,
    },
}

impl FetchAuth {
    pub fn http_headers(&self) -> Result<HashMap<String, String>, String> {
        // Header names/values must not contain CR/LF (or other control chars):
        // an injected newline would let a crafted bootstrap config split the
        // HTTP request (header injection) the moment a fetcher is wired up.
        let check = |kind: &str, s: &str| -> Result<(), String> {
            if s.is_empty() || s.chars().any(|c| c.is_control()) {
                return Err(format!("fetch auth {kind} contains control characters"));
            }
            Ok(())
        };
        match self {
            FetchAuth::NoAuth => Ok(HashMap::new()),
            FetchAuth::CustomHeader { name, value } => {
                check("header name", name)?;
                check("header value", value)?;
                let mut m = HashMap::new();
                m.insert(name.clone(), value.clone());
                Ok(m)
            }
            FetchAuth::BearerToken { token } => {
                check("bearer token", token)?;
                let mut m = HashMap::new();
                m.insert("Authorization".into(), format!("Bearer {token}"));
                Ok(m)
            }
            FetchAuth::InstanceIdentity { provider } => Err(format!(
                "InstanceIdentity for {provider} is not implemented (M6 stub)"
            )),
        }
    }
}
