//! The values each service holds for its `[vars]`, in `service_vars`.

use crate::state::store::Store;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::collections::BTreeMap;

/// The value `scope/name` holds for `var`, `None` when it holds none.
pub(crate) async fn read(
    store: &Store,
    scope: &str,
    name: &str,
    var: String,
) -> Result<Option<String>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            conn.query_row(
                "SELECT value FROM service_vars WHERE scope = ?1 AND name = ?2 AND var = ?3",
                params![scope, name, var],
                |row| row.get(0),
            )
            .optional()
        })
        .await
}

/// Every value `scope/name` holds, by var.
pub(crate) async fn list(
    store: &Store,
    scope: &str,
    name: &str,
) -> Result<BTreeMap<String, String>> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .read(move |conn| {
            let mut statement =
                conn.prepare("SELECT var, value FROM service_vars WHERE scope = ?1 AND name = ?2")?;
            statement
                .query_map(params![scope, name], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .await
}

/// Set `scope/name`'s value of `var`, or with `None` drop it, so the var
/// reads its default.
pub(crate) async fn set(
    store: &Store,
    scope: &str,
    name: &str,
    var: String,
    value: Option<String>,
) -> Result<()> {
    let (scope, name) = (scope.to_owned(), name.to_owned());
    store
        .run(move |conn| match value {
            Some(value) => put(conn, &scope, &name, &var, &value),
            None => unset(conn, &scope, &name, &var),
        })
        .await
}

/// Inside a deploy's `tx`: drop the values of vars `declared` no longer
/// names, then write `values`.
pub(crate) fn deploy(
    tx: &Transaction<'_>,
    scope: &str,
    name: &str,
    declared: &[String],
    values: &BTreeMap<String, String>,
) -> rusqlite::Result<()> {
    let held: Vec<String> = tx
        .prepare("SELECT var FROM service_vars WHERE scope = ?1 AND name = ?2")?
        .query_map(params![scope, name], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for var in held.iter().filter(|var| !declared.contains(var)) {
        unset(tx, scope, name, var)?;
    }
    for (var, value) in values {
        put(tx, scope, name, var, value)?;
    }
    Ok(())
}

fn put(conn: &Connection, scope: &str, name: &str, var: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO service_vars (scope, name, var, value) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (scope, name, var) DO UPDATE
            SET value = excluded.value, updated_at = unixepoch()",
        params![scope, name, var, value],
    )?;
    Ok(())
}

fn unset(conn: &Connection, scope: &str, name: &str, var: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM service_vars WHERE scope = ?1 AND name = ?2 AND var = ?3",
        params![scope, name, var],
    )?;
    Ok(())
}
