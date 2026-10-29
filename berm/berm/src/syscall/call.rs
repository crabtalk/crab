//! `berm.call` — one service reaching another.
//!
//! berm serves it against the set of deployed services it holds. The target is
//! named in the request; which deployed service a name reaches is the
//! embedder's [`Calls::resolve`].

use crate::{
    Berm, Budget, Caller, Callsite, Changes, Ended, Link, Meter, Pool, Refused, Service, Syscall,
    abi, clock, wire,
};
use anyhow::{Result, bail};
use berm_api::{Output, host::Outcome};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Weak},
};

/// The deployed name a call's target reaches, asked with the calling site and
/// the tool called. An `Err` refuses the call.
pub type Resolve = Arc<dyn Fn(&Callsite, &str, &str) -> Result<String> + Send + Sync>;

/// What a called service is told the service calling it is named, given the
/// name that one was deployed as.
pub type Named = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// What an invocation of `service` runs inside, asked with the invocation's
/// site before it runs: the chain's first call and every nested one.
pub type Enter = Arc<dyn Fn(&Service, &Callsite) -> Opening + Send + Sync>;

/// The budget of one invocation of `tool` of the service deployed as
/// `service`. `None` takes the engine's [`crate::Config`].
pub type Budgeted = Arc<dyn Fn(&str, &str) -> Option<Budget> + Send + Sync>;

/// The pool the service deployed as a name draws its memory from, asked once
/// as it loads.
pub type Pooled = Arc<dyn Fn(&str) -> Arc<Pool> + Send + Sync>;

/// What `berm.now` answers: milliseconds since the Unix epoch. Called inside a
/// syscall, where an unwind aborts the process.
pub type Now = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Told of every nested call that reached a deployed service: the calling
/// site, the deployed name, the tool, and how it ended.
pub type Reported = Arc<dyn Fn(&Callsite, &str, &str, Outcome) + Send + Sync>;

/// Told of every invocation once it ran, trapped or not: its site, the tool,
/// what it used itself, the services it called left out, and how it ran.
pub type Invoked = Arc<dyn Fn(&Callsite, &str, Meter, Ran) + Send + Sync>;

/// How one invocation ran.
#[derive(Clone, Copy, Debug)]
pub struct Ran {
    /// When it began, in epoch milliseconds.
    pub at: u64,
    pub ms: u64,
    /// A trap is [`Outcome::Failed`].
    pub outcome: Outcome,
}

/// Reaches a service deployed outside this runtime: asked with the calling
/// site, the deployed name, the tool, its arguments, and the link the target
/// runs as. `None` when nothing is deployed under that name elsewhere either.
/// The target's runtime commits what it changes; nothing lands in the
/// caller's journal.
pub type Remote =
    Arc<dyn Fn(&Callsite, &str, &str, Vec<u8>, Link) -> Option<Reaching> + Send + Sync>;

/// A call to a service outside this runtime, under way.
pub type Reaching = Pin<Box<dyn Future<Output = Result<Ended>> + Send>>;

/// A [`Scope`], being opened. An `Err` fails the invocation before it runs.
pub type Opening = Pin<Box<dyn Future<Output = Result<Box<dyn Scope>>> + Send>>;

/// What one invocation runs inside, as the host opened it. Dropped without
/// [`Scope::close`], it undoes what it holds.
pub trait Scope: Send {
    /// Whether the invocation holds its writes and logs on a journal of its
    /// own, handed to [`Scope::close`].
    fn own(&self) -> bool;

    /// The invocation returned ok. Handed what its own journal holds, empty
    /// without one, and answers what is left for the journal of the call that
    /// made it, or for the host when it is the chain's first.
    fn close(
        self: Box<Self>,
        changes: Changes,
    ) -> Pin<Box<dyn Future<Output = Result<Changes>> + Send>>;
}

/// The scope [`Calls::by_name`] opens: nothing held, on the caller's journal.
struct Unheld;

impl Scope for Unheld {
    fn own(&self) -> bool {
        false
    }

    fn close(
        self: Box<Self>,
        changes: Changes,
    ) -> Pin<Box<dyn Future<Output = Result<Changes>> + Send>> {
        Box::pin(std::future::ready(Ok(changes)))
    }
}

/// How `berm.call` reaches services.
#[derive(Clone)]
pub struct Calls {
    /// See [`Calls::DEFAULT_DEPTH`].
    pub depth: u32,
    pub resolve: Resolve,
    pub named: Named,
    pub enter: Enter,
    pub budget: Budgeted,
    pub now: Now,
    pub called: Reported,
    pub invoked: Invoked,
    pub remote: Remote,
    pub pool: Pooled,
}

impl Calls {
    /// How deep a chain of services calling services may go before the next
    /// call is refused. Zero turns composition off.
    ///
    /// Not a bound on the native stack, which a nesting level costs ~720
    /// bytes of and would allow thousands: it bounds how far a mechanical
    /// composition can run away from the turn that asked for it, and how much
    /// guest address space one chain reserves — 64 MiB a level.
    pub const DEFAULT_DEPTH: u32 = 4;

    /// A target reaches the service deployed under that name, and is told the
    /// caller's deployed name.
    pub fn by_name(depth: u32) -> Self {
        Self {
            depth,
            resolve: Arc::new(|_, target, _| Ok(target.to_owned())),
            named: Arc::new(str::to_owned),
            enter: Arc::new(|_, _| Box::pin(async { Ok(Box::new(Unheld) as Box<dyn Scope>) })),
            budget: Arc::new(|_, _| None),
            now: Arc::new(clock::now),
            called: Arc::new(|_, _, _, _| {}),
            invoked: Arc::new(|_, _, _, _| {}),
            remote: Arc::new(|_, _, _, _, _| None),
            pool: Arc::new(|_| Arc::default()),
        }
    }
}

/// Serve `berm.call` against `runtime`.
pub(crate) fn syscalls(runtime: Weak<Berm>, calls: Calls) -> Syscall {
    let Calls {
        depth: limit,
        resolve,
        named,
        enter: _,
        budget: _,
        now: _,
        called,
        invoked: _,
        remote,
        pool: _,
    } = calls;
    Syscall {
        name: abi::CALL.to_owned(),
        call: Arc::new(move |at: Callsite, request: Vec<u8>| {
            let runtime = runtime.clone();
            let (resolve, named, called, remote) = (
                resolve.clone(),
                named.clone(),
                called.clone(),
                remote.clone(),
            );
            Box::pin(async move {
                let fields = abi::wire::fields(&request)?;
                let service = wire::text(&fields, 0, "service")?;
                let tool = wire::text(&fields, 1, "tool")?;
                let args = wire::text(&fields, 2, "arguments")?;

                if at.depth > limit {
                    return Err(Refused(format!(
                        "call depth {limit} reached before {service}.{tool}; a service cannot nest deeper"
                    ))
                    .into());
                }

                let Some(runtime) = runtime.upgrade() else {
                    return Err(Refused(format!(
                        "the runtime is shutting down, so {service}.{tool} cannot run"
                    ))
                    .into());
                };

                // A deploy holding the write lock refuses the call. The guard
                // is dropped before the guest below runs: held across a nested
                // call, it blocks the next deploy for as long as that call
                // takes.
                let deployed = resolve(&at, service, tool)
                    .map_err(|error| Refused(format!("{service}.{tool}: {error:#}")))?;
                let target = {
                    let Ok(services) = runtime.services.try_read() else {
                        return Err(Refused(format!(
                            "the deployed set is being written; {service}.{tool} was not reached"
                        ))
                        .into());
                    };
                    services.get(&deployed).cloned()
                };

                let caller = Some(Caller::Service(named(&at.service).into()));
                let Some(target) = target else {
                    let link = Link {
                        caller,
                        user: at.user.clone(),
                        client: at.client.clone(),
                        depth: at.depth + 1,
                        query: at.query,
                        deadline: at.deadline,
                        stack: at
                            .stack
                            .iter()
                            .cloned()
                            .chain([Arc::from(deployed.as_str())])
                            .collect(),
                    };
                    let Some(reaching) =
                        remote(&at, &deployed, tool, args.as_bytes().to_vec(), link)
                    else {
                        let missing = match deployed == service {
                            true => format!("no service named {service:?} is deployed"),
                            false => {
                                format!("{service:?} reaches {deployed:?}, which is not deployed")
                            }
                        };
                        return Err(Refused(missing).into());
                    };
                    let invoked = reaching.await;
                    called(&at, &deployed, tool, ended(&invoked));
                    return answer(invoked);
                };
                let refuse = |message: String| -> Result<Vec<u8>> {
                    called(&at, &deployed, tool, Outcome::Refused);
                    Err(Refused(message).into())
                };
                if !target.public(tool) {
                    return refuse(format!("{service} has no public tool named {tool:?}"));
                }
                if !target.reached_by_services(tool) {
                    return refuse(format!("{service}.{tool} takes no calls from services"));
                }

                let stack: Arc<[Arc<str>]> = at
                    .stack
                    .iter()
                    .cloned()
                    .chain([target.name.clone()])
                    .collect();
                let invoked = target
                    .invoke(
                        tool,
                        args.as_bytes().to_vec(),
                        caller,
                        at.user.clone(),
                        at.client.clone(),
                        at.depth + 1,
                        at.query,
                        at.deadline,
                        at.chain,
                        at.journal.clone(),
                        at.meters.clone(),
                        stack,
                        None,
                        Some(&at),
                    )
                    .await;
                called(&at, &deployed, tool, ended(&invoked));
                answer(invoked)
            })
        }),
    }
}

/// What the calling guest is handed back.
fn answer(invoked: Result<Ended>) -> Result<Vec<u8>> {
    match invoked {
        Ok(Ended {
            result: Output::Done(result),
            ..
        }) => Ok(result.into_bytes()),
        // The target ran and said no. Not a `Refused`: the caller is told the
        // difference, and may act on it.
        Ok(Ended {
            result: Output::Failed(failure),
            ..
        }) => bail!(failure),
        Err(error) => Err(error),
    }
}

/// How an invocation ended, as [`Reported`] is told it.
fn ended(invoked: &Result<Ended>) -> Outcome {
    match invoked {
        Ok(Ended { result, denied }) => Outcome::of(result, *denied),
        Err(_) => Outcome::Failed,
    }
}
