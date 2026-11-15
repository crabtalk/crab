//! This service's SQLite storage, declared in `crab.toml` with
//! `[database] migrations = "migrations"`.

use crate::{
    abi::wire,
    host::{self, CallError},
    json, sys,
};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Run `statement` with `params`, a JSON array bound to its `?` placeholders,
/// and answer `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
/// Statements in a writing tool call share a transaction with its key/value
/// writes. Reads see earlier writes in that call. Changes commit only on `Ok`;
/// `Err`, a trap or an exceeded deadline discards pending changes. Transaction
/// control statements are refused; a `query` tool cannot write.
///
/// ```ignore
/// let found = berm_lang::sql("SELECT title FROM items WHERE id = ?", "[42]")?;
/// ```
pub fn sql(statement: &str, params: &str) -> Result<String, CallError> {
    let reply = host::call(
        sys::SQL,
        &wire::request(&[statement.as_bytes(), params.as_bytes()]),
    )?;
    String::from_utf8(reply)
        .map_err(|_| CallError::Failed(String::from("host answered SQL with invalid UTF-8")))
}

/// What [`sql`] answers, each row as `T`.
#[derive(Deserialize)]
struct Answer<T> {
    rows: Vec<T>,
}

/// Run `statement` with `params`, a tuple bound to its `?`s in order, and
/// answer its rows, each a tuple of its columns in order.
///
/// ```ignore
/// let found = berm_lang::rows::<(String,)>("SELECT title FROM items WHERE id = ?", &(42,))?;
/// ```
pub fn rows<T: DeserializeOwned>(
    statement: &str,
    params: &impl Serialize,
) -> Result<Vec<T>, CallError> {
    let answer: Answer<T> = json::from(&sql(statement, &json::to(params)?)?)?;
    Ok(answer.rows)
}
