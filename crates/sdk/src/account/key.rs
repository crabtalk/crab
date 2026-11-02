//! API-key wire types.

use crate::{error::RequestLink, services::price::Price};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    pub id: String,
    pub owner_id: String,
    pub created_by: String,
    pub label: String,
    pub hint: String,
    pub scope: Vec<Scope>,
    pub created_at: i64,
    pub revoked_at: Option<i64>,
    pub expires_at: Option<i64>,
    /// When it last authenticated a request, to the minute.
    #[serde(default)]
    pub used_at: Option<i64>,
    /// What the key may spend on tool prices in a UTC day, in USD micros.
    pub budget: i64,
    /// What it spent on tool prices today, UTC, in USD micros.
    #[serde(default)]
    pub spent: i64,
    /// The other accounts' priced services it may call.
    #[serde(default)]
    pub services: Vec<KeyService>,
    /// Its pending grant request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestLink>,
}

/// Another account's service, as a key's grant names it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyService {
    /// As `owner/name`, under the account's handle, or its id when it has
    /// none.
    pub service: String,
    /// Each priced tool's price a call, by tool.
    #[cfg_attr(feature = "utoipa", schema(value_type = BTreeMap<String, u64>))]
    #[serde(default)]
    pub prices: BTreeMap<String, Price>,
}

/// A key's daily budget when none is given, in USD micros: $5.
pub const BUDGET: i64 = 5_000_000;

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    #[serde(rename = "services:read")]
    ServicesRead,
    #[serde(rename = "services:write")]
    ServicesWrite,
    #[serde(rename = "deployments:read")]
    DeploymentsRead,
    #[serde(rename = "deployments:write")]
    DeploymentsWrite,
    #[serde(rename = "secrets:read")]
    SecretsRead,
    #[serde(rename = "secrets:write")]
    SecretsWrite,
    #[serde(rename = "schedules:read")]
    SchedulesRead,
    #[serde(rename = "schedules:write")]
    SchedulesWrite,
    #[serde(rename = "calls:read")]
    CallsRead,
    #[serde(rename = "logs:read")]
    LogsRead,
    #[serde(rename = "usage:read")]
    UsageRead,
    #[serde(rename = "billing:read")]
    BillingRead,
    #[serde(rename = "earnings:read")]
    EarningsRead,
    #[serde(rename = "apps:read")]
    AppsRead,
    #[serde(rename = "apps:write")]
    AppsWrite,
    #[serde(rename = "orgs:read")]
    OrgsRead,
    #[serde(rename = "orgs:write")]
    OrgsWrite,
    #[serde(rename = "members:read")]
    MembersRead,
    #[serde(rename = "members:write")]
    MembersWrite,
    #[serde(rename = "invites:read")]
    InvitesRead,
    #[serde(rename = "invites:write")]
    InvitesWrite,
    #[serde(rename = "teams:read")]
    TeamsRead,
    #[serde(rename = "teams:write")]
    TeamsWrite,
    #[serde(rename = "keys:read")]
    KeysRead,
    #[serde(rename = "alerts:read")]
    AlertsRead,
    #[serde(rename = "alerts:write")]
    AlertsWrite,
}

berm_api::text_enum!(Scope);

impl Scope {
    pub const ALL: [Self; 26] = [
        Self::ServicesRead,
        Self::ServicesWrite,
        Self::DeploymentsRead,
        Self::DeploymentsWrite,
        Self::SecretsRead,
        Self::SecretsWrite,
        Self::SchedulesRead,
        Self::SchedulesWrite,
        Self::CallsRead,
        Self::LogsRead,
        Self::UsageRead,
        Self::BillingRead,
        Self::EarningsRead,
        Self::AppsRead,
        Self::AppsWrite,
        Self::OrgsRead,
        Self::OrgsWrite,
        Self::MembersRead,
        Self::MembersWrite,
        Self::InvitesRead,
        Self::InvitesWrite,
        Self::TeamsRead,
        Self::TeamsWrite,
        Self::KeysRead,
        Self::AlertsRead,
        Self::AlertsWrite,
    ];

    pub const DEFAULT: [Self; 4] = [
        Self::ServicesRead,
        Self::DeploymentsRead,
        Self::CallsRead,
        Self::UsageRead,
    ];

    pub fn defaults() -> Vec<Self> {
        Self::DEFAULT.to_vec()
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDefaults {
    pub scope: Vec<Scope>,
    pub budget: i64,
}

impl Default for KeyDefaults {
    fn default() -> Self {
        Self {
            scope: Scope::defaults(),
            budget: BUDGET,
        }
    }
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub label: String,
    #[serde(default = "Scope::defaults")]
    pub scope: Vec<Scope>,
    pub expires_at: Option<i64>,
    /// Its daily budget, in USD micros; [`BUDGET`] when none is given.
    #[serde(default)]
    pub budget: Option<i64>,
}

/// What changes of a key, at `PATCH …/keys/{key_id}`; what is left out
/// stays.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateKeyRequest {
    /// Its daily budget, in USD micros.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<Scope>>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    pub key: Key,
    pub token: String,
}
