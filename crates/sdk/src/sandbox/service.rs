//! Service wire types.

use crate::utils::text_enum;
use berm_api::{
    Meter, Program,
    host::{self, Call, ServedDay},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a service runs next: the image a registry reference names, or one
/// uploaded through `POST /services/{owner}/{name}/image`, named by its digest,
/// `sha256:` and its SHA-256 in hex.
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
    /// [`MAX_DATABASE_SIZE`](berm_api::host::MAX_DATABASE_SIZE). Absent, a
    /// service keeps what it holds, and one deployed for the first time holds
    /// [`DATABASE_SIZE`](berm_api::host::DATABASE_SIZE).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
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
    /// The project it is deployed in: the workspace it came from. Absent, it
    /// stays in the project it was in. Refused, unless `force`, when it is in
    /// another project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A project's services deployed together, each by its name in the account:
/// every one is checked before any switches, and one refused refuses them all.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploySetRequest {
    pub services: BTreeMap<String, DeployRequest>,
}

/// A file to upload, a seed or an image: its SHA-256 in hex, and its size in
/// bytes.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRequest {
    pub sha256: String,
    pub size: u64,
}

/// Where a file's bytes go.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upload {
    /// A URL to `PUT` the bytes to, with their `Content-Length`, within an
    /// hour; none when Cloud holds that file already.
    pub upload: Option<String>,
}

/// One day of a service's meter, with what Cloud charges past each run's
/// included CPU time.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeteredDay {
    #[serde(flatten)]
    pub used: berm_api::host::Metered,
    /// The CPU microseconds each run spent in the service past the run's first
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
    pub served: host::Served,
    /// The project it was last deployed in.
    pub project: Option<String>,
    /// The registry reference it runs, which another account can deploy as
    /// its own; none for an uploaded image.
    pub reference: Option<String>,
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

/// What one tool's invocations used over the last 30 days, as its service's
/// meters count them, with what that use costs its service's owner.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCosted {
    /// Its meters summed.
    pub meter: Meter,
    /// Each day it ran, oldest first.
    pub days: Vec<ServedDay>,
    /// What its meters cost at the fleet's rates, in USD micros: each day's
    /// [`CostedDay::cost_micros`], summed.
    pub cost_micros: i64,
}

/// One deploy of a service, refused and failed ones included.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: i64,
    /// The service it deployed, by its name in the account.
    pub name: String,
    pub outcome: DeployOutcome,
    /// The version it went live on; none for one that did not.
    pub version: Option<String>,
    /// `sha256:` and the hex SHA-256 of the image; none when the deploy ended
    /// before its reference was resolved.
    pub digest: Option<String>,
    /// What it was deployed from: a registry reference, or `sha256:` and an
    /// upload's SHA-256.
    pub reference: String,
    /// Why it was refused or failed, as the deploy answered.
    pub reason: Option<String>,
    /// The migrations it applied, a kit's as `{kit}/{name}`.
    pub migrations: Vec<String>,
    /// Its database started from a seed.
    pub seeded: bool,
    /// The node it was placed on; none when it ended before placement.
    pub node: Option<String>,
    /// Its image can be deployed again: one from a registry, or an upload
    /// kept on the node the service is placed on now.
    pub redeployable: bool,
    /// The project it was deployed in.
    pub project: Option<String>,
    /// Who deployed it, by handle, or by id when they have none.
    pub by: String,
    /// When, in epoch seconds.
    pub at: i64,
}

/// The public service of another account a service was deployed from: one
/// running an image of the same registry repository.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Origin {
    /// Its owner, by handle, or by id when they have none.
    pub owner: String,
    pub name: String,
    /// The registry reference it runs now.
    pub reference: String,
    /// The version it runs now.
    pub version: String,
}

/// How a deploy ended.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeployOutcome {
    /// What the service runs now.
    Live,
    /// Went live, and a later deploy replaced it.
    Replaced,
    /// Refused: the service runs what it ran before.
    Refused,
    /// Did not finish: Cloud, the registry or the node did not answer.
    Failed,
}

text_enum!(DeployOutcome { Live => "live", Replaced => "replaced", Refused => "refused", Failed => "failed" });

/// Deploy a past deploy's image again.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RedeployRequest {
    /// Deploy even when it breaks a tool or event a caller used in the last
    /// 30 days, or undoes a cron its owners set since the last deploy that set
    /// them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

/// A call of one of an account's services, with what its meter costs.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCall {
    /// The service's name in the account.
    pub service: String,
    #[serde(flatten)]
    pub call: Costed,
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
