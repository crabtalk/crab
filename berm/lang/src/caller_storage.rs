//! SQL and key-value storage for the verified caller, declared in
//! `[storage.caller]` in `crab.toml`.

use crate::{host::CallError, sql as shared_sql, store, sys};
use alloc::{string::String, vec::Vec};
use serde::{Serialize, de::DeserializeOwned};

/// Run a statement with a JSON array of bound parameters.
pub fn sql(statement: &str, params: &str) -> Result<String, CallError> {
    shared_sql::execute(sys::CALLER_SQL, statement, params)
}

/// Bind a tuple of parameters and deserialize each row into `T`.
pub fn rows<T: DeserializeOwned>(
    statement: &str,
    params: &impl Serialize,
) -> Result<Vec<T>, CallError> {
    shared_sql::query(sys::CALLER_SQL, statement, params)
}

/// Read a value within the caller's storage.
pub fn get(key: &str) -> Result<Option<Vec<u8>>, CallError> {
    store::read(sys::CALLER_GET, key)
}

/// Stage a value within the caller's storage.
pub fn set(key: &str, value: &[u8]) -> Result<(), CallError> {
    store::write(sys::CALLER_SET, key, value)
}
