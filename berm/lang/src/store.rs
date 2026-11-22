//! Persistent values scoped to a service, shared by all its callers.
//! For values scoped to a caller, see [`crate::caller_storage`].
//!
//! [`get`] and [`set`] store values as JSON; [`get_raw`] and [`set_raw`]
//! store bytes as given.

use crate::{
    abi::wire,
    host::{self, CallError},
    sys,
};
use alloc::{format, string::String, vec::Vec};
use serde::{Serialize, de::DeserializeOwned};

/// The host calls one keyspace is served by.
pub(crate) struct Keyspace {
    pub get: fn(usize, usize) -> u64,
    pub set: fn(usize, usize) -> u64,
    pub delete: fn(usize, usize) -> u64,
}

/// The service's own keyspace.
const SHARED: Keyspace = Keyspace {
    get: sys::GET,
    set: sys::SET,
    delete: sys::DELETE,
};

impl Keyspace {
    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CallError> {
        let Some(bytes) = self.get_raw(key)? else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| CallError::Failed(format!("{key} holds unreadable JSON: {error}")))
    }

    pub(crate) fn set<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<(), CallError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| CallError::Failed(format!("unserializable value: {error}")))?;
        self.set_raw(key, &bytes)
    }

    pub(crate) fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, CallError> {
        let reply = host::call(self.get, &wire::request(&[key.as_bytes()]))?;
        // One field is a value, a stored empty one included; none is an absent key.
        let Ok(fields) = wire::fields(&reply) else {
            return Err(CallError::Failed(String::from(
                "host framed a reply this guest cannot read",
            )));
        };
        Ok(fields.first().map(|value| value.to_vec()))
    }

    pub(crate) fn set_raw(&self, key: &str, value: &[u8]) -> Result<(), CallError> {
        host::call(self.set, &wire::request(&[key.as_bytes(), value]))?;
        Ok(())
    }

    pub(crate) fn delete(&self, key: &str) -> Result<(), CallError> {
        host::call(self.delete, &wire::request(&[key.as_bytes()]))?;
        Ok(())
    }
}

/// Read `key` as JSON into `T`, or `None` if this service has never written it.
/// All callers of the service read the same keyspace. Reads include writes
/// made earlier in the same tool call, before they are committed.
///
/// ```ignore
/// let config: Config = berm_lang::get("config")?.unwrap_or_default();
/// ```
pub fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>, CallError> {
    SHARED.get(key)
}

/// Stage `value` as JSON under `key`.
/// The change commits only when the tool call succeeds with `Ok`. A call that
/// returns `Err`, traps or exceeds its deadline discards its pending changes.
pub fn set<T: Serialize + ?Sized>(key: &str, value: &T) -> Result<(), CallError> {
    SHARED.set(key, value)
}

/// Read the bytes stored under `key`, or `None` if this service has never written it.
pub fn get_raw(key: &str) -> Result<Option<Vec<u8>>, CallError> {
    SHARED.get_raw(key)
}

/// Stage `value`'s bytes under `key` as given.
pub fn set_raw(key: &str, value: &[u8]) -> Result<(), CallError> {
    SHARED.set_raw(key, value)
}

/// Stage the removal of `key`. Removing an absent key succeeds.
pub fn delete(key: &str) -> Result<(), CallError> {
    SHARED.delete(key)
}
