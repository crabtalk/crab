//! Who calls each service's tools and takes its events: counted in memory per
//! day and written in batches to `service_usage` on the host the service runs
//! on.

use crate::{host, state::store::Store};
use anyhow::Result;
use berm::Now;
use berm_api::host::{Kind, Outcome, Source, Usage, Used};
use parking_lot::Mutex;
use rusqlite::{OptionalExtension, params};
use std::{collections::HashMap, mem, ops::Range};

/// Days of usage a deploy and `/usage` look back over.
pub(crate) const WINDOW: i32 = 30;

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    service: String,
    kind: Kind,
    item: String,
    source: Source,
    caller: String,
    outcome: Outcome,
    day: i32,
}

/// Counts not yet written: calls and the last call's epoch seconds.
pub(crate) struct Counts {
    pending: Mutex<HashMap<Key, (i64, i64)>>,
    now: Now,
}

impl Counts {
    pub(crate) fn new(now: Now) -> Self {
        Self {
            pending: Mutex::default(),
            now,
        }
    }

    /// Count one use of `item` on the service keyed `service`, ended as
    /// `outcome`.
    pub(crate) fn record(
        &self,
        service: &str,
        kind: Kind,
        item: &str,
        source: Source,
        caller: &str,
        outcome: Outcome,
    ) {
        let now = seconds(&self.now);
        let key = Key {
            service: service.to_owned(),
            kind,
            item: item.to_owned(),
            source,
            caller: caller.to_owned(),
            outcome,
            day: (now / 86_400) as i32,
        };
        let mut pending = self.pending.lock();
        let (calls, last) = pending.entry(key).or_default();
        *calls += 1;
        *last = now;
    }

    /// Take what is pending, as rows.
    pub(crate) fn take(&self) -> Vec<Used> {
        mem::take(&mut *self.pending.lock())
            .into_iter()
            .map(|(key, (calls, last))| key.row(calls, last))
            .collect()
    }

    /// What `service` used that is not yet written, as rows.
    pub(crate) fn unsaved(&self, service: &str) -> Vec<Used> {
        self.pending
            .lock()
            .iter()
            .filter(|(key, _)| key.service == service)
            .map(|(key, (calls, last))| key.clone().row(*calls, *last))
            .collect()
    }
}

impl Key {
    fn row(self, calls: i64, last: i64) -> Used {
        Used {
            service: self.service,
            kind: self.kind,
            item: self.item,
            source: self.source,
            caller: self.caller,
            outcome: self.outcome,
            day: self.day,
            calls,
            last,
        }
    }
}

/// Add `used` to what is recorded. Uses of a service this host does not run
/// are dropped.
pub(crate) async fn write(store: &Store, used: Vec<Used>) -> Result<()> {
    store
        .each(used, |tx, used| {
            let (scope, name) = host::split(&used.service);
            tx.prepare_cached(
                "INSERT INTO service_usage
                        (scope, name, kind, item, source, caller, outcome, day, calls, last)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                  WHERE EXISTS (SELECT 1 FROM services WHERE scope = ?1 AND name = ?2)
                 ON CONFLICT (scope, name, kind, item, source, caller, outcome, day)
                 DO UPDATE SET calls = service_usage.calls + excluded.calls,
                               last = MAX(service_usage.last, excluded.last)",
            )?
            .execute(params![
                scope,
                name,
                used.kind,
                used.item,
                used.source,
                used.caller,
                used.outcome,
                used.day,
                used.calls,
                used.last
            ])?;
            Ok(())
        })
        .await
}

/// The last [`WINDOW`] days through the one `now`, in epoch seconds, falls on,
/// counted from the Unix epoch.
pub(crate) fn window(now: i64) -> Range<i32> {
    let today = now.div_euclid(86_400) as i32;
    today - WINDOW + 1..today + 1
}

/// `scope/name`'s usage on `days`, counted from the Unix epoch, as written and
/// in `unsaved`, one row per item, caller and outcome, of one item and outcome
/// when `of` names them. A service caller is named by its name, an account by
/// its id.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    of: Option<(Kind, &str, Outcome)>,
    days: Range<i32>,
    unsaved: Vec<Used>,
) -> Result<Vec<Usage>> {
    let (kind, item, outcome) = of.map_or((None, None, None), |(kind, item, outcome)| {
        (Some(kind), Some(item.to_owned()), Some(outcome))
    });
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            let mut statement = conn.prepare(
                "SELECT kind, item, source, caller, outcome, SUM(calls), MAX(last)
                   FROM service_usage
                  WHERE scope = ?1 AND name = ?2 AND day >= ?3 AND day < ?7
                    AND (?4 IS NULL OR (kind = ?4 AND item = ?5 AND outcome = ?6))
                  GROUP BY kind, item, source, caller, outcome
                  ORDER BY kind, item, MAX(last) DESC",
            )?;
            let rows = statement.query_map(
                params![scope, name, days.start, kind, item, outcome, days.end],
                |row| {
                    let source = row.get(2)?;
                    Ok(Usage {
                        kind: row.get(0)?,
                        item: row.get(1)?,
                        caller: caller(source, row.get(3)?),
                        source,
                        outcome: row.get(4)?,
                        calls: row.get(5)?,
                        last: row.get(6)?,
                        public: None,
                    })
                },
            )?;
            let mut usage = rows.collect::<rusqlite::Result<Vec<_>>>()?;
            for used in unsaved {
                if !days.contains(&used.day)
                    || kind.is_some_and(|kind| {
                        (kind, item.as_deref(), outcome)
                            != (used.kind, Some(used.item.as_str()), Some(used.outcome))
                    })
                {
                    continue;
                }
                let caller = caller(used.source, used.caller);
                match usage.iter_mut().find(|row| {
                    (&row.kind, &row.item, &row.source, &row.caller, &row.outcome)
                        == (&used.kind, &used.item, &used.source, &caller, &used.outcome)
                }) {
                    Some(row) => {
                        row.calls += used.calls;
                        row.last = row.last.max(used.last);
                    }
                    None => usage.push(Usage {
                        kind: used.kind,
                        item: used.item,
                        source: used.source,
                        caller,
                        outcome: used.outcome,
                        calls: used.calls,
                        last: used.last,
                        public: None,
                    }),
                }
            }
            usage.sort_by(|a, b| {
                (&a.kind, &a.item)
                    .cmp(&(&b.kind, &b.item))
                    .then(b.last.cmp(&a.last))
            });
            Ok(usage)
        })
        .await
}

/// When `tool` was first deployed deprecated, and its sunset if one is set.
pub(crate) async fn deprecation(
    store: &Store,
    scope: &str,
    name: &str,
    tool: &str,
) -> Result<Option<(i64, Option<i64>)>> {
    let row = [scope, name, tool].map(str::to_owned);
    store
        .read(move |conn| {
            let [scope, name, tool] = row;
            conn.query_row(
                "SELECT since, sunset FROM service_deprecations
                  WHERE scope = ?1 AND name = ?2 AND tool = ?3",
                params![scope, name, tool],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
        })
        .await
}

/// Set or clear a deprecated tool's sunset. `false` when the tool is not
/// deprecated.
pub(crate) async fn sunset(
    store: &Store,
    scope: &str,
    name: &str,
    tool: &str,
    at: Option<i64>,
) -> Result<bool> {
    let row = [scope, name, tool].map(str::to_owned);
    store
        .run(move |conn| {
            let [scope, name, tool] = row;
            let set = conn.execute(
                "UPDATE service_deprecations SET sunset = ?4
                  WHERE scope = ?1 AND name = ?2 AND tool = ?3",
                params![scope, name, tool, at],
            )?;
            Ok(set == 1)
        })
        .await
}

/// How a caller from `source` is named: a service by its name, the rest as
/// recorded.
pub(crate) fn caller(source: Source, caller: String) -> String {
    match source {
        Source::Service => host::split(&caller).1.to_owned(),
        _ => caller,
    }
}

/// Seconds since the Unix epoch, as `now` tells it.
pub(crate) fn seconds(now: &Now) -> i64 {
    (now() / 1000) as i64
}
