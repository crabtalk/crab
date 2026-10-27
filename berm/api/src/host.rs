//! What a host's daemons and peers send it, and what it answers with.

use crate::{Meter, Output};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

/// Header carrying a tool call's [`Meter`], summed over every
/// program the call reached, as JSON.
pub const METER: &str = "berm-meter";

/// Header carrying the [`Caller`] a call to the local host is made as, as
/// JSON. The local host verifies none of it.
pub const CALLER: &str = "x-berm-caller";

/// Bytes a service's database may hold when its deploy sets none.
pub const DATABASE_SIZE: u64 = 1 << 30;

/// The fewest bytes a deploy may let a service's database hold.
pub const MIN_DATABASE_SIZE: u64 = 64 << 10;

/// The most bytes a deploy may let a service's database hold.
pub const MAX_DATABASE_SIZE: u64 = 10 << 30;

/// Who called from outside, as the daemon hands them to the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Caller {
    /// The account calling, or for an app's call, the account owning the app.
    pub id: String,
    /// The app a browser called through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// The end user the call is made for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// For an app's anonymous call, the client's address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// For an app's call, the client's user agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

impl Caller {
    /// An app's call with no user: no account stands behind it.
    pub fn anonymous(&self) -> bool {
        self.app.is_some() && self.user.is_none()
    }
}

/// An end user, as an app's identity provider named them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub issuer: String,
    pub subject: String,
}

/// A deploy: the image `digest` names, published as `reference`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deploy {
    pub reference: String,
    pub digest: String,
    /// Deploy even when it breaks what callers used, or undoes a cron its
    /// owner set.
    #[serde(default)]
    pub force: bool,
    /// The cron expression of each cron tool, as the deploy's config sets
    /// them; a cron tool it leaves out has none. `None` leaves each as its
    /// owner set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crons: Option<BTreeMap<String, String>>,
    /// Bytes the service's database may hold, from [`MIN_DATABASE_SIZE`] to
    /// [`MAX_DATABASE_SIZE`].
    #[serde(default = "database_size")]
    pub size: u64,
    /// Install it stopped, to serve once started.
    #[serde(default)]
    pub stopped: bool,
    /// The project it is deployed in: the workspace it came from. `None`
    /// keeps the project it was in. Refused, unless `force`, when the service
    /// is in another project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Services of the same scope whose calls the breaks check leaves out:
    /// services deployed with it whose new version no longer calls it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exempt: Vec<String>,
}

/// The days back a deploy's breaks check counts what callers used.
pub const USED_DAYS: i32 = 30;

/// What a deploy would do, checked without installing it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checked {
    /// The version it would go live on.
    pub version: String,
    /// The services it calls by name, as its manifest declares them.
    pub calls: Vec<String>,
    /// How it breaks what callers of the running version used in the last
    /// [`USED_DAYS`] days, the callers `exempt` names left out.
    pub breaks: Vec<Broken>,
}

/// A break of what callers used, and who used it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Broken {
    /// The break, as [`Break`](crate::Break) words it.
    pub broken: String,
    pub by: Vec<LastUse>,
}

/// A caller of a broken tool or event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastUse {
    pub source: Source,
    /// A service by its name, the rest as the host names them.
    pub caller: String,
    /// The UTC day of its last call, `YYYY-MM-DD`.
    pub last: String,
}

/// A deploy refused for what it breaks: the message the refusal carries.
pub struct Breaking<'a>(pub &'a [Broken]);

impl fmt::Display for Breaking<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "this deploy breaks what callers used in the last {USED_DAYS} days: "
        )?;
        for (i, broken) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" / ")?;
            }
            write!(f, "{} (used by ", broken.broken)?;
            for (j, by) in broken.by.iter().enumerate() {
                if j > 0 {
                    f.write_str("; ")?;
                }
                write!(f, "{} {}, last {}", by.source, by.caller, by.last)?;
            }
            f.write_str(")")?;
        }
        f.write_str(". Deploy with `force` to override")
    }
}

impl Broken {
    /// `breaks` without the calls of the services `exempt` names, and without
    /// a break no caller is left using.
    pub fn exempted(breaks: Vec<Self>, exempt: &[String]) -> Vec<Self> {
        breaks
            .into_iter()
            .filter_map(|mut broken| {
                broken
                    .by
                    .retain(|by| !(by.source == Source::Service && exempt.contains(&by.caller)));
                (!broken.by.is_empty()).then_some(broken)
            })
            .collect()
    }
}

/// How a set of services deployed together switches, from each one's
/// [`Checked`], by name.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// The order they switch in: a service after those it calls, and a caller
    /// that stops calling a service before that service.
    pub order: Vec<String>,
    /// Each service's exempt callers: the others whose new version does not
    /// call it.
    pub exempt: BTreeMap<String, Vec<String>>,
    /// What each one breaks, its exempt callers left out.
    pub breaks: BTreeMap<String, Vec<Broken>>,
}

impl Plan {
    pub fn new(checked: &BTreeMap<String, Checked>) -> Self {
        let mut plan = Plan::default();
        // `before[b]` holds the services that switch before `b`.
        let mut before: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (name, check) in checked {
            let exempt: Vec<String> = checked
                .iter()
                .filter(|(other, checked)| *other != name && !checked.calls.contains(name))
                .map(|(other, _)| other.clone())
                .collect();
            for by in check.breaks.iter().flat_map(|broken| &broken.by) {
                if by.source == Source::Service && exempt.contains(&by.caller) {
                    before.entry(name).or_default().push(&by.caller);
                }
            }
            for callee in &check.calls {
                if let Some((callee, _)) = checked.get_key_value(callee) {
                    before.entry(name).or_default().push(callee);
                }
            }
            plan.breaks.insert(
                name.clone(),
                Broken::exempted(check.breaks.clone(), &exempt),
            );
            plan.exempt.insert(name.clone(), exempt);
        }
        // Kahn's sort, by name among the ready; a cycle switches in name order.
        let mut left: Vec<&str> = checked.keys().map(String::as_str).collect();
        while !left.is_empty() {
            let ready = left
                .iter()
                .position(|name| {
                    before
                        .get(name)
                        .is_none_or(|first| first.iter().all(|first| !left.contains(first)))
                })
                .unwrap_or(0);
            plan.order.push(left.remove(ready).to_owned());
        }
        plan
    }
}

/// A set refused for what each of `refused`, by name, was refused for: the
/// message the refusal carries.
pub struct SetRefused<'a>(pub &'a BTreeMap<String, String>);

impl fmt::Display for SetRefused<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the set is refused, and nothing in it switched: ")?;
        for (i, (name, reason)) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" | ")?;
            }
            write!(f, "{name}: {reason}")?;
        }
        Ok(())
    }
}

/// A set whose service `name` failed to switch for `reason`, the services
/// switched before it put back: the message the failure carries.
pub struct SetFailed<'a> {
    pub name: &'a str,
    pub reason: &'a str,
}

impl fmt::Display for SetFailed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} failed to switch, and the set was put back on what it ran: {}",
            self.name, self.reason
        )
    }
}

/// [`DATABASE_SIZE`], for `#[serde(default = "...")]`.
pub fn database_size() -> u64 {
    DATABASE_SIZE
}

/// A change to one of a service's tools.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUpdate {
    /// When a deprecated tool stops being served, in epoch seconds; `null`
    /// clears it.
    pub sunset: Option<i64>,
}

/// Where a service this host does not run lives: the peer it runs on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub service: String,
    pub peer: String,
}

/// A call from a service on one host to a service on another, as the link of
/// the chain it runs as.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// What the called service is told called it: a service's name, or
    /// nothing for a wake.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    /// The key of the service the call is counted against.
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The outside client's per-day key, as the chain's first host supplied it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub depth: u32,
    pub query: bool,
    /// Milliseconds left before the chain's deadline.
    pub timeout: u64,
    /// The services the call sits under, outermost first, the called one last.
    pub stack: Vec<String>,
}

/// How a call between hosts ended, when the called service ran.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    pub result: Output,
    /// The service refused its caller, through `berm.deny`.
    pub denied: bool,
    /// What the call used, by each service it reached and the tool of it
    /// that ran.
    #[serde(default)]
    pub meters: BTreeMap<String, BTreeMap<String, Meter>>,
}

/// What was used. Variants are in the order SQLite sorts their names.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Event,
    Tool,
}

/// Who used it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A service in the same scope.
    Service,
    Account,
    /// An app a browser called through.
    App,
    /// A call from outside without a caller.
    Anonymous,
    /// A Stripe event its daemon took for the service.
    Stripe,
}

/// How a use ended.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Done,
    Failed,
    /// The tool refused its caller.
    Denied,
    /// The host refused it before the tool ran.
    Refused,
}

impl Outcome {
    /// How a call that ran ended: `denied` is whether the tool refused its
    /// caller.
    pub fn of(result: &Output, denied: bool) -> Self {
        match (result, denied) {
            (Output::Done(_), _) => Self::Done,
            (Output::Failed(_), true) => Self::Denied,
            (Output::Failed(_), false) => Self::Failed,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serialize(f)
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serialize(f)
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.serialize(f)
    }
}

/// Uses of a service counted on another host, for the host it runs on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Used {
    pub service: String,
    pub kind: Kind,
    pub item: String,
    pub source: Source,
    pub caller: String,
    pub outcome: Outcome,
    pub day: i32,
    pub calls: i64,
    pub last: i64,
}

/// A service's runs, as anyone it is visible to sees them: the [`Metered`]
/// counts that say how it ran, and nothing it cost.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Served {
    /// Each of the last 30 days with any, oldest first.
    pub days: Vec<ServedDay>,
    /// Runs since the service was first deployed.
    pub runs: u64,
}

/// One day of [`Served`], counted as [`Metered`] counts it.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServedDay {
    /// Days since the Unix epoch.
    pub day: i32,
    pub runs: u64,
    pub calls: u64,
    pub failed: u64,
}

/// One caller of one of a service's tools or events over the last 30 days.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub kind: Kind,
    pub item: String,
    pub source: Source,
    /// The service's name, or the account or app.
    pub caller: String,
    pub outcome: Outcome,
    pub calls: i64,
    /// The last call, in epoch seconds.
    pub last: i64,
    /// For a tool, whether the deployed version makes it public.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}

/// `?topic=&call=&after=&limit=` on a service's logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Only logs carrying this topic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Only logs emitted in this call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<i64>,
    /// Only logs after this sequence number.
    #[serde(default)]
    pub after: i64,
    #[serde(default = "limit")]
    pub limit: i64,
}

fn limit() -> i64 {
    100
}

impl Page {
    /// The `Link` header naming the logs after `seq` as `next`.
    pub fn next(&self, seq: i64) -> String {
        rel(
            serde_urlencoded::to_string(Page {
                after: seq,
                ..self.clone()
            }),
            "next",
        )
    }
}

/// `?service=&tool=&event=&q=&outcome=&since=&until=&before=&limit=` on a
/// service's calls, or a scope's.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallPage {
    /// Only calls of this service, on a scope's calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Only calls of this tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Only calls that emitted a log carrying this topic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Only calls that emitted a log whose data or a topic holds this text,
    /// its ASCII letters in either case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// Only calls that ended so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    /// Only calls begun at or after this, in epoch milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    /// Only calls begun before this, in epoch milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
    /// Only calls numbered below this id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
    #[serde(default = "limit")]
    pub limit: i64,
}

impl CallPage {
    /// The `Link` header naming the calls below `id` as `next`.
    pub fn next(&self, id: u64) -> String {
        rel(
            serde_urlencoded::to_string(CallPage {
                before: Some(id),
                ..self.clone()
            }),
            "next",
        )
    }
}

/// One `Link` entry: a reference relative to the request, with `query`.
fn rel(query: Result<String, serde_urlencoded::ser::Error>, name: &str) -> String {
    format!("<?{}>; rel=\"{name}\"", query.unwrap_or_default())
}

/// A service's cron tools.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Crons {
    pub crons: Vec<CronJob>,
    /// Its scope is held: none runs until it is let go.
    pub held: bool,
}

/// A cron tool: one its manifest marks `#[tool(cron)]`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub tool: String,
    /// Its expression, in UTC; `None` while it has none and is not called.
    pub cron: Option<String>,
    /// When it next runs, in epoch milliseconds; `None` while it has no
    /// expression.
    pub due: Option<u64>,
    /// Times it failed in a row.
    pub failures: u64,
    /// Its last run, in the last 30 days.
    pub last: Option<Call>,
}

/// The wakes armed to call a service's tools that have not run.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wakes {
    /// Soonest first.
    pub wakes: Vec<Wake>,
    /// Its scope is held: none runs until it is let go.
    pub held: bool,
}

/// A wake armed with `berm.call.after`.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wake {
    /// The tool it calls.
    pub tool: String,
    /// The arguments it calls the tool with, as JSON text.
    pub args: String,
    /// When it fires, in epoch milliseconds.
    pub due: u64,
    /// The service that armed it, by its name.
    pub by: String,
    /// Times it failed in a row since it was last armed.
    pub failures: u64,
    /// The last time a wake armed by the same service ran this tool, in the
    /// last 30 days.
    pub last: Option<Call>,
}

/// One run of a service: a call from outside or a wake.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    /// Its position among the service's calls; newer calls have larger ids.
    pub id: u64,
    /// When it began, in epoch milliseconds.
    pub at: u64,
    pub tool: String,
    pub source: Source,
    /// As [`Usage::caller`] names it.
    pub caller: String,
    pub outcome: Outcome,
    /// How long it ran, in milliseconds.
    pub ms: u64,
    /// What it used, summed over every service it reached. `None` for a call
    /// the host refused.
    pub meter: Option<Meter>,
    /// How many logs it emitted.
    pub logs: u64,
}

/// One call a run made to another of its owner's services, or to its own
/// tools, on the host the run ran on.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NestedCall {
    /// Its number in the run, in the order the run's calls began; the run's
    /// first call is 0.
    pub id: u32,
    /// The number of the call that made it.
    pub parent: u32,
    /// The service's name, in the run's scope.
    pub service: String,
    pub tool: String,
    /// When it began, in epoch milliseconds.
    pub at: u64,
    pub ms: u64,
    pub outcome: Outcome,
    /// CPU microseconds it spent itself, the calls it made excluded.
    pub cpu: u64,
    pub read: u64,
    pub written: u64,
}

/// What a page of calls matches, counted: by time and by what it can be
/// narrowed to.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallStats {
    /// Each bucket's length, in milliseconds.
    pub bucket: u64,
    /// Each bucket with a call, oldest first.
    pub buckets: Vec<Bucket>,
    /// The calls of each tool, every filter but `tool` applied.
    pub tools: BTreeMap<String, u64>,
    /// The calls that ended each way, every filter but `outcome` applied.
    pub outcomes: BTreeMap<Outcome, u64>,
    /// The calls of each service, every filter but `service` applied; empty
    /// for one service's calls.
    pub services: BTreeMap<String, u64>,
}

/// The calls begun in one bucket of time, by how they ended.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    /// Its start, in epoch milliseconds.
    pub at: u64,
    pub outcomes: BTreeMap<Outcome, u64>,
}

impl CallStats {
    /// The bucket length, in milliseconds, that cuts `since..until` into at
    /// most 60: a minute, 5 or 15 minutes, an hour, 3 or 6 hours, or a day.
    pub fn bucket(since: u64, until: u64) -> u64 {
        const STEPS: [u64; 7] = [
            60_000, 300_000, 900_000, 3_600_000, 10_800_000, 21_600_000, 86_400_000,
        ];
        let span = until.saturating_sub(since);
        STEPS
            .into_iter()
            .find(|step| span / step <= 60)
            .unwrap_or(86_400_000)
    }

    /// Add `other`, read over the same range and buckets.
    pub fn merge(&mut self, other: CallStats) {
        self.bucket = other.bucket;
        for bucket in other.buckets {
            match self.buckets.iter_mut().find(|mine| mine.at == bucket.at) {
                Some(mine) => {
                    for (outcome, calls) in bucket.outcomes {
                        *mine.outcomes.entry(outcome).or_default() += calls;
                    }
                }
                None => self.buckets.push(bucket),
            }
        }
        self.buckets.sort_by_key(|bucket| bucket.at);
        for (tool, calls) in other.tools {
            *self.tools.entry(tool).or_default() += calls;
        }
        for (outcome, calls) in other.outcomes {
            *self.outcomes.entry(outcome).or_default() += calls;
        }
        for (service, calls) in other.services {
            *self.services.entry(service).or_default() += calls;
        }
    }
}

/// One run of one of a scope's services.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCall {
    /// The service's name in its scope.
    pub service: String,
    #[serde(flatten)]
    pub call: Call,
}

/// What a call that failed ran with, kept so it can be run again.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallInput {
    pub tool: String,
    /// Its arguments as sent: JSON, or a Stripe event's body.
    pub args: String,
    /// Who it was made as; none for a wake, a Stripe event or an anonymous
    /// call.
    pub caller: Option<Caller>,
    /// The image it ran, as `sha256:` and hex.
    pub digest: String,
}

/// What a service is using now.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Live {
    /// Its invocations running now.
    pub running: u64,
    /// Bytes of linear memory they hold.
    pub memory: u64,
    /// Bytes it keeps: its database file, its write-ahead log, its state, its
    /// logs kept, and its image and compiled code.
    pub stored: u64,
    /// CPU time spent, in microseconds.
    pub cpu: Count,
    /// Rows read and written in its database and state.
    pub read: Count,
    pub written: Count,
    /// Bytes sent and received through `berm.http.proxy`.
    pub proxied: Count,
    /// Bytes of the logs it emitted.
    pub logged: Count,
}

/// The memory a scope's services hold now, and the most they may.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Memory {
    /// Bytes; `0` is no limit.
    pub limit: u64,
    /// Bytes of the images and compiled code of its services deployed.
    pub loaded: u64,
    /// Bytes of linear memory its invocations running now hold.
    pub running: u64,
}

/// One of a [`Live`]'s counts.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Count {
    /// Since its first deploy.
    pub total: u64,
    /// Per second over the UTC day so far.
    pub per_second: f64,
}

/// One day of a service's meter, of one of its tools.
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metered {
    /// Days since the Unix epoch.
    pub day: i32,
    /// The tool whose invocations these are. Empty for what the service
    /// holds itself, `stored`, and for a day summed over its tools.
    #[serde(default)]
    pub tool: String,
    /// Chains whose first link the service was: a call from outside, a wake
    /// or a Stripe event.
    #[serde(default)]
    pub runs: u64,
    /// Invocations of the service: from outside, from other services, and
    /// wakes.
    pub calls: u64,
    /// CPU time those invocations spent, in microseconds.
    pub cpu: u64,
    /// The largest linear memory one of them reached, in bytes.
    pub memory: u64,
    /// Their budgets of memory, each held from its start to its return,
    /// waiting included, in MiB-microseconds.
    #[serde(default)]
    pub held: u64,
    /// Rows read and written in the service's database and state.
    pub read: u64,
    pub written: u64,
    /// Bytes sent and received through `berm.http.proxy`.
    pub proxied: u64,
    /// Bytes of the logs those invocations emitted.
    #[serde(default)]
    pub logged: u64,
    /// Of `calls`, those that trapped or failed, but not those the tool
    /// refused through `berm.deny`.
    #[serde(default)]
    pub failed: u64,
    /// Bytes sent off the host: the bodies of answers to outside callers, and
    /// each request `berm.http.fetch` and `berm.http.proxy` sent.
    #[serde(default)]
    pub sent: u64,
    /// Bytes the service kept when the day was sampled: its database file,
    /// its write-ahead log, its state, its logs kept, and its image and
    /// compiled code.
    pub stored: u64,
}

impl std::ops::AddAssign<Meter> for Metered {
    /// Sums every count but `memory`, which keeps the larger.
    fn add_assign(&mut self, used: Meter) {
        self.runs += used.runs;
        self.calls += used.calls;
        self.cpu += used.cpu;
        self.memory = self.memory.max(used.memory);
        self.held += used.held;
        self.read += used.read;
        self.written += used.written;
        self.proxied += used.proxied;
        self.logged += used.logged;
        self.failed += used.failed;
        self.sent += used.sent;
    }
}
