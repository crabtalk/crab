//! API-key wire types.

use crate::utils::text_enum;
use serde::{Deserialize, Serialize};

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
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    #[serde(rename = "deploy")]
    Deploy,
}

text_enum!(Scope { Deploy => "deploy" });

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    pub label: String,
    #[serde(default)]
    pub scope: Vec<Scope>,
    pub expires_at: Option<i64>,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKeyResponse {
    pub key: Key,
    pub token: String,
}
