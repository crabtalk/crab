use crate::host::User;
use serde::{Deserialize, Serialize};

pub const ANONYMOUS: &str = "E0085";
pub const LIMIT: &str = "E0086";
pub const MIGRATION_FAILED: &str = "E0087";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub account: String,
    pub user: Option<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Key {
    pub service: String,
    pub identity: Identity,
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
