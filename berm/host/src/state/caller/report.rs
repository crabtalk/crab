use crate::state::caller::{Storage, name};
use anyhow::Result;
use berm_api::storage::{Grant, Key, Report};
use rusqlite::{OptionalExtension, Row};
use std::collections::HashMap;

pub(crate) struct Pending {
    pub(crate) report: Report,
    generation: i64,
}

impl Storage {
    pub(super) async fn key(&self, id: &str) -> Result<Option<Key>> {
        let id = id.to_owned();
        let stored: Option<Vec<u8>> = self
            .store
            .read(move |conn| {
                conn.query_row("SELECT key FROM caller_files WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()
            })
            .await?;
        Ok(stored.as_deref().map(serde_json::from_slice).transpose()?)
    }

    pub(super) async fn file_state(&self, name: &str) -> Result<Option<(Grant, i64, i64)>> {
        let name = name.to_owned();
        self.store
            .read(move |conn| {
                conn.query_row(
                    "SELECT id, token, bytes, plan_limit, writable, pending, called_at
                       FROM caller_files WHERE name = ?1",
                    [name],
                    |row| Ok((grant(row)?, row.get(5)?, row.get(6)?)),
                )
                .optional()
            })
            .await
    }

    pub(super) async fn track(&self, key: &Key, grant: &Grant, called_at: i64) -> Result<()> {
        let name = name("callers", key);
        let key = serde_json::to_vec(key)?;
        let Grant {
            id,
            token,
            bytes,
            limit,
            writable,
        } = grant.clone();
        let (bytes, limit) = (i64::try_from(bytes)?, i64::try_from(limit)?);
        self.store.run(move |conn| {
            conn.execute(
                "INSERT INTO caller_files (name, id, key, token, bytes, plan_limit, writable, pending, called_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8)
                 ON CONFLICT (name) DO UPDATE SET id = excluded.id, token = excluded.token,
                   bytes = excluded.bytes, plan_limit = excluded.plan_limit, writable = excluded.writable,
                   pending = caller_files.pending + 1,
                   called_at = MAX(caller_files.called_at, excluded.called_at)",
                (name, id, key, token, bytes, limit, writable, called_at),
            )
        }).await?;
        Ok(())
    }

    pub(super) async fn called(&self, name: &str, called_at: i64) -> Result<Option<Grant>> {
        let name = name.to_owned();
        self.store
            .run(move |conn| {
                conn.query_row(
                    "UPDATE caller_files SET called_at = MAX(called_at, ?2), pending = pending + 1
                      WHERE name = ?1 RETURNING id, token, bytes, plan_limit, writable",
                    (name, called_at),
                    grant,
                )
                .optional()
            })
            .await
    }

    pub(crate) async fn pending(&self) -> Result<Vec<Pending>> {
        let keys: Vec<Vec<u8>> = self
            .store
            .read(|conn| {
                let mut query = conn.prepare("SELECT key FROM caller_files WHERE pending > 0")?;
                query.query_map([], |row| row.get(0))?.collect()
            })
            .await?;
        let mut reports = Vec::new();
        for key in keys {
            let key: Key = serde_json::from_slice(&key)?;
            let Ok(_service) = self.service(&key.storage_id).try_read_owned() else {
                continue;
            };
            let Ok(_file) = self.file(&key).try_lock_owned() else {
                continue;
            };
            let Some((grant, generation, called_at)) =
                self.file_state(&name("callers", &key)).await?
            else {
                continue;
            };
            let path = self.located(&key);
            let (size, schema) = if self.databases.path(&path).exists() {
                let database = self.databases.get_with_limit(&path, Some(grant.bytes))?;
                (
                    database.usage(called_at).await?.used,
                    database.schema().await?,
                )
            } else {
                (0, 0)
            };
            reports.push(Pending {
                report: Report {
                    key,
                    id: grant.id,
                    token: grant.token,
                    bytes: grant.bytes,
                    size,
                    schema,
                    called_at,
                },
                generation,
            });
        }
        Ok(reports)
    }

    pub(crate) async fn reported(&self, reports: &[Pending], grants: Vec<Grant>) -> Result<()> {
        let grants: HashMap<_, _> = grants
            .into_iter()
            .map(|grant| (grant.id.clone(), grant))
            .collect();
        for Pending { report, generation } in reports {
            let Some(grant) = grants.get(&report.id) else {
                continue;
            };
            let key = &report.key;
            let Ok(_service) = self.service(&key.storage_id).try_read_owned() else {
                continue;
            };
            let Ok(_file) = self.file(key).try_lock_owned() else {
                continue;
            };
            let name = name("callers", key);
            let Some((current, pending, called_at)) = self.file_state(&name).await? else {
                continue;
            };
            if current.id != report.id || current.token != report.token {
                continue;
            }

            let path = self.located(key);
            let mut bytes = grant.bytes.min(current.bytes);
            if bytes < current.bytes && self.databases.path(&path).exists() {
                let database = self.databases.get_with_limit(&path, Some(current.bytes))?;
                bytes = bytes.max(database.usage(called_at).await?.used);
                database.limit(bytes).await?;
            }

            // Fleet releases bytes after the next report confirms the cap applied here.
            let pending = if bytes != current.bytes || current.token != grant.token {
                pending + 1
            } else if pending == *generation {
                0
            } else {
                pending
            };
            let Grant {
                token,
                limit,
                writable,
                ..
            } = grant.clone();
            let (bytes, limit) = (i64::try_from(bytes)?, i64::try_from(limit)?);
            self.store
                .run(move |conn| {
                    conn.execute(
                        "UPDATE caller_files SET token = ?2, bytes = ?3, plan_limit = ?4,
                                writable = ?5, pending = ?6 WHERE name = ?1",
                        (name, token, bytes, limit, writable, pending),
                    )
                })
                .await?;
        }
        Ok(())
    }
}

fn grant(row: &Row<'_>) -> rusqlite::Result<Grant> {
    Ok(Grant {
        id: row.get(0)?,
        token: row.get(1)?,
        bytes: row.get::<_, i64>(2)? as u64,
        limit: row.get::<_, i64>(3)? as u64,
        writable: row.get(4)?,
    })
}
