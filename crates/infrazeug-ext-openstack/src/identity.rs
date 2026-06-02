//! Keystone EC2/S3 credential management.

use crate::client::OpenstackClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// S3 access/secret key pair issued by Keystone (`OS-EC2` credential type).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ec2Credential {
    pub access: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Ec2CredentialList {
    #[serde(default)]
    credentials: Vec<Ec2CredentialRecord>,
}

#[derive(Debug, Deserialize)]
struct Ec2CredentialRecord {
    access: String,
    #[serde(default)]
    secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Ec2CredentialCreateResponse {
    credential: Ec2CredentialRecord,
}

#[derive(Serialize)]
struct Ec2CredentialCreateBody<'a> {
    tenant_id: &'a str,
}

impl OpenstackClient {
    /// `GET /v3/users/{user_id}/credentials/OS-EC2` — list EC2 credentials.
    pub async fn list_ec2_credentials(&self, user_id: &str) -> Result<Vec<Ec2Credential>> {
        let path = format!("/users/{}/credentials/OS-EC2", urlencoding::encode(user_id));
        let list: Ec2CredentialList = self.identity_get(&path).await?;
        Ok(list
            .credentials
            .into_iter()
            .map(|c| Ec2Credential {
                access: c.access,
                secret: c.secret,
            })
            .collect())
    }

    /// `POST /v3/users/{user_id}/credentials/OS-EC2` — create an EC2 credential.
    pub async fn create_ec2_credential(
        &self,
        user_id: &str,
        project_id: &str,
    ) -> Result<Ec2Credential> {
        let path = format!("/users/{}/credentials/OS-EC2", urlencoding::encode(user_id));
        let body = Ec2CredentialCreateBody {
            tenant_id: project_id,
        };
        let resp: Ec2CredentialCreateResponse = self.identity_post(&path, &body).await?;
        Ok(Ec2Credential {
            access: resp.credential.access,
            secret: resp.credential.secret,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_ec2_credential() {
        let body = r#"{"credential":{"access":"AKIAEXAMPLE","secret":"s3cr3t","type":"ec2"}}"#;
        let parsed: Ec2CredentialCreateResponse = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.credential.access, "AKIAEXAMPLE");
        assert_eq!(parsed.credential.secret.as_deref(), Some("s3cr3t"));
    }

    #[test]
    fn deserializes_ec2_list() {
        let body = r#"{"credentials":[{"access":"AK1","secret":"sk1"},{"access":"AK2"}]}"#;
        let parsed: Ec2CredentialList = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.credentials.len(), 2);
        assert!(parsed.credentials[1].secret.is_none());
    }
}
