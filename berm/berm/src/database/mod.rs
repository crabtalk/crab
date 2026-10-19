//! A program's own SQLite database, for a host serving `berm.sql`.
//!
//! One file per program, under the directory [`Databases`] opens them in. An
//! invocation of a program keeping one, unless a query, holds the database's
//! writer for its whole run inside one transaction, which its program's SQL and
//! `berm.set` write into. A call back into a program the chain
//! already holds runs inside that transaction. A `query` reads the last commit
//! on a pooled read-only connection, or through the writer when its chain
//! holds it.
//! The program's SQL cannot reach the tables named `_berm_*`, where the host
//! keeps the program's `berm.get`/`berm.set` state and the migrations applied.

use crate::{Callsite, Program, database::lock::Graph, readers::Readers, syscall::sql::Statement};
use anyhow::{Context, Result, anyhow, bail};
use berm_api::Migration;
pub use lock::Held;
pub use query::Answer;
use rusqlite::{Connection, OptionalExtension};
use std::{
    collections::HashMap,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
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
        name TEXT PRIMARY KEY, sql TEXT NOT NULL,
        applied_at INTEGER NOT NULL DEFAULT (unixepoch()));";

/// Work for the writer's thread, on the one connection that writes.
type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Every program's database, opened on first use at `{dir}/{program}.sqlite`.
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

    /// `program`'s database.
    pub fn get(&self, program: &str) -> Result<Arc<Database>> {
        let mut open = self.open.lock();
        if let Some(database) = open.get(program) {
            return Ok(database.clone());
        }
        let database = Arc::new(Database::open(
            &self.dir.join(format!("{program}.sqlite")),
            program.into(),
            self.graph.clone(),
        )?);
        open.insert(program.to_owned(), database.clone());
        Ok(database)
    }

    /// What an invocation of `program` at `at` holds: a savepoint on each
    /// database its chain holds the writer of, and, for a program keeping a
    /// database whose writer the chain does not hold, unless a query, that
    /// writer. Waiting for a writer ends at the chain's deadline, and is
    /// refused when the chain holding it waits, through others or itself, on
    /// one this chain holds.
    pub fn enter(
        &self,
        program: &Program,
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
            if program.manifest().database.is_some() {
                taking = self
                    .get(&program.name)
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

/// One program's database.
pub struct Database {
    readers: Arc<Readers>,
    /// The program it is the database of.
    name: Arc<str>,
    /// The writer's thread, which owns the writing connection.
    jobs: mpsc::Sender<Job>,
    /// Held by the call writing or a migration.
    writing: Arc<Mutex<()>>,
    graph: Arc<parking_lot::Mutex<Graph>>,
}

impl Database {
    /// Open the database at `path`, creating it and its directory if needed.
    fn open(path: &Path, name: Arc<str>, graph: Arc<parking_lot::Mutex<Graph>>) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let mut conn =
            Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        query::guard(&conn)?;
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
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
            writing: Arc::new(Mutex::new(())),
            graph,
        })
    }

    /// Run the program's statement inside the transaction `chain` holds.
    pub async fn exec(&self, chain: u64, statement: Statement) -> Result<Answer> {
        self.held(chain)?;
        on(&self.jobs, move |conn| query::run(conn, &statement, false)).await
    }

    /// Run the program's statement, refusing one that writes: inside the
    /// transaction `chain` holds, or against the last commit.
    pub async fn read(&self, chain: u64, statement: Statement) -> Result<Answer> {
        if self.holds(chain) {
            return on(&self.jobs, move |conn| query::run(conn, &statement, true)).await;
        }
        self.readers
            .with(move |conn| query::run(conn, &statement, true))
            .await
    }

    /// The program's `key`: inside the transaction `chain` holds, or as last
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

    /// Set the program's `key` inside the transaction `chain` holds.
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
    /// their names. Refused, with nothing applied, when the applied ones are not
    /// the first of `migrations`, unchanged, or when one fails.
    pub async fn migrate(&self, migrations: Vec<Migration>) -> Result<Vec<String>> {
        let _held = self.writing.clone().lock_owned().await;
        on(&self.jobs, move |conn| {
            let tx = conn.transaction()?;
            let applied: Vec<(String, String)> = tx
                .prepare("SELECT name, sql FROM _berm_migrations ORDER BY rowid")?
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            for (at, (name, sql)) in applied.iter().enumerate() {
                match migrations.get(at) {
                    Some(next) if next.name == *name && next.sql == *sql => {}
                    Some(next) if next.name == *name => {
                        bail!("migration {name} was applied and has changed since")
                    }
                    _ => bail!(
                        "migration {name} was applied and is not migration {} any more",
                        at + 1
                    ),
                }
            }
            let mut ran = Vec::new();
            for next in &migrations[applied.len()..] {
                query::GUEST.set(true);
                let applying = tx.execute_batch(&next.sql);
                query::GUEST.set(false);
                applying.with_context(|| format!("migration {} failed", next.name))?;
                tx.execute(
                    "INSERT INTO _berm_migrations (name, sql) VALUES (?1, ?2)",
                    (&next.name, &next.sql),
                )?;
                ran.push(next.name.clone());
            }
            tx.commit()?;
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
