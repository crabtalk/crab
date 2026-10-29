//! What services emit: each log taken as it is emitted, whatever its call
//! ends as, held in memory and written in batches to `service_logs` beside
//! the call it was emitted in, kept [`usage::WINDOW`] days or until its fleet
//! stores it and the call it was emitted in.

use crate::{
    env::Env,
    host::{self, Inner},
    record::{
        flight::{Flight, Table, Taken, Unflushed},
        usage,
    },
    state::store::Store,
};
use anyhow::Result;
use berm::{Callsite, Log, Logged, Now, Syscall, syscall::emit};
use berm_api::host as wire;
use parking_lot::Mutex;
use rusqlite::{OptionalExtension, Params, Row, Transaction, params};
use std::{collections::HashMap, mem, sync::Weak};

/// `berm.emit`: each log taken into `host`'s pending logs as it is emitted.
pub(crate) fn syscall<E: Env>(host: &Weak<Inner<E>>) -> Syscall {
    let host = host.clone();
    emit::programs(move |at: Callsite, log: Log| {
        let taken = host::alive(&host).map(|host| {
            host.logs.take(&at, log);
            true
        });
        async move { taken }
    })
}

/// Logs not yet written, and the call each running chain is.
pub(crate) struct Pending {
    pending: Mutex<Vec<Emitted>>,
    flight: Mutex<Flight<Emitted>>,
    /// The call each chain started here is, by [`Callsite::chain`].
    runs: Mutex<HashMap<u64, u64>>,
    now: Now,
}

/// A log not yet written.
#[derive(Clone)]
pub(crate) struct Emitted {
    /// The key of the service that emitted it.
    service: String,
    pub call: Option<u64>,
    at: u64,
    pub topics: Vec<String>,
    data: String,
}

impl Emitted {
    /// Whether its data or a topic holds `text`, lowercase, its ASCII letters
    /// in either case.
    pub(crate) fn holds(&self, text: &str) -> bool {
        self.data.to_ascii_lowercase().contains(text)
            || self
                .topics
                .iter()
                .any(|topic| topic.to_ascii_lowercase().contains(text))
    }

    /// The log as it reads once written as number `seq`.
    pub(crate) fn logged(self, seq: u64) -> Logged {
        Logged {
            seq,
            call: self.call,
            at: self.at,
            topics: self.topics,
            data: self.data,
        }
    }
}

impl Pending {
    /// Holding logs after `flushed`, the last generation written.
    pub(crate) fn new(now: Now, flushed: u64) -> Self {
        Self {
            pending: Mutex::default(),
            flight: Mutex::new(Flight::new(Table::Logs, flushed)),
            runs: Mutex::default(),
            now,
        }
    }

    /// Hold that chain `chain` is call `call` until [`Pending::ended`].
    pub(crate) fn started(&self, chain: u64, call: u64) {
        self.runs.lock().insert(chain, call);
    }

    pub(crate) fn ended(&self, chain: u64) {
        self.runs.lock().remove(&chain);
    }

    /// The call chain `chain` is, while it runs.
    pub(crate) fn run(&self, chain: u64) -> Option<u64> {
        self.runs.lock().get(&chain).copied()
    }

    /// Hold `log`, emitted at `at`.
    pub(crate) fn take(&self, at: &Callsite, log: Log) {
        let call = self.run(at.chain);
        self.pending.lock().push(Emitted {
            service: at.program.to_string(),
            call,
            at: (self.now)(),
            topics: log.topics,
            data: log.data,
        });
    }

    /// Take what is pending, held in flight until [`Pending::land`].
    pub(crate) fn take_all(&self) -> Taken<Emitted> {
        let mut pending = self.pending.lock();
        self.flight.lock().take(mem::take(&mut *pending))
    }

    /// Drop what generation `generation` took.
    pub(crate) fn land(&self, generation: u64) {
        let _pending = self.pending.lock();
        self.flight.lock().land(generation);
    }

    /// The logs of the service keyed `service` not yet written, in the order
    /// they are numbered.
    pub(crate) fn unsaved(&self, service: &str) -> Unflushed<Emitted> {
        self.unsaved_of(|log| log.service == service)
    }

    /// The logs of `scope`'s services not yet written.
    pub(crate) fn unsaved_in(&self, scope: &str) -> Unflushed<Emitted> {
        self.unsaved_of(|log| host::split(&log.service).0 == scope)
    }

    fn unsaved_of(&self, of: impl Fn(&Emitted) -> bool) -> Unflushed<Emitted> {
        let pending = self.pending.lock();
        let logs = pending.iter().filter(|log| of(log)).cloned().collect();
        self.flight.lock().unflushed(of, logs)
    }
}

/// Add `logs`, each numbered after its service's last, and drop what is older
/// than [`usage::WINDOW`] days before `now`. Logs of a service this host does
/// not run are dropped.
pub(crate) async fn write(store: &Store, logs: Taken<Emitted>, now: u64) -> Result<()> {
    let since = now.saturating_sub(usage::WINDOW as u64 * 86_400_000);
    logs.write(store, |tx, logs| {
        let logs: Vec<(&Emitted, String)> = logs
            .iter()
            .map(|log| {
                let topics = serde_json::to_string(&log.topics).unwrap_or_default();
                (log, topics)
            })
            .collect();
        let mut counts: HashMap<&str, (i64, i64)> = HashMap::new();
        for (log, topics) in &logs {
            let (count, bytes) = counts.entry(&log.service).or_default();
            *count += 1;
            *bytes += (topics.len() + log.data.len()) as i64;
        }
        // The seq each service's next log in the chunk takes.
        let mut numbered: HashMap<&str, i64> = HashMap::with_capacity(counts.len());
        for (service, (count, bytes)) in counts {
            let (scope, name) = host::split(service);
            let last = tx
                .prepare_cached(
                    "UPDATE services SET seq = seq + ?3, log_bytes = log_bytes + ?4
                          WHERE scope = ?1 AND name = ?2
                         RETURNING seq",
                )?
                .query_row(params![scope, name, count, bytes], |row| {
                    row.get::<_, i64>(0)
                })
                .optional()?;
            if let Some(last) = last {
                numbered.insert(service, last - count + 1);
            }
        }
        let mut insert = tx.prepare_cached(
            "INSERT INTO service_logs (scope, name, seq, call, at, topics, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for (log, topics) in &logs {
            let Some(seq) = numbered.get_mut(log.service.as_str()) else {
                continue;
            };
            let (scope, name) = host::split(&log.service);
            insert.execute(params![
                scope,
                name,
                *seq,
                log.call.map(|call| call as i64),
                log.at as i64,
                topics,
                log.data
            ])?;
            *seq += 1;
        }
        Ok(())
    })
    .await?;
    store
        .run(move |conn| {
            let tx = conn.transaction()?;
            delete(&tx, "at < ?1", params![since as i64])?;
            tx.commit()
        })
        .await
}

/// Delete the logs `matching`, a condition on `service_logs` with `params`
/// bound to its placeholders, and their bytes from each service's
/// `log_bytes`.
pub(crate) fn delete(
    tx: &Transaction,
    matching: &str,
    params: impl Params,
) -> rusqlite::Result<()> {
    let mut dropped: HashMap<(String, String), i64> = HashMap::new();
    {
        let mut statement = tx.prepare_cached(&format!(
            "DELETE FROM service_logs WHERE {matching}
             RETURNING scope, name, LENGTH(CAST(topics AS BLOB)) + LENGTH(CAST(data AS BLOB))"
        ))?;
        let mut rows = statement.query(params)?;
        while let Some(row) = rows.next()? {
            *dropped.entry((row.get(0)?, row.get(1)?)).or_default() += row.get::<_, i64>(2)?;
        }
    }
    for ((scope, name), bytes) in dropped {
        tx.prepare_cached(
            "UPDATE services SET log_bytes = log_bytes - ?3 WHERE scope = ?1 AND name = ?2",
        )?
        .execute(params![scope, name, bytes])?;
    }
    Ok(())
}

/// `scope/name`'s logs on `page`, oldest first, as written and then in
/// `unsaved`, its logs not yet written.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    page: wire::Page,
    unsaved: Unflushed<Emitted>,
) -> Result<Vec<Logged>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            const MATCHING: &str = "FROM service_logs
                  WHERE scope = ?1 AND name = ?2 AND seq > ?3
                    AND (?4 IS NULL OR EXISTS (SELECT 1 FROM json_each(topics) WHERE value = ?4))
                    AND (?5 IS NULL OR call = ?5)";
            let seq: i64 = conn
                .prepare_cached("SELECT seq FROM services WHERE scope = ?1 AND name = ?2")?
                .query_row(params![scope, name], |row| row.get(0))
                .optional()?
                .unwrap_or(0);
            let unsaved: Vec<Logged> = unsaved
                .resolve(conn)?
                .into_iter()
                .zip(seq + 1..)
                .filter(|(log, seq)| {
                    *seq > page.after
                        && page
                            .topic
                            .as_ref()
                            .is_none_or(|topic| log.topics.contains(topic))
                        && page.call.is_none_or(|call| log.call == Some(call as u64))
                })
                .map(|(log, seq)| log.logged(seq as u64))
                .collect();
            let mut logs = conn
                .prepare_cached(&format!(
                    "SELECT {COLUMNS} {MATCHING} ORDER BY seq LIMIT ?6"
                ))?
                .query_map(
                    params![scope, name, page.after, page.topic, page.call, page.limit],
                    logged,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let room = (page.limit as usize).saturating_sub(logs.len());
            logs.extend(unsaved.into_iter().take(room));
            Ok(logs)
        })
        .await
}

/// The columns [`logged`] reads, in order, of `service_logs`.
pub(crate) const COLUMNS: &str = "seq, call, at, topics, data";

/// A row of [`COLUMNS`] as a log.
pub(crate) fn logged(row: &Row) -> rusqlite::Result<Logged> {
    let topics: String = row.get(3)?;
    Ok(Logged {
        seq: row.get::<_, i64>(0)? as u64,
        call: row.get::<_, Option<i64>>(1)?.map(|call| call as u64),
        at: row.get::<_, i64>(2)? as u64,
        topics: serde_json::from_str(&topics).unwrap_or_default(),
        data: row.get(4)?,
    })
}
