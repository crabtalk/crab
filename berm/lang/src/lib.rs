//! Build a berm service.
//!
//! A service is one WebAssembly module, confined to its own memory and reaching
//! the world only through the host calls it was given. Each `#[tool]` function
//! is a tool; its doc comment is what the model reads when deciding whether to
//! call it, its parameters are its arguments, and the `Ok` of its [`Result`] is
//! its reply, each typed by [`Schema`]. A service is a `cdylib` built for
//! `wasm32-unknown-unknown` by `crab build`, which appends what its `crab.toml`
//! and its kits declare; a host refuses an image cargo alone links. berm serves
//! [`call`], [`emit()`] ([`emit!`] for an `#[event]` struct), [`now`] and
//! [`random`] itself; [`after`], [`get`], [`set`], [`http::fetch`] and
//! [`http::proxy`] trap as unknown host calls under a host that does not
//! register them. Storage reads see earlier writes in the same tool call.
//! Storage changes commit only when the tool returns `Ok`; `Err`, a trap or
//! an exceeded deadline discards pending changes. What [`emit()`] emits is kept
//! whatever the call returns.
//!
//! ```ignore
//! #![cfg_attr(target_arch = "wasm32", no_std)]
//!
//! extern crate alloc;
//!
//! use alloc::{format, string::String};
//! use berm_lang::{Result, tool};
//!
//! /// Greets `name`.
//! #[tool]
//! pub fn greet(
//!     /// Who to greet.
//!     name: String,
//! ) -> Result<String> {
//!     Ok(format!("hello, {name}"))
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

// Off a guest target the stand-in host in `sys/stub.rs` runs on std.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "std")))]
extern crate std;

extern crate alloc;

// So `#[derive(Schema)]` resolves `::berm_lang` inside this crate too.
extern crate self as berm_lang;

pub use berm_abi as abi;
pub use berm_codegen::{
    Branch, Schema, declare_service, error_code, event, syscalls, tool, workflow,
};
pub use call::{
    CallError, Caller, Service, User, after, call, call_id, caller, client, client_ip, deny, limit,
    service, user, vouch,
};
#[doc(hidden)]
pub use emit::event as __event;
pub use emit::{Event, emit};
pub use error::{Error, Result};
pub use host::{args_len, deadline, now, random, trace};
pub use schema::Schema;
pub use serde_json::value::RawValue;
pub use sql::{rows, sql};
pub use store::{get, get_json, set, set_json};
pub use var::var;
// Named as `::berm_lang::serde` by what the macros generate.
pub use serde;
pub use serde_json;

mod call;
pub mod caller_storage;
#[doc(hidden)]
pub mod dispatch;
mod emit;
mod error;
mod events;
mod heap;
#[doc(hidden)]
pub mod host;
pub mod http;
#[doc(hidden)]
pub mod json;
pub mod key;
mod requests;
pub mod schema;
pub mod secret;
mod sql;
pub mod store;
pub mod test;
mod var;
pub mod workflow;

// `wasm.rs` makes real host calls; `stub.rs` is the host a test stands in for.
#[cfg_attr(target_arch = "wasm32", path = "sys/wasm.rs")]
#[cfg_attr(not(target_arch = "wasm32"), path = "sys/stub.rs")]
#[doc(hidden)]
pub mod sys;

/// What `declare_service!`'s types are written in, reachable without the
/// author's `extern crate alloc`.
#[doc(hidden)]
pub mod __private {
    pub use alloc::{collections::BTreeMap, string::String, vec::Vec};
}

/// Traces the panic's file and traps. Guest targets only.
#[cfg(all(target_arch = "wasm32", not(feature = "std")))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        host::trace(location.file());
    }
    sys::abort()
}
