//! One deployment and the state of one invocation of it.

use crate::{
    Caller, Callsite, Client, Deps, Meter, Meters, Syscall, User, abi, chain,
    code::Source,
    control,
    engine::{self, Engine, Image},
    journal::{Changes, Journal, Mark},
    meter::{Held, Own, Pool, Running},
    syscall::{
        self,
        call::{Calls, Enter, Invoked, Now, Ran, Scope},
    },
    tape::{Deck, Taping},
    workflow,
};
use anyhow::{Context, Result, bail};
use berm_api::{
    Budget, Callers, Manifest, Output, Runtime, digest, host,
    workflow::{
        graph::Target,
        run::{Binding, Failure},
        spend::Context as SpendContext,
    },
};
use std::{
    collections::BTreeMap,
    error, fmt,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use wasmparser::{ExternalKind, Parser, Payload};
use wasmtime::Memory;

/// One deployment's metadata, source image and host bindings.
/// Executable code may be cold; no guest state crosses between calls.
pub struct Service {
    /// What this service was deployed as, as every syscall reports it.
    pub name: Arc<str>,
    /// sha256 of the image; different bytes deployed under one name differ here.
    pub digest: String,
    image: Arc<Source>,
    reserve: Arc<Pool>,
    /// Image length plus compiled image range recorded at deployment.
    pub bytes: u64,
    /// Read from the image at load, without running anything.
    pub manifest: Manifest,
    /// The manifest's deps, as every invocation's [`Callsite`] carries them.
    deps: Arc<Deps>,
    /// Tool metadata, independent of executable residency.
    tools: BTreeMap<String, Tool>,
    pub controls: BTreeMap<String, Budget>,
    /// What each invocation runs inside.
    enter: Enter,
    control_enter: Option<Enter>,
    /// What `berm.now` answers.
    now: Now,
    /// What the services it calls are told it is named: the issuer of a user
    /// it vouches for.
    issuer: Arc<str>,
    /// Told of each invocation once it ran.
    invoked: Invoked,
    /// Retained code and active invocations across deployment generations.
    pub running: Arc<Running>,
    /// What it draws its memory from, its image counted in it while it is
    /// loaded.
    pub pool: Arc<Pool>,
    /// How long a chain it starts runs, [`crate::Config::deadline`].
    deadline: Duration,
}

struct Tool {
    query: bool,
    /// What one invocation of it is held to.
    budget: Budget,
}

pub(crate) enum Entry<'a> {
    Tool(&'a str),
    Control { workflow: &'a str, node: &'a str },
}

pub(crate) enum Load {
    Deploy,
    Restore(u64),
    Legacy,
}

impl Service {
    pub(crate) fn load(
        engine: &Engine,
        image: &[u8],
        name: impl Into<Arc<str>>,
        syscalls: &[Syscall],
        calls: &Calls,
        load: Load,
        running: Arc<Running>,
    ) -> Result<Self> {
        let table = syscall::table(syscalls)?;
        let (compiled, bytes) = match load {
            Load::Restore(bytes) => (None, bytes),
            load => {
                let (compiled, bytes) = Image::compile(engine, image, table.clone())
                    .context("failed to compile service")?;
                (matches!(load, Load::Deploy).then_some(compiled), bytes)
            }
        };

        let manifest = Manifest::try_from(image)?;
        let mut tools = BTreeMap::new();
        for payload in Parser::new(0).parse_all(image) {
            if let Payload::ExportSection(exports) = payload? {
                for export in exports {
                    let export = export?;
                    if export.kind != ExternalKind::Func {
                        continue;
                    }
                    let Some(name) = export.name.strip_prefix(abi::TOOL_PREFIX) else {
                        continue;
                    };
                    let spec = manifest.tools.iter().find(|spec| spec.name == name);
                    tools.insert(
                        name.to_owned(),
                        Tool {
                            query: spec.is_some_and(|spec| spec.query),
                            budget: spec
                                .and_then(|spec| spec.budget)
                                .map_or(engine.budget, |limits| limits.over(engine.budget)),
                        },
                    );
                }
            }
        }
        if tools.is_empty() && manifest.workflows.is_empty() {
            bail!("service exports no tools or workflows");
        }

        for (name, tool) in &tools {
            if let Some(key) = tool.budget.past(&engine.most) {
                return Err(engine::PastMost {
                    tool: name.clone(),
                    key,
                }
                .into());
            }
        }
        for workflow in &manifest.workflows {
            if let Some(key) = workflow
                .config
                .control_budget(engine.budget)
                .past(&engine.most)
            {
                return Err(engine::PastMost {
                    tool: format!("workflow {}", workflow.entry.name),
                    key,
                }
                .into());
            }
        }
        let controls = manifest
            .workflows
            .iter()
            .map(|workflow| {
                (
                    workflow.entry.name.clone(),
                    workflow.config.control_budget(engine.budget),
                )
            })
            .collect();
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
            if tool.webhook.is_some() && tool.public {
                bail!(
                    "tool {:?} is a webhook tool, and cannot be public",
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
        let source = Source::new(
            engine,
            image,
            table,
            pool.clone(),
            running.clone(),
            compiled,
        )?;
        Ok(Self {
            issuer: (calls.named)(&name).into(),
            name,
            digest: digest::hex(image),
            image: source,
            reserve: engine.reserve.clone(),
            bytes,
            deps: Arc::new(manifest.deps.clone()),
            manifest,
            tools,
            controls,
            enter: calls.enter.clone(),
            control_enter: calls.control_enter.clone(),
            now: calls.now.clone(),
            invoked: calls.invoked.clone(),
            running,
            pool,
            deadline: engine.deadline,
        })
    }

    /// What one invocation of `tool` is held to; `None` for a tool it does
    /// not export.
    pub fn budget(&self, tool: &str) -> Option<Budget> {
        self.tools.get(tool).map(|tool| tool.budget)
    }

    /// The tools this service exports, as its export table reports them.
    pub fn tools(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(String::as_str)
    }

    pub fn binding(&self, target: Target) -> Result<Binding> {
        let tool = self
            .manifest
            .tools
            .iter()
            .find(|tool| tool.name == target.tool())
            .context("workflow target has no declared tool")?;
        if matches!(target, Target::Service { .. }) {
            anyhow::ensure!(
                tool.public && self.reached_by_services(&tool.name),
                "workflow target does not take service calls"
            );
        }
        Ok(Binding {
            service: self.name.to_string(),
            image: digest::prefixed(&self.digest),
            input: tool.parameters.clone(),
            output: tool
                .returns
                .clone()
                .unwrap_or_else(|| serde_json::json!({"type":"null"})),
            budget: self
                .budget(&tool.name)
                .context("workflow target has no budget")?,
            query: tool.query,
            idempotency_ms: tool.idempotency_ms,
            authorization: digest::of(&serde_json::to_vec(&(tool.public, tool.callers))?),
            target,
        })
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
            public: _,
            deploy: _,
            crons: _,
            name: _,
            lang: _,
            imports: _,
            buffer: _,
            database: _,
            database_size: _,
            kits: _,
            deps,
            events: _,
            version,
            description,
            homepage,
            repository,
            readme,
            logo,
            tools,
            workflows,
        } = &self.manifest;
        berm_api::Service {
            name: name.to_owned(),
            version: version.clone(),
            description: description.clone(),
            homepage: homepage.clone(),
            repository: repository.clone(),
            readme: readme.clone(),
            logo: logo.clone(),
            digest: self.digest.clone(),
            tools: tools
                .iter()
                .filter(|tool| tool.public || self.manifest.deploy)
                .cloned()
                .collect(),
            workflows: workflows.iter().filter_map(|spec| spec.public()).collect(),
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
            workflows: self.manifest.workflows.clone(),
            deps: self.manifest.deps.clone(),
            events: self.manifest.events.clone(),
            lang: self.manifest.lang.clone(),
            imports: self.manifest.imports.clone(),
            unresolved: Default::default(),
            project: None,
        }
    }

    /// Run a registered control export with no application syscalls.
    pub async fn control(
        &self,
        request: control::Request<'_>,
    ) -> Result<abi::workflow::ControlOutput> {
        let deadline = request.deadline.min(Instant::now() + self.deadline);
        let ended = self
            .invoke(
                Entry::Control {
                    workflow: request.workflow,
                    node: request.node,
                },
                serde_json::to_vec(request.input)?,
                None,
                None,
                None,
                Client::default(),
                1,
                true,
                deadline,
                chain::next().into(),
                Arc::new(Journal::default()),
                request.meters,
                Arc::from([self.name.clone()]),
                None,
                None,
            )
            .await?;
        match ended.result {
            Output::Done(result) => {
                serde_json::from_str(&result).context("invalid workflow control result")
            }
            Output::Failed(message) => bail!("{message}"),
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
            Client::default(),
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
        key: Option<Arc<str>>,
        user: Option<User>,
        client: Client,
        tool: &str,
        args: impl Into<Vec<u8>>,
        meters: Arc<Meters>,
        chain: impl Into<chain::Id>,
        taping: Taping,
    ) -> Result<Outcome> {
        let journal = Arc::new(Journal::default());
        let Ended { result, denied } = self
            .invoke(
                Entry::Tool(tool),
                args.into(),
                caller.map(Caller::Outside),
                key,
                user,
                client,
                1,
                false,
                Instant::now() + self.deadline,
                chain.into(),
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
            key,
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
                Entry::Tool(tool),
                args.into(),
                caller,
                key,
                user,
                client,
                depth,
                query,
                deadline,
                chain::next().into(),
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
        entry: Entry<'_>,
        args: Vec<u8>,
        caller: Option<Caller>,
        key: Option<Arc<str>>,
        user: Option<User>,
        client: Client,
        depth: u32,
        query: bool,
        deadline: Instant,
        chain: chain::Id,
        journal: Arc<Journal>,
        meters: Arc<Meters>,
        stack: Arc<[Arc<str>]>,
        tape: Option<Deck>,
        parent: Option<&Callsite>,
    ) -> Result<Ended> {
        let (at_ms, began) = ((self.now)(), Instant::now());
        let (label, export, budget, declared, control) = match entry {
            Entry::Tool(tool) => {
                let Some(Tool { query, budget }) = self.tools.get(tool) else {
                    bail!("service exports no tool named {tool:?}");
                };
                (
                    tool.to_owned(),
                    format!("{}{tool}", abi::TOOL_PREFIX),
                    *budget,
                    *query,
                    false,
                )
            }
            Entry::Control { workflow, node } => {
                let spec = self
                    .manifest
                    .workflows
                    .iter()
                    .find(|spec| spec.entry.name == workflow)
                    .with_context(|| format!("service has no workflow {workflow:?}"))?;
                let node = spec
                    .entry
                    .graph
                    .nodes
                    .iter()
                    .find(|entry| entry.id == node)
                    .with_context(|| format!("workflow {workflow:?} has no node {node:?}"))?;
                (
                    format!("{workflow}/{}", node.id),
                    node.export.clone(),
                    self.controls[workflow],
                    true,
                    true,
                )
            }
        };
        let tool = label.as_str();

        let spend = parent
            .and_then(|parent| parent.workflow.clone())
            .or_else(workflow::current);
        let deadline = if let Some(context) = &spend {
            let left = context.deadline_at.saturating_sub((self.now)());
            if left == 0 {
                return Err(Failure { status: 409,headers: BTreeMap::new(),body: serde_json::json!({"error":"workflow attempt deadline expired","explain":SpendContext::EXPIRED}) }.into());
            }
            deadline.min(Instant::now() + Duration::from_millis(left))
        } else {
            deadline
        };
        let mut at = Callsite {
            service: self.name.clone(),
            tool: tool.into(),
            budget,
            control,
            workflow: spend,
            run: (!control && parent.is_none())
                .then(workflow::step_current)
                .flatten(),
            digest: self.digest.as_str().into(),
            deps: self.deps.clone(),
            caller,
            key,
            user,
            client,
            depth,
            stack,
            query: query || declared,
            deadline,
            chain: chain.number,
            record: parent.and_then(|at| at.record).or(chain.record),
            span: parent.map_or(0, |parent| parent.spans.fetch_add(1, Ordering::Relaxed)),
            parent: parent.map(|parent| parent.span),
            spans: parent.map_or_else(
                || Arc::new(AtomicU32::new(1)),
                |parent| parent.spans.clone(),
            ),
            journal: journal.clone(),
            meters,
            own: Arc::new(Own::new(budget.read, budget.written)),
            #[cfg(feature = "sqlite")]
            database: Arc::default(),
        };
        at.meter(Meter {
            runs: u64::from(depth == 1 && !control),
            calls: 1,
            ..Meter::default()
        });
        let mut scope = if control {
            match &self.control_enter {
                Some(enter) => Some(enter(self, &at).await?),
                None => None,
            }
        } else {
            Some((self.enter)(self, &at).await?)
        };
        let prepared: Result<_> = async {
            let image = if args.len() <= self.manifest.buffer {
                Some(self.image.executable(deadline).await?)
            } else {
                None
            };
            let reserved = Held::new(
                self.running.clone(),
                self.pool.clone(),
                self.reserve.clone(),
                budget.memory,
            )?;
            Ok((image, reserved))
        }
        .await;
        let (image, reserved) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                (self.invoked)(
                    &at,
                    tool,
                    Meter::default(),
                    Ran {
                        at: at_ms,
                        ms: began.elapsed().as_millis() as u64,
                        outcome: host::Outcome::Refused,
                    },
                );
                if let Some(scope) = &mut scope {
                    scope.complete().await?;
                }
                return Err(error);
            }
        };
        let own = scope.as_ref().is_some_and(|scope| scope.own());
        if own {
            at.journal = Arc::new(Journal::default());
        }
        let held = own.then(|| at.journal.clone());
        let mark = journal.mark();
        let site = at.clone();
        let buffer = self.manifest.buffer;
        let invocation = Invocation {
            buffer,
            control,
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
        let ended = match invocation.args.len() > buffer {
            true => Ok(Ended {
                result: Output::Failed("arguments exceeded the input buffer".to_owned()),
                denied: false,
            }),
            false => image
                .as_ref()
                .expect("input fits the buffer")
                .call(&export, invocation)
                .await
                .with_context(|| format!("service trapped in {tool}"))
                .and_then(|finished| ended(finished, buffer)),
        };
        if control
            && !matches!(
                &ended,
                Ok(Ended {
                    result: Output::Done(_),
                    ..
                })
            )
        {
            site.meter(Meter {
                failed: 1,
                ..Meter::default()
            });
        }
        if !control {
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
        }
        if control {
            (self.invoked)(
                &site,
                tool,
                site.own.take(),
                Ran {
                    at: at_ms,
                    ms: began.elapsed().as_millis() as u64,
                    outcome: match &ended {
                        Ok(ended) => host::Outcome::of(&ended.result, ended.denied),
                        Err(_) => host::Outcome::Failed,
                    },
                },
            );
        }
        if let Some(scope) = &mut scope {
            scope.complete().await?;
        }
        if let Some(scope) = scope {
            settle(&ended, scope, held, &journal, mark).await?;
        }
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
    pub key: Option<Arc<str>>,
    pub user: Option<User>,
    pub client: Client,
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
    pub(crate) control: bool,
    pub(crate) buffer: usize,
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
