//! This service's settings, each a var of `[vars]` in its `crab.toml`.

use crate::{
    abi::wire,
    host::{self, CallError},
    sys,
};
use alloc::string::String;

/// The value this deployed copy holds for `name`, or the var's default when
/// it holds none. Refused for a var `[vars]` does not declare.
///
/// ```ignore
/// let keywords = berm_lang::var("keywords")?;
/// ```
pub fn var(name: &str) -> Result<String, CallError> {
    let reply = host::call(sys::VAR, &wire::request(&[name.as_bytes()]))?;
    String::from_utf8(reply)
        .map_err(|_| CallError::Failed(alloc::format!("the host answered {name} as non-UTF-8")))
}
