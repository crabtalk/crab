//! What `/meta` says about the deployment.

use serde::{Deserialize, Serialize};

/// CPU microseconds a run spends, over every call in it, before those past
/// them are charged as overage.
pub const INCLUDED_CPU: u64 = 100;

/// MiB-microseconds of memory held in a GB-hour.
pub const GB_HOUR: u64 = 1024 * 3_600_000_000;

/// `GET /meta/errors/{code}`: what an error code means and what to do about
/// it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    /// As `E0001`.
    pub code: String,
    /// Markdown: a summary line, then the rest.
    pub text: String,
}

/// What is deployed, served unauthenticated at `GET /meta/version`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    /// The running build's crate version, e.g. `0.1.1`, plain semver. It moves
    /// only when someone bumps it, not every deploy; `commit` identifies a
    /// build.
    pub version: String,
    /// The commit this binary was built from — the full 40-character SHA, e.g.
    /// `9bb4d4f11e60a4da6c8ca91a9dba2c5e874d2035`; truncate for display.
    /// `unknown` outside a git tree.
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
    /// CPU microseconds of each run `overage` does not count.
    pub included_cpu: u64,
    /// Bytes one log holds at most.
    pub max_log: usize,
}

/// What an owner is charged for its services' meters, each in USD micros per
/// billion units.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rates {
    pub runs: u64,
    /// CPU microseconds past each run's first [`INCLUDED_CPU`].
    pub overage: u64,
    pub read: u64,
    pub written: u64,
    /// MiB-microseconds of a call's memory budget held, waiting included.
    #[serde(default)]
    pub held: u64,
    /// Bytes through `berm.http.proxy`; unset is free.
    pub proxied: Option<u64>,
}
