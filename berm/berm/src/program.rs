//! One compiled program, and the state of one invocation of it.

use crate::{
    Caller, Callsite, Deps, Meter, Meters, Syscall, User, abi,
    bound::watchdog,
    engine::{Engine, Image},
    journal::{Changes, Journal, Mark},
    meter::{Held, Own, Running},
    syscall::{
        self,
        call::{Budgeted, Calls, Enter, Invoked, Now, Scope},
    },
};
use anyhow::{Context, Result, bail};
use berm_api::{Callers, Manifest, Output, Runtime};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

/// The last [`Callsite::chain`] handed out.
static CHAINS: AtomicU64 = AtomicU64::new(0);

/// A [`Callsite::chain`] no chain in this process has had, for
/// [`Program::call_as`].
pub fn chain() -> u64 {
    CHAINS.fetch_add(1, Ordering::Relaxed) + 1
}

/// One compiled program: an image, its tools, and the syscalls it was linked
/// against. Compilation is paid once; every invocation gets a fresh instance so
/// no guest state crosses between calls.
pub struct Program {
    /// What this program was deployed as, as every syscall reports it. An
    /// `Arc` because it is cloned into each invocation.
    pub name: Arc<str>,
    /// sha256 of the image. Redeploying different bytes under the same name is
    /// a different program, and this is what says so.
    pub digest: String,
    image: Image,
    /// Read from the image at load, without running anything.
    manifest: Manifest,
    /// The manifest's deps, as every invocation's [`Callsite`] carries them.
    deps: Arc<Deps>,
    /// Each tool beside the symbol it is exported as, resolved once so a call
    /// spends no allocation rebuilding the name, and whether it is a query.
    tools: BTreeMap<String, Tool>,
    /// What each invocation runs inside.
    enter: Enter,
    /// Each invocation's budget, by tool.
    budget: Budgeted,
    /// What `berm.now` answers.
    now: Now,
    /// Told of each invocation once it ran.
    invoked: Invoked,
    /// What its invocations hold while they run.
    pub running: Arc<Running>,
}

struct Tool {
    symbol: String,
    query: bool,
}

impl Program {
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
        let compiled = Image::compile(engine, image, syscall::table(syscalls)?)
            .context("failed to compile program")?;

        let manifest = Manifest::from_image(image)?;
        let tools: BTreeMap<String, Tool> = compiled
            .exports()
            .into_iter()
            .filter_map(|export| {
                let name = export.strip_prefix(abi::TOOL_PREFIX)?;
                let query = manifest
                    .tools
                    .iter()
                    .any(|spec| spec.name == name && spec.query);
                Some((
                    name.to_owned(),
                    Tool {
                        symbol: export.to_owned(),
                        query,
                    },
                ))
            })
            .collect();
        if tools.is_empty() {
            bail!("program exports no tools");
        }
        if manifest.tools.iter().filter(|tool| tool.stripe).count() > 1 {
            bail!("program declares more than one stripe tool");
        }

        // A program that advertises a tool it does not export would fail at
        // dispatch, on a model's turn, as a missing symbol. The export table
        // and the manifest are both in hand here, so disagreement is caught
        // before the program is ever offered.
        for tool in &manifest.tools {
            if !tools.contains_key(&tool.name) {
                bail!(
                    "program manifest declares tool {:?}, which it does not export",
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

        Ok(Self {
            name: name.into(),
            digest: digest(image),
            image: compiled,
            deps: Arc::new(manifest.deps.clone()),
            manifest,
            tools,
            enter: calls.enter.clone(),
            budget: calls.budget.clone(),
            now: calls.now.clone(),
            invoked: calls.invoked.clone(),
            running: Arc::default(),
        })
    }

    /// The tools this program exports, as its export table reports them.
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

    /// Whether `tool` takes calls from other programs, as its manifest's
    /// `callers` says.
    pub fn reached_by_programs(&self, tool: &str) -> bool {
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

    /// Bytes its image and its compiled code take.
    pub fn bytes(&self) -> u64 {
        self.image.bytes
    }

    /// What the program says it is: ABI version, tools, and README.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Its IDL, deployed as `name`: the JSON a host serves for it.
    pub fn idl(&self, name: &str) -> berm_api::Program {
        let Manifest {
            abi_version: _,
            database: _,
            kits: _,
            deps: _,
            events: _,
            version,
            description,
            homepage,
            repository,
            readme,
            tools,
        } = self.manifest.clone();
        berm_api::Program {
            name: name.to_owned(),
            version,
            description,
            homepage,
            repository,
            readme,
            digest: self.digest.clone(),
            tools: tools.into_iter().filter(|tool| tool.public).collect(),
        }
    }

    /// How it runs, for its owners; `unresolved` is the host's to fill.
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
        }
    }

    /// Run one tool by name, as the first link of a chain.
    ///
    /// `Err` is the host's — a missing tool, a trap, a broken image. The
    /// program's own reported failure is [`Outcome::result`].
    ///
    /// `caller` is who is calling, as the embedder names them; a syscall reads
    /// it off its [`Callsite`] as [`Caller::Outside`]. Any tool may be called
    /// this way: which tools an outside caller reaches is the embedder's to
    /// decide.
    pub async fn call(
        &self,
        caller: Option<Arc<str>>,
        tool: &str,
        args: impl Into<Vec<u8>>,
    ) -> Result<Outcome> {
        self.call_as(caller, None, None, tool, args, Arc::default(), chain())
            .await
    }

    /// [`Program::call`] on behalf of `user` and `client`, which every call in
    /// the chain reads off its [`Callsite`], metered in `meters`. `chain`, from
    /// [`chain`], is the chain's [`Callsite::chain`].
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
                watchdog::fresh(),
                chain,
                journal.clone(),
                meters,
                Arc::from([self.name.clone()]),
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
                chain(),
                journal.clone(),
                meters,
                stack,
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
    /// has to be done by `deadline`, under the programs in `stack`. `query` is
    /// whether a link above it is a query. What it keeps lands in `journal`,
    /// and a failure takes it back out; what it uses lands in `meters`, failed
    /// or not.
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
    ) -> Result<Ended> {
        let Some(Tool {
            symbol,
            query: declared,
        }) = self.tools.get(tool)
        else {
            bail!("program exports no tool named {tool:?}");
        };

        let budget = (self.budget)(&self.name, tool).unwrap_or(self.image.budget);
        let mut at = Callsite {
            program: self.name.clone(),
            deps: self.deps.clone(),
            caller,
            user,
            client,
            depth,
            stack,
            query: query || *declared,
            deadline,
            chain,
            journal: journal.clone(),
            meters,
            own: Arc::new(Own::new(budget.rows)),
        };
        at.meter(Meter {
            calls: 1,
            ..Meter::default()
        });
        let scope = (self.enter)(self, &at).await?;
        let own = scope.own();
        if own {
            at.journal = Arc::new(Journal::default());
        }
        let held = own.then(|| at.journal.clone());
        let mark = journal.mark();
        let site = at.clone();
        let invocation = Invocation {
            held: Held::new(self.running.clone()),
            at,
            now: self.now.clone(),
            args,
            staged: Vec::new(),
            outcome: None,
            denied: false,
        };
        let ended = self
            .image
            .call(symbol, invocation, budget.instructions)
            .await
            .with_context(|| format!("program trapped in {tool}"))
            .and_then(ended);
        self.ran(&site, tool, &ended);
        settle(&ended, scope, held, &journal, mark).await?;
        ended
    }

    /// Count a failed invocation at `site` and report what it used itself.
    fn ran(&self, site: &Callsite, tool: &str, ended: &Result<Ended>) {
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
        (self.invoked)(site, tool, site.own.take());
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
/// wrote nothing, which is an empty result.
fn ended(finished: Invocation) -> Result<Ended> {
    let result = match finished.outcome {
        Some(Err(failure)) => Output::Failed(failure),
        Some(Ok(result)) => {
            Output::Done(String::from_utf8(result).context("program returned invalid UTF-8")?)
        }
        None => Output::Done(String::new()),
    };
    Ok(Ended {
        denied: finished.denied && matches!(result, Output::Failed(_)),
        result,
    })
}

/// Where an invocation sits in a chain that reached it from outside this
/// runtime. `stack` ends with the program being invoked.
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
    /// The program failed the call through `berm.deny`.
    pub denied: bool,
}

/// What one chain answered with, and what it left for the host to commit.
#[derive(Debug)]
pub struct Outcome {
    /// [`Output::Failed`] is the program's own reported failure, which is a
    /// tool result rather than an error.
    pub result: Output,
    /// `result` is [`Output::Failed`] because the program refused its caller,
    /// through `berm.deny`.
    pub denied: bool,
    /// Empty when `result` is [`Output::Failed`].
    pub changes: Changes,
}

/// sha256 of `image`, in lowercase hex: what [`Program::digest`] holds.
pub fn digest(image: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(image))
}

/// Guest state for one invocation. Memory is per-invocation; anything a
/// program needs to survive belongs in a storage program, not here.
pub struct Invocation {
    /// The program this invocation is of, and the chain it sits in. Built
    /// once with the invocation, so a syscall costs no lookup.
    pub(crate) at: Callsite,
    /// What `berm.now` answers.
    pub(crate) now: Now,
    pub(crate) args: Vec<u8>,
    /// The last syscall's reply, waiting for the guest to pull it. Staged
    /// rather than pushed because its size is not known until the work is
    /// done, and doing the work twice to measure it is not an option.
    pub(crate) staged: Vec<u8>,
    /// What the tool answered with, once it has. `Err` is the program's own
    /// reported failure, which is how a tool that failed is told apart from one
    /// that returned the word "error".
    pub(crate) outcome: Option<Result<Vec<u8>, String>>,
    /// Set by `berm.deny`.
    pub(crate) denied: bool,
    /// Its part of its program's [`Running`].
    pub(crate) held: Held,
}

/// A syscall's answer when it refused the call and nothing ran.
///
/// Returned in an `Err` — on its own or as the source of a richer one — it
/// reaches the guest with [`abi::REFUSED`] set. Anything else is the other
/// kind of failure: whatever the syscall reached did run, and said no.
#[derive(Debug)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}
