//! This service's own SQLite database, which its `crab.toml` declares with
//! `[database] migrations = "migrations"`.
//!
//! One statement per call. The host runs every statement of an invocation in
//! one transaction, committed when the invocation returns ok; transaction
//! statements are refused. Under a `query`, a statement that writes is refused.

use crate::{
    abi::{self, wire},
    host::{self, CallError},
    json,
};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Run `statement` with `params`, a JSON array bound to its `?` placeholders,
/// and answer `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
///
/// ```ignore
/// let found = berm_lang::sql("SELECT title FROM items WHERE id = ?", "[42]")?;
/// ```
pub fn sql(statement: &str, params: &str) -> Result<String, CallError> {
    let reply = host::call(
        abi::HOST_SQL,
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
