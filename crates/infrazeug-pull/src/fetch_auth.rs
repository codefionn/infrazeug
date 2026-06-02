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
        match self {
            FetchAuth::NoAuth => Ok(HashMap::new()),
            FetchAuth::CustomHeader { name, value } => {
                let mut m = HashMap::new();
                m.insert(name.clone(), value.clone());
                Ok(m)
            }
            FetchAuth::BearerToken { token } => {
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
