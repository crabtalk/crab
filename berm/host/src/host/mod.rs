//! The services this host runs: one [`Berm`] holding every deployed service,
//! under the name its daemon gave it. A name's part before its last `/` is its
//! scope. Qualified calls cross scopes through the daemon's account gateway.

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
        self, caller, commit::Commits, hold::Holds, pool::Pools, routes::Routes, service, sql,
        stop::Stops, store::Store,
    },
    wake,
    workflow::{Eligibility, Ledger},
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
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::{
    sync::{self, Notify, Semaphore},
    task, time,
};
use tower::Layer;

pub use deploy::Installed;

mod control;
mod deploy;
mod export;
mod fetch;
mod nest;
mod secrets;
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
    pub workflows: Ledger,
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
    pub(crate) scheduling: Notify,
    /// Uses not yet written.
    pub(crate) usage: Counts,
    /// Runs not yet written.
    pub(crate) calls: calls::Pending,
    /// Logs not yet written.
    pub(crate) logs: Arc<logs::Pending>,
    /// Meters not yet written.
    pub(crate) meters: Pending,
    /// Held by a flush.
    flushing: sync::Mutex<()>,
    /// Held by a flush from writing what it took to dropping it from memory.
    #[cfg(feature = "test")]
    pub(crate) landing: sync::Mutex<()>,
    /// The databases of the services that keep one.
    pub databases: Arc<Databases>,
    /// Where a chain's changes are committed.
    pub(crate) commits: Commits,
    pub(crate) caller_storage: Arc<caller::Storage>,
    /// Where the services this host does not run live.
    pub(crate) routes: Routes,
    /// The scopes held.
    pub(crate) holds: Holds,
    /// The services stopped.
    pub(crate) stops: Stops,
    pub(crate) peers: Arc<E::Peers>,
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
    /// and outbound HTTP through `edges.fetch` with `edges.secrets`.
    /// Images come from `edges.images`, a service it does not run is reached
    /// through `edges.peers`, a call from outside goes through `edges.layers`,
    /// and `edges.observe` is told of each invocation.
    pub async fn open(state: &Path, config: &Config, edges: Edges<E>) -> Result<Self> {
        let Edges {
            fetch,
            images,
            peers,
            secrets,
            layers,
            enter,
            observe,
            clock,
        } = edges;
        let now: Now = Arc::new(move || clock.now());
        let store = Store::open(state)?;
        let workflows = Ledger::open(&state.with_extension("workflows.sqlite3"))?;
        workflows.recover(now()).await?;
        let routes = Routes::open(store.clone()).await?;
        let holds = Holds::open(store.clone()).await?;
        let stops = Stops::open(store.clone()).await?;
        let engine = Engine::new(&berm::Config {
            cache_dir: Some(
                config
                    .cache_dir
                    .clone()
                    .unwrap_or_else(|| state.parent().unwrap_or(Path::new(".")).join("cache")),
            ),
            code_cache: config.code_cache,
            code_idle: config.code_idle,
            instances: config.instances,
            memory: config.memory,
            reserved: config.reserved,
            cpu: config.cpu,
            read: config.read,
            written: config.written,
            budget: config.budget,
            deadline: config.deadline,
        })?;
        let mut databases = Databases::new(config.databases.clone());
        databases.commit_hook = config.commit_hook.clone();
        let databases = Arc::new(databases);

        let ran = calls::Pending::new(
            now.clone(),
            calls::newest(&store).await?,
            Table::Calls.last(&store).await?,
        );
        let used = Counts::new(now.clone(), Table::Usage.last(&store).await?);
        let emitted = Arc::new(logs::Pending::new(
            now.clone(),
            Table::Logs.last(&store).await?,
        ));
        let peers = Arc::new(peers);
        let caller_storage = Arc::new(caller::Storage::new(
            databases.clone(),
            store.clone(),
            emitted.clone(),
            now.clone(),
            peers.clone(),
        ));
        let commits = Commits::new(store.clone(), databases.clone(), caller_storage.clone());
        let metered = Pending::new(now.clone(), Table::Meters.last(&store).await?);
        let chains = (config.instances / (config.depth + 1)).max(1);
        let pools = Arc::new(Pools::new(config.held));
        Ok(Self(Arc::new_cyclic(|host: &Weak<Inner<E>>| {
            let mut syscalls = vec![
                fetch::syscall(host, fetch.clone(), Door::Fetch),
                fetch::syscall(host, fetch.clone(), Door::Rotating),
                fetch::syscall(host, fetch.clone(), Door::FetchAll),
                fetch::syscall(host, fetch, Door::FetchAllLiteralBody),
            ];
            syscalls.extend(state::syscalls(host));
            syscalls.extend(caller::call::syscalls(host));
            syscalls.push(logs::syscall(host));
            syscalls.push(sql::syscall(host));
            syscalls.extend(secrets::syscalls(host));
            syscalls.push(wake::syscall(host));
            let calls = nest::calls(
                host,
                config.depth,
                commits.clone(),
                enter,
                observe,
                now.clone(),
                pools.clone(),
            );
            let berm = Berm::new(&engine, calls, syscalls);
            Inner {
                store,
                workflows,
                berm,
                gate: Semaphore::new(chains as usize),
                chains: chains as usize,
                pools,
                reserve: engine.reserve.clone(),
                waking: Notify::new(),
                scheduling: Notify::new(),
                usage: used,
                calls: ran,
                logs: emitted,
                meters: metered,
                flushing: sync::Mutex::default(),
                #[cfg(feature = "test")]
                landing: sync::Mutex::default(),
                databases,
                commits,
                caller_storage,
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

    /// Restore deployment metadata, fire due wakes and write counted uses.
    /// Rows without a recorded code footprint compile once during restoration.
    pub fn boot(&self) {
        let flushing = self.clone();
        tokio::spawn(async move {
            let mut interval = time::interval_at(time::Instant::now() + FLUSH, FLUSH);
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = interval.tick() => {}
                    _ = flushing.calls.batch_ready.notified() => {}
                }
                if let Err(error) = flushing.flush_records().await {
                    tracing::error!(%error, "writing local service records failed");
                }
            }
        });
        let maintaining = self.clone();
        tokio::spawn(async move {
            loop {
                time::sleep(MAINTENANCE).await;
                if let Err(error) = maintaining.maintain().await {
                    tracing::error!(%error, "maintaining service records failed");
                }
            }
        });
        let loading = self.clone();
        tokio::spawn(async move { loading.reconcile().await });
        let waking = self.clone();
        tokio::spawn(async move { waking.wakes().await });
        let scheduled = self.clone();
        tokio::spawn(async move { scheduled.workflow_timer().await });
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
            let lock = self
                .deploying
                .lock()
                .entry(key.clone())
                .or_default()
                .clone();
            let _deploying = lock.lock().await;
            let row = match service::row(&self.store, &row.scope, &row.name).await {
                Ok(Some(row)) => row,
                Ok(None) => continue,
                Err(error) => {
                    tracing::error!(service = key, %error, "reading deployment failed");
                    continue;
                }
            };
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

    /// Restore one deployment, compiling only legacy rows without a footprint.
    async fn load(&self, row: service::Row) -> Result<Arc<Service>> {
        let key = join(&row.scope, &row.name);
        if let Some(service) = self.berm.get(&key)
            && row.digest.strip_prefix(digest::PREFIX) == Some(service.digest.as_str())
        {
            return Ok(service);
        }
        let image = self.images.pull(&row.reference, &row.digest).await?;
        let berm = self.berm.clone();
        let restored = task::spawn_blocking(move || berm.restore(&key, &image, row.code_bytes))
            .await
            .context("restoring a deployment panicked")??;
        let _ = restored.storage_id.set(row.storage_id);
        if row.code_bytes.is_none() {
            service::footprint(&self.store, &row.scope, &row.name, restored.bytes).await?;
        }
        Ok(restored)
    }

    #[cfg(feature = "test")]
    pub(crate) async fn flush(&self) -> Result<()> {
        self.flush_records().await?;
        self.maintain().await
    }

    async fn flush_records(&self) -> Result<()> {
        let _flushing = self.flushing.lock().await;
        let used = self
            .usage
            .take_local(|service| self.berm.get(service).is_some());
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
        let written = [
            usage::write(&self.store, used).await,
            logs::write(&self.store, emitted).await,
            calls::write(&self.store, ran).await,
            meter::write(&self.store, metered).await,
        ];
        #[cfg(feature = "test")]
        let _landing = self.landing.lock().await;
        self.usage.land(generations.0);
        self.calls.land(generations.1);
        self.logs.land(generations.2);
        self.meters.land(generations.3);
        written.into_iter().collect()
    }

    async fn maintain(&self) -> Result<()> {
        let now = (self.now)();
        self.workflows.expire(now).await?;
        logs::prune(&self.store, now).await?;
        calls::prune(&self.store, now).await?;
        let mut remote: HashMap<String, Vec<Used>> = HashMap::new();
        for used in self
            .usage
            .take_remote(|service| self.berm.get(service).is_some())
        {
            if let Some(peer) = self.routes.peer(&used.service) {
                remote.entry(peer).or_default().push(used);
            }
        }
        for (peer, used) in remote {
            if let Err(e) = self.peers.ship(&peer, &used).await {
                tracing::warn!(peer, "handing over service usage failed: {e:#}");
            }
        }
        let reports = self.caller_storage.pending().await?;
        for batch in reports.chunks(128) {
            let reports = batch
                .iter()
                .map(|pending| pending.report.clone())
                .collect::<Vec<_>>();
            match self.peers.caller_usage(&reports).await {
                Ok(grants) => self.caller_storage.reported(batch, grants).await?,
                Err(error) => {
                    tracing::warn!(%error, "reporting caller storage usage failed");
                    break;
                }
            }
        }
        for service in self.berm.list() {
            self.sample(&service.name).await?;
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

    /// The service keys and storage identities recorded on this host.
    pub async fn database_inventory(&self) -> Result<Vec<(String, String)>> {
        Ok(service::all(&self.store)
            .await?
            .into_iter()
            .map(|row| (join(&row.scope, &row.name), row.storage_id))
            .collect())
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

    /// Remove the service and its shared state. Its caller files remain
    /// available for caller export and deletion.
    pub async fn remove(&self, service: &str) -> Result<bool> {
        let lock = self
            .deploying
            .lock()
            .entry(service.to_owned())
            .or_default()
            .clone();
        let _deploying = lock.lock().await;
        let (scope, name) = split(service);
        self.workflows.invalidate(service).await?;
        self.berm.remove(service);
        self.databases.remove(service)?;
        if let Some(row) = service::row(&self.store, scope, name).await? {
            self.caller_storage.retain(&row.storage_id).await?;
        }
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

    pub async fn workflow_eligibility(&self, id: &str, now: u64) -> Result<Eligibility> {
        let run = self
            .workflows
            .get(id)
            .await?
            .context("workflow run was not found")?;
        if self.get(&run.service).is_none() {
            return Ok(Eligibility::Removed);
        }
        if self.stopped(&run.service) {
            return Ok(Eligibility::Stopped);
        }
        self.workflows.checkpoint_eligibility(id, now).await
    }

    /// Whether `service` is stopped.
    pub fn stopped(&self, service: &str) -> bool {
        self.stops.stopped(service)
    }

    pub(crate) async fn switch(&self, service: &str, running: bool) -> Result<bool> {
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

const FLUSH: Duration = Duration::from_secs(1);
const MAINTENANCE: Duration = Duration::from_secs(60);

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
