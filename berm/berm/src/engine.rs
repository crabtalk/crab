//! wasmtime — where a service's bytes become code that runs.
//!
//! Every syscall arrives through one import, `berm.syscall`, carrying the
//! number its name hashes to alongside the request.
//!
//! A guest runs on a fiber: a hosted syscall suspends it, and the thread is
//! free until the reply resolves.

use crate::{
    Budget, Invocation, Meter, Pool,
    bound::{cpu::Clock, watchdog},
    syscall::{self, Door, Table},
    tape::{self, Answer},
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use wasmtime::{
    Caller, Extern, InstanceAllocationStrategy, InstancePre, Linker, Memory, Module, ModuleExport,
    PoolingAllocationConfig, Store, ToWasmtimeResult, Trap, UpdateDeadline,
};

/// The module every syscall is imported from, and the one name under it.
const MODULE: &str = "berm";
const SYSCALL: &str = "syscall";
/// The export a guest's linear memory is reached through.
const MEMORY: &str = "memory";

/// How often the engine's epoch advances, and so how often a running guest's
/// deadline and CPU time are checked.
const TICK: Duration = Duration::from_millis(100);

/// How a runtime compiles and holds code.
#[derive(Clone, Debug)]
pub struct Config {
    /// Where compiled code is cached, if anywhere.
    ///
    /// Entries are keyed on contents: a stale directory yields misses, and it
    /// is safe to share between processes.
    pub cache_dir: Option<PathBuf>,
    /// How many guests may be live at once, across every service and nested
    /// calls included. Slots are reserved when the engine is built; a call
    /// past the last one fails.
    pub instances: u32,
    /// The largest linear memory one guest may grow to, in bytes.
    pub memory: usize,
    /// Bytes the invocations running at once may reserve between them, each
    /// its tool's budget of memory; `0` is no limit.
    pub reserved: u64,
    /// The most CPU time one invocation may spend, nested calls excluded.
    pub cpu: Duration,
    /// The most rows one invocation may read. A row syscall made once its
    /// rows read or its rows written are used is refused; the statement that
    /// crosses either runs to its end.
    pub read: u64,
    /// The most rows one invocation may write.
    pub written: u64,
    /// What one invocation of a tool whose `crab.toml` sets no budget is held
    /// to, key by key, each at most the most above.
    pub budget: Budget,
    /// How long a chain of invocations runs before it is stopped.
    pub deadline: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cache_dir: None,
            instances: 1_000,
            memory: 128 << 20,
            reserved: 0,
            cpu: Duration::from_millis(100),
            read: 1_000_000,
            written: 1_000_000,
            budget: Budget {
                cpu: 100_000,
                read: 1_000_000,
                written: 10_000,
                memory: 128 << 20,
            },
            deadline: watchdog::TIMEOUT,
        }
    }
}

/// A compilation target, shared by every service a runtime holds.
///
/// Cheap to clone; each compiled image keeps its own handle.
#[derive(Clone)]
pub struct Engine {
    wasmtime: wasmtime::Engine,
    /// What its invocations reserve, held to [`Config::reserved`].
    pub reserve: Arc<Pool>,
    /// [`Config::cpu`], [`Config::read`], [`Config::written`] and
    /// [`Config::memory`].
    most: Budget,
    /// [`Config::budget`].
    budget: Budget,
    /// [`Config::deadline`].
    pub(crate) deadline: Duration,
}

impl Engine {
    pub fn new(config: &Config) -> Result<Self> {
        let mut wasmtime = wasmtime::Config::new();
        // Without this the compiled code has no checks to notice a deadline
        // or a spent CPU budget, and a guest that loops forever holds the
        // calling thread forever.
        wasmtime.epoch_interruption(true);
        if let Some(dir) = &config.cache_dir {
            let mut cache = wasmtime::CacheConfig::new();
            cache.with_directory(dir.join("wasm"));
            wasmtime.cache(Some(wasmtime::Cache::new(cache)?));
        }

        // An instance, its memory, its table and the fiber it runs on are
        // taken from slots reserved here. One of each per guest: a guest asking
        // for a second memory or table fails to instantiate.
        let mut pool = PoolingAllocationConfig::new();
        pool.total_core_instances(config.instances)
            .total_memories(config.instances)
            .total_tables(config.instances)
            .total_stacks(config.instances)
            .max_memory_size(config.memory);
        wasmtime.allocation_strategy(InstanceAllocationStrategy::Pooling(pool));
        let engine = wasmtime::Engine::new(&wasmtime)?;
        tick(&engine);
        let reserve = Arc::new(Pool::default());
        reserve.limit(config.reserved);
        let most = Budget {
            cpu: config.cpu.as_micros() as u64,
            read: config.read,
            written: config.written,
            memory: config.memory as u64,
        };
        if let Some(key) = past(&config.budget, &most) {
            bail!("the default budget's {key} is past the most one invocation may use");
        }
        Ok(Self {
            wasmtime: engine,
            reserve,
            most,
            budget: config.budget,
            deadline: config.deadline,
        })
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(&Config::default()).expect("the host target is supported")
    }
}

/// Advance `engine`'s epoch until it is dropped.
///
/// One thread for every guest the engine runs: a deadline is a countdown each
/// store holds, so nothing here has to know which of them are running.
fn tick(engine: &wasmtime::Engine) {
    let engine = engine.weak();
    thread::Builder::new()
        .name("berm-epoch".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(TICK);
                let Some(engine) = engine.upgrade() else {
                    return;
                };
                engine.increment_epoch();
            }
        })
        .expect("spawn the berm epoch ticker");
}

/// One compiled service, ready to instantiate.
pub(crate) struct Image {
    engine: wasmtime::Engine,
    /// [`Engine::most`].
    pub(crate) most: Budget,
    /// [`Config::budget`].
    pub(crate) budget: Budget,
    /// [`Engine::reserve`].
    pub(crate) reserve: Arc<Pool>,
    /// Imports resolved once, at deploy. An image importing something no
    /// syscall is registered for is refused by its deploy.
    pre: InstancePre<Invocation>,
    exports: Vec<(String, ModuleExport)>,
    /// Its [`MEMORY`] export.
    memory: Option<ModuleExport>,
}

impl Image {
    /// Compile `image` against `table`, beside the bytes the image and its
    /// compiled code take.
    pub(crate) fn compile(engine: &Engine, image: &[u8], table: Table) -> Result<(Self, u64)> {
        let (most, budget) = (engine.most, engine.budget);
        let reserve = engine.reserve.clone();
        let engine = engine.wasmtime.clone();
        let module = Module::new(&engine, image)?;

        let mut linker: Linker<Invocation> = Linker::new(&engine);
        let table = Arc::new(table);
        linker.func_wrap_async(
            MODULE,
            SYSCALL,
            move |mut caller: Caller<'_, Invocation>, (number, ptr, len): (u64, u32, u32)| {
                // A syscall the guest was not given traps: the absence is the
                // enforcement.
                let door = table.get(&number).cloned();
                Box::new(async move {
                    let (ptr, len) = (ptr as u64, len as u64);
                    let answer = match door {
                        Some(Door::Builtin(call)) => call(&mut caller, ptr, len),
                        door => match syscall::request(&mut caller, ptr, len) {
                            Ok((at, request)) => {
                                let deck = caller.data().tape.clone();
                                let asked = deck.as_ref().map(|_| tape::asked(&request));
                                let replayed =
                                    deck.as_ref().zip(asked).and_then(|(deck, asked)| {
                                        deck.lock().replay(&at, number, asked)
                                    });
                                match (replayed, door) {
                                    (Some(Err(diverged)), _) => Err(diverged),
                                    (Some(Ok(answer)), door) => {
                                        let through =
                                            deck.is_some_and(|deck| deck.lock().through(number));
                                        if let (true, Some(Door::Hosted(call))) = (through, door) {
                                            let _ = call(at, request).await;
                                        }
                                        Ok(syscall::stage(&mut caller, answer.reply()))
                                    }
                                    (None, Some(Door::Hosted(call))) => {
                                        let reply = call(at, request).await;
                                        if let Some((deck, asked)) = deck.zip(asked) {
                                            deck.lock().record(number, asked, Answer::of(&reply));
                                        }
                                        Ok(syscall::stage(&mut caller, reply))
                                    }
                                    (None, _) => Err(anyhow!("no syscall for {number}")),
                                }
                            }
                            Err(error) => Err(error),
                        },
                    };
                    answer.to_wasmtime_result()
                })
            },
        )?;

        let compiled = module.image_range();
        let bytes = image.len() as u64 + (compiled.end as usize - compiled.start as usize) as u64;
        let image = Self {
            exports: module
                .exports()
                .filter_map(|export| {
                    let name = export.name();
                    Some((name.to_owned(), module.get_export_index(name)?))
                })
                .collect(),
            memory: module.get_export_index(MEMORY),
            pre: linker.instantiate_pre(&module)?,
            engine,
            most,
            budget,
            reserve,
        };
        Ok((image, bytes))
    }

    /// Every name this image exports. Which of them are tools is the ABI's
    /// question.
    pub(crate) fn exports(&self) -> impl Iterator<Item = (&str, ModuleExport)> {
        self.exports
            .iter()
            .map(|(name, export)| (name.as_str(), *export))
    }

    /// Instantiate and run one tool, spending at most its invocation's budget
    /// of CPU, and meter the CPU it spent, the linear memory it reached and its
    /// budget of memory over its wall-clock time against its service, trapped
    /// or not.
    ///
    /// The invocation goes in carrying the arguments and comes back carrying
    /// whatever the tool answered with. Nothing else survives: the instance is
    /// built for this call and dropped with it.
    pub(crate) async fn call(
        &self,
        tool: &ModuleExport,
        invocation: Invocation,
    ) -> Result<Invocation> {
        let deadline = invocation.at.deadline;
        let cpu = invocation.cpu.clone();
        let mut store = Store::new(&self.engine, invocation);
        store.limiter(|invocation| &mut invocation.held);

        // Checked at every tick: a guest past its deadline traps as interrupted,
        // and one past its CPU budget is stopped and marked so.
        let clock = Arc::new(Clock::default());
        let over = Arc::new(AtomicBool::new(false));
        store.set_epoch_deadline(1);
        store.epoch_deadline_callback({
            let (clock, over, cpu) = (clock.clone(), over.clone(), cpu.clone());
            move |_| {
                if Instant::now() >= deadline {
                    return Err(Trap::Interrupt.into());
                }
                if clock.spent() / 1_000 > cpu.load(Ordering::Relaxed) {
                    over.store(true, Ordering::Relaxed);
                    return Err(Trap::Interrupt.into());
                }
                Ok(UpdateDeadline::Continue(1))
            }
        });

        // Instantiating runs the guest's own start-up, which is counted with
        // its call.
        let (instance, called) = clock
            .time(async {
                let instance = self.pre.instantiate_async(&mut store).await;
                let called = match &instance {
                    Ok(instance) => {
                        store.data_mut().memory = self
                            .memory
                            .and_then(|memory| instance.get_module_export(&mut store, &memory))
                            .and_then(Extern::into_memory);
                        match instance
                            .get_module_export(&mut store, tool)
                            .and_then(Extern::into_func)
                            .ok_or_else(|| wasmtime::Error::msg("tool export is not a function"))
                            .and_then(|func| func.typed::<(), ()>(&store))
                        {
                            Ok(tool) => tool.call_async(&mut store, ()).await,
                            Err(error) => Err(error),
                        }
                    }
                    Err(_) => Ok(()),
                };
                (instance, called)
            })
            .await;
        let memory = store
            .data()
            .memory
            .map_or(0, |memory| memory.data_size(&store) as u64);
        store.data().at.meter(Meter {
            cpu: clock.spent() / 1_000,
            memory,
            held: store.data().held.held(),
            ..Meter::default()
        });
        if over.load(Ordering::Relaxed) {
            bail!(
                "ran past its budget of {}µs of CPU",
                cpu.load(Ordering::Relaxed)
            );
        }
        instance?;
        called?;
        Ok(store.into_data())
    }
}

/// Copy `len` bytes of guest memory at `addr`.
pub(crate) fn read(caller: &mut Caller<'_, Invocation>, addr: u64, len: u64) -> Result<Vec<u8>> {
    Ok(slice_mut(caller, addr, len)?.to_vec())
}

/// The `len` bytes of guest memory at `addr`, refused when any lies outside
/// it.
pub(crate) fn slice_mut<'a>(
    caller: &'a mut Caller<'_, Invocation>,
    addr: u64,
    len: u64,
) -> Result<&'a mut [u8]> {
    let memory = memory(caller)?;
    let size = memory.data_size(&*caller) as u64;
    let end = addr
        .checked_add(len)
        .filter(|end| *end <= size)
        .with_context(|| format!("{len} bytes at {addr} lie outside guest memory"))?;
    Ok(&mut memory.data_mut(caller)[addr as usize..end as usize])
}

/// Copy `bytes` into guest memory at `addr`.
pub(crate) fn write(caller: &mut Caller<'_, Invocation>, addr: u64, bytes: &[u8]) -> Result<()> {
    let memory = memory(caller)?;
    Ok(memory.write(&mut *caller, addr as usize, bytes)?)
}

/// The guest's memory, looked up by name only for a syscall made while it
/// starts up, before [`Image::call`] has set [`Invocation::memory`].
fn memory(caller: &mut Caller<'_, Invocation>) -> Result<Memory> {
    if let Some(memory) = caller.data().memory {
        return Ok(memory);
    }
    let memory = caller
        .get_export(MEMORY)
        .and_then(Extern::into_memory)
        .context("service exports no memory")?;
    caller.data_mut().memory = Some(memory);
    Ok(memory)
}

/// The first key of `budget` past `most`, by name.
pub(crate) fn past(budget: &Budget, most: &Budget) -> Option<&'static str> {
    [
        ("cpu", budget.cpu, most.cpu),
        ("read", budget.read, most.read),
        ("written", budget.written, most.written),
        ("memory", budget.memory, most.memory),
    ]
    .into_iter()
    .find_map(|(key, value, most)| (value > most).then_some(key))
}

/// A deploy refused for a tool whose budget sets `key` past the most one
/// invocation may use.
#[derive(Debug)]
pub struct PastMost {
    pub tool: String,
    pub key: &'static str,
}

impl std::fmt::Display for PastMost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tool {:?}'s budget sets {} past this host's most",
            self.tool, self.key
        )
    }
}

impl std::error::Error for PastMost {}
