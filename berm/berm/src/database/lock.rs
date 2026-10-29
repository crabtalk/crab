//! Which chain holds each database's writer, and what one invocation holds.

use crate::{
    Refused,
    database::{self, Database},
};
use anyhow::{Result, bail};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};
use tokio::sync::OwnedMutexGuard;

/// The savepoint a nested call takes on each database its chain holds.
pub(crate) const SAVEPOINT: &str = "_berm_call";

/// [`Database::holder`] while no chain holds the writer. [`crate::chain`]
/// numbers chains from 1.
pub(crate) const FREE: u64 = 0;

/// Which database each chain is waiting for the writer of. Each
/// [`Database::holder`] changes only under it, so a walk over both sees one
/// state.
#[derive(Default)]
pub(crate) struct Graph {
    waits: HashMap<u64, Arc<Database>>,
}

impl Database {
    /// Whether `chain` holds the writer.
    pub(crate) fn holds(&self, chain: u64) -> bool {
        self.holder.load(Ordering::Acquire) == chain
    }

    /// Take the writer for `chain` and open a transaction, held until the
    /// [`Writing`] is committed or dropped. A dropped one rolls back.
    pub(crate) async fn write(self: &Arc<Self>, chain: u64, deadline: Instant) -> Result<Writing> {
        let waiting = {
            let mut graph = self.graph.lock();
            let mut wanted = self.clone();
            for _ in 0..=graph.waits.len() {
                let holder = wanted.holder.load(Ordering::Acquire);
                if holder == FREE {
                    break;
                }
                if holder == chain {
                    return Err(Refused(format!(
                        "{}'s database is held by a chain waiting on this one",
                        self.name
                    ))
                    .into());
                }
                let Some(next) = graph.waits.get(&holder) else {
                    break;
                };
                wanted = next.clone();
            }
            graph.waits.insert(chain, self.clone());
            Waiting {
                graph: self.graph.clone(),
                chain,
            }
        };
        let taken = tokio::time::timeout_at(
            tokio::time::Instant::from_std(deadline),
            self.writing.clone().lock_owned(),
        )
        .await;
        drop(waiting);
        let Ok(held) = taken else {
            bail!(
                "the call's deadline passed waiting for {}'s database",
                self.name
            );
        };
        {
            let _graph = self.graph.lock();
            self.holder.store(chain, Ordering::Release);
        }
        let writing = Writing {
            database: self.clone(),
            _held: held,
            open: true,
        };
        database::on(
            &self.jobs,
            |conn| Ok(conn.execute_batch("BEGIN IMMEDIATE")?),
        )
        .await?;
        Ok(writing)
    }
}

/// What one invocation holds, from [`Databases::enter`]. Dropped without
/// [`Held::commit`], it rolls back the writer it took and every savepoint.
pub struct Held {
    pub(crate) writing: Option<Writing>,
    /// The databases held before the invocation began, each under a savepoint.
    pub(crate) marked: Vec<Arc<Database>>,
}

impl Held {
    /// The service whose writer the invocation took, if it took one.
    pub fn took(&self) -> Option<&str> {
        self.writing.as_ref().map(|writing| &*writing.database.name)
    }

    /// Commit the writer the invocation took, and release every savepoint.
    pub async fn commit(mut self) -> Result<()> {
        if let Some(writing) = self.writing.take() {
            writing.commit().await?;
        }
        for database in std::mem::take(&mut self.marked) {
            database::on(&database.jobs, |conn| {
                Ok(conn.execute_batch(&format!("RELEASE {SAVEPOINT}"))?)
            })
            .await?;
        }
        Ok(())
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        for database in &self.marked {
            let name = database.name.clone();
            let _ = database.jobs.send(Box::new(move |conn| {
                let rollback = format!("ROLLBACK TO {SAVEPOINT}; RELEASE {SAVEPOINT}");
                if let Err(error) = conn.execute_batch(&rollback) {
                    tracing::warn!(%name, %error, "rolling back to the savepoint failed");
                }
            }));
        }
    }
}

/// A chain's hold on the writer and its open transaction.
pub(crate) struct Writing {
    database: Arc<Database>,
    _held: OwnedMutexGuard<()>,
    open: bool,
}

impl Writing {
    async fn commit(mut self) -> Result<()> {
        database::on(
            &self.database.jobs,
            |conn| Ok(conn.execute_batch("COMMIT")?),
        )
        .await?;
        self.open = false;
        Ok(())
    }
}

impl Drop for Writing {
    fn drop(&mut self) {
        if self.open {
            let name = self.database.name.clone();
            let _ = self.database.jobs.send(Box::new(move |conn| {
                if !conn.is_autocommit()
                    && let Err(error) = conn.execute_batch("ROLLBACK")
                {
                    tracing::warn!(%name, %error, "rolling back the transaction failed");
                }
            }));
        }
        let _graph = self.database.graph.lock();
        self.database.holder.store(FREE, Ordering::Release);
    }
}

/// A chain's place in the waits of the [`Graph`], taken out when dropped.
struct Waiting {
    graph: Arc<parking_lot::Mutex<Graph>>,
    chain: u64,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let waited = self.graph.lock().waits.remove(&self.chain);
        drop(waited);
    }
}
