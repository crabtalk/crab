//! The process's one async runtime, on which `crab`'s synchronous commands run
//! the SDK's calls.

use std::{future::Future, sync::LazyLock};
use tokio::runtime::{Builder, Runtime};

/// One runtime for the process, on every core for the local host's calls.
/// `berm-oci`'s blocking `reqwest` panics inside a runtime, so `crab`'s
/// commands stay synchronous.
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime builds")
});

/// Run `future` to completion.
pub fn wait<F: Future>(future: F) -> F::Output {
    RUNTIME.block_on(future)
}
