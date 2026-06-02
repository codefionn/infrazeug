//! TCP/TLS client for the MikroTik RouterOS API.

use crate::auth::Credentials;
use crate::error::{MikrotikError, Result};
use crate::wire::{exchange, Reply, Sentence};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// Default plain API port.
pub const DEFAULT_PLAIN_PORT: u16 = 8728;
/// Default API-SSL port.
pub const DEFAULT_TLS_PORT: u16 = 8729;

/// Connection configuration for a RouterOS API client.
#[derive(Debug, Clone)]
pub struct MikrotikConfig {
    /// Router hostname or IP.
    pub host: String,
    /// API port (defaults to 8728 plain / 8729 TLS).
    pub port: u16,
    /// Use API-SSL (TLS) instead of plain TCP.
    pub tls: bool,
    /// Skip TLS certificate verification (self-signed RouterOS certs).
    pub accept_invalid_certs: bool,
}

impl MikrotikConfig {
    /// Plain TCP on port 8728 with certificate verification enabled (TLS only).
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: DEFAULT_PLAIN_PORT,
            tls: false,
            accept_invalid_certs: false,
        }
    }

    /// Override the API port.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Use API-SSL (port 8729 unless overridden).
    pub fn with_tls(mut self, tls: bool) -> Self {
        self.tls = tls;
        if tls && self.port == DEFAULT_PLAIN_PORT {
            self.port = DEFAULT_TLS_PORT;
        }
        self
    }

    /// Toggle acceptance of invalid TLS certificates.
    pub fn with_accept_invalid_certs(mut self, accept: bool) -> Self {
        self.accept_invalid_certs = accept;
        self
    }

    /// Skip TLS certificate verification for API-SSL (ignore the router's
    /// self-signed certificate). Shorthand for `with_accept_invalid_certs(true)`.
    /// Has no effect on plain TCP (port 8728).
    pub fn insecure(self) -> Self {
        self.with_accept_invalid_certs(true)
    }
}

enum Transport {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

impl AsyncRead for Transport {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Transport::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            Transport::Tls(s) => std::pin::Pin::new(s).as_mut().poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Transport {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            Transport::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            Transport::Tls(s) => std::pin::Pin::new(s).as_mut().poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Transport::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            Transport::Tls(s) => std::pin::Pin::new(s).as_mut().poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Transport::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            Transport::Tls(s) => std::pin::Pin::new(s).as_mut().poll_shutdown(cx),
        }
    }
}

/// An authenticated MikroTik RouterOS API client (one connection per instance).
pub struct MikrotikClient {
    config: MikrotikConfig,
    credentials: Credentials,
    transport: Option<Transport>,
}

impl MikrotikClient {
    /// Build a client; call [`connect`](Self::connect) before issuing commands.
    pub fn new(config: MikrotikConfig, credentials: Credentials) -> Self {
        Self {
            config,
            credentials,
            transport: None,
        }
    }

    /// Connect and return a ready client.
    pub async fn open(config: MikrotikConfig, credentials: Credentials) -> Result<Self> {
        let mut client = Self::new(config, credentials);
        client.connect().await?;
        Ok(client)
    }

    /// Client configuration.
    pub fn config(&self) -> &MikrotikConfig {
        &self.config
    }

    /// Open TCP (or TLS) and log in (RouterOS 6.43+ plain-text password login).
    pub async fn connect(&mut self) -> Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let tcp = TcpStream::connect(&addr)
            .await
            .map_err(|e| MikrotikError::Transport(format!("connect {addr}: {e}")))?;
        let transport = if self.config.tls {
            let connector = build_tls_connector(self.config.accept_invalid_certs)?;
            let name = ServerName::try_from(self.config.host.as_str())
                .map_err(|_| MikrotikError::Tls("invalid server name".into()))?
                .to_owned();
            let tls = connector
                .connect(name, tcp)
                .await
                .map_err(|e| MikrotikError::Tls(e.to_string()))?;
            Transport::Tls(Box::new(tls))
        } else {
            Transport::Plain(tcp)
        };
        self.transport = Some(transport);
        self.login().await
    }

    async fn login(&mut self) -> Result<()> {
        let creds = self.credentials.clone();
        let sentence = Sentence::command("/login")
            .attr("name", &creds.username)
            .attr("password", &creds.password);
        let replies = self.run_sentence(sentence).await?;
        check_command_ok(&replies)
    }

    /// Run a raw API sentence and collect all reply sentences.
    pub async fn run_sentence(&mut self, sentence: Sentence) -> Result<Vec<Reply>> {
        let transport = self
            .transport
            .as_mut()
            .ok_or_else(|| MikrotikError::Transport("not connected".into()))?;
        exchange(transport, &sentence).await
    }

    /// Run a command path (e.g. `/ip/address/print`) with attribute words.
    pub async fn run_command(
        &mut self,
        command: &str,
        attrs: &[(&str, &str)],
    ) -> Result<Vec<HashMap<String, String>>> {
        let mut sentence = Sentence::command(command);
        for (k, v) in attrs {
            sentence = sentence.attr(*k, *v);
        }
        self.run_records(sentence).await
    }

    /// `print` on a resource path — returns `!re` attribute maps.
    pub async fn print(
        &mut self,
        path: &str,
        proplist: Option<&[&str]>,
        queries: &[(&str, &str)],
    ) -> Result<Vec<HashMap<String, String>>> {
        let mut sentence = Sentence::command(format!("{path}/print"));
        if let Some(props) = proplist {
            sentence = sentence.proplist(props);
        }
        for (k, v) in queries {
            sentence = sentence.query(*k, *v);
        }
        self.run_records(sentence).await
    }

    /// `add` on a resource path.
    pub async fn add(
        &mut self,
        path: &str,
        attrs: &[(&str, &str)],
    ) -> Result<HashMap<String, String>> {
        let mut sentence = Sentence::command(format!("{path}/add"));
        for (k, v) in attrs {
            sentence = sentence.attr(*k, *v);
        }
        let records = self.run_records(sentence).await?;
        records
            .into_iter()
            .next()
            .ok_or_else(|| MikrotikError::Api {
                message: format!("{path}/add returned no record"),
                category: None,
            })
    }

    /// `set` by internal `.id`.
    pub async fn set(&mut self, path: &str, id: &str, attrs: &[(&str, &str)]) -> Result<()> {
        let mut sentence = Sentence::command(format!("{path}/set")).attr(".id", id);
        for (k, v) in attrs {
            sentence = sentence.attr(*k, *v);
        }
        let replies = self.run_sentence(sentence).await?;
        check_command_ok(&replies)
    }

    /// `remove` by internal `.id`.
    pub async fn remove(&mut self, path: &str, id: &str) -> Result<()> {
        let sentence = Sentence::command(format!("{path}/remove")).attr(".id", id);
        let replies = self.run_sentence(sentence).await?;
        check_command_ok(&replies)
    }

    async fn run_records(&mut self, sentence: Sentence) -> Result<Vec<HashMap<String, String>>> {
        let replies = self.run_sentence(sentence).await?;
        Ok(replies
            .into_iter()
            .filter(|r| r.kind == "re")
            .map(|r| r.attrs)
            .collect())
    }
}

fn check_command_ok(replies: &[Reply]) -> Result<()> {
    for reply in replies {
        match reply.kind.as_str() {
            "trap" | "fatal" => {
                let message = reply
                    .attrs
                    .get("message")
                    .cloned()
                    .unwrap_or_else(|| "router command failed".into());
                let category = reply.attrs.get("category").and_then(|c| c.parse().ok());
                return Err(MikrotikError::Api { message, category });
            }
            _ => {}
        }
    }
    Ok(())
}

fn build_tls_connector(accept_invalid: bool) -> Result<TlsConnector> {
    let config = if accept_invalid {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    } else {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth()
    };
    Ok(TlsConnector::from(Arc::new(config)))
}

#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_plain() {
        let cfg = MikrotikConfig::new("192.168.88.1");
        assert_eq!(cfg.port, DEFAULT_PLAIN_PORT);
        assert!(!cfg.tls);
    }

    #[test]
    fn tls_switches_default_port() {
        let cfg = MikrotikConfig::new("r").with_tls(true);
        assert_eq!(cfg.port, DEFAULT_TLS_PORT);
        assert!(cfg.tls);
    }
}
