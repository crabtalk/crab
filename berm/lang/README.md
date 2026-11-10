# berm-lang

Build a service for [berm](https://crab.talk/docs).

A service is one WebAssembly module, confined to its own memory, reaching the world only through the host calls it was given. This crate owns the ABI, so you never see a call number or a pointer pair.

```rust
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Echo the query back.
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

A function marked `#[tool(...)]`, in any module of the service's crate, is a tool, and its doc comment is what the model reads when deciding to call it. Its parameters are its arguments — their types, their doc comments, and `Option` for the ones the model may omit — and the `Ok` of its `Result` is its reply. An `Option<T>` result permits `null`, represented by `anyOf` in its JSON Schema. `#[tool]` takes `query`, `cron`, `stripe`, `webhook` and `events = "name"`. `#[derive(Schema)]` gives a type its JSON Schema and its serde impls, in any module or crate.

Enums with payloads use external snake-case tags: `Platform::Grok(Connection { … })` is `{"grok":{…}}`. Payload variants accept one unnamed field or named fields. Enums containing only unit variants remain strings.

**A tool is internal unless its table in `crab.toml` gives it `access`.** Only the host calls an internal tool, for the service's own wakes, crons, Stripe events and webhooks.

```toml
[tools.echo]
access = "public"
title = "Echo a query"
```

A `public` tool can be called by any account the host authenticates and any service allowed to reach it, with arguments the caller chooses. `anonymous` is public and also takes a call from outside with no caller. `accounts` and `services` are restricted: the first takes callers from outside alone, the second other services alone. `title` is the tool's display name, at most 60 characters on one line. A cron, stripe or webhook tool takes no `access`. `berm_lang::caller()` says who called, as the host set it; a tool that limits its callers checks it.

Off the guest's target this is an ordinary library, which is why the `cfg_attr` above is worth copying: tools then run under `cargo nextest run` with no wasm toolchain and no daemon. See `berm_lang::test`.

## Durable event sources

`#[tool(events = "message.created")]` marks a read-only history source. It takes `cursor: Option<String>` alongside its filters and returns `berm_lang::abi::events::Page<T>`, where `T: Schema` is the event payload. Give the tool ordinary caller access in `crab.toml`; each read runs under that authorization.

With no cursor, return an empty page at the current head. With a cursor, return committed events strictly after it, in order. Every event carries a stable `id`, its own resume `cursor`, creation time `at` in epoch milliseconds, and typed `data`. The page cursor equals its last event's cursor; an empty page preserves or advances the checkpoint. Cursors must survive restarts. Report expired history as an error. Persist history in the same transaction as the application change. Diagnostic `emit` logs remain available after failed calls and cannot serve this contract.

The host can read these sources through `Host::event_page`. MCP subscriptions and callback delivery are not implemented.

## Standard-library dependencies

Services using standard-library dependencies enable `berm-lang = { workspace = true, features = ["std"] }` and omit `#![no_std]`. This selects Rust’s standard panic handler and allocator. The default remains `no_std`. HTTP and other host operations still use the berm ABI.

## Building

A service is a library crate with `[lib] crate-type = ["cdylib", "rlib"]` and a `crab.toml` beside its `Cargo.toml`; it has no bin target and no `fn main`. [Writing a Service](https://crab.talk/docs/development/complete-service#the-files) has the layout.

```sh
crab build
```

`crab build` adds the `wasm32-unknown-unknown` target through rustup when the toolchain lacks it, and prints the path of the finished image, `berm/<crate>.wasm` under cargo's target directory. A bare `cargo build` writes an image without the sections the build appends, which a host refuses.

Leaving `[build] target` unset keeps the host build, and therefore `cargo nextest run`, working without a flag.

The resulting `.wasm` is the whole artifact: one file, every platform.

## Runtime secret resources

Declare `secret_resources = true` and the destination under `[http]` to use separately provisioned secret resources. The owner grants a resource ID to the deployed service for exact hosts. Use `{secret:ID}` in HTTP headers or bodies; the host substitutes the value only when the grant and HTTP permission allow it. An absent or revoked grant refuses dispatch. Creating or rotating resources does not rebuild the service.

## Reaching a runtime

Past its arguments, a service reaches who called, its own state, a database, its vars, logs, HTTP, other services, wakes and the clock, each through a syscall its host serves. [The Guest API](https://crab.talk/docs/development/api) covers each.

## License

Apache-2.0
