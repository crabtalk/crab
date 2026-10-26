//! What a program says it is, and what the local host's control API speaks.
//!
//! Here rather than in the service so a client can read an image and talk to
//! the API without linking a compiler.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use compat::{Break, Surface};
pub use image::Image;
pub use manifest::{
    Credential, CredentialSecret, Database, Deps, EventSpec, Kit, Manifest, Migration,
    ProgramRecord, Reach, Sequence,
};

mod compat;
pub mod host;
mod image;
mod manifest;
pub mod project;
#[cfg(feature = "rusqlite")]
mod sql;

/// The ABI this host speaks. A program built against a different one is
/// refused rather than dispatched into a syscall its author did not
/// mean.
pub const ABI_VERSION: u32 = 5;

/// The section of what the compiler wrote of a program. A wasm custom
/// section, which is what `#[link_section]` emits on `wasm32`. It holds JSON
/// records back to back, in the order the linker joined them: one
/// `{"tool": …, "abi_version": …, "crate": …}` per `#[tool]` and one
/// `{"event": …}` per `#[event]`.
pub const ABI_SECTION: &str = ".berm.abi";

/// The section the build appends for the program: one [`ProgramRecord`].
pub const PROGRAM_SECTION: &str = ".berm.program";

/// The section the build appends for the program's kits: a JSON array of
/// [`Kit`], by name.
pub const KITS_SECTION: &str = ".berm.kits";

/// The most bytes a call's arguments, and a tool's result, may hold, when a
/// program's `crab.toml` sets no `buffer`.
pub const DEFAULT_BUFFER: usize = 16 * 1024;

/// One log a program emitted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logged {
    /// Its position in the program's logs, counting from 1.
    pub seq: u64,
    /// The call it was emitted in, as the host's call log numbers it. `None`
    /// for a log emitted in a call into the program from another host.
    pub call: Option<u64>,
    /// When it was emitted, in epoch milliseconds.
    pub at: u64,
    pub topics: Vec<String>,
    pub data: String,
}

/// A deployed program as its callers see it: the IDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
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
}

/// How a deployed program runs, for its owners: what [`Program`] leaves out.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Runtime {
    /// Its tools that are not public.
    pub tools: Vec<ToolSpec>,
    pub deps: Deps,
    pub events: Vec<EventSpec>,
    /// The part of `deps` this host answers to nothing for.
    #[serde(default, skip_serializing_if = "Deps::is_empty")]
    pub unresolved: Deps,
    /// Stopped by its owners, or deployed stopped: it takes no calls and fires
    /// no wakes.
    #[serde(default)]
    pub stopped: bool,
}

/// A deploy's answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployed {
    pub program: Program,
    /// How it broke callers of the program it replaced, as
    /// [`Manifest::breaks`] words it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaks: Vec<String>,
}

/// A deployed program as anyone it is visible to sees it: its IDL and what it
/// served.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub program: Program,
    pub served: host::Served,
}

/// What a service's owner set for calling it, as the host answering holds it.
/// Changes live, from the next call.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Terms {
    /// Each public tool's, by name.
    pub tools: BTreeMap<String, ToolTerms>,
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

/// The most one invocation may use: CPU time in microseconds, nested calls
/// excluded, rows read and written together, and bytes of linear memory.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    pub cpu: u64,
    pub rows: u64,
    /// Held from the invocation's start to its return, and reserved from the
    /// host's memory for that long; `0` is the most the host lets one guest
    /// grow to.
    #[serde(default)]
    pub memory: u64,
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
    /// Callable by accounts and other programs. A tool that is not is only
    /// called by its host, for the program's own wakes, crons and Stripe
    /// events.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public: bool,
    /// Callable from outside with no caller: no account, or an app's call with
    /// no user. A public tool that is not is refused such a call.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub anonymous: bool,
    /// Which callers a public tool takes. `None` takes both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callers: Option<Callers>,
    /// Called by its host when the cron expression its deploy or its owner
    /// sets fires. A cron tool is not public.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cron: bool,
    /// Called by its host with each Stripe event its daemon takes for the
    /// service, the event as the argument object. Not public; a program has
    /// at most one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stripe: bool,
}

/// One kind of caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Callers {
    /// Programs reaching it through `berm.call`.
    Services,
    /// Callers from outside, as the host authenticates them.
    Accounts,
}

/// What running a tool produced.
///
/// [`Self::Failed`] is the program's own report, and it arrives with the same
/// `200` a result does: the call was fine and the tool said no. A refusal — no
/// such program, no such tool, a trap — is a [`Failed`] with a status to match.
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

/// What a call used of one program: its runs, the chains it was the first link
/// of; its invocations, the CPU time they spent in microseconds, nested calls
/// excluded, the largest linear memory one
/// reached, in bytes, the linear memory they held over their wall-clock time
/// in MiB-microseconds, the rows they read and wrote, the bytes sent and
/// received through the proxy, the bytes of the logs they emitted, and the
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
}
