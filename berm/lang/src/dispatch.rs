//! What `#[tool]`'s exports call around a tool.

use crate::{Error, abi};
use alloc::{format, vec, vec::Vec};

/// Read this invocation's arguments, run `tool` on them and hand the host what
/// it answered or why it failed.
pub fn run(tool: impl FnOnce(&[u8]) -> Result<Vec<u8>, Error>) {
    let mut arguments = vec![0; abi::args_len()];
    let length = abi::read_args(&mut arguments);
    if length != arguments.len() {
        return abi::fail(b"the host handed arguments of a different length");
    }
    match tool(&arguments) {
        Ok(reply) => abi::done(&reply),
        Err(error) => abi::fail(error.message.as_bytes()),
    }
}

/// A tool's arguments, read as `T`.
pub fn args<T: for<'de> serde::Deserialize<'de>>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|error| format!("invalid arguments: {error}").into())
}

/// A tool's reply as JSON.
pub fn reply<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|error| format!("unserializable result: {error}").into())
}
