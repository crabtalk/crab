//! SQL and key-value storage for the verified caller, declared in
//! `[storage.caller]` in `crab.toml`. Key-value calls encode as
//! [`crate::store`]'s do.

use crate::{host::CallError, sql as shared_sql, store::Keyspace, sys};
use alloc::{string::String, vec::Vec};
use serde::{Serialize, de::DeserializeOwned};

/// The verified caller's keyspace.
const CALLER: Keyspace = Keyspace {
    get: sys::CALLER_GET,
    set: sys::CALLER_SET,
    delete: sys::CALLER_DELETE,
};

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

/// Read `key` as JSON into `T` from the caller's storage.
pub fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>, CallError> {
    CALLER.get(key)
}

/// Stage `value` as JSON under `key` in the caller's storage.
pub fn set<T: Serialize + ?Sized>(key: &str, value: &T) -> Result<(), CallError> {
    CALLER.set(key, value)
}

/// Read the bytes stored under `key` in the caller's storage.
pub fn get_raw(key: &str) -> Result<Option<Vec<u8>>, CallError> {
    CALLER.get_raw(key)
}

/// Stage `value`'s bytes under `key` in the caller's storage as given.
pub fn set_raw(key: &str, value: &[u8]) -> Result<(), CallError> {
    CALLER.set_raw(key, value)
}

/// Stage the removal of `key` from the caller's storage.
pub fn delete(key: &str) -> Result<(), CallError> {
    CALLER.delete(key)
}
