//! Each run of a service this host runs: held in memory and written in batches
//! to `service_calls` on the host the service runs on, kept [`usage::WINDOW`]
//! days.

use crate::{
    host::{self, Invoked},
    record::{logs::Emitted, usage},
    state::store::Store,
};
use anyhow::Result;
use berm::{Now, tape::Tape};
use berm_api::{
    Meter,
    host::{
        Bucket, Call, CallInput, CallPage, CallStats, Outcome, ServiceCall, Source, Spread,
        ToolUsage,
    },
};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, Row, named_params, params};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    mem,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// The most bytes of tapes a service keeps; past it, the oldest are dropped.
const TAPES_MAX: usize = 64 << 20;

/// Runs not yet written, each with the key of the service it ran.
pub(crate) struct Pending {
    pending: Mutex<Vec<Ran>>,
    /// The last id handed out.
    last: AtomicU64,
    now: Now,
}

/// A run not yet written.
#[derive(Clone)]
pub(crate) struct Ran {
    id: u64,
    /// The key of the service it ran.
    service: String,
    at: u64,
    tool: String,
    source: Source,
    caller: String,
    outcome: Outcome,
    ms: u64,
    meter: Option<Meter>,
    /// What the tool's own invocation used.
    own: Option<Meter>,
    /// Kept for a run that failed.
    input: Option<CallInput>,
    /// Kept for a run that failed, encoded.
    tape: Option<Vec<u8>>,
}

/// A service's runs and logs not yet written.
pub(crate) struct Unsaved {
    pub ran: Vec<Ran>,
    pub emitted: Vec<Emitted>,
}

/// A run being timed, from [`Pending::start`].
#[derive(Clone, Copy)]
pub(crate) struct Started {
    /// The id its row takes.
    pub id: u64,
    at: u64,
    began: Instant,
}

impl Pending {
    /// Handing out ids after `last`, the largest written.
    pub(crate) fn new(now: Now, last: u64) -> Self {
        Self {
            pending: Mutex::default(),
            last: AtomicU64::new(last),
            now,
        }
    }

    pub(crate) fn start(&self) -> Started {
        Started {
            id: self.last.fetch_add(1, Ordering::Relaxed) + 1,
            at: (self.now)(),
            began: Instant::now(),
        }
    }

    /// Hold a run of `tool` on the service keyed `service`, begun at
    /// `started`, keeping `input` and `tape` when it failed.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &self,
        service: &str,
        started: Started,
        tool: &str,
        source: Source,
        caller: &str,
        outcome: Outcome,
        invoked: Option<&Invoked>,
        input: Option<CallInput>,
        tape: Option<&Tape>,
    ) {
        let failed = outcome == Outcome::Failed;
        self.pending.lock().push(Ran {
            id: started.id,
            service: service.to_owned(),
            at: started.at,
            tool: tool.to_owned(),
            source,
            caller: caller.to_owned(),
            outcome,
            ms: started.began.elapsed().as_millis() as u64,
            meter: invoked.map(|invoked| invoked.meter),
            own: invoked.map(|invoked| invoked.own),
            input: input.filter(|_| failed),
            tape: tape.filter(|_| failed).map(Vec::from),
        });
    }

    pub(crate) fn take(&self) -> Vec<Ran> {
        mem::take(&mut *self.pending.lock())
    }

    /// The runs of the service keyed `service` not yet written.
    pub(crate) fn unsaved(&self, service: &str) -> Vec<Ran> {
        self.pending
            .lock()
            .iter()
            .filter(|ran| ran.service == service)
            .cloned()
            .collect()
    }

    /// The runs of `scope`'s services not yet written.
    pub(crate) fn unsaved_in(&self, scope: &str) -> Vec<Ran> {
        self.pending
            .lock()
            .iter()
            .filter(|ran| host::split(&ran.service).0 == scope)
            .cloned()
            .collect()
    }
}

impl Ran {
    /// Whether `page`'s filters but `event` take it.
    fn on(&self, page: &CallPage) -> bool {
        page.tool.as_ref().is_none_or(|tool| *tool == self.tool)
            && page.outcome.is_none_or(|outcome| outcome == self.outcome)
            && page.since.is_none_or(|since| self.at >= since)
            && page.until.is_none_or(|until| self.at < until)
            && page.before.is_none_or(|before| self.id < before)
    }

    /// The run as a call of its service that emitted `logs` logs.
    fn call(self, logs: u64) -> ServiceCall {
        ServiceCall {
            service: host::split(&self.service).1.to_owned(),
            call: Call {
                id: self.id,
                at: self.at,
                tool: self.tool,
                source: self.source,
                caller: usage::caller(self.source, self.caller),
                outcome: self.outcome,
                ms: self.ms,
                meter: self.meter,
                logs,
            },
        }
    }
}

/// Add `ran`, and drop what is older than [`usage::WINDOW`] days before `now`.
/// Runs of a service this host does not run are dropped.
pub(crate) async fn write(store: &Store, ran: Vec<Ran>, now: u64) -> Result<()> {
    let since = now.saturating_sub(usage::WINDOW as u64 * 86_400_000);
    store
        .each(ran, |tx, call| {
            let (scope, name) = host::split(&call.service);
            let meter = call
                .meter
                .as_ref()
                .map(|meter| serde_json::to_string(meter).expect("a meter is JSON"));
            let input = call
                .input
                .as_ref()
                .map(|input| serde_json::to_string(input).expect("an input is JSON"));
            tx.prepare_cached(
                "INSERT INTO service_calls
                        (id, scope, name, at, tool, source, caller, outcome, ms, meter, input, tape,
                         cpu, read, written)
                 SELECT ?10, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?11, ?12, ?13, ?14, ?15
                  WHERE EXISTS (SELECT 1 FROM services WHERE scope = ?1 AND name = ?2)",
            )?
            .execute(params![
                scope,
                name,
                call.at as i64,
                call.tool,
                call.source,
                call.caller,
                call.outcome,
                call.ms as i64,
                meter,
                call.id as i64,
                input,
                call.tape,
                call.own.map(|own| own.cpu as i64),
                call.own.map(|own| own.read as i64),
                call.own.map(|own| own.written as i64)
            ])?;
            Ok(())
        })
        .await?;
    store
        .run(move |conn| {
            conn.execute(
                "DELETE FROM service_calls WHERE at < ?1",
                params![since as i64],
            )?;
            conn.execute(
                "UPDATE service_calls SET tape = NULL
                  WHERE rowid IN (
                        SELECT rowid FROM (
                               SELECT rowid, SUM(LENGTH(tape)) OVER (
                                      PARTITION BY scope, name ORDER BY id DESC) AS kept
                                 FROM service_calls WHERE tape IS NOT NULL)
                         WHERE kept > ?1)",
                params![TAPES_MAX as i64],
            )?;
            Ok(())
        })
        .await
}

/// `scope/name`'s run `id`'s tape, encoded, as written or in `unsaved`:
/// `Ok(None)` for a run that kept none, an error for no such run.
pub(crate) async fn tape(
    store: &Store,
    scope: &str,
    name: &str,
    id: u64,
    unsaved: Vec<Ran>,
) -> Result<Option<Option<Vec<u8>>>> {
    if let Some(ran) = unsaved.into_iter().find(|ran| ran.id == id) {
        return Ok(Some(ran.tape));
    }
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT tape FROM service_calls WHERE scope = ?1 AND name = ?2 AND id = ?3",
                params![scope, name, id as i64],
                |row| row.get(0),
            )
            .optional()
        })
        .await
}

/// What `scope/name`'s run `id` ran with, as written or in `unsaved`:
/// `Ok(None)` for a run that kept none, an error for no such run.
pub(crate) async fn input(
    store: &Store,
    scope: &str,
    name: &str,
    id: u64,
    unsaved: Vec<Ran>,
) -> Result<Option<Option<CallInput>>> {
    if let Some(ran) = unsaved.into_iter().find(|ran| ran.id == id) {
        return Ok(Some(ran.input));
    }
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            let input: Option<Option<String>> = conn
                .query_row(
                    "SELECT input FROM service_calls WHERE scope = ?1 AND name = ?2 AND id = ?3",
                    params![scope, name, id as i64],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(input.map(|input| input.and_then(|input| serde_json::from_str(&input).ok())))
        })
        .await
}

/// The calls a page names, of `service_calls c`, as `:scope`, `:name`,
/// `:tool`, `:outcome`, `:since`, `:until`, `:before`, `:event` and `:q` bind
/// them; each `NULL` names every call.
const MATCHING: &str = "FROM service_calls c
                  WHERE c.scope = :scope AND (:name IS NULL OR c.name = :name)
                    AND (:tool IS NULL OR c.tool = :tool)
                    AND (:outcome IS NULL OR c.outcome = :outcome)
                    AND (:since IS NULL OR c.at >= :since)
                    AND (:until IS NULL OR c.at < :until)
                    AND (:before IS NULL OR c.id < :before)
                    AND (:event IS NULL OR EXISTS (
                        SELECT 1 FROM service_logs l, json_each(l.topics) t
                         WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id
                           AND t.value = :event))
                    AND (:q IS NULL OR EXISTS (
                        SELECT 1 FROM service_logs l
                         WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id
                           AND (instr(lower(l.data), :q) > 0 OR instr(lower(l.topics), :q) > 0)))";

/// The runs on `page` of `scope/name`, or of every service in `scope`
/// without a `name`, newest first, as written and in `unsaved`. A service
/// caller is named by its name.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: Option<&str>,
    page: CallPage,
    unsaved: Unsaved,
) -> Result<Vec<ServiceCall>> {
    let (scope, name) = (scope.to_owned(), name.map(str::to_owned));
    store
        .read(move |conn| {
            let since = page.since.map(|since| since as i64);
            let until = page.until.map(|until| until as i64);
            let before = page.before.map(|before| before as i64);
            let q = page.q.as_deref().map(str::to_ascii_lowercase);
            // The written calls a page can hold: runs not yet written only
            // push them further down.
            let written = conn
                .prepare_cached(&format!(
                    "SELECT c.id {MATCHING} ORDER BY c.id DESC LIMIT :limit"
                ))?
                .query_map(
                    named_params! {
                        ":scope": scope, ":name": name, ":tool": page.tool,
                        ":event": page.event, ":q": q, ":outcome": page.outcome,
                        ":since": since, ":until": until, ":before": before,
                        ":limit": page.limit,
                    },
                    |row| row.get::<_, i64>(0),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let Unsaved { ran, emitted } = unsaved;
            let ran = ran
                .into_iter()
                .filter(|ran| {
                    name.as_ref()
                        .is_none_or(|name| host::split(&ran.service).1 == name)
                        && ran.on(&page)
                })
                .collect();
            let unsaved = unwritten(
                conn,
                &scope,
                ran,
                &emitted,
                page.event.as_deref(),
                q.as_deref(),
            )?;
            let mut ids: Vec<(i64, bool)> = written
                .into_iter()
                .map(|id| (id, true))
                .chain(unsaved.iter().map(|ran| (ran.call.id as i64, false)))
                .collect();
            ids.sort_unstable_by_key(|(id, _)| Reverse(*id));
            ids.truncate(page.limit as usize);
            let shown: Vec<i64> = ids
                .iter()
                .filter(|(_, written)| *written)
                .map(|(id, _)| *id)
                .collect();
            let mut found: HashMap<u64, ServiceCall> = conn
                .prepare_cached(&format!(
                    "SELECT {COLUMNS}, c.name FROM service_calls c
                      WHERE c.id IN (SELECT value FROM json_each(?1))"
                ))?
                .query_map(
                    params![serde_json::to_string(&shown).expect("ids are JSON")],
                    |row| {
                        Ok(ServiceCall {
                            call: call(row)?,
                            service: row.get(9)?,
                        })
                    },
                )?
                .map(|ran| ran.map(|ran| (ran.call.id, ran)))
                .collect::<rusqlite::Result<_>>()?;
            found.extend(unsaved.into_iter().map(|ran| (ran.call.id, ran)));
            let calls = ids
                .into_iter()
                .filter_map(|(id, _)| found.remove(&(id as u64)))
                .collect();
            Ok(calls)
        })
        .await
}

/// What the calls on `page` of `scope/name`, or of every service in `scope`
/// without a `name`, count to, as written and in `unsaved`, by `bucket`
/// milliseconds of their start and by what each facet narrows them to. Its
/// `before` and `limit` are not read.
pub(crate) async fn stats(
    store: &Store,
    scope: &str,
    name: Option<&str>,
    page: CallPage,
    bucket: u64,
    unsaved: Unsaved,
) -> Result<CallStats> {
    let (scope, name) = (scope.to_owned(), name.map(str::to_owned));
    store
        .read(move |conn| {
            let q = page.q.as_deref().map(str::to_ascii_lowercase);
            // Each count is one query of the same calls: the facets drop
            // their own filter.
            let count = |group: &str,
                         name: Option<&str>,
                         tool: Option<&str>,
                         outcome: Option<Outcome>|
             -> rusqlite::Result<Vec<(String, Outcome, i64)>> {
                conn.prepare_cached(&format!(
                    "SELECT {group}, c.outcome, COUNT(*) {MATCHING} GROUP BY 1, 2"
                ))?
                .query_map(
                    named_params! {
                        ":scope": scope, ":name": name, ":tool": tool,
                        ":event": page.event, ":q": q, ":outcome": outcome,
                        ":since": page.since.map(|since| since as i64),
                        ":until": page.until.map(|until| until as i64),
                        ":before": None::<i64>,
                    },
                    |row| {
                        Ok((
                            row.get::<_, rusqlite::types::Value>(0).map(text)?,
                            row.get(1)?,
                            row.get(2)?,
                        ))
                    },
                )?
                .collect()
            };
            // A scope's calls are narrowed to `page.service` but by the services facet.
            let (tool, name_) = (
                page.tool.as_deref(),
                name.as_deref().or(page.service.as_deref()),
            );
            let mut stats = CallStats {
                bucket,
                ..CallStats::default()
            };
            let mut buckets: BTreeMap<u64, BTreeMap<Outcome, u64>> = BTreeMap::new();
            // The length is the host's own number, so it is written in.
            let by_time = format!("(c.at / {bucket}) * {bucket}");
            for (at, outcome, calls) in count(&by_time, name_, tool, page.outcome)? {
                *buckets
                    .entry(at.parse().unwrap_or(0))
                    .or_default()
                    .entry(outcome)
                    .or_default() += calls as u64;
            }
            for (tool, _, calls) in count("c.tool", name_, None, page.outcome)? {
                *stats.tools.entry(tool).or_default() += calls as u64;
            }
            for (_, outcome, calls) in count("''", name_, tool, None)? {
                *stats.outcomes.entry(outcome).or_default() += calls as u64;
            }
            if name.is_none() {
                for (service, _, calls) in count("c.name", None, tool, page.outcome)? {
                    *stats.services.entry(service).or_default() += calls as u64;
                }
            }
            let Unsaved { ran, emitted } = unsaved;
            let ran = ran
                .into_iter()
                .filter(|ran| {
                    page.since.is_none_or(|since| ran.at >= since)
                        && page.until.is_none_or(|until| ran.at < until)
                })
                .collect();
            for ran in unwritten(
                conn,
                &scope,
                ran,
                &emitted,
                page.event.as_deref(),
                q.as_deref(),
            )? {
                let ServiceCall { service, call } = ran;
                let mine = name.as_ref().is_none_or(|name| *name == service);
                let of_service = page.service.as_ref().is_none_or(|named| *named == service);
                let of_tool = page.tool.as_ref().is_none_or(|tool| *tool == call.tool);
                let of_outcome = page.outcome.is_none_or(|outcome| outcome == call.outcome);
                if !mine {
                    continue;
                }
                if of_service && of_tool && of_outcome {
                    let at = call.at / bucket * bucket;
                    *buckets
                        .entry(at)
                        .or_default()
                        .entry(call.outcome)
                        .or_default() += 1;
                }
                if of_service && of_outcome {
                    *stats.tools.entry(call.tool.clone()).or_default() += 1;
                }
                if of_service && of_tool {
                    *stats.outcomes.entry(call.outcome).or_default() += 1;
                }
                if name.is_none() && of_tool && of_outcome {
                    *stats.services.entry(service).or_default() += 1;
                }
            }
            stats.buckets = buckets
                .into_iter()
                .map(|(at, outcomes)| Bucket { at, outcomes })
                .collect();
            Ok(stats)
        })
        .await
}

/// What each tool of `scope/name` used over its calls written, by tool.
pub(crate) async fn tools(
    store: &Store,
    scope: &str,
    name: &str,
) -> Result<BTreeMap<String, ToolUsage>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            #[derive(Default)]
            struct Counted {
                calls: u64,
                read: u64,
                written: u64,
                cpu: BTreeMap<usize, u64>,
                rows: BTreeMap<usize, u64>,
                cpu_max: u64,
                rows_max: u64,
            }
            let mut tools: BTreeMap<String, Counted> = BTreeMap::new();
            let mut statement = conn.prepare_cached(
                "SELECT tool, cpu, read, written FROM service_calls
                  WHERE scope = ?1 AND name = ?2 AND cpu IS NOT NULL",
            )?;
            let mut rows = statement.query(params![scope, name])?;
            while let Some(row) = rows.next()? {
                let counted = tools.entry(row.get(0)?).or_default();
                let cpu = row.get::<_, i64>(1)? as u64;
                let (read, written) = (row.get::<_, i64>(2)? as u64, row.get::<_, i64>(3)? as u64);
                counted.calls += 1;
                counted.read += read;
                counted.written += written;
                *counted.cpu.entry(Spread::index(cpu)).or_default() += 1;
                *counted
                    .rows
                    .entry(Spread::index(read + written))
                    .or_default() += 1;
                counted.cpu_max = counted.cpu_max.max(cpu);
                counted.rows_max = counted.rows_max.max(read + written);
            }
            Ok(tools
                .into_iter()
                .map(|(tool, counted)| {
                    let usage = ToolUsage {
                        calls: counted.calls,
                        read: counted.read,
                        written: counted.written,
                        cpu: Spread::of(&counted.cpu, counted.cpu_max),
                        rows: Spread::of(&counted.rows, counted.rows_max),
                    };
                    (tool, usage)
                })
                .collect())
        })
        .await
}

/// A value SQLite answered, as text.
fn text(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Integer(n) => n.to_string(),
        rusqlite::types::Value::Text(text) => text,
        _ => String::new(),
    }
}

/// `ran`, runs of `scope`'s services not yet written, as calls, each with the
/// logs it emitted, written and in `emitted`; only those that emitted one
/// carrying `event`, and one holding `q` as [`Emitted::holds`] reads it, when
/// given. A call's id is its host's alone.
fn unwritten(
    conn: &Connection,
    scope: &str,
    ran: Vec<Ran>,
    emitted: &[Emitted],
    event: Option<&str>,
    q: Option<&str>,
) -> rusqlite::Result<Vec<ServiceCall>> {
    if ran.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<u64> = ran.iter().map(|ran| ran.id).collect();
    let written: HashMap<u64, (u64, bool, bool)> = conn
        .prepare_cached(
            "SELECT l.call, COUNT(*),
                    MAX(:event IS NOT NULL AND EXISTS (
                        SELECT 1 FROM json_each(l.topics) t WHERE t.value = :event)),
                    MAX(:q IS NOT NULL
                        AND (instr(lower(l.data), :q) > 0 OR instr(lower(l.topics), :q) > 0))
               FROM service_logs l
              WHERE l.scope = :scope
                AND l.call IN (SELECT value FROM json_each(:ids))
              GROUP BY l.call",
        )?
        .query_map(
            named_params! {
                ":scope": scope, ":event": event, ":q": q,
                ":ids": serde_json::to_string(&ids).expect("ids are JSON"),
            },
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    (row.get::<_, i64>(1)? as u64, row.get(2)?, row.get(3)?),
                ))
            },
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ran
        .into_iter()
        .filter_map(|ran| {
            let (mut logs, mut carries, mut holds) =
                written.get(&ran.id).copied().unwrap_or_default();
            for log in emitted.iter().filter(|log| log.call == Some(ran.id)) {
                logs += 1;
                carries |= event.is_some_and(|event| log.topics.iter().any(|topic| topic == event));
                holds |= q.is_some_and(|q| log.holds(q));
            }
            ((event.is_none() || carries) && (q.is_none() || holds)).then(|| ran.call(logs))
        })
        .collect())
}

/// The newer of `written` and the last run of `tool` of a service in `scope`
/// in `unsaved`, that service's own, that `caller`, a service key, made.
pub(crate) fn last(
    conn: &Connection,
    scope: &str,
    written: Option<Call>,
    unsaved: &Unsaved,
    tool: &str,
    caller: &str,
) -> rusqlite::Result<Option<Call>> {
    let newer = unsaved
        .ran
        .iter()
        .filter(|ran| {
            ran.tool == tool
                && ran.source == Source::Service
                && ran.caller == caller
                && written.as_ref().is_none_or(|call| ran.id > call.id)
        })
        .max_by_key(|ran| ran.id);
    match newer {
        Some(ran) => Ok(
            unwritten(conn, scope, vec![ran.clone()], &unsaved.emitted, None, None)?
                .pop()
                .map(|ran| ran.call),
        ),
        None => Ok(written),
    }
}

/// The columns [`call`] reads, in order, of `service_calls c`.
pub(crate) const COLUMNS: &str = "c.id, c.at, c.tool, c.source, c.caller, c.outcome, c.ms, c.meter,
    (SELECT COUNT(*) FROM service_logs l
      WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id)";

/// A row of [`COLUMNS`] as a call. A service caller is named by its name.
pub(crate) fn call(row: &Row) -> rusqlite::Result<Call> {
    let source = row.get(3)?;
    let meter: Option<String> = row.get(7)?;
    Ok(Call {
        id: row.get::<_, i64>(0)? as u64,
        at: row.get::<_, i64>(1)? as u64,
        tool: row.get(2)?,
        caller: usage::caller(source, row.get(4)?),
        source,
        outcome: row.get(5)?,
        ms: row.get::<_, i64>(6)? as u64,
        meter: meter.and_then(|meter| serde_json::from_str(&meter).ok()),
        logs: row.get::<_, i64>(8)? as u64,
    })
}

/// The largest call id written, or 0.
pub(crate) async fn newest(store: &Store) -> Result<u64> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT COALESCE(MAX(id), 0) FROM service_calls",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .map(|last| last as u64)
}
