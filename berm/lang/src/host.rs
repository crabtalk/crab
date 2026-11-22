//! How a syscall call works, once, for all of them.
//!
//! A call hands the host a request and gets back a length. The bytes stay
//! host-side until the guest pulls them, as the argument blob is pulled
//! (`args.len` then `args.read`); the host never enters a guest to give it
//! something. The high bit of the returned length says the staged bytes are a
//! failure message, and a second bit marks a policy refusal.

use crate::{
    abi::{ERROR, REFUSED},
    sys,
};
use alloc::{string::String, vec, vec::Vec};
use serde::Deserialize;

/// Why a syscall did not produce a result, as the host told it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// The operation or its transport failed.
    Failed(String),
    /// The host refused the operation under its policy.
    Refused(Refusal),
}

/// The host's refusal and its explanation code, when supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub message: String,
    pub explanation: Option<String>,
}

impl From<String> for Refusal {
    fn from(message: String) -> Self {
        Self {
            message,
            explanation: None,
        }
    }
}

impl From<&str> for Refusal {
    fn from(message: &str) -> Self {
        String::from(message).into()
    }
}

impl Refusal {
    pub(crate) fn from_wire(message: String, explanation: Option<&str>) -> Self {
        if let Some(code) = explanation {
            return Self {
                message,
                explanation: Some(code.into()),
            };
        }
        // Single errors and older batch hosts carry the explanation in a
        // refusal's JSON envelope.
        #[derive(Deserialize)]
        struct Explained {
            status: u16,
            #[serde(rename = "error")]
            _error: serde::de::IgnoredAny,
            explain: String,
        }
        if message.starts_with('{')
            && let Some(Ok(explained)) = serde_json::Deserializer::from_str(&message)
                .into_iter::<Explained>()
                .next()
            && (400..500).contains(&explained.status)
        {
            return Self {
                message,
                explanation: Some(explained.explain),
            };
        }
        message.into()
    }
}

impl CallError {
    /// What went wrong, whichever kind it was.
    pub fn message(&self) -> &str {
        match self {
            Self::Failed(message) => message,
            Self::Refused(refusal) => &refusal.message,
        }
    }

    /// The host explanation code, when one accompanies the refusal.
    pub fn explanation(&self) -> Option<&str> {
        match self {
            Self::Refused(refusal) => refusal.explanation.as_deref(),
            Self::Failed(_) => None,
        }
    }

    /// Whether the host reported a policy refusal.
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
pub fn call(
    import: impl FnOnce(usize, usize) -> u64,
    request: &[u8],
) -> Result<Vec<u8>, CallError> {
    let staged = import(request.as_ptr() as usize, request.len());
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
            true => CallError::Refused(Refusal::from_wire(message, None)),
            false => CallError::Failed(message),
        });
    }
    Ok(result)
}

/// Write a line to the host's trace.
pub fn trace(message: &str) {
    sys::TRACE(message.as_ptr() as usize, message.len());
}

/// What time the host says it is, in milliseconds since the Unix epoch.
///
/// The one clock a service has.
pub fn now() -> u64 {
    sys::NOW(0, 0)
}

/// Milliseconds remaining before the host stops this chain.
pub fn deadline() -> u64 {
    sys::DEADLINE(0, 0)
}

/// Fill `buffer` with bytes from the host's secure random source.
pub fn random(buffer: &mut [u8]) {
    sys::RANDOM(buffer.as_mut_ptr() as usize, buffer.len());
}

/// Mark this invocation's failure as refusing its caller.
pub fn deny() {
    sys::DENY(0, 0);
}

/// How many bytes this invocation was given.
pub fn args_len() -> usize {
    sys::ARG_LEN(0, 0) as usize
}

/// Pull the argument blob into `buffer`, returning the blob's *full* length. A
/// caller that gets back more than it offered was truncated, and must fail.
pub fn read_args(buffer: &mut [u8]) -> usize {
    sys::ARG_READ(buffer.as_mut_ptr() as usize, buffer.len()) as usize
}

/// Hand the host what this tool produced.
pub fn done(result: &[u8]) {
    sys::DONE(result.as_ptr() as usize, result.len());
}

/// Report failure, the other half of [`done`]. The host marks the invocation
/// an error.
pub fn fail(message: &[u8]) {
    sys::FAIL(message.as_ptr() as usize, message.len());
}

/// Pull the last syscall's staged result, returning its *full* length as
/// [`read_args`] does.
pub(crate) fn read_result(buffer: &mut [u8]) -> usize {
    sys::RESULT_READ(buffer.as_mut_ptr() as usize, buffer.len()) as usize
}
