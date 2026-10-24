//! Service wire types.

use berm_api::{
    Program,
    host::{self, Call},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a service runs next: the image a registry reference names.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRequest {
    pub reference: String,
    /// Deploy even when it breaks a tool or event a caller used in the last
    /// 30 days, or undoes a cron its owners set since the last deploy that set
    /// them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
    /// The cron expression of each cron tool, five fields in UTC; a cron tool
    /// it leaves out has none. Absent, each stays as its owners set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crons: Option<BTreeMap<String, String>>,
    /// Bytes the service's database may hold, from
    /// [`MIN_DATABASE_SIZE`](berm_api::host::MIN_DATABASE_SIZE) to
    /// [`MAX_DATABASE_SIZE`](berm_api::host::MAX_DATABASE_SIZE).
    #[serde(default = "host::database_size")]
    pub size: u64,
    /// The SHA-256, in hex, of a seed uploaded through
    /// `POST /services/{owner}/{name}/seed`: a SQLite file the service's
    /// database starts as, held to the image's migrations. Refused when the
    /// service has a database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    /// Install it stopped: it takes no calls and fires no wakes until
    /// started.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stopped: bool,
}

/// A seed to upload: its SHA-256 in hex, and its size in bytes.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedRequest {
    pub sha256: String,
    pub size: u64,
}

/// Where a seed's bytes go.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedUpload {
    /// A URL to `PUT` the bytes to, with their `Content-Length`, within an
    /// hour; none when Cloud holds that seed already.
    pub upload: Option<String>,
}

/// One day of a service's meter, with what Cloud charges past each
/// invocation's included CPU time.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteredDay {
    #[serde(flatten)]
    pub used: berm_api::host::Metered,
    /// The CPU microseconds each invocation spent past its first
    /// [`INCLUDED_CPU`](crate::system::meta::INCLUDED_CPU), summed.
    #[serde(default)]
    pub overage: u64,
}

/// One of a service's secrets, by name. Its value is never read back.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    pub name: String,
    pub created_at: i64,
}

impl Secret {
    /// The longest name a secret may have.
    pub const NAME_MAX: usize = 128;

    /// Whether `name` may name a secret: up to [`Self::NAME_MAX`] ASCII
    /// letters, digits and `_`, not starting with a digit.
    pub fn valid(name: &str) -> bool {
        let mut chars = name.chars();
        name.len() <= Self::NAME_MAX
            && chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }
}

/// The body of `PUT …/secrets/{name}`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSecret {
    pub value: String,
}

impl SetSecret {
    /// The most bytes a secret's value may hold.
    pub const VALUE_MAX: usize = 8 * 1024;

    /// Whether `value` may be a secret's: 1 to [`Self::VALUE_MAX`] bytes on
    /// one line.
    pub fn valid(value: &str) -> bool {
        !value.is_empty() && value.len() <= Self::VALUE_MAX && !value.chars().any(char::is_control)
    }
}

/// One of an account's services, as its owners list them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    #[serde(flatten)]
    pub program: Program,
    pub private: bool,
}

/// Who a service answers.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Visibility {
    /// Only the owning account and its members see and call it; anyone else
    /// is answered `404`.
    pub private: bool,
}

/// One day of a service's meters, with what it cost.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostedDay {
    #[serde(flatten)]
    pub used: MeteredDay,
    /// What the day's meters cost, in USD micros.
    pub cost_micros: i64,
}

/// A call, with what its meter costs its service's owner.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Costed {
    #[serde(flatten)]
    pub call: Call,
    /// What the call's meter costs at the fleet's rates, exactly, as a decimal
    /// of USD micros. Its day's storage is charged with the day. `None` for a
    /// call the host refused.
    pub cost_micros_decimal: Option<String>,
}

/// A service's billing in one UTC calendar month.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Billing {
    /// The month, `YYYY-MM`.
    pub month: String,
    /// Each day of the month it used anything, oldest first.
    pub days: Vec<CostedDay>,
    /// Each charged resource over the month.
    pub resources: Vec<Resource>,
    /// What its owner owes past what its credit covered, in USD micros, over
    /// every period: drawn once the owner's credit covers it.
    pub owed_micros: i64,
    /// When its meters were last read, in epoch seconds; `null` before the
    /// first read. Each day's `cost_micros` is as of then.
    pub updated_at: Option<i64>,
    /// Its first metered day, counted from the Unix epoch; `null` before the
    /// first read.
    pub since: Option<i32>,
}

/// One charged resource of a service over a month.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// As its rate is named in `GET /meta/rates`.
    pub resource: String,
    pub units: u64,
    /// USD micros per billion units.
    pub rate: u64,
    /// What the units cost at the rate, exactly, as a decimal of USD micros.
    pub cost_micros_decimal: String,
}
