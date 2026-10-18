//! Read-only connections to one SQLite file.

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};
use std::{path::PathBuf, sync::Arc, time::Duration};

/// Connections kept open while unused.
const IDLE: usize = 8;

/// Read-only connections to the file at `path`, at most [`IDLE`] of them kept
/// open while unused.
pub struct Readers {
    path: PathBuf,
    idle: Mutex<Vec<Connection>>,
    /// Run on each connection once, when it is opened.
    setup: fn(&Connection) -> Result<()>,
}

impl Readers {
    pub fn new(path: PathBuf, setup: fn(&Connection) -> Result<()>) -> Self {
        Self {
            path,
            idle: Mutex::default(),
            setup,
        }
    }

    /// Run `work` against one connection, off the async workers.
    pub async fn with<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce(&Connection) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let readers = self.clone();
        tokio::task::spawn_blocking(move || {
            let popped = readers.idle.lock().pop();
            let conn = match popped {
                Some(conn) => conn,
                None => readers.open()?,
            };
            let read = work(&conn);
            let mut idle = readers.idle.lock();
            if idle.len() < IDLE {
                idle.push(conn);
            }
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
