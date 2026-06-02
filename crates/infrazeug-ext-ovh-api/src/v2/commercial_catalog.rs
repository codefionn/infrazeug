//! OVHcloud API v2 **commercialCatalog** bindings (`/v2/commercialCatalog`).
//!
//! Generated from the official schema; do not edit by hand.

#![allow(unused_imports, unused_variables)]

use crate::client::{percent_encode, OvhClient, PageParams, V2RequestOptions};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// `commercialCatalog.BlobContentTechnicalCPU`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalCPU {
    pub brand: Option<String>,
    pub cores: Option<i64>,
    pub frequency: Option<f64>,
    pub max_frequency: Option<f64>,
    pub model: Option<String>,
    pub threads: Option<i64>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalConnectionClients`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalConnectionClients {
    pub concurrency: Option<i64>,
    pub number: Option<i64>,
}

/// `commercialCatalog.BlobContentTechnicalConnection`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalConnection {
    pub clients: Option<BlobContentTechnicalConnectionClients>,
    pub total: Option<i64>,
}

/// `commercialCatalog.BlobContentTechnicalDisk`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalDisk {
    pub capacity: Option<f64>,
    pub interface: Option<String>,
    pub iops: Option<i64>,
    pub maximum_capacity: Option<f64>,
    pub number: Option<i64>,
    pub size_unit: Option<String>,
    pub technology: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalEphemeralStorage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalEphemeralStorage {
    pub disks: Option<Vec<BlobContentTechnicalDisk>>,
}

/// `commercialCatalog.BlobContentTechnicalMemory`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalMemory {
    pub interface: Option<String>,
    pub size: Option<f64>,
    pub size_unit: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalGPU`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalGPU {
    pub memory: Option<BlobContentTechnicalMemory>,
    pub model: Option<String>,
    pub number: Option<i64>,
    pub performance: Option<f64>,
}

/// `commercialCatalog.BlobContentTechnicalNetwork`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalNetwork {
    pub guaranteed: Option<bool>,
    pub is_max: Option<bool>,
    pub level: Option<f64>,
    pub max: Option<f64>,
    pub max_unit: Option<String>,
    pub unit: Option<String>,
    pub unlimited: Option<bool>,
}

/// `commercialCatalog.BlobContentTechnicalNodes`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalNodes {
    pub number: Option<i64>,
}

/// `commercialCatalog.BlobContentTechnicalNvme`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalNvme {
    pub disks: Option<Vec<BlobContentTechnicalDisk>>,
}

/// `commercialCatalog.BlobContentTechnicalOS`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalOS {
    pub family: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalPerSeconds`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalPerSeconds {
    pub total: Option<i64>,
    pub unit: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalStorage`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalStorage {
    pub disks: Option<Vec<BlobContentTechnicalDisk>>,
    pub raid: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalThroughput`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalThroughput {
    pub level: Option<i64>,
}

/// `commercialCatalog.BlobContentTechnicalVolumeCapacity`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalVolumeCapacity {
    pub max: Option<i64>,
}

/// `commercialCatalog.BlobContentTechnicalVolumeIops`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalVolumeIops {
    pub guaranteed: Option<bool>,
    pub level: Option<i64>,
    pub max: Option<i64>,
    pub max_unit: Option<String>,
    pub unit: Option<String>,
}

/// `commercialCatalog.BlobContentTechnicalVolume`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnicalVolume {
    pub capacity: Option<BlobContentTechnicalVolumeCapacity>,
    pub iops: Option<BlobContentTechnicalVolumeIops>,
}

/// `commercialCatalog.BlobContentTechnical`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContentTechnical {
    pub bandwidth: Option<BlobContentTechnicalNetwork>,
    pub connection: Option<BlobContentTechnicalConnection>,
    pub connection_per_seconds: Option<BlobContentTechnicalPerSeconds>,
    pub cpu: Option<BlobContentTechnicalCPU>,
    pub ephemeral_local_storage: Option<BlobContentTechnicalEphemeralStorage>,
    pub gpu: Option<BlobContentTechnicalGPU>,
    pub memory: Option<BlobContentTechnicalMemory>,
    pub name: Option<String>,
    pub nodes: Option<BlobContentTechnicalNodes>,
    pub nvme: Option<BlobContentTechnicalNvme>,
    pub os: Option<BlobContentTechnicalOS>,
    pub request_per_seconds: Option<BlobContentTechnicalPerSeconds>,
    pub storage: Option<BlobContentTechnicalStorage>,
    pub throughput: Option<BlobContentTechnicalThroughput>,
    pub volume: Option<BlobContentTechnicalVolume>,
    pub vrack: Option<BlobContentTechnicalNetwork>,
}

/// `commercialCatalog.BlobContent`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobContent {
    pub technical: Option<BlobContentTechnical>,
}

/// `commercialCatalog.Blob`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blob {
    pub content: Option<BlobContent>,
}

/// `commercialCatalog.Catalog`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub id: Option<i64>,
    pub name: Option<String>,
}

/// `common.LanguageEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommonLanguage {
    #[serde(rename = "cs_CZ")]
    CsCz,
    #[serde(rename = "de_DE")]
    DeDe,
    #[serde(rename = "en_AS")]
    EnAs,
    #[serde(rename = "en_AU")]
    EnAu,
    #[serde(rename = "en_CA")]
    EnCa,
    #[serde(rename = "en_GB")]
    EnGb,
    #[serde(rename = "en_IE")]
    EnIe,
    #[serde(rename = "en_IN")]
    EnIn,
    #[serde(rename = "en_SG")]
    EnSg,
    #[serde(rename = "en_US")]
    EnUs,
    #[serde(rename = "en_WW")]
    EnWw,
    #[serde(rename = "es_ES")]
    EsEs,
    #[serde(rename = "es_SA")]
    EsSa,
    #[serde(rename = "fi_FI")]
    FiFi,
    #[serde(rename = "fr_CA")]
    FrCa,
    #[serde(rename = "fr_FR")]
    FrFr,
    #[serde(rename = "fr_MA")]
    FrMa,
    #[serde(rename = "fr_SN")]
    FrSn,
    #[serde(rename = "fr_TN")]
    FrTn,
    #[serde(rename = "it_IT")]
    ItIt,
    #[serde(rename = "lt_LT")]
    LtLt,
    #[serde(rename = "nl_NL")]
    NlNl,
    #[serde(rename = "pl_PL")]
    PlPl,
    #[serde(rename = "pt_PT")]
    PtPt,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.Description`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub language: Option<CommonLanguage>,
    pub long_label: Option<String>,
    pub short_label: Option<String>,
}

/// `commercialCatalog.CommercialProduct`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommercialProduct {
    pub code: Option<String>,
    pub descriptions: Option<Vec<Description>>,
}

/// `commercialCatalog.InvoiceLabel`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLabel {
    pub label: Option<String>,
    pub language: Option<CommonLanguage>,
}

/// `commercialCatalog.CurrencyCodeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogCurrencyCode {
    #[serde(rename = "AUD")]
    Aud,
    #[serde(rename = "CAD")]
    Cad,
    #[serde(rename = "CZK")]
    Czk,
    #[serde(rename = "EUR")]
    Eur,
    #[serde(rename = "GBP")]
    Gbp,
    #[serde(rename = "INR")]
    Inr,
    #[serde(rename = "LTL")]
    Ltl,
    #[serde(rename = "MAD")]
    Mad,
    #[serde(rename = "PLN")]
    Pln,
    #[serde(rename = "SGD")]
    Sgd,
    #[serde(rename = "TND")]
    Tnd,
    #[serde(rename = "USD")]
    Usd,
    #[serde(rename = "XOF")]
    Xof,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.Price`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub amount: Option<i64>,
    pub currency_code: Option<CommercialCatalogCurrencyCode>,
}

/// `commercialCatalog.RatingValueTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogRatingValueType {
    #[serde(rename = "PRICE")]
    Price,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.RatingValue`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatingValue {
    pub prices: Option<Vec<Price>>,
    #[serde(rename = "type")]
    pub kind: Option<CommercialCatalogRatingValueType>,
}

/// `commercialCatalog.CommercialRatingValue`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommercialRatingValue {
    pub invoice_labels: Option<Vec<InvoiceLabel>>,
    pub rating_value: Option<RatingValue>,
}

/// `commercialCatalog.CompositeOfferCommercialOffer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeOfferCommercialOffer {
    pub commercial_offer: Option<String>,
    pub max: Option<i64>,
    pub min: Option<i64>,
}

/// `commercialCatalog.Engagement`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engagement {
    pub auto_reactivation: Option<bool>,
    #[serde(default)]
    pub valid_duration: serde_json::Value,
}

/// `commercialCatalog.Legacy`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Legacy {
    pub blobs: Option<Blob>,
    pub catalog: Option<Catalog>,
    pub plan: Option<String>,
}

/// `commercialCatalog.MerchantEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogMerchant {
    #[serde(rename = "ASIA")]
    Asia,
    #[serde(rename = "AU")]
    Au,
    #[serde(rename = "CA")]
    Ca,
    #[serde(rename = "DE")]
    De,
    #[serde(rename = "ES")]
    Es,
    #[serde(rename = "FR")]
    Fr,
    #[serde(rename = "GB")]
    Gb,
    #[serde(rename = "IE")]
    Ie,
    #[serde(rename = "IN")]
    In,
    #[serde(rename = "IT")]
    It,
    #[serde(rename = "MA")]
    Ma,
    #[serde(rename = "NL")]
    Nl,
    #[serde(rename = "PL")]
    Pl,
    #[serde(rename = "PT")]
    Pt,
    #[serde(rename = "QC")]
    Qc,
    #[serde(rename = "SG")]
    Sg,
    #[serde(rename = "SN")]
    Sn,
    #[serde(rename = "TN")]
    Tn,
    #[serde(rename = "US")]
    Us,
    #[serde(rename = "WE")]
    We,
    #[serde(rename = "WS")]
    Ws,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.OfferCommercialProduct`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferCommercialProduct {
    pub commercial_product: Option<CommercialProduct>,
    pub max: Option<i64>,
    pub min: Option<i64>,
}

/// `commercialCatalog.OfferNatureEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogOfferNature {
    #[serde(rename = "BILLING_PLAN")]
    BillingPlan,
    #[serde(rename = "REGULAR")]
    Regular,
    #[serde(rename = "STRUCTURAL")]
    Structural,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.OfferTypeEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogOfferType {
    #[serde(rename = "ATOMIC")]
    Atomic,
    #[serde(rename = "COMPOSITE")]
    Composite,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

/// `commercialCatalog.TechnicalRequirement`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicalRequirement {
    pub default: Option<String>,
    pub name: Option<String>,
    pub values: Option<Vec<String>>,
}

/// `commercialCatalog.Validity`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Validity {
    pub eol_date: Option<String>,
    pub eos_date: Option<String>,
    pub start_date: Option<String>,
}

/// `commercialCatalog.Offer`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub code: Option<String>,
    pub commercial_rating_values: Option<Vec<CommercialRatingValue>>,
    pub descriptions: Option<Vec<Description>>,
    pub engagements: Option<Vec<Engagement>>,
    pub id: Option<String>,
    pub legacy: Option<Legacy>,
    pub nature: Option<CommercialCatalogOfferNature>,
    pub offers: Option<Vec<CompositeOfferCommercialOffer>>,
    pub product: Option<OfferCommercialProduct>,
    pub technical_requirements: Option<Vec<TechnicalRequirement>>,
    #[serde(rename = "type")]
    pub kind: Option<CommercialCatalogOfferType>,
    pub validity: Option<Validity>,
    pub version: Option<i64>,
}

/// `commercialCatalog.OfferStateEnum`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommercialCatalogOfferState {
    #[serde(rename = "ACTIVE")]
    Active,
    #[serde(rename = "DEPRECATED")]
    Deprecated,
    /// Value not present in the schema this crate was built against.
    #[serde(other)]
    Other,
}

impl OvhClient {
    /// `GET /commercialCatalog/offers` — List all offers
    pub async fn commercial_catalog_offers(
        &self,
        query: &[(&str, &str)],
        page: &PageParams,
    ) -> Result<Vec<Offer>> {
        self.get_page(
            &Self::append_query("/commercialCatalog/offers", query),
            &[],
            page,
        )
        .await
        .map(|p| p.items)
    }

    /// `GET /commercialCatalog/offers/{id}` — Get details of an offer
    pub async fn commercial_catalog_offers_get(&self, id: &str) -> Result<Offer> {
        self.get(&format!("/commercialCatalog/offers/{}", percent_encode(id)))
            .await
    }
}
