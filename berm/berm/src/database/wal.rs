use anyhow::{Result, ensure};
use rusqlite::{Connection, ffi};
use std::{
    ffi::{c_char, c_int, c_void},
    fmt,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

#[derive(Clone, Copy)]
pub enum Commit {
    /// The current generation's committed frame count, including this transaction.
    Appended(u32),
    /// Make the appended frames durable before any of them can be reused.
    BeforeCheckpoint,
}

type Callback = dyn Fn(&Path, Commit) -> Result<bool> + Send + Sync;

#[derive(Clone)]
pub struct CommitHook(Arc<Callback>);

impl CommitHook {
    /// The callback runs on the writer. Return false or an error to defer checkpointing.
    pub fn new(callback: impl Fn(&Path, Commit) -> Result<bool> + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }
}

impl fmt::Debug for CommitHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommitHook").finish_non_exhaustive()
    }
}

pub(super) struct Hook {
    database: PathBuf,
    callback: CommitHook,
    threshold: c_int,
    measured: bool,
    calls: u32,
    nanos: u64,
    max_nanos: u64,
    failing: bool,
}

impl Hook {
    pub(super) fn install(
        conn: &Connection,
        database: PathBuf,
        callback: CommitHook,
    ) -> Result<Box<Self>> {
        let threshold = conn.pragma_query_value(None, "wal_autocheckpoint", |row| row.get(0))?;
        let mut hook = Box::new(Self {
            database,
            callback,
            threshold,
            measured: tracing::enabled!(tracing::Level::DEBUG),
            calls: 0,
            nanos: 0,
            max_nanos: 0,
            failing: false,
        });
        // The writer drops the connection before freeing this box.
        unsafe {
            ffi::sqlite3_wal_hook(
                conn.handle(),
                Some(committed),
                (&mut *hook as *mut Self).cast(),
            )
        };
        Ok(hook)
    }

    fn append(&mut self, frames: u32) -> Result<bool> {
        let started = self.measured.then(Instant::now);
        let result = (self.callback.0)(&self.database, Commit::Appended(frames));
        if let Some(started) = started {
            let nanos = started.elapsed().as_nanos() as u64;
            self.calls += 1;
            self.nanos += nanos;
            self.max_nanos = self.max_nanos.max(nanos);
            if self.calls == 1024 {
                self.report();
            }
        }
        result
    }

    fn report(&mut self) {
        if self.calls != 0 {
            tracing::debug!(path = %self.database.display(), calls = self.calls, nanos = self.nanos, max_nanos = self.max_nanos, "service WAL commit callback timing");
            self.calls = 0;
            self.nanos = 0;
            self.max_nanos = 0;
        }
    }
}

impl Drop for Hook {
    fn drop(&mut self) {
        self.report();
    }
}

unsafe extern "C" fn committed(
    data: *mut c_void,
    conn: *mut ffi::sqlite3,
    schema: *const c_char,
    frames: c_int,
) -> c_int {
    // SQLite invokes this synchronously while the writer owns the live connection.
    let hook = unsafe { &mut *data.cast::<Hook>() };
    let result = panic::catch_unwind(AssertUnwindSafe(|| -> Result<()> {
        if hook.append(frames as u32)? && hook.threshold > 0 && frames >= hook.threshold {
            let started = Instant::now();
            if !(hook.callback.0)(&hook.database, Commit::BeforeCheckpoint)? {
                return Ok(());
            }
            let sync_nanos = started.elapsed().as_nanos() as u64;
            let checkpoint = Instant::now();
            let (mut end, mut backfilled) = (0, 0);
            // Installing a WAL hook replaces SQLite's built-in callback.
            let code = unsafe {
                ffi::sqlite3_wal_checkpoint_v2(
                    conn,
                    schema,
                    ffi::SQLITE_CHECKPOINT_PASSIVE,
                    &mut end,
                    &mut backfilled,
                )
            };
            tracing::debug!(path = %hook.database.display(), end, backfilled, nanos = checkpoint.elapsed().as_nanos() as u64, sync_nanos, "writer checkpointed spooled service WAL");
            ensure!(
                code == ffi::SQLITE_OK || code == ffi::SQLITE_BUSY,
                "SQLite checkpoint returned {code}"
            );
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => hook.failing = false,
        Ok(Err(error)) if !hook.failing => {
            hook.failing = true;
            tracing::warn!(path = %hook.database.display(), %error, "service WAL hook deferred checkpointing");
        }
        Err(_) => tracing::error!(path = %hook.database.display(), "service WAL hook panicked"),
        _ => {}
    }
    // SQLite has already committed. Preserve that result and leave the WAL on failure.
    ffi::SQLITE_OK
}

/// Committed and backfilled frames in SQLite’s current WAL generation.
pub fn progress(connection: &Connection) -> Result<(u32, u32)> {
    let (busy, end, backfilled) =
        connection.query_row("PRAGMA wal_checkpoint(NOOP)", [], |row| {
            Ok((
                row.get::<_, i32>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, i32>(2)?,
            ))
        })?;
    ensure!(busy == 0, "SQLite WAL index is busy");
    Ok((end.max(0) as u32, backfilled.max(0) as u32))
}
