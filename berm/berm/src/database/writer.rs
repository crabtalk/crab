use crate::database::{
    lock, query,
    wal::{CommitHook, Hook},
};
use anyhow::{Context, Result, bail};
use lru::LruCache;
use parking_lot::{Condvar, Mutex};
use rusqlite::{Connection, config::DbConfig};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

const IDLE: usize = 64;
const JOURNAL_SIZE_LIMIT: u32 = 4 * 1024 * 1024;
pub(crate) type Job = Box<dyn FnOnce(Result<&mut Connection>) + Send>;

pub(crate) struct Writers(Mutex<LruCache<PathBuf, Weak<Writer>>>);

impl Writers {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(LruCache::new(
            NonZeroUsize::new(IDLE).unwrap(),
        ))))
    }

    fn retain(&self, writer: &Arc<Writer>) {
        let removed = {
            let mut idle = self.0.lock();
            let removed = if !idle.contains(&writer.path) && idle.len() == IDLE {
                idle.pop_lru()
            } else {
                None
            };
            idle.put(writer.path.clone(), Arc::downgrade(writer));
            removed
        };
        if let Some((_, writer)) = removed
            && let Some(writer) = writer.upgrade()
        {
            writer.close_idle();
        }
    }
}

pub(crate) struct Writer {
    path: PathBuf,
    commit_hook: Option<CommitHook>,
    limit: Arc<AtomicU64>,
    holder: Arc<AtomicU64>,
    cache: Arc<Writers>,
    state: Mutex<State>,
    done: Condvar,
}

struct State {
    connection: Option<WriterConnection>,
    queue: VecDeque<Job>,
    running: bool,
    closed: bool,
}

pub(crate) struct WriterConnection {
    conn: Connection,
    _hook: Option<Box<Hook>>,
}

impl Deref for WriterConnection {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl DerefMut for WriterConnection {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

impl Writer {
    pub(crate) fn new(
        path: PathBuf,
        connection: WriterConnection,
        limit: Arc<AtomicU64>,
        holder: Arc<AtomicU64>,
        cache: Arc<Writers>,
        commit_hook: Option<CommitHook>,
    ) -> Arc<Self> {
        let writer = Arc::new(Self {
            path,
            commit_hook,
            limit,
            holder,
            cache,
            state: Mutex::new(State {
                connection: Some(connection),
                queue: VecDeque::new(),
                running: false,
                closed: false,
            }),
            done: Condvar::new(),
        });
        writer.idle();
        writer
    }

    /// Jobs retain submission order across transaction release.
    pub(crate) fn send(self: &Arc<Self>, job: Job) -> Result<()> {
        let mut state = self.state.lock();
        if state.closed {
            bail!("the database is closed");
        }
        state.queue.push_back(job);
        if !state.running {
            state.running = true;
            let writer = self.clone();
            tokio::task::spawn_blocking(move || writer.drain());
        }
        Ok(())
    }

    fn drain(self: Arc<Self>) {
        let mut connection = self.state.lock().connection.take();
        loop {
            let job = {
                let mut state = self.state.lock();
                match state.queue.pop_front() {
                    Some(job) => job,
                    None => {
                        state.connection = connection;
                        state.running = false;
                        self.done.notify_all();
                        drop(state);
                        self.idle();
                        return;
                    }
                }
            };
            let called = panic::catch_unwind(AssertUnwindSafe(|| {
                if connection.is_none() {
                    match self.open() {
                        Ok(opened) => connection = Some(opened),
                        Err(error) => {
                            job(Err(error));
                            return;
                        }
                    }
                }
                job(Ok(connection.as_mut().expect("connection was opened")));
            }));
            if called.is_err() {
                drop(connection.take());
                let mut state = self.state.lock();
                state.closed = true;
                state.queue.clear();
                state.running = false;
                self.done.notify_all();
                tracing::error!(path=%self.path.display(),"the database writer panicked");
                return;
            }
        }
    }

    fn open(&self) -> Result<WriterConnection> {
        Self::connection(
            &self.path,
            Some(self.limit.load(Ordering::Acquire)),
            self.commit_hook.as_ref(),
        )
    }

    pub(crate) fn connection(
        path: &Path,
        limit: Option<u64>,
        commit_hook: Option<&CommitHook>,
    ) -> Result<WriterConnection> {
        let conn = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
        if commit_hook.is_some() {
            conn.set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)?;
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        if commit_hook.is_some() {
            conn.pragma_update(None, "journal_size_limit", JOURNAL_SIZE_LIMIT)?;
        }
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        if let Some(limit) = limit {
            let page: i64 = conn.pragma_query_value(None, "page_size", |row| row.get(0))?;
            let pages = i64::try_from(limit / page as u64)?;
            if pages == 0 {
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL),
                    None,
                )
                .into());
            }
            conn.pragma_update(None, "max_page_count", pages)?;
        }
        query::guard(&conn, commit_hook.is_some())?;
        let hook = commit_hook
            .map(|hook| Hook::install(&conn, path.to_owned(), hook.clone()))
            .transpose()?;
        Ok(WriterConnection { conn, _hook: hook })
    }

    pub(crate) fn idle(self: &Arc<Self>) {
        let state = self.state.lock();
        let idle = !state.running
            && !state.closed
            && self.holder.load(Ordering::Acquire) == lock::FREE
            && state
                .connection
                .as_ref()
                .is_some_and(|conn| conn.is_autocommit());
        drop(state);
        if idle && self.path != Path::new(":memory:") {
            self.cache.retain(self);
        }
    }

    fn close_idle(&self) {
        let mut state = self.state.lock();
        if !state.running
            && self.holder.load(Ordering::Acquire) == lock::FREE
            && state
                .connection
                .as_ref()
                .is_some_and(|conn| conn.is_autocommit())
        {
            state.connection.take();
        }
    }

    pub(crate) fn close(&self) {
        let mut state = self.state.lock();
        state.closed = true;
        while state.running {
            self.done.wait(&mut state);
        }
        state.connection.take();
    }
}
