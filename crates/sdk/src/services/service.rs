//! Service wire types.

use crate::services::price::Price;
use berm_api::{
    Credential, Meter, Service,
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
    /// Each public tool's price a call, by name, in place of the prices the
    /// service had: a deploy without them leaves it with none. Refused when
    /// one names a tool that is not a public tool of the image.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prices: BTreeMap<String, Price>,
    /// The value its copy holds for each var it names, by name; a var it
    /// leaves out keeps its value. Refused when one names a var its `[vars]`
    /// does not declare, or a value past 4096 bytes.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, String>,
}

/// What calling a service is held to and costs: each public tool's terms, by
/// name.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Terms {
    pub tools: BTreeMap<String, ToolTerms>,
    /// Milliseconds a chain of calls runs before the host stops it.
    pub deadline: u64,
}

/// What one call of a tool is held to, and what it costs its caller.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTerms {
    #[serde(flatten)]
    pub held: berm_api::ToolTerms,
    /// Its price a call, set by the service's last deploy; none for a tool
    /// that is free.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// What its runs use; none before its first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<Estimate>,
}

/// What a tool's runs that ended `done` in the last 30 days used, from the
/// call log: each run summed over every service it reached, and each measure's
/// median and 99th percentile taken on its own.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estimate {
    /// Runs measured.
    pub runs: u64,
    /// Each measure's median.
    pub typical: Measured,
    /// Each measure's 99th percentile, which below [`Estimate::P99`] runs is
    /// its largest.
    pub slow: Measured,
}

impl Estimate {
    /// The fewest runs whose 99th percentile is not their largest.
    pub const P99: u64 = 100;
}

/// One measure of a run: wall time in milliseconds, CPU time in microseconds,
/// rows read and written, memory held in MiB-microseconds, and its cost at the
/// fleet's rates as a decimal of USD micros.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measured {
    pub ms: u64,
    pub cpu: u64,
    pub read: u64,
    pub written: u64,
    #[serde(default)]
    pub held: u64,
    pub cost_micros_decimal: String,
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
    /// The hosts its value is sent to, each named in full. A secret with none
    /// is never sent.
    #[serde(default)]
    pub hosts: Vec<String>,
    pub created_at: i64,
}

impl Secret {
    /// Whether its value is sent anywhere: it has hosts.
    pub fn sent(&self) -> bool {
        !self.hosts.is_empty()
    }

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
    /// The hosts its value is sent to, as [`Hosts`] holds them; none sends it
    /// nowhere.
    #[serde(default)]
    pub hosts: Vec<String>,
}

impl SetSecret {
    /// The most bytes a secret's value may hold.
    pub const VALUE_MAX: usize = 8 * 1024;

    /// Whether `value` may be a secret's: 1 to [`Self::VALUE_MAX`] bytes on
    /// one line.
    pub fn valid(value: &str) -> bool {
        !value.is_empty() && value.len() <= Self::VALUE_MAX && !value.chars().any(char::is_control)
    }

    /// Refused, with why, when it may not be set as secret `name`.
    pub fn check(&self, name: &str) -> Result<(), String> {
        if !Secret::valid(name) {
            return Err(format!(
                "a secret's name is up to {} ASCII letters, digits and `_`, not starting with a digit",
                Secret::NAME_MAX
            ));
        }
        if !Self::valid(&self.value) {
            return Err(format!(
                "{name}'s value is 1 to {} bytes on one line",
                Self::VALUE_MAX
            ));
        }
        Hosts::check(&self.hosts)
    }
}

/// The body of `PUT …/secrets/{name}/hosts`: the hosts a secret's value is
/// sent to, replacing those it had.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hosts {
    pub hosts: Vec<String>,
}

impl Hosts {
    /// The most hosts one secret is sent to.
    pub const MAX: usize = 16;

    /// Refused, with why, unless `hosts` holds at most [`Self::MAX`] distinct
    /// hosts, each named in full.
    pub fn check(hosts: &[String]) -> Result<(), String> {
        if hosts.len() > Self::MAX {
            return Err(format!("a secret is sent to at most {} hosts", Self::MAX));
        }
        if let Some(host) = hosts.iter().find(|host| !Credential::host(host)) {
            return Err(format!(
                "{host:?} is not a host named in full, as `api.example.com`: no scheme, port, path or `*`"
            ));
        }
        if let Some((_, host)) = hosts
            .iter()
            .enumerate()
            .find(|(at, host)| hosts[..*at].contains(host))
        {
            return Err(format!("{host} is given twice"));
        }
        Ok(())
    }
}

/// One of an account's services, as its owners list them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Owned {
    #[serde(flatten)]
    pub service: Service,
    pub private: bool,
    pub served: host::Served,
    /// The project it was last deployed in.
    pub project: Option<String>,
    /// The registry reference it runs, which another account can deploy as
    /// its own; none for an uploaded image.
    pub reference: Option<String>,
    pub status: Status,
}

/// Whether a service is serving, and how its runs are ending.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Serving, its newest run done or none failed.
    Running,
    /// Stopped by its owners, or deployed stopped: it takes no calls and
    /// fires no wakes.
    Stopped,
    /// Serving, and its runs have failed since the last one that was done.
    Failing,
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
    /// What its runs use; none before one ended `done`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<Estimate>,
    /// A budget that fits its runs: 1.5× the CPU time, rows read and rows
    /// written of its slowest run that ended `done`, and 1.5× the most linear
    /// memory one of its own invocations reached in MiB, each rounded up;
    /// none before one ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fits: Option<berm_api::Limits>,
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

berm_api::text_enum!(DeployOutcome);

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
