//! Account listing and id resolution (`/accounts`).

use crate::client::CloudflareClient;
use crate::error::{CloudflareError, Result};
use crate::types::ListQuery;
use serde::Deserialize;

/// A Cloudflare account visible to the configured credentials.
#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct Account {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
}

impl Account {
    fn from_api(mut raw: AccountRaw) -> Option<Self> {
        let id = raw.id.take()?;
        Some(Self {
            id,
            name: raw.name.unwrap_or_default(),
            account_type: raw.account_type,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct AccountRaw {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type", default)]
    account_type: Option<String>,
}

impl CloudflareClient {
    /// `GET /accounts` — list accounts (all pages).
    pub async fn accounts(&self, query: &ListQuery) -> Result<Vec<Account>> {
        let raw: Vec<AccountRaw> = self.get_all("/accounts", query.clone()).await?;
        Ok(raw.into_iter().filter_map(Account::from_api).collect())
    }

    /// `GET /accounts/{id}` — fetch one account.
    pub async fn account(&self, account_id: &str) -> Result<Account> {
        let path = format!("/accounts/{}", self.encode_path(account_id));
        let raw: AccountRaw = self.get(&path, &ListQuery::default()).await?.0;
        Account::from_api(raw).ok_or_else(|| CloudflareError::Api {
            status: 404,
            codes: vec![],
            message: format!("account not found: {account_id}"),
        })
    }

    /// Resolve an account id from an explicit id, config default, or account name.
    pub async fn resolve_account_id(
        &self,
        account_id: Option<&str>,
        account_name: Option<&str>,
    ) -> Result<String> {
        if let Some(id) = account_id {
            return Ok(id.to_string());
        }
        if let Some(id) = &self.config().account_id {
            return Ok(id.clone());
        }
        if let Some(name) = account_name {
            return self.account_id_by_name(name).await;
        }
        let accounts = self
            .accounts(&ListQuery {
                per_page: Some(50),
                ..Default::default()
            })
            .await?;
        match accounts.len() {
            0 => Err(CloudflareError::Api {
                status: 404,
                codes: vec![],
                message: "no accessible cloudflare accounts".into(),
            }),
            1 => Ok(accounts[0].id.clone()),
            _ => Err(CloudflareError::Auth(
                "multiple cloudflare accounts: set account_id, account_name, or CLOUDFLARE_ACCOUNT_ID"
                    .into(),
            )),
        }
    }

    /// Resolve an account id from its display name (exact match).
    pub async fn account_id_by_name(&self, name: &str) -> Result<String> {
        let query = ListQuery {
            name: Some(name.into()),
            per_page: Some(50),
            ..Default::default()
        };
        let accounts = self.accounts(&query).await?;
        accounts
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .map(|a| a.id)
            .ok_or_else(|| CloudflareError::Api {
                status: 404,
                codes: vec![],
                message: format!("account not found: {name}"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_deserializes() {
        let raw = r#"{"id":"acc123","name":"Example","type":"standard"}"#;
        let account: AccountRaw = serde_json::from_str(raw).unwrap();
        let account = Account::from_api(account).unwrap();
        assert_eq!(account.id, "acc123");
        assert_eq!(account.name, "Example");
        assert_eq!(account.account_type.as_deref(), Some("standard"));
    }
}
