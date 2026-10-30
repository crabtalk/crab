//! One compiled service, and the state of one invocation of it.

use crate::{
    Caller, Callsite, Deps, Meter, Meters, Syscall, User, abi, chain,
    engine::{Engine, Image},
    journal::{Changes, Journal, Mark},
    meter::{Held, Own, Pool, Running},
    syscall::{
        self,
        call::{Budgeted, Calls, Enter, Invoked, Now, Ran, Scope},
    },
    tape::{Deck, Taping},
};
use anyhow::{Context, Result, bail};
use berm_api::{Callers, Manifest, Output, Runtime, digest, host};
use std::{
    collections::BTreeMap,
    error, fmt,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use wasmtime::{Memory, ModuleExport};

/// One compiled service: an image, its tools, and the syscalls it was linked
/// against. Compilation is paid once; every invocation gets a fresh instance,
/// and no guest state crosses between calls.
pub struct Service {
    /// What this service was deployed as, as every syscall reports it.
    pub name: Arc<str>,
    /// sha256 of the image; different bytes deployed under one name differ here.
    pub digest: String,
    image: Image,
    /// Bytes its image and its compiled code take.
    pub bytes: u64,
    /// Read from the image at load, without running anything.
    pub manifest: Manifest,
    /// The manifest's deps, as every invocation's [`Callsite`] carries them.
    deps: Arc<Deps>,
    /// Each tool beside its export, resolved once, and whether it is a query.
    tools: BTreeMap<String, Tool>,
    /// What each invocation runs inside.
    enter: Enter,
    /// Each invocation's budget, by tool.
    budget: Budgeted,
    /// What `berm.now` answers.
    now: Now,
    /// What the services it calls are told it is named: the issuer of a user
    /// it vouches for.
    issuer: Arc<str>,
    /// Told of each invocation once it ran.
    invoked: Invoked,
    /// What its invocations hold while they run.
    pub running: Arc<Running>,
    /// What it draws its memory from, its image counted in it while it is
    /// loaded.
    pub pool: Arc<Pool>,
    /// How long a chain it starts runs, [`crate::Config::deadline`].
    deadline: Duration,
}

struct Tool {
    export: ModuleExport,
    query: bool,
}

impl Drop for Service {
    fn drop(&mut self) {
        self.pool.unload(self.bytes);
    }
}

impl Service {
    /// Compile `image` and resolve its exports, giving it `syscalls`. The
    /// engine's code cache makes a second load of the same bytes cheap across
    /// processes as well as within one.
    pub(crate) fn load(
        engine: &Engine,
        image: &[u8],
        name: impl Into<Arc<str>>,
        syscalls: &[Syscall],
        calls: &Calls,
    ) -> Result<Self> {
        let (compiled, bytes) = Image::compile(engine, image, syscall::table(syscalls)?)
            .context("failed to compile service")?;

        let manifest = Manifest::try_from(image)?;
        let tools: BTreeMap<String, Tool> = compiled
            .exports()
            .filter_map(|(symbol, export)| {
                let name = symbol.strip_prefix(abi::TOOL_PREFIX)?;
                let query = manifest
                    .tools
                    .iter()
                    .any(|spec| spec.name == name && spec.query);
                Some((name.to_owned(), Tool { export, query }))
            })
            .collect();
        if tools.is_empty() {
            bail!("service exports no tools");
        }
        if manifest.tools.iter().filter(|tool| tool.stripe).count() > 1 {
            bail!("service declares more than one stripe tool");
        }

        for tool in &manifest.tools {
            if !tools.contains_key(&tool.name) {
                bail!(
                    "service manifest declares tool {:?}, which it does not export",
                    tool.name
                );
            }
            if tool.cron && tool.public {
                bail!("tool {:?} is a cron tool, and cannot be public", tool.name);
            }
            if tool.stripe && tool.public {
                bail!(
                    "tool {:?} is a stripe tool, and cannot be public",
                    tool.name
                );
            }
            if tool.anonymous && (!tool.public || tool.callers == Some(Callers::Services)) {
                bail!(
                    "tool {:?} takes anonymous calls, and must be public to callers from outside",
                    tool.name
                );
            }
        }

        let name: Arc<str> = name.into();
        let pool = (calls.pool)(&name);
        pool.load(bytes);
        Ok(Self {
            issuer: (calls.named)(&name).into(),
            name,
            digest: digest::hex(image),
            image: compiled,
            bytes,
            deps: Arc::new(manifest.deps.clone()),
            manifest,
            tools,
            enter: calls.enter.clone(),
            budget: calls.budget.clone(),
            now: calls.now.clone(),
            invoked: calls.invoked.clone(),
            running: Arc::default(),
            pool,
            deadline: engine.deadline,
        })
    }

    /// The tools this service exports, as its export table reports them.
    pub fn tools(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(String::as_str)
    }

    /// Whether `tool` is one any caller may reach.
    pub fn public(&self, tool: &str) -> bool {
        self.manifest
            .tools
            .iter()
            .any(|spec| spec.name == tool && spec.public)
    }

    /// Whether `tool` takes calls from other services, as its manifest's
    /// `callers` says.
    pub fn reached_by_services(&self, tool: &str) -> bool {
        self.manifest
            .tools
            .iter()
            .any(|spec| spec.name == tool && spec.callers != Some(Callers::Accounts))
    }

    /// Whether `tool` takes a call from outside with no caller.
    pub fn reached_anonymously(&self, tool: &str) -> bool {
        self.manifest
            .tools
            .iter()
            .any(|spec| spec.name == tool && spec.anonymous)
    }

    /// Whether `tool` takes calls from outside, as its manifest's `callers`
    /// says.
    pub fn reached_by_accounts(&self, tool: &str) -> bool {
        self.manifest
            .tools
            .iter()
            .any(|spec| spec.name == tool && spec.callers != Some(Callers::Services))
    }

    /// Its IDL, deployed as `name`: the JSON a host serves for it.
    pub fn idl(&self, name: &str) -> berm_api::Service {
        let Manifest {
            abi_version: _,
            name: _,
            buffer: _,
            database: _,
            kits: _,
            deps,
            events: _,
            version,
            description,
            homepage,
            repository,
            readme,
            tools,
        } = &self.manifest;
        berm_api::Service {
            name: name.to_owned(),
            version: version.clone(),
            description: description.clone(),
            homepage: homepage.clone(),
            repository: repository.clone(),
            readme: readme.clone(),
            digest: self.digest.clone(),
            tools: tools.iter().filter(|tool| tool.public).cloned().collect(),
            vars: deps.vars.clone(),
        }
    }

    /// How it runs, for its owners; `unresolved` and `project` are the host's
    /// to fill.
    pub fn runtime(&self) -> Runtime {
        Runtime {
            tools: self
                .manifest
                .tools
                .iter()
                .filter(|tool| !tool.public)
                .cloned()
                .collect(),
            deps: self.manifest.deps.clone(),
            events: self.manifest.events.clone(),
            unresolved: Default::default(),
            project: None,
        }
    }

    /// Run one tool by name, as the first link of a chain.
    ///
    /// `Err` is the host's — a missing tool, a trap, a broken image. The
    /// service's own reported failure is [`Outcome::result`].
    ///
    /// `caller` is who is calling, as the embedder names them; a syscall reads
    /// it off its [`Callsite`] as [`Caller::Outside`]. Any tool may be called
    /// this way; the embedder decides which tools an outside caller reaches.
    pub async fn call(
        &self,
        caller: Option<Arc<str>>,
        tool: &str,
        args: impl Into<Vec<u8>>,
    ) -> Result<Outcome> {
        self.call_as(
            caller,
            None,
            None,
            tool,
            args,
            Arc::default(),
            chain::next(),
            Taping::Off,
        )
        .await
    }

    /// [`Service::call`] on behalf of `user` and `client`, which every call in
    /// the chain reads off its [`Callsite`], metered in `meters`. `chain`, from
    /// [`chain::next`], is the chain's [`Callsite::chain`].
    #[allow(clippy::too_many_arguments)]
    pub async fn call_as(
        &self,
        caller: Option<Arc<str>>,
        user: Option<User>,
        client: Option<Arc<str>>,
        tool: &str,
        args: impl Into<Vec<u8>>,
        meters: Arc<Meters>,
        chain: u64,
        taping: Taping,
    ) -> Result<Outcome> {
        let journal = Arc::new(Journal::default());
        let Ended { result, denied } = self
            .invoke(
                tool,
                args.into(),
                caller.map(Caller::Outside),
                user,
                client,
                1,
                false,
                Instant::now() + self.deadline,
                chain,
                journal.clone(),
                meters,
                Arc::from([self.name.clone()]),
                taping.deck(),
                None,
            )
            .await?;
        let changes = match result {
            Output::Done(_) => journal.take(),
            Output::Failed(_) => Changes::default(),
        };
        Ok(Outcome {
            result,
            denied,
            changes,
        })
    }

    /// Run one tool as `link`, a link of a chain another runtime holds,
    /// metered in `meters`. The chain's journal starts here, and what this link
    /// changes is handed back as [`Outcome::changes`] like a first call's.
    pub async fn link(
        &self,
        link: Link,
        tool: &str,
        args: impl Into<Vec<u8>>,
        meters: Arc<Meters>,
    ) -> Result<Outcome> {
        let Link {
            caller,
            user,
            client,
            depth,
            query,
            deadline,
            stack,
        } = link;
        let journal = Arc::new(Journal::default());
        let Ended { result, denied } = self
            .invoke(
                tool,
                args.into(),
                caller,
                user,
                client,
                depth,
                query,
                deadline,
                chain::next(),
                journal.clone(),
                meters,
                stack,
                None,
                None,
            )
            .await?;
        let changes = match result {
            Output::Done(_) => journal.take(),
            Output::Failed(_) => Changes::default(),
        };
        Ok(Outcome {
            result,
            denied,
            changes,
        })
    }

    /// Run one tool as link `depth` of chain `chain`, called by `caller`, that
    /// has to be done by `deadline`, under the services in `stack`. `query` is
    /// whether a link above it is a query. What it keeps lands in `journal`,
    /// and a failure takes it back out; what it uses lands in `meters`, failed
    /// or not. `parent` is the invocation that called it on this runtime.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn invoke(
        &self,
        tool: &str,
        args: Vec<u8>,
        caller: Option<Caller>,
        user: Option<User>,
        client: Option<Arc<str>>,
        depth: u32,
        query: bool,
        deadline: Instant,
        chain: u64,
        journal: Arc<Journal>,
        meters: Arc<Meters>,
        stack: Arc<[Arc<str>]>,
        tape: Option<Deck>,
        parent: Option<&Callsite>,
    ) -> Result<Ended> {
        let (at_ms, began) = ((self.now)(), Instant::now());
        let Some(Tool {
            export,
            query: declared,
        }) = self.tools.get(tool)
        else {
            bail!("service exports no tool named {tool:?}");
        };

        let budget = (self.budget)(&self.name, tool).unwrap_or(self.image.budget);
        let mut at = Callsite {
            service: self.name.clone(),
            tool: tool.into(),
            deps: self.deps.clone(),
            caller,
            user,
            client,
            depth,
            stack,
            query: query || *declared,
            deadline,
            chain,
            span: parent.map_or(0, |parent| parent.spans.fetch_add(1, Ordering::Relaxed)),
            parent: parent.map(|parent| parent.span),
            spans: parent.map_or_else(
                || Arc::new(AtomicU32::new(1)),
                |parent| parent.spans.clone(),
            ),
            journal: journal.clone(),
            meters,
            own: Arc::new(Own::new(budget.rows)),
            #[cfg(feature = "sqlite")]
            database: Arc::default(),
        };
        at.meter(Meter {
            runs: u64::from(depth == 1),
            calls: 1,
            ..Meter::default()
        });
        let scope = (self.enter)(self, &at).await?;
        // A refusal drops the scope, which undoes what it holds.
        let largest = self.image.budget.memory;
        let reserved = Held::new(
            self.running.clone(),
            self.pool.clone(),
            self.image.reserve.clone(),
            match budget.memory {
                0 => largest,
                memory => memory.min(largest),
            },
        )?;
        let own = scope.own();
        if own {
            at.journal = Arc::new(Journal::default());
        }
        let held = own.then(|| at.journal.clone());
        let mark = journal.mark();
        let site = at.clone();
        let invocation = Invocation {
            cpu: Arc::new(AtomicU64::new(budget.cpu)),
            limited: false,
            held: reserved,
            at,
            now: self.now.clone(),
            issuer: self.issuer.clone(),
            args,
            staged: Vec::new(),
            outcome: None,
            denied: false,
            tape,
            memory: None,
        };
        let buffer = self.manifest.buffer;
        let ended = match invocation.args.len() > buffer {
            true => Ok(Ended {
                result: Output::Failed("arguments exceeded the input buffer".to_owned()),
                denied: false,
            }),
            false => self
                .image
                .call(export, invocation)
                .await
                .with_context(|| format!("service trapped in {tool}"))
                .and_then(|finished| ended(finished, buffer)),
        };
        self.ran(
            &site,
            tool,
            &ended,
            Ran {
                at: at_ms,
                ms: began.elapsed().as_millis() as u64,
                outcome: match &ended {
                    Ok(ended) => host::Outcome::of(&ended.result, ended.denied),
                    Err(_) => host::Outcome::Failed,
                },
            },
        );
        settle(&ended, scope, held, &journal, mark).await?;
        ended
    }

    /// Count a failed invocation at `site` and report what it used itself.
    fn ran(&self, site: &Callsite, tool: &str, ended: &Result<Ended>, ran: Ran) {
        if matches!(
            ended,
            Err(_)
                | Ok(Ended {
                    result: Output::Failed(_),
                    denied: false
                })
        ) {
            site.meter(Meter {
                failed: 1,
                ..Meter::default()
            });
        }
        (self.invoked)(site, tool, site.own.take(), ran);
    }
}

/// Close `scope` over what a done invocation kept on `held`, its own journal
/// when it has one, or take back what it wrote to `journal` past `mark`.
async fn settle(
    ended: &Result<Ended>,
    scope: Box<dyn Scope>,
    held: Option<Arc<Journal>>,
    journal: &Journal,
    mark: Mark,
) -> Result<()> {
    let Ok(Ended {
        result: Output::Done(_),
        ..
    }) = ended
    else {
        journal.revert(mark);
        return Ok(());
    };
    let changes = held.map_or_else(Changes::default, |held| held.take());
    let kept = scope
        .close(changes)
        .await
        .and_then(|left| journal.extend(left));
    if kept.is_err() {
        journal.revert(mark);
    }
    kept
}

/// How a finished invocation ended. A tool that returned without answering
/// wrote nothing, which is an empty result; one that answered more than
/// `buffer` bytes failed.
fn ended(finished: Invocation, buffer: usize) -> Result<Ended> {
    let result = match finished.outcome {
        Some(Err(failure)) => Output::Failed(failure),
        Some(Ok(result)) if result.len() > buffer => {
            Output::Failed("result exceeded the output buffer".to_owned())
        }
        Some(Ok(result)) => {
            Output::Done(String::from_utf8(result).context("service returned invalid UTF-8")?)
        }
        None => Output::Done(String::new()),
    };
    Ok(Ended {
        denied: finished.denied && matches!(result, Output::Failed(_)),
        result,
    })
}

/// Where an invocation sits in a chain that reached it from outside this
/// runtime. `stack` ends with the service being invoked.
#[derive(Clone, Debug)]
pub struct Link {
    pub caller: Option<Caller>,
    pub user: Option<User>,
    pub client: Option<Arc<str>>,
    pub depth: u32,
    pub query: bool,
    pub deadline: Instant,
    pub stack: Arc<[Arc<str>]>,
}

/// How one invocation ended.
#[derive(Debug)]
pub struct Ended {
    pub result: Output,
    /// The service failed the call through `berm.deny`.
    pub denied: bool,
}

/// What one chain answered with, and what it left for the host to commit.
#[derive(Debug)]
pub struct Outcome {
    /// [`Output::Failed`] is the service's own reported failure.
    pub result: Output,
    /// The service refused its caller through `berm.deny`; `result` is then
    /// [`Output::Failed`].
    pub denied: bool,
    /// Empty when `result` is [`Output::Failed`].
    pub changes: Changes,
}

/// Guest state for one invocation. Memory is per-invocation.
pub struct Invocation {
    /// The service this invocation is of, and the chain it sits in.
    pub(crate) at: Callsite,
    /// What `berm.now` answers.
    pub(crate) now: Now,
    /// Its service's [`Service::issuer`].
    pub(crate) issuer: Arc<str>,
    pub(crate) args: Vec<u8>,
    /// The last syscall's reply, waiting for the guest to pull it.
    pub(crate) staged: Vec<u8>,
    /// What the tool answered with, once it has. `Err` is the service's own
    /// reported failure.
    pub(crate) outcome: Option<Result<Vec<u8>, String>>,
    /// Set by `berm.deny`.
    pub(crate) denied: bool,
    /// Its budget of CPU, in microseconds.
    pub(crate) cpu: Arc<AtomicU64>,
    /// Set once `berm.limit` lowered its budget.
    pub(crate) limited: bool,
    /// Its part of its service's [`Running`].
    pub(crate) held: Held,
    /// The tape it records or answers from; only a chain's first link has one.
    pub(crate) tape: Option<Deck>,
    /// Its instance's linear memory, once instantiated.
    pub(crate) memory: Option<Memory>,
}

/// A syscall's answer when it refused the call and nothing ran.
///
/// Returned in an `Err` — on its own or as the source of a richer one — it
/// reaches the guest with [`abi::REFUSED`] set. Anything else is the other
/// kind of failure: whatever the syscall reached did run, and said no.
#[derive(Debug)]
pub struct Refused(pub String);

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl error::Error for Refused {}
