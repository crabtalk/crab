//! Each run of a service this host runs: held in memory and written in batches
//! to `service_calls` on the host the service runs on, kept [`usage::WINDOW`]
//! days or until its fleet stores it. A failed run's input and tape are kept
//! in `service_replays`, past its row.

use crate::{
    host::{self, Invoked},
    record::{
        flight::{Flight, Table, Taken, Unflushed},
        logs::Emitted,
        usage,
    },
    state::store::Store,
};
use anyhow::Result;
use berm::{Now, tape::Tape};
use berm_api::{
    Meter, digest,
    host::{
        Bucket, Call, CallInput, CallPage, CallStats, Caller, NestedCall, Outcome, ServiceCall,
        Source,
    },
};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, Row, ToSql, named_params, params};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, HashMap},
    mem,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

/// The most bytes of tapes a service keeps; past it, the oldest are dropped.
const TAPES_MAX: usize = 64 << 20;

/// Runs not yet written, each with the key of the service it ran.
pub(crate) struct Pending {
    pending: Mutex<Vec<Ran>>,
    flight: Mutex<Flight<Ran>>,
    /// The calls each run in flight made past its first, by its id.
    nested: Mutex<HashMap<u64, Vec<NestedCall>>>,
    /// The ids of runs started and not yet held.
    open: Open,
    /// The last id handed out.
    last: AtomicU64,
    now: Now,
}

/// The ids of runs started and not yet held, shared with each [`Started`].
type Open = Arc<Mutex<BTreeSet<u64>>>;

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
    input: Option<Arc<CallInput>>,
    /// Kept for a run that failed, encoded.
    tape: Option<Arc<[u8]>>,
    /// The calls it made past its first.
    nested: Arc<[NestedCall]>,
}

/// A service's runs and logs not yet written, as a read took them.
pub(crate) struct Unsaved {
    pub ran: Unflushed<Ran>,
    pub emitted: Unflushed<Emitted>,
}

impl Unsaved {
    /// Those `conn`'s read transaction does not see written.
    pub(crate) fn resolve(self, conn: &Connection) -> rusqlite::Result<Unwritten> {
        Ok(Unwritten {
            ran: self.ran.resolve(conn)?,
            emitted: self.emitted.resolve(conn)?,
        })
    }
}

/// A service's runs and logs a read transaction does not see written.
pub(crate) struct Unwritten {
    pub ran: Vec<Ran>,
    pub emitted: Vec<Emitted>,
}

/// A run being timed, from [`Pending::start`]; open until it drops.
pub(crate) struct Started {
    /// The id its row takes.
    pub id: u64,
    at: u64,
    began: Instant,
    open: Open,
}

impl Drop for Started {
    fn drop(&mut self) {
        self.open.lock().remove(&self.id);
    }
}

impl Pending {
    /// Handing out ids after `last`, the largest written, and holding runs
    /// after `flushed`, the last generation written.
    pub(crate) fn new(now: Now, last: u64, flushed: u64) -> Self {
        Self {
            pending: Mutex::default(),
            flight: Mutex::new(Flight::new(Table::Calls, flushed)),
            nested: Mutex::default(),
            open: Open::default(),
            last: AtomicU64::new(last),
            now,
        }
    }

    pub(crate) fn start(&self) -> Started {
        let mut open = self.open.lock();
        let id = self.last.fetch_add(1, Ordering::Relaxed) + 1;
        open.insert(id);
        Started {
            id,
            at: (self.now)(),
            began: Instant::now(),
            open: self.open.clone(),
        }
    }

    /// The smallest id a run not yet written may take: one started and not
    /// held, one held or in flight, or the next handed out. Runs in flight
    /// keep it down until they land, written or not.
    pub(crate) fn floor(&self) -> u64 {
        let (next, open) = {
            let open = self.open.lock();
            (self.last.load(Ordering::Relaxed) + 1, open.first().copied())
        };
        let held = {
            let pending = self.pending.lock();
            let flight = self.flight.lock();
            pending.iter().chain(flight.taken()).map(|ran| ran.id).min()
        };
        [open, held].into_iter().flatten().fold(next, u64::min)
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
            input: input.filter(|_| failed).map(Arc::new),
            tape: tape.filter(|_| failed).map(|tape| Vec::from(tape).into()),
            nested: invoked.map_or_else(|| Arc::from([]), |invoked| invoked.nested.clone()),
        });
    }

    /// Hold `nested`, a call run `call` made past its first.
    pub(crate) fn nested(&self, call: u64, nested: NestedCall) {
        self.nested.lock().entry(call).or_default().push(nested);
    }

    /// The calls run `call` made past its first, taken out.
    pub(crate) fn take_nested(&self, call: u64) -> Arc<[NestedCall]> {
        self.nested.lock().remove(&call).unwrap_or_default().into()
    }

    /// Take what is pending, held in flight until [`Pending::land`].
    pub(crate) fn take(&self) -> Taken<Ran> {
        let mut pending = self.pending.lock();
        self.flight.lock().take(mem::take(&mut *pending))
    }

    /// Drop what generation `generation` took.
    pub(crate) fn land(&self, generation: u64) {
        let _pending = self.pending.lock();
        self.flight.lock().land(generation);
    }

    /// The runs of the service keyed `service` not yet written.
    pub(crate) fn unsaved(&self, service: &str) -> Unflushed<Ran> {
        self.unsaved_of(|ran| ran.service == service)
    }

    /// The runs of `scope`'s services not yet written.
    pub(crate) fn unsaved_in(&self, scope: &str) -> Unflushed<Ran> {
        self.unsaved_of(|ran| host::split(&ran.service).0 == scope)
    }

    fn unsaved_of(&self, of: impl Fn(&Ran) -> bool) -> Unflushed<Ran> {
        let pending = self.pending.lock();
        let ran = pending.iter().filter(|ran| of(ran)).cloned().collect();
        self.flight.lock().unflushed(of, ran)
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

/// What a run of `tool` of the image `digest` names ran with, `args` read as
/// UTF-8 with invalid bytes replaced.
pub(crate) fn ran_with(
    tool: &str,
    args: Vec<u8>,
    caller: Option<Caller>,
    digest: &str,
) -> CallInput {
    CallInput {
        tool: tool.to_owned(),
        args: String::from_utf8(args)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
        caller,
        digest: digest::prefixed(digest),
    }
}

/// Add `ran`, and drop what is older than [`usage::WINDOW`] days before `now`.
/// Runs of a service this host does not run are dropped.
pub(crate) async fn write(store: &Store, ran: Taken<Ran>, now: u64) -> Result<()> {
    let since = now.saturating_sub(usage::WINDOW as u64 * 86_400_000);
    let taped: BTreeSet<String> = ran
        .rows
        .iter()
        .filter(|ran| ran.tape.is_some())
        .map(|ran| ran.service.clone())
        .collect();
    ran.write(store, |tx, ran| {
        ran.iter().try_for_each(|call| {
            let (scope, name) = host::split(&call.service);
            let meter = call
                .meter
                .as_ref()
                .map(|meter| serde_json::to_string(meter).expect("a meter is JSON"));
            let input = call
                .input
                .as_deref()
                .map(|input| serde_json::to_string(input).expect("an input is JSON"));
            let inserted = tx
                .prepare_cached(
                    "INSERT INTO service_calls
                            (id, scope, name, at, tool, source, caller, outcome, ms, meter,
                             cpu, read, written)
                     SELECT ?10, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?11, ?12, ?13
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
                    call.own.map(|own| own.cpu as i64),
                    call.own.map(|own| own.read as i64),
                    call.own.map(|own| own.written as i64)
                ])?;
            if inserted == 0 {
                return Ok(());
            }
            if input.is_some() || call.tape.is_some() {
                tx.prepare_cached(
                    "INSERT INTO service_replays (scope, name, call, at, input, tape)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )?
                .execute(params![
                    scope,
                    name,
                    call.id as i64,
                    call.at as i64,
                    input,
                    call.tape.as_deref()
                ])?;
                if let Some(tape) = &call.tape {
                    tx.prepare_cached(
                        "UPDATE services SET tape_bytes = tape_bytes + ?3
                          WHERE scope = ?1 AND name = ?2",
                    )?
                    .execute(params![scope, name, tape.len() as i64])?;
                }
            }
            // A service's call of its own cron tool is that tool's last run.
            if call.source == Source::Service && call.caller == call.service {
                tx.prepare_cached(
                    "UPDATE service_wakes
                        SET last_call = ?4, last_at = ?5, last_source = ?6, last_caller = ?7,
                            last_outcome = ?8, last_ms = ?9, last_meter = ?10,
                            last_logs = (SELECT COUNT(*) FROM service_logs
                                          WHERE scope = ?1 AND name = ?2 AND call = ?4)
                      WHERE scope = ?1 AND name = ?2 AND slot = ?3 AND slot <> ''
                        AND (last_call IS NULL OR last_call < ?4)",
                )?
                .execute(params![
                    scope,
                    name,
                    call.tool,
                    call.id as i64,
                    call.at as i64,
                    call.source,
                    call.caller,
                    call.outcome,
                    call.ms as i64,
                    meter
                ])?;
            }
            let mut nested = tx.prepare_cached(
                "INSERT INTO service_nested_calls
                        (scope, name, call, id, parent, service, tool, at, ms, outcome, cpu, read,
                         written)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            )?;
            for made in call.nested.iter() {
                nested.execute(params![
                    scope,
                    name,
                    call.id as i64,
                    made.id,
                    made.parent,
                    made.service,
                    made.tool,
                    made.at as i64,
                    made.ms as i64,
                    made.outcome,
                    made.cpu as i64,
                    made.read as i64,
                    made.written as i64
                ])?;
            }
            Ok(())
        })
    })
    .await?;
    store
        .run(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM service_calls WHERE at < ?1",
                params![since as i64],
            )?;
            tx.execute(
                "DELETE FROM service_nested_calls WHERE at < ?1",
                params![since as i64],
            )?;
            let mut dropped: HashMap<(String, String), i64> = HashMap::new();
            {
                let mut statement = tx.prepare_cached(
                    "DELETE FROM service_replays WHERE at < ?1
                     RETURNING scope, name, COALESCE(LENGTH(tape), 0)",
                )?;
                let mut rows = statement.query(params![since as i64])?;
                while let Some(row) = rows.next()? {
                    *dropped.entry((row.get(0)?, row.get(1)?)).or_default() +=
                        row.get::<_, i64>(2)?;
                }
            }
            for ((scope, name), bytes) in dropped {
                tx.prepare_cached(
                    "UPDATE services SET tape_bytes = tape_bytes - ?3
                      WHERE scope = ?1 AND name = ?2",
                )?
                .execute(params![scope, name, bytes])?;
            }
            tx.execute(
                "UPDATE service_wakes
                    SET last_call = NULL, last_at = NULL, last_source = NULL, last_caller = NULL,
                        last_outcome = NULL, last_ms = NULL, last_meter = NULL, last_logs = NULL
                  WHERE last_at < ?1",
                params![since as i64],
            )?;
            for service in &taped {
                let (scope, name) = host::split(service);
                let over: bool = tx
                    .prepare_cached(
                        "SELECT tape_bytes > ?3 FROM services WHERE scope = ?1 AND name = ?2",
                    )?
                    .query_row(params![scope, name, TAPES_MAX as i64], |row| row.get(0))
                    .optional()?
                    .unwrap_or(false);
                if !over {
                    continue;
                }
                tx.prepare_cached(
                    "UPDATE service_replays SET tape = NULL
                      WHERE rowid IN (
                            SELECT rowid FROM (
                                   SELECT rowid, SUM(LENGTH(tape)) OVER (
                                          ORDER BY call DESC) AS kept
                                     FROM service_replays
                                    WHERE scope = ?1 AND name = ?2 AND tape IS NOT NULL)
                             WHERE kept > ?3)",
                )?
                .execute(params![scope, name, TAPES_MAX as i64])?;
                tx.prepare_cached(
                    "UPDATE services SET tape_bytes = (
                            SELECT COALESCE(SUM(LENGTH(tape)), 0) FROM service_replays
                             WHERE scope = ?1 AND name = ?2)
                      WHERE scope = ?1 AND name = ?2",
                )?
                .execute(params![scope, name])?;
            }
            tx.commit()
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
    unsaved: Unflushed<Ran>,
) -> Result<Option<Option<Vec<u8>>>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            if let Some(ran) = unsaved.resolve(conn)?.into_iter().find(|ran| ran.id == id) {
                return Ok(Some(ran.tape.map(|tape| tape.to_vec())));
            }
            let kept: Option<Option<Vec<u8>>> = conn
                .query_row(
                    "SELECT tape FROM service_replays WHERE scope = ?1 AND name = ?2 AND call = ?3",
                    params![scope, name, id as i64],
                    |row| row.get(0),
                )
                .optional()?;
            match kept {
                Some(tape) => Ok(Some(tape)),
                None => Ok(written(conn, &scope, &name, id)?.then_some(None)),
            }
        })
        .await
}

/// Whether `scope/name`'s run `id` is written.
fn written(conn: &Connection, scope: &str, name: &str, id: u64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM service_calls WHERE scope = ?1 AND name = ?2 AND id = ?3)",
        params![scope, name, id as i64],
        |row| row.get(0),
    )
}

/// The calls `scope/name`'s run `id` made past its first, as written or in
/// `unsaved`, in the order they began: `None` for no such run.
pub(crate) async fn nested(
    store: &Store,
    scope: &str,
    name: &str,
    id: u64,
    unsaved: Unflushed<Ran>,
) -> Result<Option<Vec<NestedCall>>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            if let Some(ran) = unsaved.resolve(conn)?.into_iter().find(|ran| ran.id == id) {
                return Ok(Some(ran.nested.to_vec()));
            }
            if !written(conn, &scope, &name, id)? {
                return Ok(None);
            }
            made(conn, &scope, &name, id).map(Some)
        })
        .await
}

/// The calls `scope/name`'s written run `id` made past its first, in the
/// order they began.
pub(crate) fn made(
    conn: &Connection,
    scope: &str,
    name: &str,
    id: u64,
) -> rusqlite::Result<Vec<NestedCall>> {
    conn.prepare_cached(
        "SELECT id, parent, service, tool, at, ms, outcome, cpu, read, written
           FROM service_nested_calls
          WHERE scope = ?1 AND name = ?2 AND call = ?3
          ORDER BY id",
    )?
    .query_map(params![scope, name, id as i64], |row| {
        Ok(NestedCall {
            id: row.get(0)?,
            parent: row.get(1)?,
            service: row.get(2)?,
            tool: row.get(3)?,
            at: row.get::<_, i64>(4)? as u64,
            ms: row.get::<_, i64>(5)? as u64,
            outcome: row.get(6)?,
            cpu: row.get::<_, i64>(7)? as u64,
            read: row.get::<_, i64>(8)? as u64,
            written: row.get::<_, i64>(9)? as u64,
        })
    })?
    .collect()
}

/// What `scope/name`'s run `id` ran with, as written or in `unsaved`:
/// `Ok(None)` for a run that kept none, an error for no such run.
pub(crate) async fn input(
    store: &Store,
    scope: &str,
    name: &str,
    id: u64,
    unsaved: Unflushed<Ran>,
) -> Result<Option<Option<CallInput>>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            if let Some(ran) = unsaved.resolve(conn)?.into_iter().find(|ran| ran.id == id) {
                return Ok(Some(ran.input.map(Arc::unwrap_or_clone)));
            }
            let kept: Option<Option<String>> = conn
                .query_row(
                    "SELECT input FROM service_replays WHERE scope = ?1 AND name = ?2 AND call = ?3",
                    params![scope, name, id as i64],
                    |row| row.get(0),
                )
                .optional()?;
            let input = match kept {
                Some(input) => Some(input),
                None => written(conn, &scope, &name, id)?.then_some(None),
            };
            Ok(input.map(|input| input.and_then(|input| serde_json::from_str(&input).ok())))
        })
        .await
}

/// What a page narrows `scope`'s calls to; each `None` names every call.
#[derive(Clone, Copy)]
struct Filters<'a> {
    scope: &'a str,
    name: Option<&'a str>,
    project: Option<&'a str>,
    tool: Option<&'a str>,
    outcome: Option<Outcome>,
    since: Option<u64>,
    until: Option<u64>,
    before: Option<u64>,
    event: Option<&'a str>,
    q: Option<&'a str>,
}

/// SQL naming calls, and what each of its placeholders binds.
struct Matching<'a> {
    sql: String,
    bound: Vec<(&'static str, Box<dyn ToSql + 'a>)>,
}

impl Matching<'_> {
    /// Its placeholders' values, as a statement takes them.
    fn params(&self) -> Vec<(&'static str, &dyn ToSql)> {
        self.bound
            .iter()
            .map(|(placeholder, value)| (*placeholder, value.as_ref() as &dyn ToSql))
            .collect()
    }
}

impl<'a> Filters<'a> {
    /// The calls it names, as `FROM service_calls c WHERE …` holding only the
    /// filters it sets.
    fn matching(self) -> Matching<'a> {
        let mut sql = "FROM service_calls c WHERE c.scope = :scope".to_owned();
        let mut bound: Vec<(&'static str, Box<dyn ToSql + 'a>)> =
            vec![(":scope", Box::new(self.scope))];
        let mut and = |condition: &str, placeholder: &'static str, value: Box<dyn ToSql + 'a>| {
            sql.push_str(" AND ");
            sql.push_str(condition);
            bound.push((placeholder, value));
        };
        if let Some(name) = self.name {
            and("c.name = :name", ":name", Box::new(name));
        }
        if let Some(project) = self.project {
            and(
                "c.name IN (SELECT s.name FROM services s
                             WHERE s.scope = c.scope AND s.project = :project)",
                ":project",
                Box::new(project),
            );
        }
        if let Some(tool) = self.tool {
            and("c.tool = :tool", ":tool", Box::new(tool));
        }
        if let Some(outcome) = self.outcome {
            and("c.outcome = :outcome", ":outcome", Box::new(outcome));
        }
        if let Some(since) = self.since {
            and("c.at >= :since", ":since", Box::new(since as i64));
        }
        if let Some(until) = self.until {
            and("c.at < :until", ":until", Box::new(until as i64));
        }
        if let Some(before) = self.before {
            and("c.id < :before", ":before", Box::new(before as i64));
        }
        if let Some(event) = self.event {
            and(
                "EXISTS (SELECT 1 FROM service_logs l, json_each(l.topics) t
                          WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id
                            AND t.value = :event)",
                ":event",
                Box::new(event),
            );
        }
        if let Some(q) = self.q {
            and(
                "EXISTS (SELECT 1 FROM service_logs l
                          WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id
                            AND (instr(lower(l.data), :q) > 0 OR instr(lower(l.topics), :q) > 0))",
                ":q",
                Box::new(q),
            );
        }
        Matching { sql, bound }
    }
}

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
            let q = page.q.as_deref().map(str::to_ascii_lowercase);
            // The written calls a page can hold: runs not yet written only
            // push them further down.
            let mut matching = Filters {
                scope: &scope,
                name: name.as_deref(),
                project: page.project.as_deref(),
                tool: page.tool.as_deref(),
                outcome: page.outcome,
                since: page.since,
                until: page.until,
                before: page.before,
                event: page.event.as_deref(),
                q: q.as_deref(),
            }
            .matching();
            matching.bound.push((":limit", Box::new(page.limit)));
            let written = conn
                .prepare_cached(&format!(
                    "SELECT c.id {} ORDER BY c.id DESC LIMIT :limit",
                    matching.sql
                ))?
                .query_map(matching.params().as_slice(), |row| row.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let projected = projected(conn, &scope, page.project.as_deref())?;
            let Unwritten { ran, emitted } = unsaved.resolve(conn)?;
            let ran = ran
                .into_iter()
                .filter(|ran| {
                    let ran_name = host::split(&ran.service).1;
                    name.as_ref().is_none_or(|name| ran_name == name)
                        && projected
                            .as_ref()
                            .is_none_or(|names| names.iter().any(|one| one == ran_name))
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
                let matching = Filters {
                    scope: &scope,
                    name,
                    project: page.project.as_deref(),
                    tool,
                    outcome,
                    since: page.since,
                    until: page.until,
                    before: None,
                    event: page.event.as_deref(),
                    q: q.as_deref(),
                }
                .matching();
                conn.prepare_cached(&format!(
                    "SELECT {group}, c.outcome, COUNT(*) {} GROUP BY 1, 2",
                    matching.sql
                ))?
                .query_map(matching.params().as_slice(), |row| {
                    Ok((
                        row.get::<_, rusqlite::types::Value>(0).map(text)?,
                        row.get(1)?,
                        row.get(2)?,
                    ))
                })?
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
            let projected = projected(conn, &scope, page.project.as_deref())?;
            let Unwritten { ran, emitted } = unsaved.resolve(conn)?;
            let ran = ran
                .into_iter()
                .filter(|ran| {
                    let ran_name = host::split(&ran.service).1;
                    projected
                        .as_ref()
                        .is_none_or(|names| names.iter().any(|one| one == ran_name))
                        && page.since.is_none_or(|since| ran.at >= since)
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

/// The names of `scope`'s services deployed in `project`, `None` without one.
fn projected(
    conn: &Connection,
    scope: &str,
    project: Option<&str>,
) -> rusqlite::Result<Option<Vec<String>>> {
    let Some(project) = project else {
        return Ok(None);
    };
    conn.prepare_cached("SELECT name FROM services WHERE scope = ?1 AND project = ?2")?
        .query_map(params![scope, project], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map(Some)
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
    unsaved: &Unwritten,
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

/// The largest call id ever written, deleted ones included, or 0.
pub(crate) async fn newest(store: &Store) -> Result<u64> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT COALESCE(
                        (SELECT seq FROM sqlite_sequence WHERE name = 'service_calls'), 0)",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .await
        .map(|last| last as u64)
}
