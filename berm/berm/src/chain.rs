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
