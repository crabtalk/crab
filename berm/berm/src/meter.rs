//! What a chain used, by service.

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
    time::Instant,
};

/// Meters by service and the tool of it that ran.
pub type ByTool = BTreeMap<(Arc<str>, Arc<str>), Meter>;

/// One chain's meters, shared by every invocation in it. Filled whether the
/// chain returns, fails or traps.
#[derive(Debug, Default)]
pub struct Meters(Mutex<ByTool>);

impl Meters {
    pub(crate) fn add(&self, service: Arc<str>, tool: Arc<str>, used: Meter) {
        *self.0.lock().entry((service, tool)).or_default() += used;
    }

    /// CPU microseconds over every service.
    pub(crate) fn cpu(&self) -> u64 {
        self.0.lock().values().map(|meter| meter.cpu).sum()
    }

    pub fn take(&self) -> ByTool {
        std::mem::take(&mut *self.0.lock())
    }
}

/// What a service's invocations hold while they run.
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

/// The memory the services drawing from it hold at once: their images and
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
            services: BTreeMap::new(),
        }
    }

    /// Whether it holds its limit already.
    pub fn full(&self) -> bool {
        let Memory {
            limit,
            loaded,
            running,
            ..
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
    pub(crate) fn grow(&self, bytes: u64) -> Result<()> {
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

/// One invocation's part of its service's [`Running`] and of its [`Pool`],
/// and its budget of memory reserved from the engine's, from when it is built
/// until it is dropped. Its store's limiter, so the memory it holds is counted
/// as it grows and held to its budget.
#[derive(Debug)]
pub(crate) struct Held {
    running: Arc<Running>,
    pool: Arc<Pool>,
    memory: u64,
    reserve: Arc<Pool>,
    /// Bytes of linear memory it may grow to, reserved from `reserve`.
    budget: u64,
    /// When `budget` was last set.
    since: Instant,
    /// Byte-microseconds of budget held before `since`.
    spent: u128,
}

impl Held {
    /// Refused when `reserve` cannot spare `budget` bytes.
    pub(crate) fn new(
        running: Arc<Running>,
        pool: Arc<Pool>,
        reserve: Arc<Pool>,
        budget: u64,
    ) -> Result<Self> {
        reserve
            .grow(budget)
            .map_err(|_| Refused("the host holds as much memory as it may; retry".to_owned()))?;
        running.calls.fetch_add(1, Ordering::Relaxed);
        Ok(Self {
            running,
            pool,
            memory: 0,
            reserve,
            budget,
            since: Instant::now(),
            spent: 0,
        })
    }

    /// Hold it to `budget` from now on, releasing the rest of its reservation.
    /// Refused past its budget, and below the memory it holds already.
    pub(crate) fn lower(&mut self, budget: u64) -> Result<()> {
        if budget > self.budget {
            bail!(Refused(format!(
                "{budget} bytes of memory is past this call's budget of {}",
                self.budget
            )));
        }
        if budget < self.memory {
            bail!(Refused(format!(
                "this call holds {} bytes of memory already",
                self.memory
            )));
        }
        self.spent = self.held_until_now();
        self.since = Instant::now();
        self.reserve
            .running
            .fetch_sub(self.budget - budget, Ordering::Relaxed);
        self.budget = budget;
        Ok(())
    }

    /// Its budget held over its wall-clock time so far, each budget for as
    /// long as it was set, in MiB-microseconds.
    pub(crate) fn held(&self) -> u64 {
        (self.held_until_now() >> 20).min(u128::from(u64::MAX)) as u64
    }

    fn held_until_now(&self) -> u128 {
        self.spent + u128::from(self.budget) * self.since.elapsed().as_micros()
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.reserve
            .running
            .fetch_sub(self.budget, Ordering::Relaxed);
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
        if desired as u64 > self.budget {
            return Err(wasmtime::Error::msg(format!(
                "growing to {desired} bytes of memory would pass this call's budget of {}",
                self.budget
            )));
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

/// What one invocation used itself, the services it called left out, and its
/// budgets of rows read and rows written.
#[derive(Debug)]
pub(crate) struct Own {
    used: Mutex<Meter>,
    read: AtomicU64,
    written: AtomicU64,
}

impl Own {
    pub(crate) fn new(read: u64, written: u64) -> Self {
        Self {
            used: Mutex::default(),
            read: AtomicU64::new(read),
            written: AtomicU64::new(written),
        }
    }

    /// Its budgets of rows read and rows written.
    pub(crate) fn budget(&self) -> (u64, u64) {
        (
            self.read.load(Ordering::Relaxed),
            self.written.load(Ordering::Relaxed),
        )
    }

    /// Hold it to `read` and `written` from now on, `0` keeping each.
    pub(crate) fn lower(&self, read: u64, written: u64) {
        if read != 0 {
            self.read.fetch_min(read, Ordering::Relaxed);
        }
        if written != 0 {
            self.written.fetch_min(written, Ordering::Relaxed);
        }
    }

    pub(crate) fn add(&self, used: Meter) {
        *self.used.lock() += used;
    }

    pub(crate) fn take(&self) -> Meter {
        std::mem::take(&mut *self.used.lock())
    }

    /// Refused once its rows read or its rows written are used.
    pub(crate) fn left(&self) -> Result<()> {
        let used = *self.used.lock();
        let (read, written) = self.budget();
        if used.read >= read {
            return Err(Refused(format!("this call has read its budget of {read} rows")).into());
        }
        if used.written >= written {
            return Err(Refused(format!(
                "this call has written its budget of {written} rows"
            ))
            .into());
        }
        Ok(())
    }
}
