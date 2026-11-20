//! Read-only connections to one SQLite file.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

/// Connections kept open while unused.
const IDLE: usize = 8;

/// Read-only connections sharing a bounded idle cache.
pub struct Readers {
    path: PathBuf,
    cache: Arc<Cache>,
    /// Run on each connection once, when it is opened.
    setup: fn(&Connection) -> Result<()>,
}

pub(crate) struct Cache {
    idle: Mutex<VecDeque<(PathBuf, Connection)>>,
    capacity: usize,
}

impl Cache {
    pub(crate) fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            idle: Mutex::default(),
            capacity,
        })
    }

    fn take(&self, path: &Path) -> Option<Connection> {
        let mut idle = self.idle.lock();
        let at = idle.iter().rposition(|(stored, _)| stored == path)?;
        idle.remove(at).map(|(_, conn)| conn)
    }

    fn keep(&self, path: PathBuf, conn: Connection) {
        let mut idle = self.idle.lock();
        if idle.len() == self.capacity {
            idle.pop_front();
        }
        idle.push_back((path, conn));
    }
}

impl Readers {
    pub fn new(path: PathBuf, setup: fn(&Connection) -> Result<()>) -> Self {
        Self::shared(path, setup, Cache::new(IDLE))
    }

    pub(crate) fn shared(
        path: PathBuf,
        setup: fn(&Connection) -> Result<()>,
        cache: Arc<Cache>,
    ) -> Self {
        Self { path, setup, cache }
    }

    pub(crate) fn close(&self) {
        self.cache
            .idle
            .lock()
            .retain(|(path, _)| path != &self.path);
    }

    /// Run `work` against one connection, off the async workers.
    pub async fn with<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce(&Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let readers = self.clone();
        tokio::task::spawn_blocking(move || {
            let popped = readers.cache.take(&readers.path);
            let conn = match popped {
                Some(conn) => conn,
                None => readers.open()?,
            };
            let read = work(&conn);
            readers.cache.keep(readers.path.clone(), conn);
            read
        })
        .await?
    }

    fn open(&self) -> Result<Connection> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening {}", self.path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        (self.setup)(&conn)?;
        Ok(conn)
    }
}
