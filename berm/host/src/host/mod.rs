//! The services this host runs: one [`Berm`] holding every deployed service,
//! under the name its daemon gave it. A name's part before its last `/` is its
//! scope, and a service reaches services by name within its own.

use crate::{
    api::outside::{Bottom, Outside},
    config::Config,
    env::{Clock, Edges, Env, Images, Peers},
    host::fetch::Door,
    record::{
        calls,
        flight::Table,
        logs,
        meter::{self, Meters as Pending},
        usage::{self, Counts},
    },
    state::{
        self, commit::Commits, hold::Holds, pool::Pools, routes::Routes, service, sql, stop::Stops,
        store::Store,
    },
    wake,
};
use anyhow::{Context, Result};
use berm::{
    Berm, Engine, Now, Pool, Service, User,
    database::{Database, Databases},
};
use berm_api::{
    digest,
    host::{self, Used},
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ops::Deref,
    path::Path,
    sync::{
        Arc, Weak,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{self, Notify, Semaphore},
    task, time,
};
use tower::Layer;

pub use deploy::Installed;
pub(crate) use invoke::Invoked;

mod deploy;
mod fetch;
mod invoke;
mod nest;
mod set;

/// Every service, compiled and callable.
#[derive(Clone)]
pub struct Host<E: Env>(pub(crate) Arc<Inner<E>>);

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
    /// The memory each scope's services hold.
    pub(crate) pools: Arc<Pools>,
    /// The memory budgets its invocations running now reserve.
    pub(crate) reserve: Arc<Pool>,
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
    /// Held by a flush.
    flushing: sync::Mutex<()>,
    /// Held by a flush from writing what it took to dropping it from memory.
    #[cfg(feature = "test")]
    pub(crate) landing: sync::Mutex<()>,
    /// The last day stored bytes were sampled, in days since the Unix epoch.
    sampled: AtomicI32,
    /// The databases of the services that keep one.
    pub databases: Arc<Databases>,
    /// Where a chain's changes are committed.
    pub(crate) commits: Commits,
    /// Where the services this host does not run live.
    pub(crate) routes: Routes,
    /// The scopes held.
    pub(crate) holds: Holds,
    /// The services stopped.
    pub(crate) stops: Stops,
    pub(crate) peers: E::Peers,
    pub(crate) images: E::Images,
    pub(crate) secrets: E::Secrets,
    /// [`Edges::layers`] around the host's own service.
    pub(crate) outside: Outside,
    /// How long a chain of calls runs before it is stopped.
    pub(crate) deadline: Duration,
    /// Held by a deploy of the service keyed by its key.
    deploying: Mutex<HashMap<String, Arc<sync::Mutex<()>>>>,
    /// [`Edges::clock`].
    pub(crate) now: Now,
}

impl<E: Env> Inner<E> {
    /// Chains in flight now, out of [`Self::chains`].
    pub fn running(&self) -> usize {
        self.chains - self.gate.available_permits()
    }

    /// The database of `service`, when it is deployed and keeps one.
    pub(crate) fn kept(&self, service: &str) -> Option<Result<Arc<Database>>> {
        self.berm
            .get(service)
            .filter(|service| keeps(service))
            .map(|_| self.databases.get(service))
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
        let stops = Stops::open(store.clone()).await?;
        let engine = Engine::new(&berm::Config {
            cache_dir: config.cache_dir.clone(),
            instances: config.instances,
            memory: config.memory,
            reserved: config.reserved,
            cpu: config.cpu,
            read: config.read,
            written: config.written,
            budget: config.budget,
            deadline: config.deadline,
        })?;
        let databases = Arc::new(Databases::new(config.databases.clone()));
        let commits = Commits::new(store.clone(), databases.clone());
        let ran = calls::Pending::new(
            now.clone(),
            calls::newest(&store).await?,
            Table::Calls.last(&store).await?,
        );
        let used = Counts::new(now.clone(), Table::Usage.last(&store).await?);
        let emitted = logs::Pending::new(now.clone(), Table::Logs.last(&store).await?);
        let metered = Pending::new(now.clone(), Table::Meters.last(&store).await?);
        let chains = (config.instances / (config.depth + 1)).max(1);
        let pools = Arc::new(Pools::new(config.held));
        Ok(Self(Arc::new_cyclic(|host: &Weak<Inner<E>>| {
            let mut syscalls = vec![
                fetch::syscall(host, fetch.clone(), Door::Fetch),
                fetch::syscall(host, fetch, Door::FetchAll),
            ];
            if let Some(proxy) = proxy {
                syscalls.push(fetch::syscall(host, proxy.clone(), Door::Proxy));
                syscalls.push(fetch::syscall(host, proxy, Door::ProxyAll));
            }
            syscalls.extend(state::syscalls(host));
            syscalls.push(logs::syscall(host));
            syscalls.push(sql::syscall(host));
            syscalls.push(wake::syscall(host));
            let calls = nest::calls(
                host,
                config.depth,
                commits.clone(),
                observe,
                now.clone(),
                pools.clone(),
            );
            let berm = Berm::new(&engine, calls, syscalls);
            Inner {
                store,
                berm,
                gate: Semaphore::new(chains as usize),
                chains: chains as usize,
                pools,
                reserve: engine.reserve.clone(),
                waking: Notify::new(),
                usage: used,
                calls: ran,
                logs: emitted,
                meters: metered,
                flushing: sync::Mutex::default(),
                #[cfg(feature = "test")]
                landing: sync::Mutex::default(),
                sampled: AtomicI32::default(),
                databases,
                commits,
                routes,
                holds,
                stops,
                peers,
                images,
                secrets,
                outside: layers.layer(Outside::new(Bottom(host.clone()))),
                deadline: config.deadline,
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
                time::sleep(FLUSH).await;
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
    pub(crate) async fn reconcile(&self) {
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
                Ok(service) => {
                    tracing::debug!(service = key, digest = %service.digest, "loaded");
                    if keeps(&service)
                        && let Err(e) = async { self.databases.get(&key)?.limit(size).await }.await
                    {
                        tracing::error!(service = key, "limiting its database failed: {e:#}");
                    }
                }
                Err(e) => tracing::error!(service = key, "loading failed: {e:#}"),
            }
        }
    }

    /// The service a `services` row names, pulled and compiled unless it
    /// already runs that digest.
    async fn load(&self, row: service::Row) -> Result<Arc<Service>> {
        let key = join(&row.scope, &row.name);
        if let Some(service) = self.berm.get(&key)
            && row.digest.strip_prefix(digest::PREFIX) == Some(service.digest.as_str())
        {
            return Ok(service);
        }
        let image = self.images.pull(&row.reference, &row.digest).await?;
        self.compile(key, image).await
    }

    /// Write what was counted: uses of services this host runs here, and the
    /// rest to the peer each runs on.
    pub(crate) async fn flush(&self) -> Result<()> {
        let _flushing = self.flushing.lock().await;
        let (used, rest) = self
            .usage
            .take(|used| self.berm.get(&used.service).is_some());
        let mut remote: HashMap<String, Vec<Used>> = HashMap::new();
        for used in rest {
            if let Some(peer) = self.routes.peer(&used.service) {
                remote.entry(peer).or_default().push(used);
            }
        }
        // Every log of a run taken is taken after it, and written before.
        let ran = self.calls.take();
        let emitted = self.logs.take_all();
        let metered = self.meters.take();
        let generations = (
            used.generation,
            ran.generation,
            emitted.generation,
            metered.generation,
        );
        let now = (self.now)();
        let written = [
            usage::write(&self.store, used).await,
            logs::write(&self.store, emitted, now).await,
            calls::write(&self.store, ran, now).await,
            meter::write(&self.store, metered).await,
        ];
        #[cfg(feature = "test")]
        let _landing = self.landing.lock().await;
        self.usage.land(generations.0);
        self.calls.land(generations.1);
        self.logs.land(generations.2);
        self.meters.land(generations.3);
        written.into_iter().collect::<Result<()>>()?;
        for (peer, used) in remote {
            if let Err(e) = self.peers.ship(&peer, &used).await {
                tracing::warn!(peer, "handing over service usage failed: {e:#}");
            }
        }
        let today = self.today();
        if self.sampled.swap(today, Ordering::Relaxed) != today {
            for service in self.berm.list() {
                meter::sample(
                    &self.store,
                    &service.name,
                    &self.databases,
                    service.bytes,
                    today,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// `service`'s runs and logs not yet written.
    pub(crate) fn unsaved(&self, service: &str) -> calls::Unsaved {
        calls::Unsaved {
            ran: self.calls.unsaved(service),
            emitted: self.logs.unsaved(service),
        }
    }

    /// The service `service` runs.
    pub fn get(&self, service: &str) -> Option<Arc<Service>> {
        self.berm.get(service)
    }

    /// Every service in `scope` this host runs, by its name within it.
    pub fn list(&self, scope: &str) -> Vec<(String, Arc<Service>)> {
        self.berm
            .list()
            .into_iter()
            .filter_map(|service| {
                let (within, name) = split(&service.name);
                (within == scope).then(|| (name.to_owned(), service.clone()))
            })
            .collect()
    }

    /// Stop running `service` and drop what it committed, its database, its
    /// wake and its usage. `false` when it was not deployed.
    pub async fn remove(&self, service: &str) -> Result<bool> {
        let lock = self
            .deploying
            .lock()
            .entry(service.to_owned())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        let (scope, name) = split(service);
        self.berm.remove(service);
        self.databases.remove(service)?;
        let removed = service::remove(&self.store, scope, name).await?;
        self.stops.mark(service, false);
        Ok(removed)
    }

    /// Stop `service`: it takes no calls and fires no wakes until
    /// [`Self::start`], and calls in flight finish. What it stored and what
    /// comes due stays. `false` when it is not deployed.
    pub async fn stop(&self, service: &str) -> Result<bool> {
        self.switch(service, false).await
    }

    /// Start `service` stopped by [`Self::stop`] or deployed stopped; wakes
    /// that came due meanwhile fire. `false` when it is not deployed.
    pub async fn start(&self, service: &str) -> Result<bool> {
        self.switch(service, true).await
    }

    /// Whether `service` is stopped.
    pub fn stopped(&self, service: &str) -> bool {
        self.stops.stopped(service)
    }

    async fn switch(&self, service: &str, running: bool) -> Result<bool> {
        let lock = self
            .deploying
            .lock()
            .entry(service.to_owned())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        let set = self.stops.set(service, !running).await?;
        if set && running {
            self.waking.notify_one();
        }
        Ok(set)
    }

    /// Days since the Unix epoch, as the clock tells it.
    pub(crate) fn today(&self) -> i32 {
        (usage::seconds(&self.now) / 86_400) as i32
    }

    /// Compile an image into the runtime, off the async workers.
    async fn compile(&self, key: String, image: Vec<u8>) -> Result<Arc<Service>> {
        let berm = self.berm.clone();
        task::spawn_blocking(move || berm.deploy(&key, &image))
            .await
            .context("compilation panicked")?
    }
}

/// The host `host` points at, while it is open.
pub(crate) fn alive<E: Env>(host: &Weak<Inner<E>>) -> Result<Arc<Inner<E>>> {
    host.upgrade().context("the host is closed")
}

/// Whether `service` keeps a database.
pub(crate) fn keeps(service: &Service) -> bool {
    service.manifest.keeps_database()
}

/// How often counted uses are written.
const FLUSH: Duration = Duration::from_secs(60);

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
