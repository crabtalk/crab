//! What a chain used, by program.

use crate::Refused;
use anyhow::{Result, bail};
use berm_api::{Meter, host::Memory};
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// One chain's meters, by program, shared by every invocation in it. Filled
/// whether the chain returns, fails or traps.
#[derive(Debug, Default)]
pub struct Meters(Mutex<BTreeMap<Arc<str>, Meter>>);

impl Meters {
    pub(crate) fn add(&self, program: &str, used: Meter) {
        let mut meters = self.0.lock();
        match meters.get_mut(program) {
            Some(meter) => *meter += used,
            None => {
                meters.insert(program.into(), used);
            }
        }
    }

    /// CPU microseconds over every program.
    pub(crate) fn cpu(&self) -> u64 {
        self.0.lock().values().map(|meter| meter.cpu).sum()
    }

    pub fn take(&self) -> BTreeMap<Arc<str>, Meter> {
        std::mem::take(&mut *self.0.lock())
    }
}

/// What a program's invocations hold while they run.
#[derive(Debug, Default)]
pub struct Running {
    calls: AtomicU64,
    memory: AtomicU64,
}

impl Running {
    /// Its invocations running now, and the bytes of linear memory they hold.
    pub fn now(&self) -> (u64, u64) {
        (
            self.calls.load(Ordering::Relaxed),
            self.memory.load(Ordering::Relaxed),
        )
    }
}

/// The memory the programs drawing from it hold at once: their images and
/// compiled code while they are deployed, and the linear memory of their
/// invocations while they run.
#[derive(Debug, Default)]
pub struct Pool {
    /// Bytes; `0` is no limit.
    limit: AtomicU64,
    loaded: AtomicU64,
    running: AtomicU64,
}

impl Pool {
    /// Hold it to `bytes` from the next growth on; `0` is no limit.
    pub fn limit(&self, bytes: u64) {
        self.limit.store(bytes, Ordering::Relaxed);
    }

    /// What it holds now, and its limit.
    pub fn now(&self) -> Memory {
        Memory {
            limit: self.limit.load(Ordering::Relaxed),
            loaded: self.loaded.load(Ordering::Relaxed),
            running: self.running.load(Ordering::Relaxed),
        }
    }

    /// Whether it holds its limit already.
    pub fn full(&self) -> bool {
        let Memory {
            limit,
            loaded,
            running,
        } = self.now();
        limit != 0 && loaded + running >= limit
    }

    pub(crate) fn load(&self, bytes: u64) {
        self.loaded.fetch_add(bytes, Ordering::Relaxed);
    }

    pub(crate) fn unload(&self, bytes: u64) {
        self.loaded.fetch_sub(bytes, Ordering::Relaxed);
    }

    /// Refused when its images, `replaced` bytes of them left out, hold more
    /// than its limit.
    pub(crate) fn fits(&self, replaced: u64) -> Result<()> {
        let Memory { limit, loaded, .. } = self.now();
        let loaded = loaded.saturating_sub(replaced);
        if limit != 0 && loaded > limit {
            bail!("its images would hold {loaded} bytes of memory, past its limit of {limit}");
        }
        Ok(())
    }

    /// Take `bytes` more for an invocation, refused past its limit.
    fn grow(&self, bytes: u64) -> Result<()> {
        let running = self.running.fetch_add(bytes, Ordering::Relaxed) + bytes;
        let held = running + self.loaded.load(Ordering::Relaxed);
        let limit = self.limit.load(Ordering::Relaxed);
        if limit != 0 && held > limit {
            self.running.fetch_sub(bytes, Ordering::Relaxed);
            bail!("growing would hold {held} bytes of memory, past its limit of {limit}");
        }
        Ok(())
    }
}

/// One invocation's part of its program's [`Running`] and of its [`Pool`],
/// from when it is built until it is dropped. Its store's limiter, so the
/// memory it holds is counted as it grows.
#[derive(Debug)]
pub(crate) struct Held {
    running: Arc<Running>,
    pool: Arc<Pool>,
    memory: u64,
}

impl Held {
    pub(crate) fn new(running: Arc<Running>, pool: Arc<Pool>) -> Self {
        running.calls.fetch_add(1, Ordering::Relaxed);
        Self {
            running,
            pool,
            memory: 0,
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.running.calls.fetch_sub(1, Ordering::Relaxed);
        self.running
            .memory
            .fetch_sub(self.memory, Ordering::Relaxed);
        self.pool.running.fetch_sub(self.memory, Ordering::Relaxed);
    }
}

impl wasmtime::ResourceLimiter for Held {
    /// Asked for an instance's first memory as well as each growth. An `Err`
    /// traps the growth, or fails the instantiation.
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if maximum.is_some_and(|maximum| desired > maximum) {
            return Ok(false);
        }
        let grown = (desired - current) as u64;
        self.pool
            .grow(grown)
            .map_err(|e| wasmtime::Error::msg(e.to_string()))?;
        self.memory += grown;
        self.running.memory.fetch_add(grown, Ordering::Relaxed);
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(maximum.is_none_or(|maximum| desired <= maximum))
    }
}

/// What one invocation used itself, the programs it called left out, and its
/// budget of rows read and written.
#[derive(Debug)]
pub(crate) struct Own {
    used: Mutex<Meter>,
    budget: u64,
}

impl Own {
    pub(crate) fn new(budget: u64) -> Self {
        Self {
            used: Mutex::default(),
            budget,
        }
    }

    pub(crate) fn add(&self, used: Meter) {
        *self.used.lock() += used;
    }

    pub(crate) fn take(&self) -> Meter {
        std::mem::take(&mut *self.used.lock())
    }

    /// Refused once the budget is used.
    pub(crate) fn left(&self) -> Result<()> {
        let used = *self.used.lock();
        if used.read + used.written >= self.budget {
            return Err(Refused(format!(
                "this call has used its budget of {} rows",
                self.budget
            ))
            .into());
        }
        Ok(())
    }
}
