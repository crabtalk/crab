//! The contract between a berm service and its host: host calls, request
//! framing, export names and image sections.
//!
//! A host call is one wasm import from the `berm` module, named as
//! [`host_calls!`] lists it. Every one takes a request at `(ptr, len)` and
//! answers with a single word: a call that needs no request is handed a pair it
//! ignores, and one whose answer is longer than a word stages it for the guest
//! to pull.
//!
//! An image built against any earlier `berm-lang` keeps running: a host call's
//! meaning never changes, a changed one gets a new name such as
//! `berm.http.fetch.v2`, the old name keeps being served, and manifest records
//! only gain fields with defaults.

#![no_std]

extern crate alloc;

pub mod wire;

/// The version of the framing a service is built against and a host speaks:
/// `(ptr, len)` and staged replies. A host refuses a service built against
/// another. Adding or changing a host call does not change it.
pub const ABI_VERSION: u32 = 7;

/// The module every host call is imported from.
pub const MODULE: &str = "berm";

/// The section of what the compiler wrote of a service. A wasm custom
/// section, which is what `#[link_section]` emits on `wasm32`. It holds JSON
/// records back to back, in the order the linker joined them: one
/// `{"tool": …, "abi_version": …, "crate": …}` per `#[tool]` and one
/// `{"event": …}` per `#[event]`.
pub const ABI_SECTION: &str = ".berm.abi";

/// The section the build appends for the service: one service record.
pub const SERVICE_SECTION: &str = ".berm.service";

/// The section the build appends for the service's kits: a JSON array of kits,
/// by name.
pub const KITS_SECTION: &str = ".berm.kits";

/// Set on a staged length when the bytes are an error message. A length never
/// reaches this bit on its own.
pub const ERROR: u64 = 1 << 63;

/// Set beside [`ERROR`] when the host refused the call outright — nothing ran
/// on the other side of it.
pub const REFUSED: u64 = 1 << 62;

/// Hands `$then!` every host call this ABI defines, each as its doc comments,
/// its constant and its name.
#[macro_export]
macro_rules! host_calls {
    ($then:ident) => {
        $then! {
            /// Write a UTF-8 message to the host's trace. `(ptr, len) -> 0`
            TRACE = "berm.trace";
            /// Byte length of this invocation's argument blob. `() -> len`
            ARG_LEN = "berm.args.len";
            /// Copy the argument blob into guest memory. `(ptr, cap) -> full length`
            ARG_READ = "berm.args.read";
            /// Finish this invocation with a result. `(ptr, len) -> 0`
            DONE = "berm.done";
            /// Fail this invocation with a message. `(ptr, len) -> 0`
            ///
            /// The other half of [`DONE`]. A tool's export takes nothing and returns
            /// nothing; its answer travels through these two.
            FAIL = "berm.fail";
            /// Copy the staged syscall result into guest memory. `(ptr, cap) -> full length`
            RESULT_READ = "berm.result.read";

            /// Call a tool on another service. `(ptr, len) -> staged length`
            ///
            /// Request fields are the service, the tool, and the argument blob; the reply
            /// is the tool's result, staged like any other. The runtime serves this one
            /// itself.
            CALL = "berm.call";

            /// Call a tool later, replacing whatever this service had pending.
            /// `(ptr, len) -> staged length`
            ///
            /// Request fields are the delay in milliseconds, the service, the tool and the
            /// argument blob; the reply is empty. One wake per service that arms it.
            CALL_AFTER = "berm.call.after";

            /// Make one HTTP request. `(ptr, len) -> staged length`
            ///
            /// Request fields are the method, the URL, the body, the milliseconds to
            /// wait for an answer as decimal text, empty for the call's deadline alone,
            /// and the most bytes of the answer's body the guest takes as decimal text,
            /// empty for all of it, then header names and values in turn. The reply is
            /// the status as decimal text and the body, then header names and values in
            /// turn. A header value or the body may carry `{secret:NAME}`, which the host
            /// substitutes; the guest never holds the value.
            HTTP_FETCH = "berm.http.fetch";

            /// Make one HTTP request through the host's proxy. The request and reply are
            /// [`HTTP_FETCH`]'s.
            HTTP_PROXY = "berm.http.proxy";

            /// Make a batch of HTTP requests at once, each to a host the manifest
            /// declares under [`HTTP_FETCH`]. `(ptr, len) -> staged length`
            ///
            /// Each request field is one [`HTTP_FETCH`] request, framed. Each reply
            /// field answers the request in its place, framed: `ok`, then
            /// [`HTTP_FETCH`]'s reply fields; or `refused` or `failed`, then why.
            HTTP_FETCH_ALL = "berm.http.fetch_all";

            /// [`HTTP_FETCH_ALL`] through the host's proxy, each request to a host
            /// the manifest declares under [`HTTP_PROXY`]. The request and reply are
            /// [`HTTP_FETCH_ALL`]'s.
            HTTP_PROXY_ALL = "berm.http.proxy_all";

            /// Emit one log. `(ptr, len) -> staged length`
            ///
            /// Request fields are the data, then each topic, all UTF-8; the reply is
            /// empty. The host takes the log, or has it held in the chain's journal until
            /// it reaches the host.
            EMIT = "berm.emit";

            /// Run one SQL statement against this service's own database.
            /// `(ptr, len) -> staged length`
            ///
            /// Request fields are the statement and its parameters as a JSON array. The
            /// reply is JSON: `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
            SQL = "berm.sql";

            /// Read one var of this service's `[vars]`. `(ptr, len) -> staged length`
            ///
            /// The request's one field is the var's name; the reply is the value this
            /// deployed copy holds, or its default, as UTF-8. A var the manifest does not
            /// declare is refused.
            VAR = "berm.var";

            /// Read one of this service's own keys. `(ptr, len) -> staged length`
            ///
            /// The request is the key; the reply is one field when the key is set and no
            /// fields when it is not. Neither this nor [`SET`] carries a service: the
            /// keyspace is whichever service is asking.
            GET = "berm.get";

            /// Write one. `(ptr, len) -> staged length`
            ///
            /// Request fields are the key and the value; the reply is empty. The host takes
            /// the write, or has it held in the chain's journal until it reaches the host.
            SET = "berm.set";

            /// Who made this call. `(ptr, len) -> staged length`, the pair ignored.
            ///
            /// The reply is no fields when the host started the chain on the service's
            /// own behalf, else two: `outside` or `service`, then the name. The runtime
            /// serves this one itself.
            CALLER = "berm.caller";

            /// The end user the chain runs for. `(ptr, len) -> staged length`, the pair
            /// ignored.
            ///
            /// The reply is no fields when neither the embedder nor a service vouched for
            /// one, else the issuer and the subject. The runtime serves this one itself.
            USER = "berm.user";

            /// Vouch for the end user the chain runs for from here on. `(ptr, len) ->
            /// staged length`
            ///
            /// The request is the subject; the issuer is the service, as the services it
            /// calls are told it is named. The reply is empty. Refused to a service whose
            /// manifest does not declare [`VOUCH`], and in a chain that already runs for a
            /// user. The runtime serves this one itself, and every call made after
            /// carries the user.
            VOUCH = "berm.vouch";

            /// The outside client the chain runs for. `(ptr, len) -> staged length`, the
            /// pair ignored.
            ///
            /// The reply is no fields when the embedder supplied none, else one: an opaque
            /// key that names the client for one day. The runtime serves this one itself.
            CLIENT = "berm.client";

            /// Mark this invocation's failure as refusing its caller. `() -> 0`
            DENY = "berm.deny";

            /// Lower this invocation's budget, once: `(ptr, len)` holds its CPU in
            /// microseconds, its rows and its bytes of memory, each a little-endian `u64`,
            /// `0` keeping that one. `-> staged length`
            ///
            /// Refused a second time in one invocation, past a budget, and below the
            /// memory it holds already.
            LIMIT = "berm.limit";

            /// Milliseconds since the Unix epoch, as the host reads its clock. `() -> millis`
            NOW = "berm.now";

            /// Fill `(ptr, len)` with bytes from the operating system's secure random
            /// source. `(ptr, len) -> 0`
            RANDOM = "berm.random";
        }
    };
}

macro_rules! names {
    ($($(#[$doc:meta])* $call:ident = $name:literal;)*) => {
        $($(#[$doc])* pub const $call: &str = $name;)*

        /// Every host call this ABI defines.
        pub const ALL: &[&str] = &[$($name),*];
    };
}

host_calls!(names);

/// Prefix on every tool's exported symbol; a tool called `init` does not
/// collide with the exports the ABI reserves. A tool takes nothing and returns
/// nothing — see [`HOST_FAIL`].
pub const TOOL_PREFIX: &str = "berm_tool_";
