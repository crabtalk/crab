//! The host's own state, in one SQLite file: what it runs, what those
//! services committed, and who used them.

use anyhow::{Context, Result};
use berm::readers::Readers;
use parking_lot::Mutex;
use rusqlite::{Connection, Transaction};
use std::{path::Path, sync::Arc, time::Duration};

/// Each entry moves the schema one version on; `user_version` counts how many
/// have run. Never edit one that has shipped.
const MIGRATIONS: &[&str] = &[
    include_str!("migrations/0001.sql"),
    include_str!("migrations/0002.sql"),
    include_str!("migrations/0003.sql"),
    include_str!("migrations/0004.sql"),
    include_str!("migrations/0005.sql"),
    include_str!("migrations/0006.sql"),
    include_str!("migrations/0007.sql"),
    include_str!("migrations/0008.sql"),
    include_str!("migrations/0009.sql"),
    include_str!("migrations/0010.sql"),
    include_str!("migrations/0011.sql"),
    include_str!("migrations/0012.sql"),
    include_str!("migrations/0013.sql"),
    // `slot` is '' for the wake a service arms, and a scheduled tool's name
    // for that tool's: `every` is its period, `declared` the one the last
    // deploy's config set, and `due` is null while it has none.
    include_str!("migrations/0014.sql"),
    // A cron tool's `cron` and `declared` are cron expressions. A period a
    // five-field expression says turns into it; any other leaves the tool
    // without one, and not called.
    include_str!("migrations/0015.sql"),
    include_str!("migrations/0016.sql"),
    include_str!("migrations/0017.sql"),
    include_str!("migrations/0018.sql"),
    // `log_bytes` is the bytes of `topics` and `data` over the service's
    // `service_logs` rows.
    include_str!("migrations/0019.sql"),
    include_str!("migrations/0020.sql"),
    include_str!("migrations/0021.sql"),
    include_str!("migrations/0022.sql"),
    include_str!("migrations/0023.sql"),
    include_str!("migrations/0024.sql"),
    include_str!("migrations/0025.sql"),
    include_str!("migrations/0026.sql"),
    // `runs` counts the chains a service was the first link of; days metered
    // before it hold 0.
    include_str!("migrations/0027.sql"),
    // A run's calls past its first, on the host it ran on: `call` is the
    // run's id, `id` and `parent` number the calls within it.
    include_str!("migrations/0028.sql"),
    include_str!("migrations/0029.sql"),
    include_str!("migrations/0030.sql"),
    // Meters are kept per tool; `tool` is '' for days metered before, and
    // for the bytes a service stores.
    include_str!("migrations/0031.sql"),
];

/// The connection that writes, shared, and connections that only read.
/// Every statement runs off the async workers.
#[derive(Clone)]
pub struct Store {
    writer: Arc<Mutex<Connection>>,
    readers: Arc<Readers>,
}

/// Statements each connection keeps prepared.
const PREPARED: usize = 64;

/// Rows [`Store::each`] writes in one transaction.
const CHUNK: usize = 512;

impl Store {
    /// Open the state file at `path`, creating it and its directory, and run
    /// what migrations it has not.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut conn =
            Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.set_prepared_statement_cache_capacity(PREPARED);
        let ran: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        for (at, migration) in MIGRATIONS.iter().enumerate().skip(ran as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(migration)
                .with_context(|| format!("migration {}", at + 1))?;
            tx.pragma_update(None, "user_version", at as i64 + 1)?;
            tx.commit()?;
        }
        Ok(Self {
            writer: Arc::new(Mutex::new(conn)),
            readers: Arc::new(Readers::new(path.to_owned(), |conn| {
                conn.set_prepared_statement_cache_capacity(PREPARED);
                Ok(())
            })),
        })
    }

    /// Run `work` against the connection that writes, off the async workers.
    pub async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Result<T> {
        let conn = self.writer.clone();
        Ok(tokio::task::spawn_blocking(move || work(&mut conn.lock())).await??)
    }

    /// Write each of `rows` with `write`, [`CHUNK`] rows to a transaction,
    /// other writes running between them.
    pub(crate) async fn each<R: Send + 'static>(
        &self,
        rows: Vec<R>,
        write: impl Fn(&Transaction<'_>, R) -> rusqlite::Result<()> + Send + Sync + 'static,
    ) -> Result<()> {
        let write = Arc::new(write);
        let mut rows = rows.into_iter().peekable();
        while rows.peek().is_some() {
            let chunk: Vec<R> = rows.by_ref().take(CHUNK).collect();
            let write = write.clone();
            self.run(move |conn| {
                let tx = conn.transaction()?;
                for row in chunk {
                    write(&tx, row)?;
                }
                tx.commit()
            })
            .await?;
        }
        Ok(())
    }

    /// Run `work` against one read-only connection, in one read transaction,
    /// off the async workers. It sees what was committed when it began.
    pub async fn read<T: Send + 'static>(
        &self,
        work: impl FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> Result<T> {
        self.readers
            .with(move |conn| Ok(conn.unchecked_transaction().and_then(|tx| work(&tx))?))
            .await
    }
}
