//! What bounds one invocation.
//!
//! Two bounds carried on the [`crate::Callsite`] a nested call is made from:
//! the depth a chain of services may nest to, and the [`watchdog`] deadline it
//! has to finish by. A third, the [`cpu`] time, is each invocation's own. None
//! is declinable by a guest, which is what separates them from a
//! [`crate::Syscall`]: a syscall is something a guest reaches for, and these
//! are conditions it runs under.

pub(crate) mod cpu;
pub(crate) mod watchdog;
