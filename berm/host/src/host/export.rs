use crate::{
    Denied, deny,
    env::Env,
    host::{self, Host},
    state::service,
};
use axum::http::StatusCode;
use berm::database;
use rusqlite::{Connection, params};
use tempfile::NamedTempFile;

impl<E: Env> Host<E> {
    pub async fn export(&self, service: &str) -> Result<NamedTempFile, Denied> {
        let lock = self
            .deploying
            .lock()
            .entry(service.to_owned())
            .or_default()
            .clone();
        let _held = lock.lock().await;
        let (scope, name) = host::split(service);
        if service::row(&self.store, scope, name).await?.is_none() {
            return Err(deny(
                StatusCode::NOT_FOUND,
                "The service and its data are no longer available",
            )
            .explained(Denied::DATA_UNAVAILABLE));
        }
        let file = NamedTempFile::new()
            .map_err(|error| deny(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        let path = file.path().to_owned();
        let result = if self.databases.path(service).exists() {
            self.databases
                .get(service)
                .map_err(|error| deny(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
                .export(path)
                .await
        } else {
            let (scope, name) = host::split(service);
            let (scope, name) = (scope.to_owned(), name.to_owned());
            self.store
                .read(move |conn| {
                    let mut target = Connection::open(path)?;
                    target.execute_batch(database::STATE_SCHEMA)?;
                    let tx = target.transaction()?;
                    let mut read = conn.prepare(
                        "SELECT key, value FROM service_state WHERE scope = ?1 AND name = ?2",
                    )?;
                    let mut rows = read.query(params![scope, name])?;
                    while let Some(row) = rows.next()? {
                        tx.execute(
                            "INSERT INTO _berm_state(key,value) VALUES(?1,?2)",
                            params![row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?],
                        )?;
                    }
                    tx.commit()
                })
                .await
        };
        result.map_err(|error| deny(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        Ok(file)
    }
}
