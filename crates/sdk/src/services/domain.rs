//! Custom domains: a hostname of the owner's own a service's MCP server is
//! served at.

use serde::{Deserialize, Serialize};

/// A custom domain of a service.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    /// The hostname, lowercase.
    pub host: String,
    pub status: DomainStatus,
    /// The DNS records its owner creates at their DNS provider.
    pub records: Vec<Record>,
    /// Why it is not active yet, as Cloudflare says, when it says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub created_at: i64,
}

/// Where a custom domain stands.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainStatus {
    /// Waiting for its records, then its certificate.
    Pending,
    /// Serving, with its certificate.
    Active,
    /// Refused or expired; remove it and add it again.
    Failed,
}

/// A DNS record a custom domain needs.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// `CNAME` or `TXT`.
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub value: String,
}

/// What a custom domain is added with.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddDomain {
    pub host: String,
}
