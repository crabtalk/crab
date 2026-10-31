//! A service's own SQLite database, for a host serving `berm.sql`.
//!
//! One file per service, under the directory [`Databases`] opens them in. An
//! invocation of a service keeping one, unless a query, holds the database's
//! writer for its whole run inside one transaction, which its service's SQL and
//! `berm.set` write into. A call back into a service the chain
//! already holds runs inside that transaction. A `query` reads the last commit
//! on a pooled read-only connection, or through the writer when its chain
//! holds it.
//! The service's SQL cannot reach the tables named `_berm_*`, where the host
//! keeps the service's `berm.get`/`berm.set` state and the migrations applied.

use crate::{Callsite, Service, database::lock::Graph, readers::Readers, syscall::sql::Statement};
use anyhow::{Context, Result, anyhow, bail};
use berm_api::Sequence;
pub use lock::Held;
pub use query::Answer;
use rusqlite::{Connection, OptionalExtension};
use std::{
    collections::HashMap,
    fmt, fs,
    future::Future,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, atomic::AtomicU64, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{Mutex, oneshot};

mod lock;
mod query;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS _berm_state (key TEXT PRIMARY KEY, value BLOB NOT NULL);
    DROP TABLE IF EXISTS _berm_outbox;
    CREATE TABLE IF NOT EXISTS _berm_migrations (
        kit TEXT NOT NULL DEFAULT '', name TEXT NOT NULL, sql TEXT NOT NULL,
        applied_at INTEGER NOT NULL DEFAULT (unixepoch()),
        PRIMARY KEY (kit, name));";

/// `_berm_migrations` as it was before kits, keyed by name alone, rebuilt
/// keyed by kit and name, its rows the service's own.
const REKEY: &str = "
    ALTER TABLE _berm_migrations RENAME TO _berm_migrations_unkeyed;
    CREATE TABLE _berm_migrations (
        kit TEXT NOT NULL DEFAULT '', name TEXT NOT NULL, sql TEXT NOT NULL,
        applied_at INTEGER NOT NULL DEFAULT (unixepoch()),
        PRIMARY KEY (kit, name));
    INSERT INTO _berm_migrations (kit, name, sql, applied_at)
        SELECT '', name, sql, applied_at FROM _berm_migrations_unkeyed ORDER BY rowid;
    DROP TABLE _berm_migrations_unkeyed;";

/// What [`Callsite::database`] keeps for an invocation: `None` for a service
/// keeping no database.
pub(crate) type Kept = OnceLock<Option<Arc<Database>>>;

/// Work for the writer's thread, on the one connection that writes.
type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Every service's database, opened on first use at `{dir}/{service}.sqlite`.
pub struct Databases {
    dir: PathBuf,
    open: parking_lot::Mutex<HashMap<String, Arc<Database>>>,
    graph: Arc<parking_lot::Mutex<Graph>>,
}

impl Databases {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            open: Default::default(),
            graph: Default::default(),
        }
    }

    /// `service`'s database.
    pub fn get(&self, service: &str) -> Result<Arc<Database>> {
        let mut open = self.open.lock();
        if let Some(database) = open.get(service) {
            return Ok(database.clone());
        }
        let database = Arc::new(Database::open(
            &self.path(service),
            service.into(),
            self.graph.clone(),
        )?);
        open.insert(service.to_owned(), database.clone());
        Ok(database)
    }

    /// Where `service`'s database file is.
    pub fn path(&self, service: &str) -> PathBuf {
        self.dir.join(format!("{service}.sqlite"))
    }

    /// Where `service`'s database's write-ahead log is.
    pub fn wal(&self, service: &str) -> PathBuf {
        self.dir.join(format!("{service}.sqlite-wal"))
    }

    /// Where `service`'s database's shared-memory index is.
    pub fn shm(&self, service: &str) -> PathBuf {
        self.dir.join(format!("{service}.sqlite-shm"))
    }

    /// Make the SQLite file at `seed` `service`'s database, moving it into
    /// place. `seed` is on the same filesystem as the databases. Refused when
    /// `service` has a database, open or on disk.
    pub fn seed(&self, service: &str, seed: &Path) -> Result<()> {
        let open = self.open.lock();
        let path = self.path(service);
        if open.contains_key(service) || path.exists() {
            bail!("{service} has a database, and a seed only starts one");
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        for stale in [self.wal(service), self.shm(service)] {
            remove(&stale)?;
        }
        fs::rename(seed, &path)
            .with_context(|| format!("moving {} to {}", seed.display(), path.display()))
    }

    /// Close `service`'s database and delete its files. Refused while
    /// anything but this holds it open: an invocation's [`Held`] holds it.
    /// Blocks until the writer has run the jobs it was sent.
    pub fn remove(&self, service: &str) -> Result<()> {
        let closing = {
            let mut open = self.open.lock();
            match open.remove(service).map(Arc::try_unwrap) {
                Some(Ok(database)) => Some(database),
                Some(Err(database)) => {
                    open.insert(service.to_owned(), database);
                    bail!("{service}'s database is in use");
                }
                None => None,
            }
        };
        if let Some(database) = closing {
            database.close();
        }
        let path = self.path(service);
        for file in [self.wal(service), self.shm(service), path] {
            remove(&file)?;
        }
        Ok(())
    }

    /// What an invocation of `service` at `at` holds: a savepoint on each
    /// database its chain holds the writer of, and, for a service keeping a
    /// database whose writer the chain does not hold, unless a query, that
    /// writer. Waiting for a writer ends at the chain's deadline, and is
    /// refused when the chain holding it waits, through others or itself, on
    /// one this chain holds.
    pub fn enter(
        &self,
        service: &Service,
        at: &Callsite,
    ) -> impl Future<Output = Result<Held>> + Send + 'static {
        let mut held: Vec<Arc<Database>> = Vec::new();
        let mut taking = Ok(None);
        if !at.query {
            let open = self.open.lock();
            for name in &at.stack[..at.stack.len() - 1] {
                if let Some(database) = open.get(&**name)
                    && database.holds(at.chain)
                    && !held.iter().any(|kept| Arc::ptr_eq(kept, database))
                {
                    held.push(database.clone());
                }
            }
            drop(open);
            if service.manifest.keeps_database() {
                taking = self
                    .get(&service.name)
                    .map(|database| Some(database).filter(|database| !database.holds(at.chain)));
            }
        }
        let (chain, deadline) = (at.chain, at.deadline);
        async move {
            let taking = taking?;
            let mut entered = Held {
                writing: None,
                marked: Vec::with_capacity(held.len()),
            };
            for database in held {
                on(&database.jobs, |conn| {
                    Ok(conn.execute_batch(&format!("SAVEPOINT {}", lock::SAVEPOINT))?)
                })
                .await?;
                entered.marked.push(database);
            }
            if let Some(database) = taking {
                entered.writing = Some(database.write(chain, deadline).await?);
            }
            Ok(entered)
        }
    }
}

impl Callsite {
    /// The database of the service at this site, as `resolve` answers it on
    /// the invocation's first ask and kept for the rest of the invocation. An
    /// `Err` is not kept.
    pub fn database(
        &self,
        resolve: impl FnOnce() -> Option<Result<Arc<Database>>>,
    ) -> Option<Result<Arc<Database>>> {
        if let Some(kept) = self.database.get() {
            return kept.clone().map(Ok);
        }
        let resolved = match resolve() {
            Some(Err(error)) => return Some(Err(error)),
            Some(Ok(database)) => Some(database),
            None => None,
        };
        self.database.get_or_init(|| resolved).clone().map(Ok)
    }
}

/// Delete `path`, when there is one.
fn remove(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => {
            Err(e).with_context(|| format!("deleting {}", path.display()))
        }
        _ => Ok(()),
    }
}

/// One service's database.
pub struct Database {
    readers: Arc<Readers>,
    /// The service it is the database of.
    name: Arc<str>,
    /// The writer's thread, which owns the writing connection.
    jobs: mpsc::Sender<Job>,
    writer: thread::JoinHandle<()>,
    /// Held by the call writing or a migration.
    writing: Arc<Mutex<()>>,
    /// The chain holding the writer, [`lock::FREE`] for none.
    holder: AtomicU64,
    graph: Arc<parking_lot::Mutex<Graph>>,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database")
            .field("name", &self.name)
            .finish()
    }
}

impl Database {
    /// Open the database at `path`, creating it and its directory if needed.
    fn open(path: &Path, name: Arc<str>, graph: Arc<parking_lot::Mutex<Graph>>) -> Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut conn =
            Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        let keyed: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('_berm_migrations') WHERE name = 'kit'",
            [],
            |row| row.get(0),
        )?;
        if !keyed {
            let tx = conn.transaction()?;
            tx.execute_batch(REKEY)?;
            tx.commit()?;
        }
        query::guard(&conn)?;
        let (jobs, queue) = mpsc::channel::<Job>();
        let writer = thread::Builder::new()
            .name("berm-sqlite".to_owned())
            .spawn(move || {
                for job in queue {
                    job(&mut conn);
                }
            })?;
        Ok(Self {
            readers: Arc::new(Readers::new(path.to_owned(), query::guard)),
            name,
            jobs,
            writer,
            writing: Arc::new(Mutex::new(())),
            holder: AtomicU64::new(lock::FREE),
            graph,
        })
    }

    /// Close every connection to the file, once the writer has run the jobs
    /// it was sent.
    fn close(self) {
        let Self {
            readers,
            jobs,
            writer,
            ..
        } = self;
        drop(readers);
        drop(jobs);
        if writer.join().is_err() {
            tracing::error!("the database writer panicked");
        }
    }

    /// Hold the database to `size` bytes: a write past it fails as
    /// `SQLITE_FULL`. Refused when it holds more already, or when `size` is
    /// under one page. Lasts as long as the writer's connection, the life of
    /// this [`Database`]. Answers the bytes it was held to before, which
    /// restore that limit when passed back.
    pub async fn limit(&self, size: u64) -> Result<u64> {
        on(&self.jobs, move |conn| {
            let page: i64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
            let held: i64 = conn.pragma_query_value(None, "page_count", |row| row.get(0))?;
            let before: i64 = conn.pragma_query_value(None, "max_page_count", |row| row.get(0))?;
            let pages = i64::try_from(size)? / page;
            if pages == 0 {
                bail!("{size} bytes is under the database's page of {page}");
            }
            if held > pages {
                bail!("the database holds {} bytes, more than {size}", held * page);
            }
            conn.pragma_update(None, "max_page_count", pages)?;
            Ok(u64::try_from(before * page)?)
        })
        .await
    }

    /// Run the service's statement inside the transaction `chain` holds.
    pub async fn exec(&self, chain: u64, statement: Statement) -> Result<Answer> {
        self.held(chain)?;
        on(&self.jobs, move |conn| query::run(conn, &statement, false)).await
    }

    /// Run the service's statement, refusing one that writes: inside the
    /// transaction `chain` holds, or against the last commit.
    pub async fn read(&self, chain: u64, statement: Statement) -> Result<Answer> {
        if self.holds(chain) {
            return on(&self.jobs, move |conn| query::run(conn, &statement, true)).await;
        }
        self.readers
            .with(move |conn| query::run(conn, &statement, true))
            .await
    }

    /// The service's `key`: inside the transaction `chain` holds, or as last
    /// committed.
    pub async fn get(&self, chain: u64, key: String) -> Result<Option<Vec<u8>>> {
        let state = |conn: &Connection, key: String| -> Result<Option<Vec<u8>>> {
            Ok(conn
                .query_row(
                    "SELECT value FROM _berm_state WHERE key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .optional()?)
        };
        if self.holds(chain) {
            return on(&self.jobs, move |conn| state(conn, key)).await;
        }
        self.readers.with(move |conn| state(conn, key)).await
    }

    /// Set the service's `key` inside the transaction `chain` holds.
    pub async fn set(&self, chain: u64, key: String, value: Vec<u8>) -> Result<()> {
        self.held(chain)?;
        on(&self.jobs, move |conn| {
            conn.execute(
                "INSERT INTO _berm_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                (&key, &value),
            )?;
            Ok(())
        })
        .await
    }

    /// Refuse a write from a chain not holding the writer.
    fn held(&self, chain: u64) -> Result<()> {
        match self.holds(chain) {
            true => Ok(()),
            false => bail!("this chain does not hold {}'s database", self.name),
        }
    }

    /// Apply the migrations not yet applied, in one transaction, answering
    /// their names, a kit's as `{kit}/{name}`. Each sequence is applied under
    /// its kit, `""` the service's own, its migrations kept under a former key
    /// moved under its kit first. Refused, with nothing applied, when a kit's
    /// applied migrations are kept under more than one of its keys, are not
    /// the first of its sequence, unchanged, or when one fails.
    pub async fn migrate(&self, sequences: Vec<Sequence>) -> Result<Vec<String>> {
        self.apply(sequences, true).await
    }

    /// [`Database::migrate`], rolled back: the names of the migrations it
    /// would apply, refused as it would be.
    pub async fn rehearse(&self, sequences: Vec<Sequence>) -> Result<Vec<String>> {
        self.apply(sequences, false).await
    }

    /// Apply `sequences` in one transaction, committed when `commit`.
    async fn apply(&self, sequences: Vec<Sequence>, commit: bool) -> Result<Vec<String>> {
        let _held = self.writing.clone().lock_owned().await;
        on(&self.jobs, move |conn| {
            let tx = conn.transaction()?;
            let mut ran = Vec::new();
            for Sequence {
                kit,
                formerly,
                migrations,
            } in &sequences
            {
                let named = |name: &str| match kit.as_str() {
                    "" => name.to_owned(),
                    kit => format!("{kit}/{name}"),
                };
                let kept = |key: &str| -> Result<bool> {
                    Ok(tx.query_row(
                        "SELECT EXISTS (SELECT 1 FROM _berm_migrations WHERE kit = ?1)",
                        [key],
                        |row| row.get(0),
                    )?)
                };
                for former in formerly {
                    if !kept(former)? {
                        continue;
                    }
                    if kept(kit)? {
                        bail!("kit {kit}'s applied migrations are kept under {kit} and {former}");
                    }
                    tx.execute(
                        "UPDATE _berm_migrations SET kit = ?1 WHERE kit = ?2",
                        (kit, former),
                    )?;
                }
                let applied: Vec<(String, String)> = tx
                    .prepare(
                        "SELECT name, sql FROM _berm_migrations WHERE kit = ?1 ORDER BY rowid",
                    )?
                    .query_map([kit], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<_, _>>()?;
                for (at, (name, sql)) in applied.iter().enumerate() {
                    match migrations.get(at) {
                        Some(next) if next.name == *name && next.sql == *sql => {}
                        Some(next) if next.name == *name => {
                            bail!(
                                "migration {} was applied and has changed since",
                                named(name)
                            )
                        }
                        _ => bail!(
                            "migration {} was applied and is not migration {} any more",
                            named(name),
                            at + 1
                        ),
                    }
                }
                for next in &migrations[applied.len()..] {
                    query::GUEST.set(true);
                    let applying = tx.execute_batch(&next.sql);
                    query::GUEST.set(false);
                    applying.with_context(|| format!("migration {} failed", named(&next.name)))?;
                    tx.execute(
                        "INSERT INTO _berm_migrations (kit, name, sql) VALUES (?1, ?2, ?3)",
                        (kit, &next.name, &next.sql),
                    )?;
                    ran.push(named(&next.name));
                }
            }
            if commit {
                tx.commit()?;
            }
            Ok(ran)
        })
        .await
    }
}

/// Run `job` on the writer's thread.
async fn on<T: Send + 'static>(
    jobs: &mpsc::Sender<Job>,
    job: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
) -> Result<T> {
    let (answer, answered) = oneshot::channel();
    jobs.send(Box::new(move |conn| {
        let _ = answer.send(job(conn));
    }))
    .map_err(|_| anyhow!("the database is closed"))?;
    answered
        .await
        .map_err(|_| anyhow!("the database is closed"))?
}
