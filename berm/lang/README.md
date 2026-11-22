# berm-lang

Build a service for [berm](https://crabtalk.ai/docs).

A service is one WebAssembly module, confined to its own memory, reaching the world only through the host calls it was given. This crate owns the ABI, so you never see a call number or a pointer pair.

```rust
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Echo the query back
#[tool]
pub fn echo(
    /// The text to echo back.
    query: String,
    /// Page number, zero-indexed.
    page: Option<u32>,
) -> Result<Echo> {
    Ok(Echo { query, page })
}

/// What `echo` answers.
#[derive(Schema)]
pub struct Echo {
    pub query: String,
    pub page: Option<u32>,
}
```

A function marked `#[tool(...)]`, in any module of the service's crate, is a tool, and its doc comment is what the model reads when deciding to call it. Its parameters are its arguments — their types, their doc comments, and `Option` for the ones the model may omit — and the `Ok` of its `Result` is its reply. An `Option<T>` result permits `null`, represented by `anyOf` in its JSON Schema. `#[tool]` takes `query`, `cron`, `webhook` and `events = "name"`. `#[derive(Schema)]` gives a type its JSON Schema and its serde impls, in any module or crate.

Enums with payloads use external snake-case tags: `Platform::Grok(Connection { … })` is `{"grok":{…}}`. Payload variants accept one unnamed field or named fields. Enums containing only unit variants remain strings.

A tool defaults to public caller access. Use `#[tool(access = "internal")]` for an internal tool; `anonymous`, `accounts` and `services` select other callers. Cron and both Standard Webhook and Stripe webhook handlers are internal. A public deployment with `services` access can accept authorized service callers from other accounts. The first doc-comment line is the title, at most 60 characters and without a final period. The remaining comment is its description. Price, catalog, `budget(...)`, idempotency and example tasks are attributes on `#[tool]`; all travel in the built image.

Off the guest's target this is an ordinary library, which is why the `cfg_attr` above is worth copying: tools then run under `cargo nextest run` with no wasm toolchain and no daemon. See `berm_lang::test`.

## Call descriptors and branches

`#[tool] fn compare(…)` also emits `CompareCall` beside the function, with the same visibility and public argument fields. It implements `workflow::Call`: `TARGET` is local, `Output` is the tool result type, and `CASES` describes union results. Type aliases preserve this contract. `declare_service!(mail)` adds `mail::SendCall` for its `send` tool; existing `mail::calls::send` and `mail::types` remain available. Imported descriptors retain the IDL's parameter and result schemas. An optional `idempotency_ms` on an imported tool declares its external redispatch window.

`#[derive(Schema, Branch)]` accepts enums whose variants each wrap one distinct payload type. `workflow::Branch` supplies case metadata and the selected name. `workflow::Projection<Payload>::project(value)` consumes the enum and returns its payload only for that case. Imported union clients implement the same traits with distinct `<Enum><Variant>Payload` types in their `types` module; tuple payloads use a transparent wrapper.

## Workflow graphs

Use a unit struct and an inherent impl marked with `#[workflow]`. Every method has one `#[step]`, `#[branch]` or `#[output]`; there is exactly one output. Parameters name earlier producers, with `input` reserved for the common input type.

```rust
use berm_lang::workflow;

struct EchoFlow;

/// Echo an input
#[workflow]
impl EchoFlow {
    #[step]
    fn first(input: &str) -> EchoCall {
        EchoCall { query: input.into(), page: None }
    }

    #[output]
    fn done(first: &Echo) -> String {
        first.query.clone()
    }
}
```

This uses the `EchoCall` and `Echo` from the tool above. A step returns a call descriptor, `Option<Call>`, or `Vec<Call>` with `#[step(each, max = N)]` (1–100). `#[step(price = N)]` charges the caller `N` USD micros per item of an `each` step, or per run of any other step, at least 100; its workflow needs caller access. A priced step is exact when its builder and guards read only the input and branches that do, so its count is known before the run, and bounded by `max` otherwise; `crab build`, `crab up`, `crab deploy` and `crab workflow inspect` print which. A branch returns a `Branch` enum. Any method may return the existing `Result<T>`. Dependencies are immutable references; each results use slices and optional dependencies use `Option<&T>`. String producers can be borrowed as `&str`; output methods returning `()` may omit the return type. Distinct branch payload types select cases. Required dependencies propagate guards; joins across exclusive paths and the final output use optional references. Failed or pending producers never become optional absence.

The macro emits typed adapters, one `berm_workflow_<workflow>_<node>` export per method, and a versioned static record in `.berm.abi`. `EchoFlow::WORKFLOW` exposes that record as JSON bytes. Unused nodes, unknown/forward dependencies, impossible required joins and guarded outputs are compile errors. No method body analysis is performed. Declare workflow configuration on `#[workflow(...)]`: public access, free task price, host control budgets, `max_cost = 100_000` USD micros, `checkpoint_bytes = 8_388_608` and seven-day retention are the defaults. Only the deployment schedule and its input stay under `[schedules]` in `crab.toml`. Build/deploy validates graph records and schedules without running controls. Native hosts prepare and execute accepted graphs through [`berm-host`](../host/README.md).

## Durable event sources

`berm_lang::key::id()` returns the API key ID authenticated by the host, or `None` when no key was supplied. It persists across nested calls and never contains a token. `caller()` still identifies the immediate caller; `user()` identifies the verified end user.

`workflow::run()?` gives a step tool the run and attempt IDs, its node and item index, and the manual caller or scheduled occurrence. Ordinary tools and calls made by the step tool receive `None`. `service()?` reads the deployment address and image digest, `deadline()` reads the milliseconds remaining in the chain, and `call_id()?` reads the call record ID (with the span for nested calls) or the process-local ID of an unrecorded call. These calls are available to tools. Workflow controls can read their inputs.

`#[tool(events = "message.created")]` marks a read-only history source. It takes `cursor: Option<String>` alongside its filters and returns `berm_lang::abi::events::Page<T>`, where `T: Schema` is the event payload. Declare its caller access on `#[tool]`; each read runs under that authorization.

With no cursor, return an empty page at the current head. With a cursor, return committed events strictly after it, in order. Every event carries a stable `id`, its own resume `cursor`, creation time `at` in epoch milliseconds, and typed `data`. The page cursor equals its last event's cursor; an empty page preserves or advances the checkpoint. Cursors must survive restarts. Report expired history as an error. Persist history in the same transaction as the application change. Diagnostic `emit` logs remain available after failed calls and cannot serve this contract.

The host can read these sources through `Host::event_page`. MCP subscriptions and callback delivery are not implemented.

## Standard-library dependencies

Services using standard-library dependencies enable `berm-lang = { workspace = true, features = ["std"] }` and omit `#![no_std]`. This selects Rust’s standard panic handler and allocator. The default remains `no_std`. HTTP and other host operations still use the berm ABI.

## Building

A service is a library crate with `[lib] crate-type = ["cdylib", "rlib"]` and a `crab.toml` beside its `Cargo.toml`; it has no bin target and no `fn main`. [Writing a Service](https://crabtalk.ai/docs/development/complete-service#the-files) has the layout.

```sh
crab build
```

`crab build` adds the `wasm32-unknown-unknown` target through rustup when the toolchain lacks it, and prints the path of the finished image, `berm/<crate>.wasm` under cargo's target directory. A bare `cargo build` writes an image without the sections the build appends, which a host refuses.

Leaving `[build] target` unset keeps the host build, and therefore `cargo nextest run`, working without a flag.

The resulting `.wasm` is the whole artifact: one file, every platform.

## Runtime secret resources

Declare `secret_resources = true` and the destination under `[http]` to use separately provisioned secret resources. On Cloud, an account owner grants a resource ID to a service for exact hosts. During a call, the service’s account or the authenticated calling account must own the resource. A scheduled run can use any grant made to the service. Use `{secret:ID}` in HTTP headers or bodies; the host substitutes the value only when the grant and HTTP permission allow it. An absent or revoked grant refuses dispatch. Creating or rotating resources does not rebuild the service.

HTTP credentials are substituted by the host at dispatch. Service code receives upstream response data, which can include credentials and can be retained on a replay tape.

## Stored values

`get` and `set` store any serde type as JSON, `get_raw` and `set_raw` store bytes as given, and `delete` removes a key. `get` fails with the key name when the stored value does not parse as the requested type. `caller_storage` has the same five functions over the verified caller's storage.

`http::Response::read_raw` consumes a provider's JSON body as `Box<RawValue>` without building a value tree. Empty responses become `null`; invalid JSON fails with the provider's unreadable-response error. Generated OpenAPI operations use it unless they declare a response handler or request the HTTP response itself.

## Storage transactions

Reads within a tool call see its earlier writes, including pending changes from `set`, `set_raw` and `delete`. Storage changes commit only when the tool succeeds with `Ok`; `Err`, a trap or an exceeded deadline discards its pending changes. Independent calls cannot read uncommitted changes. SQL statements and key/value writes share a transaction when SQL storage is declared. HTTP actions and changes already committed by other services are outside that transaction. See [Transactions](https://crabtalk.ai/docs/development/data#transactions).

## Reaching a runtime

Past its arguments, a service reaches who called, its own state, a database, its vars, logs, HTTP, other services, wakes and the clock, each through a syscall its host serves. [The Guest API](https://crabtalk.ai/docs/development/api) covers each.

`CallError::Refused` reports host policy. `CallError::Failed` reports an operation or transport failure and can occur before dispatch. Establish delivery status before retrying a call with side effects.

## License

Apache-2.0

`berm_lang::secret::owner(id, host)` reads ownership metadata for a resource granted to the deployment at that host, never its value. `http::fetch_all_literal_body` sends body bytes unchanged and substitutes secrets only in header values. Use it for user-supplied message content.

Repeat `example = "find recent Rust releases"` on `#[tool]` or an OpenAPI operation entry for up to three example tasks, at most 120 characters each. The built image stores them in `examples`, and `crab inspect` lists them.
