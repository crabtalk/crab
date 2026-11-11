use crate::{CallError, abi::wire, host, sys};
use alloc::string::String;

/// The API key ID authenticated at the chain's start, carried into nested calls.
/// `None` when the host supplied no key. This never returns a token.
pub fn id() -> Result<Option<String>, CallError> {
    let reply = host::call(sys::KEY_ID, &[])?;
    let unreadable = || CallError::Failed(String::from("host framed an invalid key ID"));
    match wire::fields(&reply).map_err(|_| unreadable())?.as_slice() {
        [] => Ok(None),
        [id] => Ok(Some(String::from_utf8_lossy(id).into_owned())),
        _ => Err(unreadable()),
    }
}
