use crate::{digest, host::User};
use serde::{Deserialize, Serialize};

pub const ANONYMOUS: &str = "E0085";
pub const LIMIT: &str = "E0086";
pub const MIGRATION_FAILED: &str = "E0087";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub account: String,
    pub user: Option<User>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Key {
    pub storage_id: String,
    pub identity: Identity,
}

impl Identity {
    pub fn digest(&self) -> String {
        digest::hex(&serde_json::to_vec(self).expect("an identity serializes"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantRequest {
    pub key: Key,
    pub size: u64,
    pub grow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub id: String,
    pub token: String,
    pub bytes: u64,
    pub limit: u64,
    pub writable: bool,
}

pub const GRANT_STEP: u64 = 16 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub key: Key,
    pub id: String,
    pub token: String,
    pub bytes: u64,
    pub size: u64,
    pub schema: u32,
    pub called_at: i64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub owner: String,
    pub name: String,
    pub deleted: bool,
    pub size: u64,
    pub called_at: i64,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caller {
    pub account: String,
    pub issuer: Option<String>,
    pub subject: Option<String>,
    pub size: u64,
    pub called_at: i64,
    pub schema: u32,
}

#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub storages: Vec<Entry>,
    pub used: u64,
    pub limit: u64,
}
