//! What `/meta` says about the deployment.

use serde::{Deserialize, Serialize};

/// Instructions an invocation executes before those past them are charged as
/// overage.
pub const INCLUDED_INSTRUCTIONS: u64 = 1_000_000;

/// What is deployed, served unauthenticated at `GET /meta/version`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// The running build's crate version, e.g. `0.1.1`. Plain semver, so it is
    /// safe to compare — but it moves only when someone bumps it, which is not
    /// every deploy. Use `commit` to identify a build.
    pub version: String,
    /// The commit this binary was built from — the full 40-character SHA, e.g.
    /// `9bb4d4f11e60a4da6c8ca91a9dba2c5e874d2035`. Full rather than abbreviated
    /// because git widens the short form as a repo grows, so its width is not a
    /// stable contract; truncate for display. `unknown` outside a git tree.
    pub commit: String,
    /// The database schema this deployment is on — the highest migration
    /// applied, matching the number a migration filename starts with. What the
    /// database holds, not merely what the binary ships.
    pub schema: i64,
}

/// `GET /meta/rates`: what metering costs a service's owner.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metering {
    pub rates: Rates,
    /// Instructions of each call `overage` does not count.
    pub included_instructions: u64,
    /// Bytes one log holds at most.
    pub max_log: usize,
}

/// What an owner is charged for its services' meters, each in USD micros per
/// billion units.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rates {
    pub calls: u64,
    /// Instructions past each call's first 1M.
    pub overage: u64,
    pub read: u64,
    pub written: u64,
    /// Bytes stored for a month; a day is charged a thirtieth.
    pub stored: u64,
    /// Bytes through `berm.http.proxy`; unset is free.
    pub proxied: Option<u64>,
}
