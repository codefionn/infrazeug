//! Firewall filter rules (`/ip/firewall/filter`).

use crate::client::MikrotikClient;
use crate::error::Result;
use std::collections::HashMap;

const PATH: &str = "/ip/firewall/filter";

/// A firewall filter rule.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FirewallFilter {
    pub id: Option<String>,
    /// Human-readable label (`comment` in RouterOS).
    pub comment: String,
    pub chain: Option<String>,
    pub action: Option<String>,
    pub protocol: Option<String>,
    pub src_address: Option<String>,
    pub dst_address: Option<String>,
    pub src_port: Option<String>,
    pub dst_port: Option<String>,
    pub disabled: Option<bool>,
    pub extra: HashMap<String, String>,
}

impl FirewallFilter {
    fn from_map(mut map: HashMap<String, String>) -> Self {
        let id = map.remove("id").or_else(|| map.remove(".id"));
        let comment = map.remove("comment").unwrap_or_default();
        let chain = map.remove("chain");
        let action = map.remove("action");
        let protocol = map.remove("protocol");
        let src_address = map.remove("src-address");
        let dst_address = map.remove("dst-address");
        let src_port = map.remove("src-port");
        let dst_port = map.remove("dst-port");
        let disabled = map.remove("disabled").map(|v| v == "true" || v == "yes");
        Self {
            id,
            comment,
            chain,
            action,
            protocol,
            src_address,
            dst_address,
            src_port,
            dst_port,
            disabled,
            extra: map,
        }
    }

    fn to_attrs(&self) -> Vec<(&str, &str)> {
        let mut attrs = Vec::new();
        if !self.comment.is_empty() {
            attrs.push(("comment", self.comment.as_str()));
        }
        if let Some(ref v) = self.chain {
            attrs.push(("chain", v.as_str()));
        }
        if let Some(ref v) = self.action {
            attrs.push(("action", v.as_str()));
        }
        if let Some(ref v) = self.protocol {
            attrs.push(("protocol", v.as_str()));
        }
        if let Some(ref v) = self.src_address {
            attrs.push(("src-address", v.as_str()));
        }
        if let Some(ref v) = self.dst_address {
            attrs.push(("dst-address", v.as_str()));
        }
        if let Some(ref v) = self.src_port {
            attrs.push(("src-port", v.as_str()));
        }
        if let Some(ref v) = self.dst_port {
            attrs.push(("dst-port", v.as_str()));
        }
        if let Some(disabled) = self.disabled {
            attrs.push(("disabled", if disabled { "yes" } else { "no" }));
        }
        for (k, v) in &self.extra {
            attrs.push((k.as_str(), v.as_str()));
        }
        attrs
    }
}

impl MikrotikClient {
    /// List firewall filter rules.
    pub async fn firewall_filters(&mut self) -> Result<Vec<FirewallFilter>> {
        let rows = self.print(PATH, None, &[]).await?;
        Ok(rows.into_iter().map(FirewallFilter::from_map).collect())
    }

    /// Add a firewall filter rule.
    pub async fn add_firewall_filter(&mut self, rule: &FirewallFilter) -> Result<FirewallFilter> {
        let attrs = rule.to_attrs();
        let created = self.add(PATH, &attrs).await?;
        Ok(FirewallFilter::from_map(created))
    }

    /// Update a firewall filter rule by `.id`.
    pub async fn set_firewall_filter(&mut self, id: &str, rule: &FirewallFilter) -> Result<()> {
        let attrs = rule.to_attrs();
        self.set(PATH, id, &attrs).await
    }

    /// Remove a firewall filter rule by `.id`.
    pub async fn remove_firewall_filter(&mut self, id: &str) -> Result<()> {
        self.remove(PATH, id).await
    }
}
