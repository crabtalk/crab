use crate::{Denied, deny, host, record::logs, storage::Repository};
use anyhow::{Context, Result};
use axum::http::StatusCode;
use berm::{
    Caller, Callsite, Log, Meter, Syscall, abi,
    bound::watchdog::Expired,
    database::{CallerDatabase, Databases, Held},
    syscall::{self, sql::Statement},
};
use berm_api::{
    Database, Sequence, digest,
    host::User,
    storage::{self, Identity, Key},
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
};
use tempfile::TempDir;
use tokio::{fs, sync::OwnedMutexGuard, time};

tokio::task_local! {
    pub(crate) static ACCOUNT: Option<String>;
    pub(crate) static SCHEDULED: bool;
}

pub(crate) fn scheduled(at: &Callsite) -> bool {
    SCHEDULED.try_with(|scheduled| *scheduled).unwrap_or(false)
        || at
            .run
            .as_ref()
            .is_some_and(|run| matches!(run.trigger, abi::workflow::Trigger::Schedule { .. }))
}

pub(crate) struct Storage {
    root: PathBuf,
    logs: Arc<logs::Pending>,
    pub(crate) repository: OnceLock<Arc<dyn Repository>>,
    writers: Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>,
}

impl Storage {
    pub(crate) fn new(root: PathBuf, logs: Arc<logs::Pending>) -> Self {
        Self {
            root,
            logs,
            repository: OnceLock::new(),
            writers: Mutex::default(),
        }
    }

    pub(crate) async fn enter(
        &self,
        at: &Callsite,
        service_id: Option<&str>,
        migrations: Database,
    ) -> Result<Session> {
        let service_id = service_id.context("caller storage has no service identity")?;
        let key = match &at.caller {
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
        let key = match at.run.as_ref().map(|run| &run.trigger) {
            Some(abi::workflow::Trigger::Manual { account, .. })
                if at.stack.first() == Some(&at.service) =>
            {
                ACCOUNT
                    .try_with(Clone::clone)
                    .unwrap_or_else(|_| Some(account.clone()))
            }
            Some(abi::workflow::Trigger::Schedule { .. }) => None,
            _ => key,
        };
        let key = key
            .filter(|account| !account.is_empty())
            .map(|account| Key {
                service: service_id.to_owned(),
                identity: Identity {
                    account,
                    user: at.user.as_ref().map(|user| User {
                        issuer: user.issuer.to_string(),
                        subject: user.subject.to_string(),
                    }),
                },
            });
        let (destination, writer) = match &key {
            Some(key) => {
                let key = digest::hex(&serde_json::to_vec(key)?);
                let writer = {
                    let mut writers = self.writers.lock();
                    writers.retain(|_, writer| writer.strong_count() > 0);
                    let entry = writers.entry(key.clone()).or_default();
                    let writer = entry.upgrade().unwrap_or_default();
                    *entry = Arc::downgrade(&writer);
                    writer
                };
                let writer =
                    time::timeout_at(time::Instant::from_std(at.deadline), writer.lock_owned())
                        .await
                        .map_err(|_| Expired)?;
                let path = self
                    .root
                    .join(digest::hex(service_id.as_bytes()))
                    .join(format!("{key}.sqlite"));
                (Some(path), Some(writer))
            }
            None => (None, None),
        };
        let scratch = tempfile::tempdir()?;
        let databases = Databases::new(scratch.path().to_owned());
        let repository = self.repository.get().cloned();
        let (mut snapshot, existing) = match (repository, key, destination) {
            (Some(repository), Some(key), _) => {
                let revision = repository.open(&key, &databases.path("storage")).await?;
                let existing = revision.is_some();
                (
                    Some(Snapshot::Remote {
                        repository,
                        key,
                        revision,
                    }),
                    existing,
                )
            }
            (_, _, Some(path)) => {
                let existing = match fs::copy(&path, databases.path("storage")).await {
                    Ok(_) => true,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                    Err(error) => return Err(error.into()),
                };
                (Some(Snapshot::Local(path)), existing)
            }
            _ => (None, false),
        };
        let database = databases.get("storage")?;
        let schema = migrations.migrations.len();
        let migrated = database
            .migrate(vec![Sequence {
                kit: String::new(),
                migrations: migrations.migrations,
            }])
            .await
            .map_err(|error| {
                self.logs.take(
                    at,
                    Log {
                        service: at.service.clone(),
                        topics: vec!["caller_storage".into()],
                        data: format!("{error:#}").chars().take(1000).collect(),
                    },
                );
                deny(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Caller storage migration failed",
                )
                .explained(storage::MIGRATION_FAILED)
            })?;
        if existing
            && !migrated.is_empty()
            && let Some(snapshot) = &mut snapshot
        {
            snapshot.publish(&database, schema, scratch.path()).await?;
        }
        let database = Arc::new(CallerDatabase::new(database, snapshot.is_some()));
        let held = database.enter(at).await?;
        assert!(at.caller_database.set(database.clone()).is_ok());
        Ok(Session {
            database,
            held,
            snapshot,
            schema,
            scratch,
            _writer: writer,
        })
    }
}

pub(crate) struct Session {
    database: Arc<CallerDatabase>,
    held: Held,
    snapshot: Option<Snapshot>,
    schema: usize,
    scratch: TempDir,
    _writer: Option<OwnedMutexGuard<()>>,
}

impl Session {
    pub(crate) fn complete(&self) -> Result<()> {
        if self.database.wrote() && !self.database.authenticated {
            return Err(anonymous().into());
        }
        Ok(())
    }

    pub(crate) async fn commit(mut self) -> Result<()> {
        self.held.commit().await?;
        if self.database.wrote()
            && let Some(snapshot) = &mut self.snapshot
        {
            snapshot
                .publish(&self.database.database, self.schema, self.scratch.path())
                .await?;
        }
        Ok(())
    }
}

enum Snapshot {
    Local(PathBuf),
    Remote {
        repository: Arc<dyn Repository>,
        key: Key,
        revision: Option<String>,
    },
}

impl Snapshot {
    async fn publish(
        &mut self,
        database: &berm::database::Database,
        schema: usize,
        scratch: &Path,
    ) -> Result<()> {
        match self {
            Self::Local(path) => {
                let dir = path.parent().context("caller storage path has no parent")?;
                fs::create_dir_all(dir).await?;
                let file = tempfile::NamedTempFile::new_in(dir)?;
                database.export(file.path().to_owned()).await?;
                file.as_file().sync_all()?;
                file.persist(path)?;
            }
            Self::Remote {
                repository,
                key,
                revision,
            } => {
                let file = tempfile::NamedTempFile::new_in(scratch)?;
                database.export(file.path().to_owned()).await?;
                *revision = Some(
                    repository
                        .commit(key, revision.as_deref(), schema, file.path())
                        .await?,
                );
            }
        }
        Ok(())
    }
}

fn anonymous() -> Denied {
    deny(
        StatusCode::FORBIDDEN,
        "Caller storage writes require a signed-in caller",
    )
    .explained(storage::ANONYMOUS)
}

fn database(at: &Callsite) -> Result<&Arc<CallerDatabase>> {
    at.caller_database.get().with_context(|| {
        if scheduled(at) {
            "scheduled runs have no caller storage"
        } else {
            "this service has no caller storage declaration"
        }
    })
}

pub(crate) fn syscalls() -> Vec<Syscall> {
    let mut syscalls = syscall::store::caller(
        |at: Callsite, key: String| async move { database(&at)?.database.get(at.chain, key).await },
        |at: Callsite, key: String, value: Vec<u8>| async move {
            let database = database(&at)?;
            database.mark_write();
            if !database.authenticated {
                return Err(anonymous().into());
            }
            database.database.set(at.chain, key, value).await?;
            Ok(true)
        },
    );
    syscalls.push(syscall::sql::services(
        abi::CALLER_SQL,
        |at: Callsite, statement: Statement| async move {
            let database = database(&at)?;
            let answer = if at.query {
                database.database.read(at.chain, statement).await?
            } else {
                database.database.exec(at.chain, statement).await?
            };
            if !answer.readonly {
                database.mark_write();
                if !database.authenticated {
                    return Err(anonymous().into());
                }
            }
            at.meter(Meter {
                read: answer.read,
                written: answer.written,
                sql_cpu: answer.cpu,
                ..Meter::default()
            });
            Ok(answer.json)
        },
    ));
    syscalls
}
