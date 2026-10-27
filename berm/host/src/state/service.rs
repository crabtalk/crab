//! The `services` table and what a service's calls commit beside it.

use crate::{state::store::Store, wake::cron};
use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction, params};
use std::collections::{BTreeMap, HashMap};

/// One service, as a row.
pub(crate) struct Row {
    pub scope: String,
    pub name: String,
    pub reference: String,
    pub digest: String,
    /// Bytes its database may hold.
    pub size: u64,
    /// The project it was last deployed in.
    pub project: Option<String>,
}

/// Every deployed service.
pub(crate) async fn all(store: &Store) -> Result<Vec<Row>> {
    store
        .read(|conn| {
            let mut statement =
                conn.prepare("SELECT scope, name, reference, digest, size, project FROM services")?;
            let rows = statement.query_map([], |row| {
                Ok(Row {
                    scope: row.get(0)?,
                    name: row.get(1)?,
                    reference: row.get(2)?,
                    digest: row.get(3)?,
                    size: row.get::<_, i64>(4)? as u64,
                    project: row.get(5)?,
                })
            })?;
            rows.collect()
        })
        .await
}

/// What `scope/name` runs, if it exists.
pub(crate) async fn row(store: &Store, scope: &str, name: &str) -> Result<Option<Row>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT scope, name, reference, digest, size, project FROM services
                  WHERE scope = ?1 AND name = ?2",
                params![scope, name],
                |row| {
                    Ok(Row {
                        scope: row.get(0)?,
                        name: row.get(1)?,
                        reference: row.get(2)?,
                        digest: row.get(3)?,
                        size: row.get::<_, i64>(4)? as u64,
                        project: row.get(5)?,
                    })
                },
            )
            .optional()
        })
        .await
}

/// The project each of `scope`'s services was last deployed in, by name; a
/// service in none is left out.
pub(crate) async fn projects(store: &Store, scope: &str) -> Result<HashMap<String, String>> {
    let scope = scope.to_owned();
    store
        .read(move |conn| {
            conn.prepare(
                "SELECT name, project FROM services WHERE scope = ?1 AND project IS NOT NULL",
            )?
            .query_map(params![scope], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
        })
        .await
}

/// `scope/name`'s commit count, if it exists.
pub(crate) async fn commits(store: &Store, scope: &str, name: &str) -> Result<Option<i64>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT commits FROM services WHERE scope = ?1 AND name = ?2",
                params![scope, name],
                |row| row.get(0),
            )
            .optional()
        })
        .await
}

/// Point `scope/name` at a new version, creating the service on its first
/// deploy, in `project` when one is given and otherwise the project it was
/// in, and record the tools `deprecated` in it: each keeps the time it was
/// first deployed deprecated, `now` in epoch seconds for one deprecated by this
/// deploy, and the rest are forgotten with their sunsets.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn install(
    store: &Store,
    scope: &str,
    name: &str,
    reference: &str,
    digest: &str,
    version: &str,
    size: u64,
    deprecated: Vec<String>,
    cron_tools: Vec<String>,
    crons: Option<BTreeMap<String, String>>,
    stopped: bool,
    project: Option<String>,
    now: i64,
) -> Result<()> {
    let row = [scope, name, reference, digest, version].map(str::to_owned);
    let declared = crons
        .map(|crons| declare(&cron_tools, &crons, now))
        .transpose()?;
    store
        .run(move |conn| {
            let [scope, name, reference, digest, version] = row;
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO services
                     (scope, name, reference, digest, version, size, stopped, project)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT (scope, name) DO UPDATE
                    SET reference = excluded.reference, digest = excluded.digest,
                        version = excluded.version, size = excluded.size,
                        stopped = excluded.stopped,
                        project = COALESCE(excluded.project, services.project),
                        updated_at = unixepoch()",
                params![
                    scope,
                    name,
                    reference,
                    digest,
                    version,
                    size as i64,
                    stopped,
                    project
                ],
            )?;
            deprecate(&tx, &scope, &name, &deprecated, now)?;
            schedule(&tx, &scope, &name, &cron_tools, declared.as_deref())?;
            tx.commit()
        })
        .await
}

/// Each of `cron_tools` with the expression `crons` sets for it and its next
/// firing after `now`, in epoch milliseconds.
fn declare(
    cron_tools: &[String],
    crons: &BTreeMap<String, String>,
    now: i64,
) -> Result<Vec<Declared>> {
    cron_tools
        .iter()
        .map(|tool| {
            let cron = crons.get(tool).cloned();
            let due = cron
                .as_deref()
                .map(|cron| cron::next(cron, now))
                .transpose()?;
            Ok(Declared {
                tool: tool.clone(),
                cron,
                due,
            })
        })
        .collect()
}

/// A cron tool as a deploy sets it.
struct Declared {
    tool: String,
    cron: Option<String>,
    /// Its next firing, in epoch milliseconds.
    due: Option<i64>,
}

/// Keep `scope/name`'s deprecations to `deprecated`, each new one since `now`,
/// in epoch milliseconds.
fn deprecate(
    tx: &Transaction,
    scope: &str,
    name: &str,
    deprecated: &[String],
    now: i64,
) -> rusqlite::Result<()> {
    let kept = serde_json::to_string(deprecated).unwrap_or_default();
    tx.execute(
        "DELETE FROM service_deprecations
          WHERE scope = ?1 AND name = ?2
            AND tool NOT IN (SELECT value FROM json_each(?3))",
        params![scope, name, kept],
    )?;
    for tool in deprecated {
        tx.execute(
            "INSERT INTO service_deprecations (scope, name, tool, since)
             VALUES (?1, ?2, ?3, ?4) ON CONFLICT DO NOTHING",
            params![scope, name, tool, now / 1000],
        )?;
    }
    Ok(())
}

/// Keep `scope/name`'s cron wakes to `cron_tools`, set as `declared` when the
/// deploy sets them and left as their owners set them otherwise.
fn schedule(
    tx: &Transaction,
    scope: &str,
    name: &str,
    cron_tools: &[String],
    declared: Option<&[Declared]>,
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM service_wakes
          WHERE scope = ?1 AND name = ?2 AND slot <> ''
            AND slot NOT IN (SELECT value FROM json_each(?3))",
        params![
            scope,
            name,
            serde_json::to_string(cron_tools).unwrap_or_default()
        ],
    )?;
    match declared {
        // An expression that changes is due at its next firing; one that
        // stays keeps its due.
        Some(declared) => {
            for Declared { tool, cron, due } in declared {
                tx.execute(
                    "INSERT INTO service_wakes
                            (scope, name, slot, program, tool, args, due, cron, declared)
                     VALUES (?1, ?2, ?3, ?2, ?3, '', ?5, ?4, ?4)
                     ON CONFLICT (scope, name, slot) DO UPDATE
                        SET due = IIF(service_wakes.cron IS excluded.cron,
                                      service_wakes.due, excluded.due),
                            cron = excluded.cron, declared = excluded.declared",
                    params![scope, name, tool, cron, due],
                )?;
            }
        }
        None => {
            for tool in cron_tools {
                tx.execute(
                    "INSERT INTO service_wakes (scope, name, slot, program, tool, args)
                     VALUES (?1, ?2, ?3, ?2, ?3, '')
                     ON CONFLICT (scope, name, slot) DO NOTHING",
                    params![scope, name, tool],
                )?;
            }
        }
    }
    Ok(())
}

/// Delete `scope/name` and every row beside it. `false` when there was none.
pub(crate) async fn remove(store: &Store, scope: &str, name: &str) -> Result<bool> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .run(move |conn| {
            let removed = conn.execute(
                "DELETE FROM services WHERE scope = ?1 AND name = ?2",
                params![scope, name],
            )?;
            Ok(removed == 1)
        })
        .await
}

/// What `scope/name` last committed under `key`.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    key: String,
) -> Result<Option<Vec<u8>>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT value FROM service_state WHERE scope = ?1 AND name = ?2 AND key = ?3",
                params![scope, name, key],
                |row| row.get(0),
            )
            .optional()
        })
        .await
}

/// Commit one service's `writes` inside `tx`: upsert them, and advance its
/// commit count.
pub(crate) fn commit(
    tx: &Transaction<'_>,
    scope: &str,
    name: &str,
    writes: &[(String, Vec<u8>)],
) -> rusqlite::Result<()> {
    tx.prepare_cached("UPDATE services SET commits = commits + 1 WHERE scope = ?1 AND name = ?2")?
        .execute(params![scope, name])?;
    let mut upsert = tx.prepare_cached(
        "INSERT INTO service_state (scope, name, key, value) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (scope, name, key) DO UPDATE SET value = excluded.value",
    )?;
    for (key, value) in writes {
        upsert.execute(params![scope, name, key, value])?;
    }
    Ok(())
}
