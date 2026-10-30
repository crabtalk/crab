//! Build a berm service.
//!
//! A service is one WebAssembly module, confined to its own memory and reaching
//! the world only through the host calls it was given. Each `#[tool]` function
//! is a tool; its doc comment is what the model reads when deciding whether to
//! call it, its parameters are its arguments, and the `Ok` of its [`Result`] is
//! its reply, each typed by [`Schema`]. A service is a `cdylib` built for
//! `wasm32-unknown-unknown` by `crab build`, which appends what its `crab.toml`
//! and its kits declare; a host refuses an image cargo alone links. berm serves
//! [`call`], [`emit`] ([`emit!`] for an `#[event]` struct), [`now`] and
//! [`random`] itself; [`after`], [`get`], [`set`], [`http::fetch`] and
//! [`http::proxy`] trap as unknown host calls under a host that does not
//! register them. What [`set`] writes is kept only if the call the host started
//! returns a result; what [`emit`] emits is kept whatever it returns.
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

#![no_std]

// Off a guest target the stand-in host in `sys/stub.rs` runs on std.
#[cfg(not(target_arch = "wasm32"))]
extern crate std;

extern crate alloc;

// So `#[derive(Schema)]` resolves `::berm_lang` inside this crate too.
extern crate self as berm_lang;

pub use berm_abi as abi;
pub use berm_codegen::{Schema, declare_service, error_code, event, syscalls, tool};
pub use call::{CallError, Caller, User, after, call, caller, client, deny, limit, user, vouch};
#[doc(hidden)]
pub use emit::event as __event;
pub use emit::{Event, emit};
pub use error::{Error, Result};
pub use host::{args_len, now, random, trace};
pub use schema::Schema;
pub use sql::{rows, sql};
pub use store::{get, get_json, set, set_json};
pub use var::var;
// Named as `::berm_lang::serde` by what the macros generate.
pub use serde;
pub use serde_json;

mod call;
#[doc(hidden)]
pub mod dispatch;
mod emit;
mod error;
mod heap;
#[doc(hidden)]
pub mod host;
pub mod http;
#[doc(hidden)]
pub mod json;
pub mod schema;
mod sql;
mod store;
pub mod test;
mod var;

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
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        host::trace(location.file());
    }
    sys::abort()
}
