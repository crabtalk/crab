//! What a service says it is, and what the local host's control API speaks.
//!
//! Links no compiler: a client reads an image and talks to the API with this
//! crate alone.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use compat::{Break, Surface};
pub use image::Image;
pub use manifest::{
    Credential, CredentialSecret, Database, Deps, EventSpec, Kit, Manifest, Migration, Reach,
    Sequence, ServiceRecord, ToolRecord, Var,
};

mod compat;
pub mod digest;
pub mod host;
mod image;
mod manifest;
pub mod project;
#[cfg(feature = "rusqlite")]
mod sql;
pub mod text;

/// The most bytes a call's arguments, and a tool's result, may hold, when a
/// service's `crab.toml` sets no `buffer`.
pub const DEFAULT_BUFFER: usize = 16 * 1024;

/// One log a service emitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logged {
    /// Its position in the service's logs, counting from 1.
    pub seq: u64,
    /// The call it was emitted in, as the host's call log numbers it. `None`
    /// for a log emitted in a call into the service from another host.
    pub call: Option<u64>,
    /// When it was emitted, in epoch milliseconds.
    pub at: u64,
    pub topics: Vec<String>,
    pub data: String,
}

/// A deployed service as its callers see it: the IDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    /// The crate's version when it was built.
    #[serde(default)]
    pub version: String,
    /// The crate's `description`, `homepage` and `repository` from its
    /// `Cargo.toml`.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub repository: String,
    /// The Markdown file `readme` names in its `crab.toml`.
    #[serde(default)]
    pub readme: String,
    /// sha256 of the image.
    pub digest: String,
    /// Its public tools.
    pub tools: Vec<ToolSpec>,
    /// The settings each deployed copy holds its own value of, by name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vars: BTreeMap<String, Var>,
}

/// How a deployed service runs, for its owners: what [`Service`] leaves out.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Runtime {
    /// Its tools that are not public.
    pub tools: Vec<ToolSpec>,
    pub deps: Deps,
    pub events: Vec<EventSpec>,
    /// The part of `deps` this host answers to nothing for.
    #[serde(default, skip_serializing_if = "Deps::is_empty")]
    pub unresolved: Deps,
    /// The project it was last deployed in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// A deploy's answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployed {
    pub service: Service,
    /// How it broke callers of the service it replaced, as
    /// [`Manifest::breaks`] words it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<String>,
    /// The migrations it applied, a kit's as `{kit}/{name}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<String>,
    /// The service's web page, on Cloud; none on the local host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// A deployed service as anyone it is visible to sees it: its IDL and what it
/// served.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub service: Service,
    pub served: host::Served,
    /// Stopped by its owners, or deployed stopped: it takes no calls and fires
    /// no wakes.
    #[serde(default)]
    pub stopped: bool,
    /// The project it was last deployed in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The registry reference it runs, which another account can deploy as
    /// its own; none for an uploaded image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// What a service's owner set for calling it, as the host answering holds it.
/// Changes live, from the next call.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Terms {
    /// Each public tool's, by name.
    pub tools: BTreeMap<String, ToolTerms>,
    /// Milliseconds a chain of calls runs before the host stops it.
    pub deadline: u64,
}

/// What one call of a tool is held to.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolTerms {
    pub budget: Budget,
}

/// A cron tool's expression, as its owner sets it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cron {
    /// Five fields, minute to day of week, in UTC: `*/5 * * * *`. `None`
    /// stops its calls.
    pub cron: Option<String>,
}

/// One var of a service's `[vars]`, as its owners read it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Setting {
    pub name: String,
    pub description: String,
    pub default: String,
    /// What this copy holds; `None` reads `default`.
    pub value: Option<String>,
}

/// A var's value, as its owners set it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarValue {
    /// `None` returns it to its default.
    pub value: Option<String>,
}

/// The most one invocation may use: CPU time in microseconds, nested calls
/// excluded, rows read, rows written, and bytes of linear memory.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub cpu: u64,
    pub read: u64,
    pub written: u64,
    /// Held from the invocation's start to its return, and reserved from the
    /// host's memory for that long.
    pub memory: u64,
}

/// A tool's `budget` in its `crab.toml`: CPU time in microseconds, rows read,
/// rows written, and memory in MiB. A key left out takes the host's.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<u64>,
}

impl Limits {
    /// `budget` with each key this sets in its place.
    pub fn over(&self, budget: Budget) -> Budget {
        Budget {
            cpu: self.cpu.unwrap_or(budget.cpu),
            read: self.read.unwrap_or(budget.read),
            written: self.written.unwrap_or(budget.written),
            memory: self.memory.map_or(budget.memory, |mib| mib << 20),
        }
    }

    /// Each key it sets, by name, memory in MiB.
    pub fn keys(&self) -> impl Iterator<Item = (&'static str, u64)> {
        [
            ("cpu", self.cpu),
            ("read", self.read),
            ("written", self.written),
            ("memory", self.memory),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
    }
}

impl std::fmt::Display for Limits {
    /// As `crab.toml` writes it: `{ cpu = 5000, written = 100 }`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let keys: Vec<String> = self
            .keys()
            .map(|(key, value)| format!("{key} = {value}"))
            .collect();
        write!(f, "{{ {} }}", keys.join(", "))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool's arguments, as the model receives it.
    pub parameters: serde_json::Value,
    /// JSON Schema for the tool's result. `None` declares no shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returns: Option<serde_json::Value>,
    /// Read-only: a call to it, and every call it makes, is refused
    /// `berm.set`, `berm.emit` and `berm.call.after`, so it commits nothing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub query: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deprecated: bool,
    /// Callable by accounts and other services. A tool that is not is only
    /// called by its host, for the service's own wakes, crons, Stripe events
    /// and webhooks.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public: bool,
    /// Callable from outside with no caller: no account, or an app's call with
    /// no user. A public tool that is not is refused such a call.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub anonymous: bool,
    /// Which callers a public tool takes. `None` takes both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callers: Option<Callers>,
    /// Its short display name, at most [`ToolSpec::TITLE`] characters on one
    /// line: MCP's tool `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The catalog job it does, such as `company.enrich`: a listed platform,
    /// `.`, then the owner's own words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    /// What its `crab.toml` sets of one call's budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Limits>,
    /// Called by its host when the cron expression its deploy or its owner
    /// sets fires. A cron tool is not public.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cron: bool,
    /// Called by its host with each Stripe event its daemon takes for the
    /// service, the event as the argument object. Not public; a service has
    /// at most one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stripe: bool,
    /// Called by its host with each webhook its daemon verified for this
    /// tool, the body as the argument object. Not public.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub webhook: bool,
}

impl ToolSpec {
    /// The most characters a title holds.
    pub const TITLE: usize = 60;

    /// Who may call it, as `access` in `crab.toml` says it; `None` is
    /// internal.
    pub fn access(&self) -> Option<Access> {
        match (self.public, self.anonymous, self.callers) {
            (false, ..) => None,
            (true, true, _) => Some(Access::Anonymous),
            (true, false, None) => Some(Access::Public),
            (true, false, Some(Callers::Accounts)) => Some(Access::Accounts),
            (true, false, Some(Callers::Services)) => Some(Access::Services),
        }
    }

    /// Set who may call it to `access`; `None` makes it internal.
    pub fn set_access(&mut self, access: Option<Access>) {
        self.public = access.is_some();
        self.anonymous = access == Some(Access::Anonymous);
        self.callers = match access {
            Some(Access::Accounts) => Some(Callers::Accounts),
            Some(Access::Services) => Some(Callers::Services),
            _ => None,
        };
    }
}

/// Who may call a tool, as `access` in `crab.toml`'s `[tools]` sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    /// Accounts and other services.
    Public,
    /// Accounts and other services, and a call from outside with no caller:
    /// no account, or an app's call with no user.
    Anonymous,
    /// Callers from outside alone.
    Accounts,
    /// Services reaching it through `berm.call` alone.
    Services,
}

/// `public`, `anonymous`, `accounts` or `services`, as `crab.toml` spells it.
impl std::fmt::Display for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Public => "public",
            Self::Anonymous => "anonymous",
            Self::Accounts => "accounts",
            Self::Services => "services",
        })
    }
}

/// One kind of caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Callers {
    /// Services reaching it through `berm.call`.
    Services,
    /// Callers from outside, as the host authenticates them.
    Accounts,
}

/// What running a tool produced.
///
/// [`Self::Failed`] is the service's own report, and it arrives with the same
/// `200` a result does: the call was fine and the tool said no. A refusal — no
/// such service, no such tool, a trap — is a [`Failed`] with a status to match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Output {
    Done(String),
    Failed(String),
}

impl From<Result<String, String>> for Output {
    fn from(outcome: Result<String, String>) -> Self {
        match outcome {
            Ok(result) => Self::Done(result),
            Err(failure) => Self::Failed(failure),
        }
    }
}

impl From<Output> for Result<String, String> {
    fn from(output: Output) -> Self {
        match output {
            Output::Done(result) => Ok(result),
            Output::Failed(failure) => Err(failure),
        }
    }
}

/// The most bytes one log may hold, its data and topics together.
pub const MAX_LOG: usize = 4096;

/// What a call used of one service: its runs, the chains it was the first link
/// of; its invocations, the CPU time they spent in microseconds, nested calls
/// excluded, the largest linear memory one
/// reached, in bytes, their budgets of memory held over their wall-clock time
/// in MiB-microseconds, the rows they read and wrote, the CPU time their SQL
/// statements spent on the database's threads in microseconds, the bytes sent
/// and received through the proxy, the bytes of the logs they emitted, and the
/// bytes sent off the host.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meter {
    #[serde(default)]
    pub runs: u64,
    pub calls: u64,
    pub cpu: u64,
    pub memory: u64,
    #[serde(default)]
    pub held: u64,
    pub read: u64,
    pub written: u64,
    #[serde(default)]
    pub sql_cpu: u64,
    pub proxied: u64,
    #[serde(default)]
    pub logged: u64,
    #[serde(default)]
    pub failed: u64,
    #[serde(default)]
    pub sent: u64,
}

impl std::ops::AddAssign for Meter {
    /// Sums every count but `memory`, which keeps the larger.
    fn add_assign(&mut self, other: Meter) {
        self.runs += other.runs;
        self.calls += other.calls;
        self.cpu += other.cpu;
        self.memory = self.memory.max(other.memory);
        self.held += other.held;
        self.read += other.read;
        self.written += other.written;
        self.sql_cpu += other.sql_cpu;
        self.proxied += other.proxied;
        self.logged += other.logged;
        self.failed += other.failed;
        self.sent += other.sent;
    }
}

/// What the control API returns instead of a resource when it refuses one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failed {
    pub error: String,
    /// The error code explaining it, as `E0013`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain: Option<String>,
}
