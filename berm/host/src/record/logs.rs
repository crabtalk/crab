//! What services emit: each log taken as it is emitted, whatever its call
//! ends as, held in memory and written in batches to `service_logs` beside
//! the call it was emitted in, kept [`usage::WINDOW`] days.

use crate::{
    env::Env,
    host::{self, Inner},
    record::usage,
    state::store::Store,
};
use anyhow::Result;
use berm::{Callsite, Log, Logged, Now, Syscall, syscall::emit};
use berm_api::host as wire;
use parking_lot::Mutex;
use rusqlite::{OptionalExtension, params};
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
    pub(crate) fn new(now: Now) -> Self {
        Self {
            pending: Mutex::default(),
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

    pub(crate) fn take_all(&self) -> Vec<Emitted> {
        mem::take(&mut *self.pending.lock())
    }

    /// The logs of the service keyed `service` not yet written, in the order
    /// they are numbered.
    pub(crate) fn unsaved(&self, service: &str) -> Vec<Emitted> {
        self.pending
            .lock()
            .iter()
            .filter(|log| log.service == service)
            .cloned()
            .collect()
    }

    /// The logs of `scope`'s services not yet written.
    pub(crate) fn unsaved_in(&self, scope: &str) -> Vec<Emitted> {
        self.pending
            .lock()
            .iter()
            .filter(|log| host::split(&log.service).0 == scope)
            .cloned()
            .collect()
    }
}

/// Add `logs`, each numbered after its service's last, and drop what is older
/// than [`usage::WINDOW`] days before `now`. Logs of a service this host does
/// not run are dropped.
pub(crate) async fn write(store: &Store, logs: Vec<Emitted>, now: u64) -> Result<()> {
    let since = now.saturating_sub(usage::WINDOW as u64 * 86_400_000);
    store
        .each(logs, |tx, log| {
            let (scope, name) = host::split(&log.service);
            let topics = serde_json::to_string(&log.topics).unwrap_or_default();
            let bytes = (topics.len() + log.data.len()) as i64;
            let Some(seq) = tx
                .prepare_cached(
                    "UPDATE services SET seq = seq + 1, log_bytes = log_bytes + ?3
                      WHERE scope = ?1 AND name = ?2
                     RETURNING seq",
                )?
                .query_row(params![scope, name, bytes], |row| row.get::<_, i64>(0))
                .optional()?
            else {
                return Ok(());
            };
            tx.prepare_cached(
                "INSERT INTO service_logs (scope, name, seq, call, at, topics, data)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?
            .execute(params![
                scope,
                name,
                seq,
                log.call.map(|call| call as i64),
                log.at as i64,
                topics,
                log.data
            ])?;
            Ok(())
        })
        .await?;
    store
        .run(move |conn| {
            let tx = conn.transaction()?;
            let mut dropped: HashMap<(String, String), i64> = HashMap::new();
            {
                let mut statement = tx.prepare_cached(
                    "DELETE FROM service_logs WHERE at < ?1
                     RETURNING scope, name,
                               LENGTH(CAST(topics AS BLOB)) + LENGTH(CAST(data AS BLOB))",
                )?;
                let mut rows = statement.query(params![since as i64])?;
                while let Some(row) = rows.next()? {
                    *dropped.entry((row.get(0)?, row.get(1)?)).or_default() +=
                        row.get::<_, i64>(2)?;
                }
            }
            for ((scope, name), bytes) in dropped {
                tx.prepare_cached(
                    "UPDATE services SET log_bytes = log_bytes - ?3 WHERE scope = ?1 AND name = ?2",
                )?
                .execute(params![scope, name, bytes])?;
            }
            tx.commit()
        })
        .await
}

/// `scope/name`'s logs on `page`, oldest first, as written and then in
/// `unsaved`, its logs not yet written.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    page: wire::Page,
    unsaved: Vec<Emitted>,
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
                    "SELECT seq, call, at, topics, data {MATCHING} ORDER BY seq LIMIT ?6"
                ))?
                .query_map(
                    params![scope, name, page.after, page.topic, page.call, page.limit],
                    |row| {
                        let topics: String = row.get(3)?;
                        Ok(Logged {
                            seq: row.get::<_, i64>(0)? as u64,
                            call: row.get::<_, Option<i64>>(1)?.map(|call| call as u64),
                            at: row.get::<_, i64>(2)? as u64,
                            topics: serde_json::from_str(&topics).unwrap_or_default(),
                            data: row.get(4)?,
                        })
                    },
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let room = (page.limit as usize).saturating_sub(logs.len());
            logs.extend(unsaved.into_iter().take(room));
            Ok(logs)
        })
        .await
}
