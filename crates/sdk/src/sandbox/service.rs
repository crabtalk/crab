//! Service wire types.

use berm_api::{Program, host::Call};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a service runs next: the image a registry reference names.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRequest {
    pub reference: String,
    /// The tools this deploy may make public that the deployed version does
    /// not. Every public tool, on a service's first deploy.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public: Vec<String>,
    /// Deploy even when it breaks a tool or event a caller used in the last
    /// 30 days, or undoes a cron its owners set since the last deploy that set
    /// them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
    /// The cron expression of each cron tool, five fields in UTC; a cron tool
    /// it leaves out has none. Absent, each stays as its owners set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crons: Option<BTreeMap<String, String>>,
}

/// One day of a service's meter, with what Cloud charges past each
/// invocation's included instructions.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteredDay {
    #[serde(flatten)]
    pub used: berm_api::host::Metered,
    /// The instructions each invocation executed past its first
    /// [`INCLUDED_INSTRUCTIONS`](crate::system::meta::INCLUDED_INSTRUCTIONS), summed.
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

/// The body of `PUT …/secrets/{name}`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetSecret {
    pub value: String,
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
