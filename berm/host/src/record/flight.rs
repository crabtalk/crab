//! What a flush took from a buffer and is writing, under the generation that
//! writes it. A table's `flushes` row names the last generation written to it,
//! in the transaction that wrote it, so a read sees each entry once: in memory
//! until its read transaction sees the generation written, then in the table.

use crate::state::store::Store;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::sync::Arc;

/// A table a flush writes.
#[derive(Clone, Copy)]
pub(crate) enum Table {
    Usage,
    Logs,
    Calls,
    Meters,
}

impl Table {
    /// Its `batch` in `flushes`.
    fn key(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Logs => "logs",
            Self::Calls => "calls",
            Self::Meters => "meters",
        }
    }

    /// The last generation written to it, as `conn` sees it; 0 for none.
    fn flushed(self, conn: &Connection) -> rusqlite::Result<u64> {
        conn.prepare_cached("SELECT generation FROM flushes WHERE batch = ?1")?
            .query_row(params![self.key()], |row| row.get::<_, i64>(0))
            .optional()
            .map(|generation| generation.unwrap_or(0) as u64)
    }

    /// The last generation written to it.
    pub(crate) async fn last(self, store: &Store) -> Result<u64> {
        store.read(move |conn| self.flushed(conn)).await
    }
}

/// What a flush took, and the generation that writes it.
pub(crate) struct Taken<T> {
    table: Table,
    pub generation: u64,
    pub rows: Arc<Vec<T>>,
}

impl<T: Send + Sync + 'static> Taken<T> {
    /// Write its rows with `write` and its generation as written, in one
    /// transaction.
    pub(crate) async fn write(
        self,
        store: &Store,
        write: impl FnOnce(&Transaction<'_>, &[T]) -> rusqlite::Result<()> + Send + 'static,
    ) -> Result<()> {
        store
            .run(move |conn| {
                let tx = conn.transaction()?;
                write(&tx, &self.rows)?;
                tx.prepare_cached(
                    "INSERT INTO flushes (batch, generation) VALUES (?1, ?2)
                     ON CONFLICT (batch) DO UPDATE SET generation = excluded.generation",
                )?
                .execute(params![self.table.key(), self.generation as i64])?;
                tx.commit()
            })
            .await
    }
}

/// A buffer's entries a flush took and has not dropped. Locked after the
/// buffer's pending entries, and changed only with them locked.
pub(crate) struct Flight<T> {
    table: Table,
    /// The generation of the last flush that took from the buffer.
    generation: u64,
    taken: Arc<Vec<T>>,
}

impl<T: Clone> Flight<T> {
    /// Nothing in flight, `flushed` the last generation written to `table`.
    pub(crate) fn new(table: Table, flushed: u64) -> Self {
        Self {
            table,
            generation: flushed,
            taken: Arc::default(),
        }
    }

    /// Hold `rows`, taken under the next generation.
    pub(crate) fn take(&mut self, rows: Vec<T>) -> Taken<T> {
        self.generation += 1;
        self.taken = Arc::new(rows);
        Taken {
            table: self.table,
            generation: self.generation,
            rows: self.taken.clone(),
        }
    }

    /// Drop what generation `generation` took, written or not.
    pub(crate) fn land(&mut self, generation: u64) {
        if self.generation == generation {
            self.taken = Arc::default();
        }
    }

    /// The entries held that `of` takes, then `pending`, the buffer's entries
    /// not taken that `of` takes.
    pub(crate) fn unflushed(&self, of: impl Fn(&T) -> bool, pending: Vec<T>) -> Unflushed<T> {
        Unflushed {
            table: self.table,
            generation: self.generation,
            taken: self.taken.iter().filter(|row| of(row)).cloned().collect(),
            pending,
        }
    }

    /// The entries held.
    pub(crate) fn taken(&self) -> &[T] {
        &self.taken
    }
}

/// A buffer's entries not yet written, as a read took them: those generation
/// `generation` took, and those the next one takes.
#[derive(Clone)]
pub(crate) struct Unflushed<T> {
    table: Table,
    generation: u64,
    taken: Vec<T>,
    pending: Vec<T>,
}

impl<T> Unflushed<T> {
    /// Those `conn`'s read transaction does not see written, in the order
    /// they were held.
    pub(crate) fn resolve(self, conn: &Connection) -> rusqlite::Result<Vec<T>> {
        let flushed = self.table.flushed(conn)?;
        let mut unwritten = Vec::new();
        if flushed < self.generation {
            unwritten.extend(self.taken);
        }
        if flushed <= self.generation {
            unwritten.extend(self.pending);
        }
        Ok(unwritten)
    }
}
