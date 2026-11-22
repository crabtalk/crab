use crate::{
    Denied, deny,
    env::Peers,
    host,
    record::logs,
    state::{
        caller::call::{Call, Session},
        store::Store,
    },
};
use anyhow::{Context, Result};
use axum::http::StatusCode;
use berm::{Caller, Callsite, abi, bound::watchdog::Expired, database::Databases};
use berm_api::{
    Database as Migrations,
    host::User,
    storage::{Grant, GrantRequest, Identity, Key},
};
use futures_util::future::BoxFuture;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Weak},
};
use tempfile::NamedTempFile;
use tokio::{
    sync::{Mutex as AsyncMutex, RwLock},
    time,
};

pub(crate) mod call;
mod report;

tokio::task_local! {
    pub(crate) static ACCOUNT: Option<String>;
    pub(crate) static SCHEDULED: bool;
}

pub(crate) fn authenticated_account() -> Option<String> {
    ACCOUNT.try_with(Clone::clone).ok().flatten()
}

pub(crate) fn scheduled(at: &Callsite) -> bool {
    SCHEDULED.try_with(|scheduled| *scheduled).unwrap_or(false)
        || at
            .run
            .as_ref()
            .is_some_and(|run| matches!(run.trigger, abi::workflow::Trigger::Schedule { .. }))
}

pub(crate) fn account(at: &Callsite) -> Option<String> {
    let account = match &at.caller {
        Some(Caller::Outside(account)) => ACCOUNT
            .try_with(Clone::clone)
            .unwrap_or_else(|_| Some(account.to_string())),
        Some(Caller::Service(service)) => {
            let owner = host::split(service).0;
            if owner.is_empty() {
                ACCOUNT.try_with(Clone::clone).ok().flatten()
            } else {
                Some(owner.to_owned())
            }
        }
        None => None,
    };
    match at.run.as_ref().map(|run| &run.trigger) {
        Some(abi::workflow::Trigger::Manual { account, .. })
            if at.stack.first() == Some(&at.service) =>
        {
            ACCOUNT
                .try_with(Clone::clone)
                .unwrap_or_else(|_| Some(account.clone()))
        }
        Some(abi::workflow::Trigger::Schedule { .. }) => None,
        _ => account,
    }
}

type Invocation = (u64, u32);
type Allowance = dyn Fn(GrantRequest) -> BoxFuture<'static, Result<Option<Grant>>> + Send + Sync;

pub(crate) struct Storage {
    databases: Arc<Databases>,
    logs: Arc<logs::Pending>,
    now: berm::Now,
    allowance: Box<Allowance>,
    services: Mutex<HashMap<String, Weak<RwLock<()>>>>,
    files: Mutex<HashMap<Key, Weak<AsyncMutex<()>>>>,
    calls: Mutex<HashMap<Invocation, Arc<AsyncMutex<Call>>>>,
    store: Store,
}

impl Storage {
    pub(crate) fn new(
        databases: Arc<Databases>,
        store: Store,
        logs: Arc<logs::Pending>,
        now: berm::Now,
        peers: Arc<impl Peers>,
    ) -> Self {
        Self {
            databases,
            logs,
            now,
            allowance: Box::new(move |request| {
                let peers = peers.clone();
                Box::pin(async move { peers.caller_allowance(&request).await })
            }),
            services: Mutex::default(),
            files: Mutex::default(),
            calls: Mutex::default(),
            store,
        }
    }

    fn service(&self, id: &str) -> Arc<RwLock<()>> {
        let mut services = self.services.lock();
        services.retain(|_, lock| lock.strong_count() > 0);
        let entry = services.entry(id.to_owned()).or_default();
        let lock = entry.upgrade().unwrap_or_default();
        *entry = Arc::downgrade(&lock);
        lock
    }

    fn file(&self, key: &Key) -> Arc<AsyncMutex<()>> {
        let mut files = self.files.lock();
        files.retain(|_, lock| lock.strong_count() > 0);
        let entry = files.entry(key.clone()).or_default();
        let lock = entry.upgrade().unwrap_or_default();
        *entry = Arc::downgrade(&lock);
        lock
    }

    pub(crate) async fn enter(
        self: &Arc<Self>,
        at: &Callsite,
        id: Option<&str>,
        migrations: Migrations,
    ) -> Result<Session> {
        let id = id.context("caller storage has no service identity")?;
        validate(id)?;
        let key = account(at)
            .filter(|account| !account.is_empty())
            .map(|account| Key {
                storage_id: id.to_owned(),
                identity: Identity {
                    account,
                    user: at.user.as_ref().map(|user| User {
                        issuer: user.issuer.to_string(),
                        subject: user.subject.to_string(),
                    }),
                },
            });
        let deadline = time::Instant::from_std(at.deadline);
        let service = time::timeout_at(deadline, self.service(id).read_owned())
            .await
            .map_err(|_| Expired)?;
        anyhow::ensure!(
            !self.directory("retained", id).exists(),
            "service caller storage was retired"
        );
        let file = match &key {
            Some(key) => Some(
                time::timeout_at(deadline, self.file(key).lock_owned())
                    .await
                    .map_err(|_| Expired)?,
            ),
            None => None,
        };
        let invocation = (at.chain, at.span);
        let session = Session::new(
            self.clone(),
            invocation,
            key,
            migrations,
            (self.now)() as i64 / 1000,
            (service, file),
        );
        {
            let mut call = session.call.lock().await;
            if call
                .name
                .as_ref()
                .is_some_and(|name| self.databases.path(name).exists())
            {
                call.open(self, at, false).await?;
            }
        }
        Ok(session)
    }

    fn directory(&self, root: &str, id: &str) -> PathBuf {
        self.databases.root().join(root).join(id)
    }

    fn located(&self, key: &Key) -> String {
        let root = if self.directory("retained", &key.storage_id).exists() {
            "retained"
        } else {
            "callers"
        };
        name(root, key)
    }

    pub(crate) async fn retain(&self, id: &str) -> Result<()> {
        validate(id)?;
        let _service = self
            .service(id)
            .try_write_owned()
            .context("caller storage is in use")?;
        let from = self.directory("callers", id);
        let to = self.directory("retained", id);
        self.databases.close_under(&format!("callers/{id}"))?;
        if from.exists() {
            fs::create_dir_all(to.parent().expect("retained directory"))?;
            fs::rename(from, to)?;
        } else if !to.exists() {
            fs::create_dir_all(to)?;
        }
        Ok(())
    }

    pub(crate) async fn export(&self, id: &str) -> Result<NamedTempFile, Denied> {
        validate(id)?;
        let missing = || deny(StatusCode::NOT_FOUND, "No such caller storage");
        let key = self.key(id).await?.ok_or_else(missing)?;
        let _service = self.service(&key.storage_id).read_owned().await;
        let _file = self.file(&key).lock_owned().await;
        let caller_name = name("callers", &key);
        let name = self.located(&key);
        let Some((grant, _, _)) = self.file_state(&caller_name).await? else {
            return Err(missing());
        };
        if grant.id != id || !self.databases.path(&name).exists() {
            return Err(missing());
        }
        let file = NamedTempFile::new().map_err(anyhow::Error::from)?;
        self.databases
            .get_with_limit(&name, Some(grant.bytes))?
            .export(file.path().to_owned())
            .await?;
        Ok(file)
    }

    pub(crate) async fn delete(&self, id: &str) -> Result<()> {
        validate(id)?;
        let Some(key) = self.key(id).await? else {
            return Ok(());
        };
        let _service = self.service(&key.storage_id).read_owned().await;
        let _file = self.file(&key).lock_owned().await;
        let name = name("callers", &key);
        if self
            .file_state(&name)
            .await?
            .is_some_and(|(grant, _, _)| grant.id == id)
        {
            self.databases.remove(&self.located(&key))?;
            self.store
                .run(move |conn| conn.execute("DELETE FROM caller_files WHERE name = ?1", [name]))
                .await?;
        }
        Ok(())
    }

    fn call(&self, at: &Callsite) -> Result<Arc<AsyncMutex<Call>>> {
        self.calls
            .lock()
            .get(&(at.chain, at.span))
            .cloned()
            .with_context(|| {
                if scheduled(at) {
                    "scheduled runs have no caller storage"
                } else {
                    "this service has no caller storage declaration"
                }
            })
    }
}

fn validate(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id).context("invalid caller storage identity")?;
    Ok(())
}

fn name(root: &str, key: &Key) -> String {
    format!("{root}/{}/{}", key.storage_id, key.identity.digest())
}
