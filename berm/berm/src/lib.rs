//! berm — the runtime of programs.
//!
//! A program is one hash-pinned WebAssembly image, run under wasmtime. berm
//! compiles it once, holds it by the name it answers to, and instantiates it
//! per invocation; nothing survives the call. A program reaches the programs
//! deployed beside it through `berm.call`, resolved by [`Calls::resolve`], and
//! otherwise only the [`Syscall`]s the embedder passed in, built with
//! [`syscall`]'s constructors; any other import traps. What `berm.set` and
//! `berm.emit` write comes back from [`Program::call`] as [`Changes`] for the
//! host to commit as one, and every invocation runs inside a [`Scope`] the host
//! opens through [`Calls::enter`].
//!
//! ```ignore
//! let calls = Calls::by_name(Calls::DEFAULT_DEPTH);
//! let berm = Berm::new(&engine, calls, vec![]);
//! berm.deploy("example", &wasm)?;
//! let outcome = berm.call(None, "example", "echo", br#"{"query":"hi"}"#.to_vec()).await?;
//! ```

// Named as `::berm::anyhow` by the `hosts` expansion.
pub use anyhow;
use anyhow::Result;
pub use berm_abi as abi;
pub use berm_api::{Budget, Deps, Logged, MAX_LOG, Manifest, Meter, Output, Reach, ToolSpec};
pub use berm_codegen::hosts;
pub use engine::{Config, Engine};
use journal::Journal;
pub use journal::{Changes, Log, Write};
use meter::Own;
pub use meter::{ByTool, Meters, Pool, Running};
pub use program::{Ended, Invocation, Link, Outcome, Program, Refused};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::{Arc, RwLock, Weak, atomic::AtomicU32},
    time::Instant,
};
pub use syscall::call::{
    Budgeted, Calls, Enter, Invoked, Named, Now, Opening, Pooled, Ran, Reaching, Remote, Reported,
    Resolve, Scope,
};

mod bound;
pub mod chain;
pub mod clock;
#[cfg(feature = "sqlite")]
pub mod database;
mod engine;
mod journal;
mod meter;
mod program;
#[cfg(feature = "sqlite")]
pub mod readers;
pub mod syscall;
pub mod tape;
pub mod wire;

/// What a syscall answers with, once it has. An `Err` reaches the guest as a
/// failure message on the same wire as a result.
pub type Reply = Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>>;

/// What a syscall does: request bytes in, result bytes out.
///
/// The guest is suspended until the reply resolves, and its thread is free
/// meanwhile.
pub type Call = Arc<dyn Fn(Callsite, Vec<u8>) -> Reply + Send + Sync>;

/// Which program is asking, on whose behalf, and the chain that reached it.
#[derive(Clone, Debug)]
pub struct Callsite {
    /// What this program was deployed as.
    pub program: Arc<str>,
    /// The tool this invocation runs.
    pub tool: Arc<str>,
    /// What this program's manifest declares it reaches for.
    pub deps: Arc<Deps>,
    /// Who made this call: whoever the embedder named at the chain's first
    /// call, or the program that called this one. `None` when the host
    /// started the chain on the program's own behalf.
    pub caller: Option<Caller>,
    /// The end user the embedder vouched for at the chain's first call, or a
    /// program in the chain vouched for through `berm.vouch`. Carried into
    /// every call made after.
    pub user: Option<User>,
    /// The outside client, as the per-day pseudonymous key the embedder
    /// supplied at the chain's first call. Carried unchanged into every nested
    /// call.
    pub client: Option<Arc<str>>,
    /// How many guests the chain holds, this one included.
    pub depth: u32,
    /// The programs this call sits under, outermost first, this one last.
    pub stack: Arc<[Arc<str>]>,
    /// Whether this invocation is of a `query` tool, or sits under one. A
    /// query is refused `berm.set`, `berm.emit` and `berm.call.after`.
    pub query: bool,
    /// Which chain this is, unique within the process.
    pub chain: u64,
    /// This invocation's number in the chain on this runtime: 0 for the first,
    /// then each in the order it began.
    pub span: u32,
    /// The number of the invocation that called this one.
    pub parent: Option<u32>,
    /// The number the chain's next invocation takes.
    pub(crate) spans: Arc<AtomicU32>,
    /// When the chain has to be done by. A guest suspended in a syscall cannot
    /// notice it until the reply resolves, so a syscall bounds its own wait
    /// inside this.
    pub deadline: Instant,
    /// What the chain has written and emitted so far.
    pub(crate) journal: Arc<Journal>,
    /// What the chain has used so far, by program.
    pub(crate) meters: Arc<Meters>,
    /// What this invocation used itself, its rows against its budget.
    pub(crate) own: Arc<Own>,
    /// Its program's database, once [`Callsite::database`] resolved it.
    #[cfg(feature = "sqlite")]
    pub(crate) database: Arc<database::Kept>,
}

impl Callsite {
    /// Count `used` against this program's tool in the chain, its rows
    /// against this invocation's budget.
    pub fn meter(&self, used: Meter) {
        self.own.add(used);
        self.meters
            .add(self.program.clone(), self.tool.clone(), used);
    }

    /// Refused once this invocation has used its budget of rows.
    pub fn rows_left(&self) -> Result<()> {
        self.own.left()
    }

    /// CPU microseconds the chain has spent so far, every program's metered
    /// here included.
    pub fn spent(&self) -> u64 {
        self.meters.cpu()
    }

    /// Count `used` against `program`'s `tool`, run elsewhere for this chain.
    pub fn meter_of(&self, program: &str, tool: &str, used: Meter) {
        self.meters.add(program.into(), tool.into(), used);
    }
}

/// Who made a call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    /// Named by the embedder at the chain's first call.
    Outside(Arc<str>),
    /// The program that called through `berm.call`, as [`Calls::named`] names
    /// it.
    Program(Arc<str>),
}

/// An end user, as the embedder verified them or a program vouched for them:
/// who vouched for them, and who they are to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub issuer: Arc<str>,
    pub subject: Arc<str>,
}

/// The programs a host is running.
///
/// Held behind an `Arc`; the syscalls berm serves reach back into it through a
/// [`Weak`] of itself.
pub struct Berm {
    engine: Engine,
    /// Every deployed program, by the name it answers to. Never held across an
    /// await.
    programs: RwLock<BTreeMap<String, Arc<Program>>>,
    /// How `berm.call` reaches programs.
    calls: Calls,
    /// What the embedder gives every program, beside what berm serves itself.
    syscalls: Vec<Syscall>,
    /// This runtime, as the syscalls berm serves hold it. A deployed program
    /// owns the linker that owns them, so an `Arc` here is a cycle that never
    /// drops.
    me: Weak<Self>,
}

impl Berm {
    /// A runtime with nothing deployed, giving every program `syscalls`.
    pub fn new(engine: &Engine, calls: Calls, syscalls: Vec<Syscall>) -> Arc<Self> {
        Arc::new_cyclic(|me| Self {
            engine: engine.clone(),
            programs: RwLock::new(BTreeMap::new()),
            calls,
            syscalls,
            me: me.clone(),
        })
    }

    /// Compile `image` and make its tools reachable as `name`.
    ///
    /// Refused when the image does not compile, or when its pool's images, the
    /// one it replaces left out, would hold more than the pool's limit.
    pub fn deploy(&self, name: &str, image: &[u8]) -> Result<Arc<Program>> {
        let program = self.load(name, image)?;
        program
            .pool
            .fits(self.get(name).map_or(0, |replaced| replaced.bytes))?;
        self.programs
            .write()
            .expect("deployed programs")
            .insert(name.to_owned(), program.clone());
        Ok(program)
    }

    /// Whether a program deployed here can make the syscall `name`.
    pub fn serves(&self, name: &str) -> bool {
        name == abi::CALL
            || syscall::is_builtin(abi::hash(name))
            || self.syscalls.iter().any(|syscall| syscall.name == name)
    }

    /// Compile an image against the syscalls this runtime serves.
    fn load(&self, name: &str, image: &[u8]) -> Result<Arc<Program>> {
        let mut syscalls = self.syscalls.clone();
        syscalls.push(syscall::call::syscalls(self.me.clone(), self.calls.clone()));
        Ok(Arc::new(Program::load(
            &self.engine,
            image,
            name,
            &syscalls,
            &self.calls,
        )?))
    }

    /// Forget one and drop its image. `false` if nothing answered to that name.
    pub fn remove(&self, name: &str) -> bool {
        self.programs
            .write()
            .expect("deployed programs")
            .remove(name)
            .is_some()
    }

    pub fn get(&self, name: &str) -> Option<Arc<Program>> {
        self.programs
            .read()
            .expect("deployed programs")
            .get(name)
            .cloned()
    }

    /// Every deployed program, as a snapshot the caller can hold.
    pub fn list(&self) -> Vec<Arc<Program>> {
        self.programs
            .read()
            .expect("deployed programs")
            .values()
            .cloned()
            .collect()
    }

    /// Run one tool on one program.
    ///
    /// `Err` is the host's — no such program, no such tool, a trap. The
    /// program's own reported failure is [`Outcome::result`].
    pub async fn call(
        &self,
        caller: Option<Arc<str>>,
        program: &str,
        tool: &str,
        args: impl Into<Vec<u8>>,
    ) -> Result<Outcome> {
        let Some(program) = self.get(program) else {
            anyhow::bail!("no program named {program:?} is deployed");
        };
        program.call(caller, tool, args).await
    }
}

/// One thing a program may reach.
///
/// A syscall absent from the list a program was deployed with is absent
/// from its linker, and that absence is the enforcement — there is no check to
/// write and none to forget. Whatever bounds it is whatever it closes over.
///
/// The name is hashed to the number the guest passes to `berm.syscall`, and
/// berm neither reserves a namespace nor recognises one: every name in the list
/// belongs to the embedder that wrote it.
#[derive(Clone)]
pub struct Syscall {
    /// What the guest calls it, e.g. `berm.http.fetch`.
    pub name: String,
    /// What it does.
    pub call: Call,
}
