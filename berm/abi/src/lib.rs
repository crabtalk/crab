//! The contract between a berm service and its host: host-call numbers,
//! request framing, export names and image sections.
//!
//! A host call's number is derived from its name by [`hash`] (RFC 0205). Every
//! one takes a request at `(ptr, len)` and answers with a single word: a call
//! that needs no request is handed a pair it ignores, and one whose answer is
//! longer than a word stages it for the guest to pull.

#![no_std]

extern crate alloc;

pub mod wire;

/// The ABI version a service is built against and a host speaks. A host
/// refuses a service built against another.
pub const ABI_VERSION: u32 = 6;

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

/// Write a UTF-8 message to the host's trace. `(ptr, len) -> 0`
pub const HOST_TRACE: u64 = hash("berm.trace");
/// Byte length of this invocation's argument blob. `() -> len`
pub const HOST_ARG_LEN: u64 = hash("berm.args.len");
/// Copy the argument blob into guest memory. `(ptr, cap) -> full length`
pub const HOST_ARG_READ: u64 = hash("berm.args.read");
/// Finish this invocation with a result. `(ptr, len) -> 0`
pub const HOST_DONE: u64 = hash("berm.done");
/// Fail this invocation with a message. `(ptr, len) -> 0`
///
/// The other half of [`HOST_DONE`]. A tool's export takes nothing and returns
/// nothing; its answer travels through these two.
pub const HOST_FAIL: u64 = hash("berm.fail");
/// Copy the staged syscall result into guest memory. `(ptr, cap) -> full length`
pub const HOST_RESULT_READ: u64 = hash("berm.result.read");

/// Set on a staged length when the bytes are an error message. A length never
/// reaches this bit on its own.
pub const ERROR: u64 = 1 << 63;

/// Set beside [`ERROR`] when the host refused the call outright — nothing ran
/// on the other side of it.
pub const REFUSED: u64 = 1 << 62;

/// FNV-1a over the syscall's name, evaluated at compile time.
pub const fn hash(name: &str) -> u64 {
    let bytes = name.as_bytes();
    let mut result: u64 = 0xcbf2_9ce4_8422_2325;
    let mut at = 0;
    while at < bytes.len() {
        result ^= bytes[at] as u64;
        result = result.wrapping_mul(0x0000_0100_0000_01b3);
        at += 1;
    }
    result
}

/// The name a host registers to serve [`HOST_CALL`].
pub const CALL: &str = "berm.call";

/// Call a tool on another service. `(ptr, len) -> staged length`
///
/// Request fields are the service, the tool, and the argument blob; the reply
/// is the tool's result, staged like any other. The runtime serves this one
/// itself.
pub const HOST_CALL: u64 = hash(CALL);

/// The name a host registers to serve [`HOST_CALL_AFTER`].
pub const CALL_AFTER: &str = "berm.call.after";

/// Call a tool later, replacing whatever this service had pending.
/// `(ptr, len) -> staged length`
///
/// Request fields are the delay in milliseconds, the service, the tool and the
/// argument blob; the reply is empty. One wake per service that arms it.
pub const HOST_CALL_AFTER: u64 = hash(CALL_AFTER);

/// The name a host registers to serve [`HOST_HTTP_FETCH`].
pub const HTTP_FETCH: &str = "berm.http.fetch";

/// Make one HTTP request. `(ptr, len) -> staged length`
///
/// Request fields are the method, the URL, the body, the milliseconds to
/// wait for an answer as decimal text, empty for the call's deadline alone,
/// and the most bytes of the answer's body the guest takes as decimal text,
/// empty for all of it, then header names and values in turn. The reply is
/// the status as decimal text and the body, then header names and values in
/// turn. A header value or the body may carry `{secret:NAME}`, which the host
/// substitutes; the guest never holds the value.
pub const HOST_HTTP_FETCH: u64 = hash(HTTP_FETCH);

/// The name a host registers to serve [`HOST_HTTP_PROXY`].
pub const HTTP_PROXY: &str = "berm.http.proxy";

/// Make one HTTP request through the host's proxy. The request and reply are
/// [`HOST_HTTP_FETCH`]'s.
pub const HOST_HTTP_PROXY: u64 = hash(HTTP_PROXY);

/// The name a host registers to serve [`HOST_HTTP_FETCH_ALL`].
pub const HTTP_FETCH_ALL: &str = "berm.http.fetch_all";

/// Make a batch of HTTP requests at once, each to a host the manifest
/// declares under [`HTTP_FETCH`]. `(ptr, len) -> staged length`
///
/// Each request field is one [`HOST_HTTP_FETCH`] request, framed. Each reply
/// field answers the request in its place, framed: `ok`, then
/// [`HOST_HTTP_FETCH`]'s reply fields; or `refused` or `failed`, then why.
pub const HOST_HTTP_FETCH_ALL: u64 = hash(HTTP_FETCH_ALL);

/// The name a host registers to serve [`HOST_HTTP_PROXY_ALL`].
pub const HTTP_PROXY_ALL: &str = "berm.http.proxy_all";

/// [`HOST_HTTP_FETCH_ALL`] through the host's proxy, each request to a host
/// the manifest declares under [`HTTP_PROXY`]. The request and reply are
/// [`HOST_HTTP_FETCH_ALL`]'s.
pub const HOST_HTTP_PROXY_ALL: u64 = hash(HTTP_PROXY_ALL);

/// The name a host registers to serve [`HOST_EMIT`].
pub const EMIT: &str = "berm.emit";

/// Emit one log. `(ptr, len) -> staged length`
///
/// Request fields are the data, then each topic, all UTF-8; the reply is
/// empty. The host takes the log, or has it held in the chain's journal until
/// it reaches the host.
pub const HOST_EMIT: u64 = hash(EMIT);

/// The name a host registers to serve [`HOST_SQL`].
pub const SQL: &str = "berm.sql";

/// Run one SQL statement against this service's own database.
/// `(ptr, len) -> staged length`
///
/// Request fields are the statement and its parameters as a JSON array. The
/// reply is JSON: `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
pub const HOST_SQL: u64 = hash(SQL);

/// The name a host registers to serve [`HOST_GET`].
pub const GET: &str = "berm.get";
/// The name a host registers to serve [`HOST_SET`].
pub const SET: &str = "berm.set";

/// Read one of this service's own keys. `(ptr, len) -> staged length`
///
/// The request is the key; the reply is one field when the key is set and no
/// fields when it is not. Neither this nor [`HOST_SET`] carries a service: the
/// keyspace is whichever service is asking.
pub const HOST_GET: u64 = hash(GET);

/// Write one. `(ptr, len) -> staged length`
///
/// Request fields are the key and the value; the reply is empty. The host takes
/// the write, or has it held in the chain's journal until it reaches the host.
pub const HOST_SET: u64 = hash(SET);

/// Who made this call. `(ptr, len) -> staged length`, the pair ignored.
///
/// The reply is no fields when the host started the chain on the service's
/// own behalf, else two: `outside` or `service`, then the name. The runtime
/// serves this one itself.
pub const HOST_CALLER: u64 = hash("berm.caller");

/// The end user the chain runs for. `(ptr, len) -> staged length`, the pair
/// ignored.
///
/// The reply is no fields when neither the embedder nor a service vouched for
/// one, else the issuer and the subject. The runtime serves this one itself.
pub const HOST_USER: u64 = hash("berm.user");

/// The name a manifest declares to make [`HOST_VOUCH`].
pub const VOUCH: &str = "berm.vouch";

/// Vouch for the end user the chain runs for from here on. `(ptr, len) ->
/// staged length`
///
/// The request is the subject; the issuer is the service, as the services it
/// calls are told it is named. The reply is empty. Refused to a service whose
/// manifest does not declare [`VOUCH`], and in a chain that already runs for a
/// user. The runtime serves this one itself, and every call made after
/// carries the user.
pub const HOST_VOUCH: u64 = hash(VOUCH);

/// The outside client the chain runs for. `(ptr, len) -> staged length`, the
/// pair ignored.
///
/// The reply is no fields when the embedder supplied none, else one: an opaque
/// key that names the client for one day. The runtime serves this one itself.
pub const HOST_CLIENT: u64 = hash("berm.client");

/// Mark this invocation's failure as refusing its caller. `() -> 0`
pub const HOST_DENY: u64 = hash("berm.deny");

/// Lower this invocation's budget, once: `(ptr, len)` holds its CPU in
/// microseconds, its rows and its bytes of memory, each a little-endian `u64`,
/// `0` keeping that one. `-> staged length`
///
/// Refused a second time in one invocation, past a budget, and below the
/// memory it holds already.
pub const HOST_LIMIT: u64 = hash("berm.limit");

/// Milliseconds since the Unix epoch, as the host reads its clock. `() -> millis`
pub const HOST_NOW: u64 = hash("berm.now");

/// Fill `(ptr, len)` with bytes from the operating system's secure random
/// source. `(ptr, len) -> 0`
pub const HOST_RANDOM: u64 = hash("berm.random");

/// Prefix on every tool's exported symbol; a tool called `init` does not
/// collide with the exports the ABI reserves. A tool takes nothing and returns
/// nothing — see [`HOST_FAIL`].
pub const TOOL_PREFIX: &str = "berm_tool_";
