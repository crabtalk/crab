//! The services this host runs: one [`Berm`] holding every deployed service,
//! under the name its daemon gave it. A name's part before its last `/` is its
//! scope, and a service reaches services by name within its own.

use crate::{
    api::outside::Layers,
    config::Config,
    env::{Clock, Edges, Env, Images, Peers},
    record::{
        calls, logs,
        meter::{self, Meters as Pending},
        usage::{self, Counts},
    },
    state::{
        self, cap::Caps, commit::Commits, hold::Holds, routes::Routes, service, sql, store::Store,
        terms::Terms,
    },
    wake,
};
use anyhow::{Context, Result};
use berm::{
    Berm, Engine, Now, Program, User,
    database::{Database, Databases},
};
use berm_api::{
    Budget,
    host::{self, Used},
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicI32, Ordering},
    },
};
use tokio::sync::{Notify, RwLock, Semaphore};

pub(crate) use invoke::Invoked;

mod deploy;
mod fetch;
mod invoke;
mod nest;
mod storage;

/// Every service, compiled and callable.
#[derive(Clone)]
pub struct Host<E: Env>(Arc<Inner<E>>);

impl<E: Env> Deref for Host<E> {
    type Target = Inner<E>;

    fn deref(&self) -> &Inner<E> {
        &self.0
    }
}

/// What a [`Host`] holds. The syscalls it serves hold it as a [`Weak`].
pub struct Inner<E: Env> {
    /// Where the host keeps its state. A daemon may keep tables of its own
    /// here, named apart from the host's.
    pub store: Store,
    pub(crate) berm: Arc<Berm>,
    /// Chains in flight. Each holds up to `depth + 1` guests, and a guest past
    /// `Config::instances` fails to instantiate.
    pub(crate) gate: Semaphore,
    /// Chains the host may run at once: the permits [`Self::gate`] was made
    /// with.
    pub chains: usize,
    /// Chains each scope may run at once, beside [`Self::gate`].
    pub(crate) caps: Caps,
    /// Wakes the timer.
    pub(crate) waking: Notify,
    /// Uses not yet written.
    pub(crate) usage: Counts,
    /// Runs not yet written.
    pub(crate) calls: calls::Pending,
    /// Logs not yet written.
    pub(crate) logs: logs::Pending,
    /// Meters not yet written.
    pub(crate) meters: Pending,
    /// Held to write what is pending, and shared to read it beside what is
    /// written.
    pub(crate) flushing: RwLock<()>,
    /// Where the databases of the services that keep one live.
    pub(crate) databases_dir: PathBuf,
    /// The last day stored bytes were sampled, in days since the Unix epoch.
    sampled: AtomicI32,
    /// The databases of the services that keep one.
    pub(crate) databases: Arc<Databases>,
    /// Where a chain's changes are committed.
    pub(crate) commits: Commits,
    /// Where the services this host does not run live.
    pub(crate) routes: Routes,
    /// The scopes held.
    pub(crate) holds: Holds,
    pub(crate) peers: E::Peers,
    images: E::Images,
    pub(crate) secrets: E::Secrets,
    /// [`Edges::layers`].
    pub(crate) layers: Layers,
    /// The most one invocation may use, and what a tool's owner has not set
    /// takes.
    pub budget: Budget,
    /// Each tool's budget its owner set.
    pub(crate) terms: Terms,
    /// Held by a deploy of the service keyed by its key.
    deploying: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// [`Edges::clock`].
    pub(crate) now: Now,
}

impl<E: Env> Inner<E> {
    /// Chains in flight now, out of [`Self::chains`].
    pub fn running(&self) -> usize {
        self.chains - self.gate.available_permits()
    }

    /// The database of `program`, when it is deployed and keeps one.
    pub(crate) fn kept(&self, program: &str) -> Option<Result<Arc<Database>>> {
        self.berm
            .get(program)
            .filter(|program| keeps(program))
            .map(|_| self.databases.get(program))
    }
}

impl<E: Env> Host<E> {
    /// A host with nothing deployed, keeping its state in `state` and sized by
    /// `config`. It serves `berm.get` and `berm.set` over `service_state` or
    /// the service's own database, `berm.call.after` over `service_wakes`,
    /// `berm.sql` over the databases kept under `config.databases`,
    /// `berm.http.fetch` through `edges.fetch` and, when given one,
    /// `berm.http.proxy` through `edges.proxy`, both with `edges.secrets`.
    /// Images come from `edges.images`, a service it does not run is reached
    /// through `edges.peers`, a call from outside goes through `edges.layers`,
    /// and `edges.observe` is told of each invocation.
    pub async fn open(state: &Path, config: &Config, edges: Edges<E>) -> Result<Self> {
        let Edges {
            fetch,
            proxy,
            images,
            peers,
            secrets,
            layers,
            observe,
            clock,
        } = edges;
        let now: Now = Arc::new(move || clock.now());
        let store = Store::open(state)?;
        let routes = Routes::open(store.clone()).await?;
        let holds = Holds::open(store.clone()).await?;
        let engine = Engine::new(&berm::Config {
            cache_dir: config.cache_dir.clone(),
            instances: config.instances,
            memory: config.memory,
            fuel: config.fuel,
            rows: config.rows,
        })?;
        let databases = Arc::new(Databases::new(config.databases.clone()));
        let commits = Commits::new(store.clone(), databases.clone());
        let ran = calls::Pending::new(now.clone(), calls::newest(&store).await?);
        let terms = Terms::default();
        terms.load(&store).await?;
        let chains = (config.instances / (config.depth + 1)).max(1);
        Ok(Self(Arc::new_cyclic(|host: &Weak<Inner<E>>| {
            let mut syscalls = vec![fetch::syscall(host, fetch, false)];
            if let Some(proxy) = proxy {
                syscalls.push(fetch::syscall(host, proxy, true));
            }
            syscalls.extend(state::syscalls(host));
            syscalls.push(logs::syscall(host));
            syscalls.push(sql::syscall(host));
            syscalls.push(wake::syscall(host));
            let calls = nest::calls(host, config.depth, commits.clone(), observe, now.clone());
            let berm = Berm::new(&engine, calls, syscalls, Arc::new(Unrecorded));
            Inner {
                store,
                berm,
                gate: Semaphore::new(chains as usize),
                chains: chains as usize,
                caps: Caps::new(config.cap),
                waking: Notify::new(),
                usage: Counts::new(now.clone()),
                calls: ran,
                logs: logs::Pending::new(now.clone()),
                meters: Pending::new(now.clone()),
                flushing: RwLock::default(),
                databases_dir: config.databases.clone(),
                sampled: AtomicI32::default(),
                databases,
                commits,
                routes,
                holds,
                peers,
                images,
                secrets,
                layers,
                budget: Budget {
                    instructions: config.fuel,
                    rows: config.rows,
                },
                terms,
                deploying: Mutex::default(),
                now,
            }
        })))
    }

    /// Load every service this host runs off the request path, fire due wakes,
    /// and write counted uses.
    pub fn boot(&self) {
        let flushing = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(FLUSH).await;
                if let Err(e) = flushing.flush().await {
                    tracing::error!("writing service usage failed: {e:#}");
                }
            }
        });
        let loading = self.clone();
        tokio::spawn(async move { loading.reconcile().await });
        let waking = self.clone();
        tokio::spawn(async move { waking.wakes().await });
    }

    /// Load every service as `services` names it.
    async fn reconcile(&self) {
        let rows = match service::all(&self.store).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!("reading services failed: {e:#}");
                return;
            }
        };
        for row in rows {
            let key = join(&row.scope, &row.name);
            let size = row.size;
            match self.load(row).await {
                Ok(program) => {
                    tracing::debug!(service = key, digest = %program.digest, "loaded");
                    if keeps(&program)
                        && let Err(e) = async { self.databases.get(&key)?.limit(size).await }.await
                    {
                        tracing::error!(service = key, "limiting its database failed: {e:#}");
                    }
                }
                Err(e) => tracing::error!(service = key, "loading failed: {e:#}"),
            }
        }
    }

    /// The program a `services` row names, pulled and compiled unless it
    /// already runs that digest.
    async fn load(&self, row: service::Row) -> Result<Arc<Program>> {
        let key = join(&row.scope, &row.name);
        if let Some(program) = self.berm.get(&key)
            && row.digest.strip_prefix("sha256:") == Some(program.digest.as_str())
        {
            return Ok(program);
        }
        let image = self.images.pull(&row.reference, &row.digest).await?;
        self.compile(key, image).await
    }

    /// Write what was counted: uses of services this host runs here, and the
    /// rest to the peer each runs on.
    pub(crate) async fn flush(&self) -> Result<()> {
        let mut remote: HashMap<String, Vec<Used>> = HashMap::new();
        {
            let _writing = self.flushing.write().await;
            let mut local = Vec::new();
            for used in self.usage.take() {
                if self.berm.get(&used.service).is_some() {
                    local.push(used);
                } else if let Some(peer) = self.routes.peer(&used.service) {
                    remote.entry(peer).or_default().push(used);
                }
            }
            usage::write(&self.store, local).await?;
            calls::write(&self.store, self.calls.take(), (self.now)()).await?;
            logs::write(&self.store, self.logs.take_all(), (self.now)()).await?;
            meter::write(&self.store, self.meters.take()).await?;
        }
        for (peer, used) in remote {
            if let Err(e) = self.peers.ship(&peer, &used).await {
                tracing::warn!(peer, "handing over service usage failed: {e:#}");
            }
        }
        let today = self.today();
        if self.sampled.swap(today, Ordering::Relaxed) != today {
            for program in self.berm.list() {
                meter::sample(
                    &self.store,
                    &program.name,
                    &self.databases_dir.join(&*program.name),
                    program.bytes(),
                    today,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// `service`'s runs and logs not yet written. Read under
    /// [`Inner::flushing`].
    pub(crate) fn unsaved(&self, service: &str) -> calls::Unsaved {
        calls::Unsaved {
            ran: self.calls.unsaved(service),
            emitted: self.logs.unsaved(service),
        }
    }

    /// The program `service` runs.
    pub fn get(&self, service: &str) -> Option<Arc<Program>> {
        self.berm.get(service)
    }

    /// Every service in `scope` this host runs, by its name within it.
    pub fn list(&self, scope: &str) -> Vec<(String, Arc<Program>)> {
        self.berm
            .list()
            .into_iter()
            .filter_map(|program| {
                let (within, name) = split(&program.name);
                (within == scope).then(|| (name.to_owned(), program.clone()))
            })
            .collect()
    }

    /// Stop running `service` and drop what it committed, its wake and its
    /// usage. `false` when it was not deployed.
    // TODO: drop the database of a service that kept one; its file stays
    // under `Config::databases` and a deploy under the same name opens it.
    pub async fn remove(&self, service: &str) -> Result<bool> {
        let lock = self
            .deploying
            .lock()
            .entry(service.to_owned())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        let (scope, name) = split(service);
        let removed = service::remove(&self.store, scope, name).await?;
        self.berm.remove(service)?;
        self.terms.forget(service);
        Ok(removed)
    }

    /// Days since the Unix epoch, as the clock tells it.
    pub(crate) fn today(&self) -> i32 {
        (usage::seconds(&self.now) / 86_400) as i32
    }

    /// Compile an image into the runtime, off the async workers.
    async fn compile(&self, key: String, image: Vec<u8>) -> Result<Arc<Program>> {
        let berm = self.berm.clone();
        tokio::task::spawn_blocking(move || berm.deploy(&key, &image))
            .await
            .context("compilation panicked")?
    }
}

/// The host `host` points at, while it is open.
pub(crate) fn alive<E: Env>(host: &Weak<Inner<E>>) -> Result<Arc<Inner<E>>> {
    host.upgrade().context("the host is closed")
}

/// Whether `program` keeps a database.
pub(crate) fn keeps(program: &Program) -> bool {
    program.manifest().keeps_database()
}

/// How often counted uses are written.
const FLUSH: std::time::Duration = std::time::Duration::from_secs(60);

/// The service named `name` in `scope`.
pub(crate) fn join(scope: &str, name: &str) -> String {
    match scope {
        "" => name.to_owned(),
        scope => format!("{scope}/{name}"),
    }
}

/// The scope `service` is in, and its name within it: `a/b` is `b` in `a`,
/// and `b` is `b` in the empty scope.
pub(crate) fn split(service: &str) -> (&str, &str) {
    service.rsplit_once('/').unwrap_or(("", service))
}

/// The runtime's user for one the daemon handed in.
pub(crate) fn user(user: &host::User) -> User {
    User {
        issuer: user.issuer.as_str().into(),
        subject: user.subject.as_str().into(),
    }
}

/// The runtime's records, kept nowhere: `services` and the images are where a
/// deploy is recorded, and [`Host::boot`] restores from them.
struct Unrecorded;
