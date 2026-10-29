//! How a syscall call works, once, for all of them.
//!
//! A call hands the host a request and gets back a length. The bytes stay
//! host-side until the guest pulls them, as the argument blob is pulled
//! (`args.len` then `args.read`); the host never enters a guest to give it
//! something. The high bit of the returned length says the staged bytes are a
//! failure message, and a second bit says whether anything ran at all.

use crate::{
    abi::{
        ERROR, HOST_ARG_LEN, HOST_ARG_READ, HOST_DENY, HOST_DONE, HOST_FAIL, HOST_NOW, HOST_RANDOM,
        HOST_RESULT_READ, HOST_TRACE, REFUSED,
    },
    sys,
};
use alloc::{string::String, vec, vec::Vec};

/// Why a syscall did not produce a result, as the host told it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// Whatever the call reached ran, and reported failure.
    Failed(String),
    /// The host refused the call. Nothing ran.
    Refused(String),
}

impl CallError {
    /// What went wrong, whichever kind it was.
    pub fn message(&self) -> &str {
        match self {
            Self::Failed(message) | Self::Refused(message) => message,
        }
    }

    /// Whether the call was refused before anything ran.
    pub fn refused(&self) -> bool {
        matches!(self, Self::Refused(_))
    }
}

impl core::fmt::Display for CallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.message())
    }
}

/// Make one syscall call. `Err` carries whatever the host said went wrong.
pub fn call(number: u64, request: &[u8]) -> Result<Vec<u8>, CallError> {
    let staged = sys::call2(number, request.as_ptr() as u64, request.len() as u64);
    let failed = staged & ERROR != 0;
    let refused = staged & REFUSED != 0;

    let mut result = vec![0u8; (staged & !(ERROR | REFUSED)) as usize];
    let full = read_result(&mut result);
    if full != result.len() {
        return Err(CallError::Failed(String::from(
            "host staged a result of a different length",
        )));
    }

    if failed {
        let message = String::from_utf8_lossy(&result).into_owned();
        return Err(match refused {
            true => CallError::Refused(message),
            false => CallError::Failed(message),
        });
    }
    Ok(result)
}

/// Write a line to the host's trace.
pub fn trace(message: &str) {
    sys::call2(HOST_TRACE, message.as_ptr() as u64, message.len() as u64);
}

/// What time the host says it is, in milliseconds since the Unix epoch.
///
/// The one clock a service has.
pub fn now() -> u64 {
    sys::call0(HOST_NOW)
}

/// Fill `buffer` with bytes from the host's secure random source.
pub fn random(buffer: &mut [u8]) {
    sys::call2(HOST_RANDOM, buffer.as_mut_ptr() as u64, buffer.len() as u64);
}

/// Mark this invocation's failure as refusing its caller.
pub fn deny() {
    sys::call0(HOST_DENY);
}

/// How many bytes this invocation was given.
pub fn args_len() -> usize {
    sys::call0(HOST_ARG_LEN) as usize
}

/// Pull the argument blob into `buffer`, returning the blob's *full* length. A
/// caller that gets back more than it offered was truncated, and must fail.
pub fn read_args(buffer: &mut [u8]) -> usize {
    sys::call2(
        HOST_ARG_READ,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    ) as usize
}

/// Hand the host what this tool produced.
pub fn done(result: &[u8]) {
    sys::call2(HOST_DONE, result.as_ptr() as u64, result.len() as u64);
}

/// Report failure, the other half of [`done`]. The host marks the invocation
/// an error.
pub fn fail(message: &[u8]) {
    sys::call2(HOST_FAIL, message.as_ptr() as u64, message.len() as u64);
}

/// Pull the last syscall's staged result, returning its *full* length as
/// [`read_args`] does.
pub(crate) fn read_result(buffer: &mut [u8]) -> usize {
    sys::call2(
        HOST_RESULT_READ,
        buffer.as_mut_ptr() as u64,
        buffer.len() as u64,
    ) as usize
}
