//! What each service's tools used, per day: counted in memory and written in
//! batches to `service_meters` on the host the service runs on, beside the
//! bytes the service stored, sampled once a day under the empty tool.

use crate::{host, record::usage, state::store::Store};
use anyhow::Result;
use berm::{Meter, Now, database::Databases};
use berm_api::host::{Metered, Served, ServedDay};
use parking_lot::Mutex;
use rusqlite::params;
use std::{
    collections::{BTreeMap, HashMap},
    fs, mem,
    ops::AddAssign,
};

/// What one service's tool used on one day.
pub(crate) struct Day {
    pub service: String,
    pub tool: String,
    pub day: i32,
    pub used: Meter,
}

/// What one service used since its first deploy, and on one day.
#[derive(Default)]
pub(crate) struct Totals {
    pub all: Meter,
    pub on: Meter,
}

impl AddAssign for Totals {
    fn add_assign(&mut self, other: Self) {
        self.all += other.all;
        self.on += other.on;
    }
}

/// Meters not yet written, by service, tool and day.
pub(crate) struct Meters {
    pending: Mutex<HashMap<(String, String, i32), Meter>>,
    now: Now,
}

impl Meters {
    pub(crate) fn new(now: Now) -> Self {
        Self {
            pending: Mutex::default(),
            now,
        }
    }

    /// Count `used` against `service`'s `tool` today.
    pub(crate) fn record(&self, service: &str, tool: &str, used: Meter) {
        let day = (usage::seconds(&self.now) / 86_400) as i32;
        *self
            .pending
            .lock()
            .entry((service.to_owned(), tool.to_owned(), day))
            .or_default() += used;
    }

    /// What `service` used that is not yet written, in all and on `day`.
    pub(crate) fn unsaved(&self, service: &str, day: i32) -> Totals {
        let mut totals = Totals::default();
        for ((pending, _, at), used) in self.pending.lock().iter() {
            if pending == service {
                totals.all += *used;
                if *at == day {
                    totals.on += *used;
                }
            }
        }
        totals
    }

    /// What each service's tools used on `since` and the days after that is
    /// not yet written.
    pub(crate) fn days(&self, since: i32) -> Vec<Day> {
        self.pending
            .lock()
            .iter()
            .filter(|((_, _, day), _)| *day >= since)
            .map(|((service, tool, day), used)| Day {
                service: service.clone(),
                tool: tool.clone(),
                day: *day,
                used: *used,
            })
            .collect()
    }

    /// Take what is pending.
    pub(crate) fn take(&self) -> Vec<Day> {
        mem::take(&mut *self.pending.lock())
            .into_iter()
            .map(|((service, tool, day), used)| Day {
                service,
                tool,
                day,
                used,
            })
            .collect()
    }
}

/// Add `metered` to what is recorded. A service no longer deployed is dropped.
pub(crate) async fn write(store: &Store, metered: Vec<Day>) -> Result<()> {
    store
        .each(
            metered,
            |tx,
             Day {
                 service,
                 tool,
                 day,
                 used: meter,
             }| {
                let (scope, name) = host::split(&service);
                tx.prepare_cached(
                    "INSERT INTO service_meters
                        (scope, name, day, calls, cpu, memory, read, written,
                         proxied, logged, failed, sent, runs, held, tool)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15
                  WHERE EXISTS (SELECT 1 FROM services WHERE scope = ?1 AND name = ?2)
                 ON CONFLICT (scope, name, tool, day) DO UPDATE SET
                    calls = calls + excluded.calls,
                    cpu = cpu + excluded.cpu,
                    memory = MAX(memory, excluded.memory),
                    read = read + excluded.read,
                    written = written + excluded.written,
                    proxied = proxied + excluded.proxied,
                    logged = logged + excluded.logged,
                    failed = failed + excluded.failed,
                    sent = sent + excluded.sent,
                    runs = runs + excluded.runs,
                    held = held + excluded.held",
                )?
                .execute(params![
                    scope,
                    name,
                    day,
                    meter.calls as i64,
                    meter.cpu as i64,
                    meter.memory as i64,
                    meter.read as i64,
                    meter.written as i64,
                    meter.proxied as i64,
                    meter.logged as i64,
                    meter.failed as i64,
                    meter.sent as i64,
                    meter.runs as i64,
                    meter.held as i64,
                    tool
                ])?;
                Ok(())
            },
        )
        .await
}

/// Record what `service` stores on `day`, as [`stored`] measures it.
pub(crate) async fn sample(
    store: &Store,
    service: &str,
    databases: &Databases,
    code: u64,
    day: i32,
) -> Result<()> {
    let stored = stored(store, service, databases, code).await?;
    let (scope, name) = host::split(service);
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .run(move |conn| {
            conn.execute(
                "INSERT INTO service_meters (scope, name, tool, day, stored)
                 SELECT ?1, ?2, '', ?3, ?4
                  WHERE EXISTS (SELECT 1 FROM services WHERE scope = ?1 AND name = ?2)
                 ON CONFLICT (scope, name, tool, day) DO UPDATE SET stored = excluded.stored",
                params![scope, name, day, stored as i64],
            )?;
            Ok(())
        })
        .await
}

/// The bytes `service` keeps now: its database file and its write-ahead log,
/// if any, in `databases`, its state, the logs and tapes kept, and `code`, its
/// image and compiled code.
pub(crate) async fn stored(
    store: &Store,
    service: &str,
    databases: &Databases,
    code: u64,
) -> Result<u64> {
    let file: u64 = [databases.path(service), databases.wal(service)]
        .iter()
        .filter_map(|file| fs::metadata(file).ok())
        .map(|metadata| metadata.len())
        .sum();
    let (scope, name) = host::split(service);
    let (scope, name) = (scope.to_owned(), name.to_owned());
    let state: i64 = store
        .read(move |conn| {
            conn.query_row(
                "SELECT (SELECT COALESCE(SUM(LENGTH(key) + LENGTH(value)), 0) FROM service_state
                          WHERE scope = ?1 AND name = ?2)
                      + COALESCE((SELECT log_bytes FROM services
                                   WHERE scope = ?1 AND name = ?2), 0)
                      + (SELECT COALESCE(SUM(LENGTH(tape)), 0) FROM service_calls
                          WHERE scope = ?1 AND name = ?2)",
                params![scope, name],
                |row| row.get(0),
            )
        })
        .await?;
    Ok(file + state as u64 + code)
}

/// `scope/name`'s meters over the [`usage::WINDOW`] days to `today`, as
/// written and in `unsaved`, by day and tool, oldest first.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    today: i32,
    unsaved: Vec<Day>,
) -> Result<Vec<Metered>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    let since = today - usage::WINDOW;
    store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT day, calls, cpu, memory, read, written, proxied, stored,
                        logged, failed, sent, runs, held, tool
                   FROM service_meters
                  WHERE scope = ?1 AND name = ?2 AND day > ?3",
            )?;
            let rows = statement.query_map(params![scope, name, since], |row| {
                Ok(Metered {
                    day: row.get(0)?,
                    tool: row.get(13)?,
                    calls: row.get::<_, i64>(1)? as u64,
                    cpu: row.get::<_, i64>(2)? as u64,
                    memory: row.get::<_, i64>(3)? as u64,
                    read: row.get::<_, i64>(4)? as u64,
                    written: row.get::<_, i64>(5)? as u64,
                    proxied: row.get::<_, i64>(6)? as u64,
                    stored: row.get::<_, i64>(7)? as u64,
                    logged: row.get::<_, i64>(8)? as u64,
                    failed: row.get::<_, i64>(9)? as u64,
                    sent: row.get::<_, i64>(10)? as u64,
                    runs: row.get::<_, i64>(11)? as u64,
                    held: row.get::<_, i64>(12)? as u64,
                })
            })?;
            let mut metered = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for Day {
                tool, day, used, ..
            } in unsaved.into_iter().filter(|unsaved| unsaved.day > since)
            {
                match metered
                    .iter_mut()
                    .find(|row| row.day == day && row.tool == tool)
                {
                    Some(row) => *row += used,
                    None => metered.push(fresh(day, tool, used)),
                }
            }
            metered.sort_by(|a, b| (a.day, &a.tool).cmp(&(b.day, &b.tool)));
            Ok(metered)
        })
        .await
}

/// The runs of each service in `scope`, or only `name` when given, as
/// written and in `unsaved`, by name: per day over the [`usage::WINDOW`] days
/// to `today`, and in total. A service never run is absent.
pub(crate) async fn served(
    store: &Store,
    scope: &str,
    name: Option<&str>,
    today: i32,
    unsaved: Vec<Day>,
) -> Result<BTreeMap<String, Served>> {
    let (scope, name) = (scope.to_owned(), name.map(str::to_owned));
    let since = today - usage::WINDOW;
    store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT name, day, runs, calls, failed
                   FROM service_meters
                  WHERE scope = ?1 AND (?2 IS NULL OR name = ?2)",
            )?;
            let mut days: BTreeMap<String, BTreeMap<i32, Meter>> = BTreeMap::new();
            let mut rows = statement.query(params![scope, name])?;
            while let Some(row) = rows.next()? {
                let (served, day): (String, i32) = (row.get(0)?, row.get(1)?);
                *days.entry(served).or_default().entry(day).or_default() += Meter {
                    runs: row.get::<_, i64>(2)? as u64,
                    calls: row.get::<_, i64>(3)? as u64,
                    failed: row.get::<_, i64>(4)? as u64,
                    ..Meter::default()
                };
            }
            for Day {
                service, day, used, ..
            } in unsaved
            {
                let (of, served) = host::split(&service);
                if of == scope && name.as_deref().is_none_or(|name| name == served) {
                    *days
                        .entry(served.to_owned())
                        .or_default()
                        .entry(day)
                        .or_default() += used;
                }
            }
            Ok(days
                .into_iter()
                .map(|(served, days)| {
                    let runs = days.values().map(|used| used.runs).sum();
                    let days = days
                        .into_iter()
                        .filter(|(day, _)| *day > since)
                        .map(|(day, used)| ServedDay {
                            day,
                            runs: used.runs,
                            calls: used.calls,
                            failed: used.failed,
                        })
                        .collect();
                    (served, Served { days, runs })
                })
                .collect())
        })
        .await
}

/// Every service's meters on `since` and the days after, as written and in
/// `unsaved`, by service, day and tool.
pub(crate) async fn since(
    store: &Store,
    since: i32,
    unsaved: Vec<Day>,
) -> Result<Vec<(String, Metered)>> {
    store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT scope, name, day, calls, cpu, memory, read, written, proxied,
                        stored, logged, failed, sent, runs, held, tool
                   FROM service_meters
                  WHERE day >= ?1",
            )?;
            let rows = statement.query_map(params![since], |row| {
                let (scope, name): (String, String) = (row.get(0)?, row.get(1)?);
                Ok((
                    host::join(&scope, &name),
                    Metered {
                        day: row.get(2)?,
                        tool: row.get(15)?,
                        calls: row.get::<_, i64>(3)? as u64,
                        cpu: row.get::<_, i64>(4)? as u64,
                        memory: row.get::<_, i64>(5)? as u64,
                        read: row.get::<_, i64>(6)? as u64,
                        written: row.get::<_, i64>(7)? as u64,
                        proxied: row.get::<_, i64>(8)? as u64,
                        stored: row.get::<_, i64>(9)? as u64,
                        logged: row.get::<_, i64>(10)? as u64,
                        failed: row.get::<_, i64>(11)? as u64,
                        sent: row.get::<_, i64>(12)? as u64,
                        runs: row.get::<_, i64>(13)? as u64,
                        held: row.get::<_, i64>(14)? as u64,
                    },
                ))
            })?;
            let mut metered = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for Day {
                service,
                tool,
                day,
                used,
            } in unsaved
            {
                match metered
                    .iter_mut()
                    .find(|(of, row)| *of == service && row.day == day && row.tool == tool)
                {
                    Some((_, row)) => *row += used,
                    None => metered.push((service, fresh(day, tool, used))),
                }
            }
            Ok(metered)
        })
        .await
}

/// A day's meter of `tool` of only `used`.
fn fresh(day: i32, tool: String, used: Meter) -> Metered {
    let mut row = Metered {
        day,
        tool,
        ..Metered::default()
    };
    row += used;
    row
}

/// What `scope/name` used as written, since its first deploy and on `day`,
/// counted from the Unix epoch.
pub(crate) async fn used(store: &Store, scope: &str, name: &str, day: i32) -> Result<Totals> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT COALESCE(SUM(cpu), 0), COALESCE(SUM(read), 0),
                        COALESCE(SUM(written), 0), COALESCE(SUM(proxied), 0),
                        COALESCE(SUM(logged), 0),
                        COALESCE(SUM(cpu) FILTER (WHERE day = ?3), 0),
                        COALESCE(SUM(read) FILTER (WHERE day = ?3), 0),
                        COALESCE(SUM(written) FILTER (WHERE day = ?3), 0),
                        COALESCE(SUM(proxied) FILTER (WHERE day = ?3), 0),
                        COALESCE(SUM(logged) FILTER (WHERE day = ?3), 0)
                   FROM service_meters
                  WHERE scope = ?1 AND name = ?2",
                params![scope, name, day],
                |row| {
                    let meter = |i: usize| -> rusqlite::Result<Meter> {
                        Ok(Meter {
                            cpu: row.get::<_, i64>(i)? as u64,
                            read: row.get::<_, i64>(i + 1)? as u64,
                            written: row.get::<_, i64>(i + 2)? as u64,
                            proxied: row.get::<_, i64>(i + 3)? as u64,
                            logged: row.get::<_, i64>(i + 4)? as u64,
                            ..Meter::default()
                        })
                    };
                    Ok(Totals {
                        all: meter(0)?,
                        on: meter(5)?,
                    })
                },
            )
        })
        .await
}
