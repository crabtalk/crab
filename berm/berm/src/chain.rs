//! Chains: one call from outside and every call it made, numbered within the
//! process.

use std::sync::atomic::{AtomicU64, Ordering};

/// The last [`Callsite::chain`](crate::Callsite::chain) handed out.
static CHAINS: AtomicU64 = AtomicU64::new(0);

/// A [`Callsite::chain`](crate::Callsite::chain) no chain in this process has
/// had, for [`Service::call_as`](crate::Service::call_as).
pub fn next() -> u64 {
    CHAINS.fetch_add(1, Ordering::Relaxed) + 1
}

/// A runtime chain number and the host's call record, when it has one.
pub struct Id {
    pub(crate) number: u64,
    pub(crate) record: Option<u64>,
}

impl Id {
    /// Start a new chain whose first call has a host record.
    pub fn recorded(call: u64) -> Self {
        Self {
            number: next(),
            record: Some(call),
        }
    }
}

impl From<u64> for Id {
    fn from(number: u64) -> Self {
        Self {
            number,
            record: None,
        }
    }
}
