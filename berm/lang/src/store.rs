//! This service's own bytes, surviving its invocations.
//!
//! Neither door names a service: the keyspace is whichever one is asking, as
//! the host reads it off the call. A host that registers nothing for these
//! traps them as unknown host calls.

use crate::{
    abi::{self, wire},
    host::{self, CallError},
};
use alloc::{string::String, vec::Vec};

/// Read `key`, or `None` if this service has never written it.
///
/// ```ignore
/// let seen = berm_lang::get("last-run")?.unwrap_or_default();
/// ```
pub fn get(key: &str) -> Result<Option<Vec<u8>>, CallError> {
    let reply = host::call(abi::HOST_GET, &wire::request(&[key.as_bytes()]))?;
    // One field is a value, a stored empty one included; none is an absent key.
    let Ok(fields) = wire::fields(&reply) else {
        return Err(CallError::Failed(String::from(
            "host framed a reply this guest cannot read",
        )));
    };
    Ok(fields.first().map(|value| value.to_vec()))
}

/// Write `key`, replacing whatever was there.
pub fn set(key: &str, value: &[u8]) -> Result<(), CallError> {
    host::call(abi::HOST_SET, &wire::request(&[key.as_bytes(), value]))?;
    Ok(())
}

/// Read `key` as JSON, or `None` if this service has never written it.
pub fn get_json<T: for<'de> serde::Deserialize<'de>>(key: &str) -> Result<Option<T>, CallError> {
    let Some(bytes) = get(key)? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| CallError::Failed(alloc::format!("{key} holds unreadable JSON: {error}")))
}

/// Write `value` as JSON under `key`, replacing whatever was there.
pub fn set_json<T: serde::Serialize>(key: &str, value: &T) -> Result<(), CallError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| CallError::Failed(alloc::format!("unserializable value: {error}")))?;
    set(key, &bytes)
}
