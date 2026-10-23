//! The `service_wakes` table: the wake each service arms and each cron tool's.

use crate::{
    host,
    record::calls::{self, Unsaved},
    state::store::Store,
    wake::cron,
};
use anyhow::Result;
use berm_api::host::{self as wire, CronJob};
use rusqlite::{OptionalExtension, params};

/// One pending wake.
pub(crate) struct Wake {
    pub scope: String,
    /// The service that armed it.
    pub name: String,
    pub program: String,
    pub tool: String,
    pub args: String,
    pub armed: i64,
    /// For a cron tool, its name.
    pub slot: String,
}

/// Each cron tool of `scope/name` whose expression its owner set since the
/// last deploy's config did: the tool, its expression, and the declared one.
pub(crate) async fn edited(
    store: &Store,
    scope: &str,
    name: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT slot, cron, declared FROM service_wakes
                  WHERE scope = ?1 AND name = ?2 AND slot <> '' AND cron IS NOT declared",
            )?;
            let rows = statement.query_map(params![scope, name], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            rows.collect()
        })
        .await
}

/// Set cron tool `tool`'s expression to `cron`, due at `due`, its next
/// firing, when it changes; `None` stops it. `false` when `scope/name` has no
/// such cron tool.
pub(crate) async fn set_cron(
    store: &Store,
    scope: &str,
    name: &str,
    tool: &str,
    cron: Option<String>,
    due: Option<i64>,
) -> Result<bool> {
    let row = [scope, name, tool].map(str::to_owned);
    store
        .run(move |conn| {
            let [scope, name, tool] = row;
            let set = conn.execute(
                "UPDATE service_wakes
                    SET due = IIF(cron IS ?4, due, ?5), cron = ?4
                  WHERE scope = ?1 AND name = ?2 AND slot = ?3",
                params![scope, name, tool, cron, due],
            )?;
            Ok(set == 1)
        })
        .await
}

/// Hold a wake for `scope/name`, replacing whatever it had pending.
pub(crate) async fn arm(
    store: &Store,
    scope: &str,
    name: &str,
    program: &str,
    tool: &str,
    args: &str,
    due: i64,
) -> Result<()> {
    let row = [scope, name, program, tool, args].map(str::to_owned);
    store
        .run(move |conn| {
            let [scope, name, program, tool, args] = row;
            conn.execute(
                "INSERT INTO service_wakes (scope, name, program, tool, args, due)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (scope, name, slot) DO UPDATE
                    SET program = excluded.program, tool = excluded.tool,
                        args = excluded.args, due = excluded.due,
                        armed = service_wakes.armed + 1, failures = 0",
                params![scope, name, program, tool, args, due],
            )?;
            Ok(())
        })
        .await
}

/// Take up to `limit` wakes due at `now` outside the held scopes and stopped
/// services, pushing each
/// one's `due` to `lease` so the timer does not take it again while it runs.
pub(crate) async fn claim(store: &Store, now: i64, lease: i64, limit: i64) -> Result<Vec<Wake>> {
    store
        .run(move |conn| {
            let mut statement = conn.prepare(
                "UPDATE service_wakes SET due = ?2
                  WHERE rowid IN (SELECT rowid FROM service_wakes
                                   WHERE due <= ?1
                                     AND scope NOT IN (SELECT scope FROM held_scopes)
                                     AND (scope, name) NOT IN
                                         (SELECT scope, name FROM services WHERE stopped = 1)
                                   ORDER BY due LIMIT ?3)
                  RETURNING scope, name, program, tool, args, armed, slot",
            )?;
            let rows = statement.query_map(params![now, lease, limit], |row| {
                Ok(Wake {
                    scope: row.get(0)?,
                    name: row.get(1)?,
                    program: row.get(2)?,
                    tool: row.get(3)?,
                    args: row.get(4)?,
                    armed: row.get(5)?,
                    slot: row.get(6)?,
                })
            })?;
            rows.collect()
        })
        .await
}

/// Settle a fired wake, at `now`. A cron tool's is due at its expression's
/// next firing; a service's is dropped, or run again at `retry` when `failed`,
/// and left as it is when it was armed again while it ran.
pub(crate) async fn settle(
    store: &Store,
    wake: &Wake,
    failed: bool,
    now: i64,
    retry: i64,
) -> Result<()> {
    let (scope, name, slot, armed) = (
        wake.scope.clone(),
        wake.name.clone(),
        wake.slot.clone(),
        wake.armed,
    );
    store
        .run(move |conn| {
            if !slot.is_empty() {
                let expression: Option<String> = conn
                    .query_row(
                        "SELECT cron FROM service_wakes WHERE scope = ?1 AND name = ?2 AND slot = ?3",
                        params![scope, name, slot],
                        |row| row.get(0),
                    )
                    .optional()?
                    .flatten();
                let due = expression.and_then(|expression| cron::next(&expression, now).ok());
                conn.execute(
                    "UPDATE service_wakes
                        SET due = ?4, failures = IIF(?5, failures + 1, 0)
                      WHERE scope = ?1 AND name = ?2 AND slot = ?3",
                    params![scope, name, slot, due, failed],
                )?;
            } else if failed {
                conn.execute(
                    "UPDATE service_wakes SET due = ?4, failures = failures + 1
                      WHERE scope = ?1 AND name = ?2 AND slot = '' AND armed = ?3",
                    params![scope, name, armed, retry],
                )?;
            } else {
                conn.execute(
                    "DELETE FROM service_wakes
                      WHERE scope = ?1 AND name = ?2 AND slot = '' AND armed = ?3",
                    params![scope, name, armed],
                )?;
            }
            Ok(())
        })
        .await
}

/// The cron tools of `scope/name`, by name, each with its last run, written
/// or in `unsaved`.
pub(crate) async fn crons(
    store: &Store,
    scope: &str,
    name: &str,
    unsaved: Unsaved,
) -> Result<Vec<CronJob>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            let caller = host::join(&scope, &name);
            let mut statement = conn.prepare_cached(&format!(
                "SELECT {COLUMNS}, w.slot, w.cron, w.due, w.failures
                   FROM service_wakes w
                   LEFT JOIN service_calls c ON c.id = (
                        SELECT MAX(id) FROM service_calls
                         WHERE scope = w.scope AND name = w.name AND tool = w.slot
                           AND source = 'service' AND caller = ?3)
                  WHERE w.scope = ?1 AND w.name = ?2 AND w.slot <> ''
                  ORDER BY w.slot",
                COLUMNS = calls::COLUMNS
            ))?;
            let rows = statement.query_map(params![scope, name, caller], |row| {
                let last = match row.get::<_, Option<i64>>(0)? {
                    Some(_) => Some(calls::call(row)?),
                    None => None,
                };
                Ok(CronJob {
                    tool: row.get(9)?,
                    cron: row.get(10)?,
                    due: row.get::<_, Option<i64>>(11)?.map(|due| due as u64),
                    failures: row.get::<_, i64>(12)? as u64,
                    last,
                })
            })?;
            let mut crons = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for job in &mut crons {
                job.last = calls::last(
                    conn,
                    &scope,
                    &name,
                    job.last.take(),
                    &unsaved,
                    &job.tool,
                    &caller,
                )?;
            }
            Ok(crons)
        })
        .await
}

/// The wakes armed to call a tool of `scope/program` that have not run,
/// soonest first, each with the last run its armer made of its tool, written
/// or in `unsaved`. A wake that is running is due when its lease ends.
pub(crate) async fn armed(
    store: &Store,
    scope: &str,
    program: &str,
    unsaved: Unsaved,
) -> Result<Vec<wire::Wake>> {
    let (scope, program) = (scope.to_owned(), program.to_owned());
    store
        .read(move |conn| {
            let mut statement = conn.prepare_cached(&format!(
                "SELECT {COLUMNS}, w.tool, w.args, w.due, w.name, w.failures
                   FROM service_wakes w
                   LEFT JOIN service_calls c ON c.id = (
                        SELECT MAX(id) FROM service_calls
                         WHERE scope = w.scope AND name = w.program AND tool = w.tool
                           AND source = 'service'
                           AND caller = IIF(w.scope = '', w.name, w.scope || '/' || w.name))
                  WHERE w.scope = ?1 AND w.program = ?2 AND w.slot = '' AND w.due IS NOT NULL
                  ORDER BY w.due",
                COLUMNS = calls::COLUMNS
            ))?;
            let rows = statement.query_map(params![scope, program], |row| {
                let last = match row.get::<_, Option<i64>>(0)? {
                    Some(_) => Some(calls::call(row)?),
                    None => None,
                };
                Ok(wire::Wake {
                    tool: row.get(9)?,
                    args: row.get(10)?,
                    due: row.get::<_, i64>(11)? as u64,
                    by: row.get(12)?,
                    failures: row.get::<_, i64>(13)? as u64,
                    last,
                })
            })?;
            let mut wakes = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for wake in &mut wakes {
                let armer = host::join(&scope, &wake.by);
                wake.last = calls::last(
                    conn,
                    &scope,
                    &program,
                    wake.last.take(),
                    &unsaved,
                    &wake.tool,
                    &armer,
                )?;
            }
            Ok(wakes)
        })
        .await
}

/// Every pending wake, beside when it is due, soonest first.
#[cfg(feature = "test")]
pub(crate) async fn wakes(store: &Store) -> Result<Vec<(Wake, i64)>> {
    store
        .read(|conn| {
            let mut statement = conn.prepare(
                "SELECT scope, name, program, tool, args, armed, slot, due
                   FROM service_wakes
                  WHERE due IS NOT NULL
                  ORDER BY due",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    Wake {
                        scope: row.get(0)?,
                        name: row.get(1)?,
                        program: row.get(2)?,
                        tool: row.get(3)?,
                        args: row.get(4)?,
                        armed: row.get(5)?,
                        slot: row.get(6)?,
                    },
                    row.get(7)?,
                ))
            })?;
            rows.collect()
        })
        .await
}

/// When the next wake outside the held scopes and stopped services is due, if
/// any is pending.
pub(crate) async fn next(store: &Store) -> Result<Option<i64>> {
    store
        .read(|conn| {
            conn.query_row(
                "SELECT MIN(due) FROM service_wakes
                  WHERE scope NOT IN (SELECT scope FROM held_scopes)
                    AND (scope, name) NOT IN
                        (SELECT scope, name FROM services WHERE stopped = 1)",
                [],
                |row| row.get(0),
            )
        })
        .await
}
