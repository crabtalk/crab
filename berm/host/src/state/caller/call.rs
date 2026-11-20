use crate::{
    Denied, deny,
    env::Env,
    host::Inner,
    state::caller::{Invocation, Storage, name},
};
use anyhow::{Context, Result};
use axum::http::StatusCode;
use berm::{
    Callsite, Log, Meter, Syscall, abi,
    bound::watchdog::Expired,
    database::{Database, Held},
    syscall::{self, sql::Statement},
};
use berm_api::{
    Database as Migrations, Sequence,
    storage::{self, Grant, GrantRequest, Key},
};
use rusqlite::ErrorCode;
use std::sync::{Arc, Weak};
use tokio::{
    sync::{Mutex as AsyncMutex, OwnedMutexGuard, OwnedRwLockReadGuard},
    time,
};

type Guards = (OwnedRwLockReadGuard<()>, Option<OwnedMutexGuard<()>>);

pub(crate) struct Session {
    storage: Arc<Storage>,
    invocation: Invocation,
    pub(super) call: Arc<AsyncMutex<Call>>,
    guards: Option<Guards>,
    committed: bool,
}

impl Session {
    pub(super) fn new(
        storage: Arc<Storage>,
        invocation: Invocation,
        key: Option<Key>,
        migrations: Migrations,
        called_at: i64,
        guards: Guards,
    ) -> Self {
        let call = Arc::new(AsyncMutex::new(Call {
            name: key.as_ref().map(|key| name("callers", key)),
            key,
            database: None,
            grant: None,
            held: None,
            migrations: vec![Sequence {
                kit: String::new(),
                migrations: migrations.migrations,
            }],
            wrote: false,
            created: false,
            failure: None,
            called_at,
        }));
        storage.calls.lock().insert(invocation, call.clone());
        Self {
            storage,
            invocation,
            call,
            guards: Some(guards),
            committed: false,
        }
    }

    pub(crate) async fn complete(&self) -> Result<()> {
        if let Some(error) = self.call.lock().await.failure.take() {
            return Err(error.into());
        }
        Ok(())
    }

    pub(crate) async fn commit(mut self) -> Result<()> {
        {
            let mut call = self.call.lock().await;
            if let Some(held) = call.held.take() {
                held.commit().await?;
            }
        }
        self.committed = true;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.storage.calls.lock().remove(&self.invocation);
        if self.committed {
            return;
        }
        let storage = self.storage.clone();
        let call = self.call.clone();
        let guards = self.guards.take();
        tokio::spawn(async move {
            let _guards = guards;
            let mut call = call.lock().await;
            call.held.take();
            call.database.take();
            if call.created
                && let Some(name) = &call.name
                && let Err(error) = storage.databases.remove(name)
            {
                tracing::warn!(%error, "removing uncommitted caller storage failed");
            }
        });
    }
}

pub(super) struct Call {
    pub(super) name: Option<String>,
    key: Option<Key>,
    database: Option<Arc<Database>>,
    grant: Option<Grant>,
    held: Option<Held>,
    migrations: Vec<Sequence>,
    wrote: bool,
    created: bool,
    failure: Option<Denied>,
    called_at: i64,
}

impl Call {
    async fn allowance(
        &mut self,
        storage: &Storage,
        at: &Callsite,
        size: u64,
        grow: bool,
    ) -> Result<()> {
        let key = self.key.clone().context("caller storage has no caller")?;
        let request = GrantRequest { key, size, grow };
        self.grant = time::timeout_at(
            time::Instant::from_std(at.deadline),
            (storage.allowance)(request.clone()),
        )
        .await
        .map_err(|_| Expired)?
        .map_err(Denied::from)?;
        if let Some(grant) = &self.grant {
            storage.track(&request.key, grant, self.called_at).await?;
        }
        Ok(())
    }

    pub(super) async fn open(
        &mut self,
        storage: &Storage,
        at: &Callsite,
        create: bool,
    ) -> Result<()> {
        let name = self
            .name
            .clone()
            .context("caller storage has no identity")?;
        let database = if create {
            self.allowance(storage, at, 0, false).await?;
            self.check_write()?;
            self.created = true;
            let cap = self.grant.as_ref().map(|grant| grant.bytes);
            match storage.databases.get_with_limit(&name, cap) {
                Ok(database) => database,
                Err(error) if full(&error) && self.grant.is_some() => {
                    self.allowance(storage, at, 0, true).await?;
                    self.check_write()?;
                    storage
                        .databases
                        .get_with_limit(&name, self.grant.as_ref().map(|grant| grant.bytes))
                        .map_err(|error| self.limit_on_full(error))?
                }
                Err(error) => return Err(error),
            }
        } else {
            self.grant = storage.called(&name, self.called_at).await?;
            let cap = self.grant.as_ref().map(|grant| grant.bytes);
            let database = storage.databases.get_with_limit(&name, cap)?;
            if let Some(bytes) = cap {
                database.limit(bytes).await?;
            }
            database
        };
        self.database = Some(database.clone());
        if let Some(grant) = &self.grant
            && !grant.writable
        {
            return Ok(());
        }
        self.migrate(storage, at, &database, true).await
    }

    async fn memory(&mut self, storage: &Storage, at: &Callsite) -> Result<Arc<Database>> {
        if let Some(database) = &self.database {
            return Ok(database.clone());
        }
        let database = storage.databases.memory()?;
        self.migrate(storage, at, &database, false).await?;
        self.database = Some(database.clone());
        Ok(database)
    }

    async fn migrate(
        &mut self,
        storage: &Storage,
        at: &Callsite,
        database: &Database,
        persistent: bool,
    ) -> Result<()> {
        let mut migrated = database.migrate(self.migrations.clone()).await;
        if persistent && migrated.as_ref().is_err_and(full) && self.grant.is_some() {
            self.grow(storage, at, database).await?;
            migrated = database.migrate(self.migrations.clone()).await;
        }
        if let Err(error) = migrated {
            if persistent && self.grant.is_some() && full(&error) {
                return Err(self.limit_on_full(error));
            }
            storage.logs.take(
                at,
                Log {
                    service: at.service.clone(),
                    topics: vec!["caller_storage".into()],
                    data: format!("{error:#}").chars().take(1000).collect(),
                },
            );
            return Err(deny(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Caller storage migration failed",
            )
            .explained(storage::MIGRATION_FAILED)
            .into());
        }
        Ok(())
    }

    async fn writer(&mut self, storage: &Storage, at: &Callsite) -> Result<Arc<Database>> {
        if let Some(error) = &self.failure {
            return Err(error.clone().into());
        }
        if self.key.is_none() {
            return Err(deny(
                StatusCode::FORBIDDEN,
                "Caller storage writes require a signed-in caller",
            )
            .explained(storage::ANONYMOUS)
            .into());
        }
        let name = self
            .name
            .as_ref()
            .context("caller storage has no identity")?;
        if !storage.databases.path(name).exists() {
            self.open(storage, at, true).await?;
        }
        self.check_write()?;
        let database = self
            .database
            .clone()
            .context("caller database was not opened")?;
        if self.held.is_none() {
            self.held = Some(Held::write(&database, at).await?);
        }
        Ok(database)
    }

    fn check_write(&self) -> Result<()> {
        if let Some(grant) = &self.grant
            && (!grant.writable || grant.bytes == 0)
        {
            return Err(limit(grant).into());
        }
        Ok(())
    }

    async fn grow(&mut self, storage: &Storage, at: &Callsite, database: &Database) -> Result<()> {
        let before = self
            .grant
            .as_ref()
            .context("caller storage has no grant")?
            .bytes;
        let size = database.usage(self.called_at).await?.used;
        self.allowance(storage, at, size, true).await?;
        let grant = self.grant.as_ref().context("caller storage has no grant")?;
        if !grant.writable || grant.bytes <= before {
            return Err(limit(grant).into());
        }
        database.limit(grant.bytes).await?;
        Ok(())
    }

    async fn retry(&mut self, storage: &Storage, at: &Callsite, database: &Database) -> Result<()> {
        self.grow(storage, at, database).await?;
        // SQLITE_FULL can roll back earlier statements in the same transaction.
        if !database.write_open(at.chain).await? {
            if self.wrote {
                return Err(limit(self.grant.as_ref().expect("growth returned a grant")).into());
            }
            database.restart_write(at.chain).await?;
        }
        Ok(())
    }

    fn limit_on_full(&self, error: anyhow::Error) -> anyhow::Error {
        match &self.grant {
            Some(grant) if full(&error) => limit(grant).into(),
            _ => error,
        }
    }

    fn remember<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(error) = &result
            && let Some(denied) = error.downcast_ref::<Denied>()
        {
            self.failure = Some(denied.clone());
        }
        result
    }
}

fn full(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<rusqlite::Error>()
        .and_then(rusqlite::Error::sqlite_error_code)
        == Some(ErrorCode::DiskFull)
}

fn limit(grant: &Grant) -> Denied {
    deny(
        StatusCode::CONFLICT,
        format!("Caller storage reached its allowance for this call. The account's plan limit is {} bytes. Open Storage to manage it.", grant.limit),
    ).explained(storage::LIMIT)
}

pub(crate) fn syscalls<E: Env>(host: &Weak<Inner<E>>) -> Vec<Syscall> {
    let (getting, setting, sql) = (host.clone(), host.clone(), host.clone());
    let mut syscalls = syscall::store::caller(
        move |at: Callsite, key: String| {
            let host = getting.upgrade();
            async move {
                let host = host.context("host went away")?;
                let call = host.caller_storage.call(&at)?;
                let call = call.lock().await;
                let Some(database) = &call.database else {
                    return Ok(None);
                };
                database.get(at.chain, key).await
            }
        },
        move |at: Callsite, key: String, value: Vec<u8>| {
            let host = setting.upgrade();
            async move {
                let host = host.context("host went away")?;
                let storage = &host.caller_storage;
                let call = storage.call(&at)?;
                let mut call = call.lock().await;
                let result = async {
                    let database = call.writer(storage, &at).await?;
                    let mut result = database.set(at.chain, key.clone(), value.clone()).await;
                    if result.as_ref().is_err_and(full) && call.grant.is_some() {
                        call.retry(storage, &at, &database).await?;
                        result = database.set(at.chain, key, value).await;
                    }
                    result.map_err(|error| call.limit_on_full(error))?;
                    call.wrote = true;
                    Ok(true)
                }
                .await;
                call.remember(result)
            }
        },
    );
    syscalls.push(syscall::sql::services(
        abi::CALLER_SQL,
        move |at: Callsite, statement: Statement| {
            let host = sql.upgrade();
            async move {
                let host = host.context("host went away")?;
                let storage = &host.caller_storage;
                let call = storage.call(&at)?;
                let mut call = call.lock().await;
                let result = async {
                    let database = call.memory(storage, &at).await?;
                    let answer = if at.query
                        || (call.held.is_none() && database.readonly(statement.sql.clone()).await?)
                    {
                        database.read(at.chain, statement).await?
                    } else {
                        let database = call.writer(storage, &at).await?;
                        let mut result = database.exec(at.chain, statement.clone()).await;
                        if result.as_ref().is_err_and(full) && call.grant.is_some() {
                            call.retry(storage, &at, &database).await?;
                            result = database.exec(at.chain, statement).await;
                        }
                        result.map_err(|error| call.limit_on_full(error))?
                    };
                    call.wrote |= !answer.readonly;
                    at.meter(Meter {
                        read: answer.read,
                        written: answer.written,
                        sql_cpu: answer.cpu,
                        ..Meter::default()
                    });
                    Ok(answer.json)
                }
                .await;
                call.remember(result)
            }
        },
    ));
    syscalls
}
