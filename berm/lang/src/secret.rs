use crate::{CallError, abi::wire, host, sys};
use alloc::string::String;

/// Owner of a resource granted to this deployment at `destination`, without its value.
pub fn owner(id: &str, destination: &str) -> Result<Option<String>, CallError> {
    let reply = host::call(
        sys::SECRET_RESOURCE,
        &wire::request(&[id.as_bytes(), destination.as_bytes()]),
    )?;
    let unreadable = || CallError::Failed(String::from("host framed invalid secret metadata"));
    match wire::fields(&reply).map_err(|_| unreadable())?.as_slice() {
        [] => Ok(None),
        [owner] => core::str::from_utf8(owner)
            .map(|owner| Some(String::from(owner)))
            .map_err(|_| unreadable()),
        _ => Err(unreadable()),
    }
}
