//! What `#[program]`'s exports call around a tool.

use crate::Error;
use alloc::{format, vec::Vec};

/// A tool's arguments, read as `T`.
pub fn args<T: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid arguments: {error}").into())
}

/// A tool's reply as JSON.
pub fn reply<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value)
        .map_err(|error| format!("unserializable result: {error}").into())
}
