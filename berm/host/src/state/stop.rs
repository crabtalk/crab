//! The services this host has stopped: a stopped service takes no calls and
//! fires no wakes; what it stored and what came due stays, and runs once it
//! starts.

use crate::{host, state::store::Store};
use anyhow::Result;
use parking_lot::RwLock;
use rusqlite::params;
use std::collections::BTreeSet;

pub(crate) struct Stops {
    store: Store,
    stopped: RwLock<BTreeSet<String>>,
}

impl Stops {
    /// The services `store` records stopped.
    pub(crate) async fn open(store: Store) -> Result<Self> {
        let stopped = store
            .read(|conn| {
                let mut statement =
                    conn.prepare("SELECT scope, name FROM services WHERE stopped = 1")?;
                let rows = statement.query_map([], |row| {
                    Ok(host::join(
                        &row.get::<_, String>(0)?,
                        &row.get::<_, String>(1)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<BTreeSet<String>>>()
            })
            .await?;
        Ok(Self {
            store,
            stopped: RwLock::new(stopped),
        })
    }

    /// Whether `service` is stopped.
    pub(crate) fn stopped(&self, service: &str) -> bool {
        self.stopped.read().contains(service)
    }

    /// Record `service` stopped or not. `false` when it is not deployed.
    pub(crate) async fn set(&self, service: &str, stopped: bool) -> Result<bool> {
        let (scope, name) = host::split(service);
        let (scope, name) = (scope.to_owned(), name.to_owned());
        let changed = self
            .store
            .run(move |conn| {
                conn.execute(
                    "UPDATE services SET stopped = ?3 WHERE scope = ?1 AND name = ?2",
                    params![scope, name, stopped],
                )
            })
            .await?;
        if changed > 0 {
            self.mark(service, stopped);
        }
        Ok(changed > 0)
    }

    /// Record `service` stopped or not in memory alone, for a write made
    /// elsewhere.
    pub(crate) fn mark(&self, service: &str, stopped: bool) {
        let mut set = self.stopped.write();
        match stopped {
            true => set.insert(service.to_owned()),
            false => set.remove(service),
        };
    }
}
