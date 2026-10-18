//! Each run of a service this host runs: held in memory and written in batches
//! to `service_calls` on the host the service runs on, kept [`usage::WINDOW`]
//! days.

use crate::{
    host,
    record::{logs::Emitted, usage},
    state::store::Store,
};
use anyhow::Result;
use berm::Now;
use berm_api::{
    Meter,
    host::{Call, CallPage, Outcome, Source},
};
use parking_lot::Mutex;
use rusqlite::{Connection, Row, named_params, params};
use std::{
    cmp::Reverse,
    collections::HashMap,
    mem,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

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
    /// `started`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &self,
        service: &str,
        started: Started,
        tool: &str,
        source: Source,
        caller: &str,
        outcome: Outcome,
        meter: Option<Meter>,
    ) {
        self.pending.lock().push(Ran {
            id: started.id,
            service: service.to_owned(),
            at: started.at,
            tool: tool.to_owned(),
            source,
            caller: caller.to_owned(),
            outcome,
            ms: started.began.elapsed().as_millis() as u64,
            meter,
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

    /// The run as a call that emitted `logs` logs.
    fn call(self, logs: u64) -> Call {
        Call {
            id: self.id,
            at: self.at,
            tool: self.tool,
            source: self.source,
            caller: usage::caller(self.source, self.caller),
            outcome: self.outcome,
            ms: self.ms,
            meter: self.meter,
            logs,
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
            tx.prepare_cached(
                "INSERT INTO service_calls
                        (id, scope, name, at, tool, source, caller, outcome, ms, meter)
                 SELECT ?10, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9
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
                call.id as i64
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
            Ok(())
        })
        .await
}

/// `scope/name`'s runs on `page`, newest first, as written and in `unsaved`,
/// and for a numbered page how many match it in all. A service caller is
/// named by its name.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    page: CallPage,
    unsaved: Unsaved,
) -> Result<(Vec<Call>, Option<i64>)> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            const MATCHING: &str = "FROM service_calls c
                  WHERE c.scope = :scope AND c.name = :name
                    AND (:tool IS NULL OR c.tool = :tool)
                    AND (:outcome IS NULL OR c.outcome = :outcome)
                    AND (:since IS NULL OR c.at >= :since)
                    AND (:until IS NULL OR c.at < :until)
                    AND (:before IS NULL OR c.id < :before)
                    AND (:event IS NULL OR EXISTS (
                        SELECT 1 FROM service_logs l, json_each(l.topics) t
                         WHERE l.scope = c.scope AND l.name = c.name AND l.call = c.id
                           AND t.value = :event))";
            let since = page.since.map(|since| since as i64);
            let until = page.until.map(|until| until as i64);
            let before = page.before.map(|before| before as i64);
            let offset = page
                .page
                .map_or(0, |at| (at - 1).saturating_mul(page.limit));
            let total: Option<i64> = page
                .page
                .map(|_| {
                    conn.prepare_cached(&format!("SELECT COUNT(*) {MATCHING}"))?
                        .query_row(
                            named_params! {
                                ":scope": scope, ":name": name, ":tool": page.tool,
                                ":event": page.event, ":outcome": page.outcome,
                                ":since": since, ":until": until, ":before": before,
                            },
                            |row| row.get(0),
                        )
                })
                .transpose()?;
            // The written calls a page can hold: runs not yet written only
            // push them further down.
            let written = conn
                .prepare_cached(&format!(
                    "SELECT c.id {MATCHING} ORDER BY c.id DESC LIMIT :limit"
                ))?
                .query_map(
                    named_params! {
                        ":scope": scope, ":name": name, ":tool": page.tool,
                        ":event": page.event, ":outcome": page.outcome,
                        ":since": since, ":until": until, ":before": before,
                        ":limit": offset.saturating_add(page.limit),
                    },
                    |row| row.get::<_, i64>(0),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let Unsaved { ran, emitted } = unsaved;
            let ran = ran.into_iter().filter(|ran| ran.on(&page)).collect();
            let unsaved = unwritten(conn, &scope, &name, ran, &emitted, page.event.as_deref())?;
            let mut ids: Vec<(i64, bool)> = written
                .into_iter()
                .map(|id| (id, true))
                .chain(unsaved.iter().map(|call| (call.id as i64, false)))
                .collect();
            ids.sort_unstable_by_key(|(id, _)| Reverse(*id));
            let ids: Vec<(i64, bool)> = ids
                .into_iter()
                .skip(offset as usize)
                .take(page.limit as usize)
                .collect();
            let shown: Vec<i64> = ids
                .iter()
                .filter(|(_, written)| *written)
                .map(|(id, _)| *id)
                .collect();
            let mut found: HashMap<u64, Call> = conn
                .prepare_cached(&format!(
                    "SELECT {COLUMNS} FROM service_calls c
                      WHERE c.id IN (SELECT value FROM json_each(?1))"
                ))?
                .query_map(
                    params![serde_json::to_string(&shown).expect("ids are JSON")],
                    call,
                )?
                .map(|call| call.map(|call| (call.id, call)))
                .collect::<rusqlite::Result<_>>()?;
            let total = total.map(|total| total + unsaved.len() as i64);
            found.extend(unsaved.into_iter().map(|call| (call.id, call)));
            let calls = ids
                .into_iter()
                .filter_map(|(id, _)| found.remove(&(id as u64)))
                .collect();
            Ok((calls, total))
        })
        .await
}

/// `ran`, runs of `scope/name` not yet written, as calls, each with the logs
/// it emitted, written and in `emitted`; only those that emitted one carrying
/// `event` when given.
fn unwritten(
    conn: &Connection,
    scope: &str,
    name: &str,
    ran: Vec<Ran>,
    emitted: &[Emitted],
    event: Option<&str>,
) -> rusqlite::Result<Vec<Call>> {
    if ran.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<u64> = ran.iter().map(|ran| ran.id).collect();
    let written: HashMap<u64, (u64, bool)> = conn
        .prepare_cached(
            "SELECT l.call, COUNT(*),
                    MAX(:event IS NOT NULL AND EXISTS (
                        SELECT 1 FROM json_each(l.topics) t WHERE t.value = :event))
               FROM service_logs l
              WHERE l.scope = :scope AND l.name = :name
                AND l.call IN (SELECT value FROM json_each(:ids))
              GROUP BY l.call",
        )?
        .query_map(
            named_params! {
                ":scope": scope, ":name": name, ":event": event,
                ":ids": serde_json::to_string(&ids).expect("ids are JSON"),
            },
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    (row.get::<_, i64>(1)? as u64, row.get(2)?),
                ))
            },
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ran
        .into_iter()
        .filter_map(|ran| {
            let (mut logs, mut carries) = written.get(&ran.id).copied().unwrap_or_default();
            for log in emitted.iter().filter(|log| log.call == Some(ran.id)) {
                logs += 1;
                carries |= event.is_some_and(|event| log.topics.iter().any(|topic| topic == event));
            }
            (event.is_none() || carries).then(|| ran.call(logs))
        })
        .collect())
}

/// The newer of `written` and the last run of `tool` on `scope/name` in
/// `unsaved` that `caller`, a service key, made.
pub(crate) fn last(
    conn: &Connection,
    scope: &str,
    name: &str,
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
        Some(ran) => {
            Ok(unwritten(conn, scope, name, vec![ran.clone()], &unsaved.emitted, None)?.pop())
        }
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
