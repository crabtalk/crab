//! A bound on how long a guest may run.
//!
//! How long, and how a nested call inherits what is left, is berm's. Stopping
//! a guest that has run past it is wasmtime's, counting down an epoch.

use std::time::Duration;

/// How long a chain of invocations may run before it is asked to stop, unless
/// [`crate::Config::deadline`] says otherwise.
///
/// A guest suspended in a syscall cannot notice until the reply resolves, so
/// an embedder's syscall has to bound its own wait inside this. The bound
/// stops non-termination; a service doing slow but finite work finishes.
///
/// A nested invocation runs under its caller's deadline, so depth *n* buys
/// one deadline, not *n*.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(60);
