//! Domain contacts (`/domain/contact`).

use crate::client::OvhClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Postal address on a domain contact (`domain.ContactAddress`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

/// A domain contact (`domain.Contact`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainContact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organisation_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<ContactAddress>,
}

impl OvhClient {
    /// `GET /domain/contact` — list contacts.
    pub async fn domain_contacts(&self) -> Result<Vec<DomainContact>> {
        self.get_v1("/domain/contact").await
    }

    /// `GET /domain/contact/{contactId}` — contact details.
    pub async fn domain_contact(&self, contact_id: i64) -> Result<DomainContact> {
        let path = format!("/domain/contact/{contact_id}");
        self.get_v1(&path).await
    }

    /// `POST /domain/contact` — create a contact.
    pub async fn domain_contact_create(&self, contact: &DomainContact) -> Result<DomainContact> {
        self.post_v1("/domain/contact", contact).await
    }

    /// `PUT /domain/contact/{contactId}` — update a contact.
    pub async fn domain_contact_update(
        &self,
        contact_id: i64,
        contact: &DomainContact,
    ) -> Result<DomainContact> {
        let path = format!("/domain/contact/{contact_id}");
        self.put_v1_typed(&path, contact).await
    }
}
