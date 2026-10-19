//! The program's statements against a connection.

use crate::{Refused, syscall::sql::Statement};
use anyhow::{Context, Result, bail};
use rusqlite::{
    Connection, Error, StatementStatus,
    hooks::{AuthAction, AuthContext, Authorization},
    types::{Value, ValueRef},
};
use std::{cell::Cell, io::Write};

/// The largest reply one statement may answer with, in bytes.
const MAX_REPLY: usize = 4 << 20;

/// What names the host's own tables.
const RESERVED: &str = "_berm_";

thread_local! {
    /// Whether the statement this thread prepares or runs is the program's.
    pub(crate) static GUEST: Cell<bool> = const { Cell::new(false) };
}

/// While [`GUEST`] is set, refuse transaction control, `ATTACH`, `DETACH`, a
/// `PRAGMA` that sets a value, and anything reaching a `_berm_*` table. A
/// cached statement is not authorized again, so only the program's statements
/// are prepared through the cache.
pub(crate) fn guard(conn: &Connection) -> Result<()> {
    conn.authorizer(Some(|context: AuthContext<'_>| {
        if !GUEST.get() {
            return Authorization::Allow;
        }
        let table = match context.action {
            AuthAction::Transaction { .. }
            | AuthAction::Savepoint { .. }
            | AuthAction::Attach { .. }
            | AuthAction::Detach { .. }
            | AuthAction::Pragma {
                pragma_value: Some(_),
                ..
            } => return Authorization::Deny,
            AuthAction::CreateIndex { table_name, .. }
            | AuthAction::CreateTable { table_name, .. }
            | AuthAction::CreateTempIndex { table_name, .. }
            | AuthAction::CreateTempTable { table_name, .. }
            | AuthAction::CreateTempTrigger { table_name, .. }
            | AuthAction::CreateTrigger { table_name, .. }
            | AuthAction::Delete { table_name, .. }
            | AuthAction::DropIndex { table_name, .. }
            | AuthAction::DropTable { table_name, .. }
            | AuthAction::DropTempIndex { table_name, .. }
            | AuthAction::DropTempTable { table_name, .. }
            | AuthAction::DropTempTrigger { table_name, .. }
            | AuthAction::DropTrigger { table_name, .. }
            | AuthAction::Insert { table_name, .. }
            | AuthAction::Read { table_name, .. }
            | AuthAction::Update { table_name, .. }
            | AuthAction::AlterTable { table_name, .. }
            | AuthAction::Analyze { table_name, .. }
            | AuthAction::CreateVtable { table_name, .. }
            | AuthAction::DropVtable { table_name, .. } => table_name,
            _ => return Authorization::Allow,
        };
        match table.starts_with(RESERVED) {
            true => Authorization::Deny,
            false => Authorization::Allow,
        }
    }))?;
    Ok(())
}

/// What one statement answered: the reply's JSON, and the rows it read and
/// wrote.
pub struct Answer {
    pub json: String,
    pub read: u64,
    pub written: u64,
}

/// Run one of the program's statements.
pub(crate) fn run(conn: &Connection, statement: &Statement, read_only: bool) -> Result<Answer> {
    let params: Vec<serde_json::Value> =
        serde_json::from_str(&statement.params).context("the parameters are not a JSON array")?;
    GUEST.set(true);
    let ran = (|| {
        let mut prepared = conn
            .prepare_cached(&statement.sql)
            .map_err(|error| match error {
                Error::MultipleStatement => Refused("one statement per call".to_owned()).into(),
                error => anyhow::Error::from(error),
            })?;
        // `None` only for SQL holding no statement.
        if prepared.expanded_sql().is_none() {
            bail!("there is no statement to run");
        }
        if read_only && !prepared.readonly() {
            return Err(Refused("a query cannot write".to_owned()).into());
        }
        let mut out = Vec::new();
        out.extend_from_slice(br#"{"columns":"#);
        serde_json::to_writer(&mut out, &prepared.column_names())?;
        out.extend_from_slice(br#","rows":["#);
        let columns = prepared.column_count();
        prepared.reset_status(StatementStatus::RowsRead);
        prepared.reset_status(StatementStatus::RowsWritten);
        let mut found =
            prepared.query(rusqlite::params_from_iter(params.into_iter().map(value)))?;
        let mut first = true;
        while let Some(row) = found.next()? {
            if !first {
                out.push(b',');
            }
            first = false;
            out.push(b'[');
            for at in 0..columns {
                if at > 0 {
                    out.push(b',');
                }
                json(&mut out, row.get_ref(at)?)?;
            }
            out.push(b']');
            if out.len() > MAX_REPLY {
                bail!("the statement answered more than {MAX_REPLY} bytes");
            }
        }
        drop(found);
        let read = prepared.get_status(StatementStatus::RowsRead) as u64;
        let written = prepared.get_status(StatementStatus::RowsWritten) as u64;
        write!(out, r#"],"read":{read},"written":{written}}}"#)?;
        Ok(Answer {
            json: String::from_utf8(out)?,
            read,
            written,
        })
    })();
    GUEST.set(false);
    ran
}

/// A JSON parameter as SQLite binds it: arrays and objects as their JSON text.
fn value(param: serde_json::Value) -> Value {
    match param {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(flag) => Value::Integer(flag as i64),
        serde_json::Value::Number(number) => match number.as_i64() {
            Some(integer) => Value::Integer(integer),
            None => Value::Real(number.as_f64().unwrap_or_default()),
        },
        serde_json::Value::String(text) => Value::Text(text),
        other => Value::Text(other.to_string()),
    }
}

/// Write a cell to `out` as JSON: a blob as its bytes.
fn json(out: &mut Vec<u8>, cell: ValueRef<'_>) -> serde_json::Result<()> {
    match cell {
        ValueRef::Null => serde_json::to_writer(out, &()),
        ValueRef::Integer(integer) => serde_json::to_writer(out, &integer),
        ValueRef::Real(real) => serde_json::to_writer(out, &real),
        ValueRef::Text(text) => serde_json::to_writer(out, &String::from_utf8_lossy(text)),
        ValueRef::Blob(bytes) => serde_json::to_writer(out, bytes),
    }
}
