//! Proxmox VE authentication (API token and login ticket flows).
//!
//! Proxmox supports two credential styles:
//!
//! - **API token** (recommended): a `user@realm!tokenid` identifier plus a secret
//!   UUID, sent verbatim in a single `Authorization: PVEAPIToken=...` header. No
//!   CSRF token is required, even for writes.
//! - **Login ticket**: exchange a `user@realm` username and password at
//!   `/access/ticket` for a short-lived ticket (carried in the `PVEAuthCookie`
//!   cookie) plus a `CSRFPreventionToken` that must accompany every mutating
//!   request (`POST`/`PUT`/`DELETE`).

use crate::error::{ProxmoxError, Result};
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Proxmox login tickets are valid for two hours; refresh well before that.
const TICKET_LIFETIME_SECS: u64 = 2 * 60 * 60;
const TICKET_REFRESH_SKEW_SECS: u64 = 10 * 60;

/// Authentication material for Proxmox VE API calls.
#[derive(Clone)]
pub enum Auth {
    /// API token: `Authorization: PVEAPIToken=<token_id>=<secret>`.
    ApiToken {
        /// Full token identifier, e.g. `root@pam!automation`.
        token_id: String,
        /// Token secret (UUID issued by Proxmox).
        secret: String,
    },
    /// Username/password login that exchanges credentials for a ticket + CSRF token.
    Ticket {
        /// Username including realm, e.g. `root@pam`.
        username: String,
        /// Account password.
        password: String,
        provider: Arc<TicketProvider>,
    },
}

impl Auth {
    /// API token authentication. `token_id` must include the realm and token name
    /// (`user@realm!tokenid`); `secret` is the issued token value.
    pub fn api_token(token_id: impl Into<String>, secret: impl Into<String>) -> Self {
        Self::ApiToken {
            token_id: token_id.into(),
            secret: secret.into(),
        }
    }

    /// Ticket (login) authentication. `username` must include the realm, e.g.
    /// `root@pam`.
    pub fn ticket(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Ticket {
            username: username.into(),
            password: password.into(),
            provider: Arc::new(TicketProvider::new()),
        }
    }

    /// Apply this credential to an outgoing request.
    ///
    /// `api_base` is the API root (`https://host:8006/api2/json`); it is only used by
    /// the ticket flow to fetch/refresh a ticket. `is_write` selects whether the CSRF
    /// header is attached (required for `POST`/`PUT`/`DELETE` under ticket auth).
    pub(crate) async fn apply(
        &self,
        http: &Client,
        api_base: &str,
        req: RequestBuilder,
        is_write: bool,
    ) -> Result<RequestBuilder> {
        match self {
            Self::ApiToken { token_id, secret } => {
                Ok(req.header("Authorization", format!("PVEAPIToken={token_id}={secret}")))
            }
            Self::Ticket {
                username,
                password,
                provider,
            } => {
                let ticket = provider.ticket(http, api_base, username, password).await?;
                let mut req = req.header("Cookie", format!("PVEAuthCookie={}", ticket.ticket));
                if is_write {
                    req = req.header("CSRFPreventionToken", ticket.csrf_token.clone());
                }
                Ok(req)
            }
        }
    }
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiToken { token_id, .. } => f
                .debug_struct("ApiToken")
                .field("token_id", token_id)
                .field("secret", &"<redacted>")
                .finish(),
            Self::Ticket { username, .. } => f
                .debug_struct("Ticket")
                .field("username", username)
                .field("password", &"<redacted>")
                .finish(),
        }
    }
}

#[derive(Clone)]
struct CachedTicket {
    ticket: String,
    csrf_token: String,
    expires_at: u64,
}

/// Fetches and caches Proxmox login tickets for the [`Auth::Ticket`] flow.
#[derive(Default)]
pub struct TicketProvider {
    cache: RwLock<Option<CachedTicket>>,
}

impl std::fmt::Debug for TicketProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TicketProvider")
            .field("cache", &"<redacted>")
            .finish()
    }
}

impl TicketProvider {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(None),
        }
    }

    async fn ticket(
        &self,
        http: &Client,
        api_base: &str,
        username: &str,
        password: &str,
    ) -> Result<CachedTicket> {
        let now = unix_now();
        if let Some(cached) = self.cache.read().await.clone() {
            if cached.expires_at > now + TICKET_REFRESH_SKEW_SECS {
                return Ok(cached);
            }
        }
        let fresh = self.fetch(http, api_base, username, password, now).await?;
        *self.cache.write().await = Some(fresh.clone());
        Ok(fresh)
    }

    async fn fetch(
        &self,
        http: &Client,
        api_base: &str,
        username: &str,
        password: &str,
        now: u64,
    ) -> Result<CachedTicket> {
        let url = format!("{}/access/ticket", api_base.trim_end_matches('/'));
        let resp = http
            .post(url)
            .form(&[("username", username), ("password", password)])
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(ProxmoxError::Auth(format!(
                "ticket request failed ({}): {}",
                status.as_u16(),
                body.trim()
            )));
        }
        let parsed: TicketEnvelope = serde_json::from_str(&body)?;
        Ok(CachedTicket {
            ticket: parsed.data.ticket,
            csrf_token: parsed.data.csrf_token,
            expires_at: now + TICKET_LIFETIME_SECS,
        })
    }
}

#[derive(Deserialize)]
struct TicketEnvelope {
    data: TicketData,
}

#[derive(Deserialize)]
struct TicketData {
    ticket: String,
    #[serde(rename = "CSRFPreventionToken")]
    csrf_token: String,
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_secrets() {
        let rendered = format!("{:?}", Auth::api_token("root@pam!ci", "super-secret"));
        assert!(rendered.contains("root@pam!ci"));
        assert!(!rendered.contains("super-secret"));

        let rendered = format!("{:?}", Auth::ticket("root@pam", "hunter2"));
        assert!(rendered.contains("root@pam"));
        assert!(!rendered.contains("hunter2"));
    }

    #[tokio::test]
    async fn api_token_sets_authorization_header() {
        let http = Client::new();
        let auth = Auth::api_token("root@pam!ci", "secret-uuid");
        let req = http.get("https://pve.example:8006/api2/json/version");
        let req = auth
            .apply(&http, "https://pve.example:8006/api2/json", req, false)
            .await
            .unwrap();
        let built = req.build().unwrap();
        assert_eq!(
            built.headers().get("Authorization").unwrap(),
            "PVEAPIToken=root@pam!ci=secret-uuid"
        );
    }

    #[test]
    fn parses_ticket_envelope() {
        let body =
            r#"{"data":{"ticket":"PVE:tkt","CSRFPreventionToken":"csrf","username":"root@pam"}}"#;
        let parsed: TicketEnvelope = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.data.ticket, "PVE:tkt");
        assert_eq!(parsed.data.csrf_token, "csrf");
    }
}
