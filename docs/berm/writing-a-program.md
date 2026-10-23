---
title: Writing a Program
---

`berm-lang` is the guest side. It owns the ABI, so an author never sees a call
number or a pointer pair.

```rust,ignore
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Answers its arguments back.
#[tool(public)]
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

A function marked `#[tool(...)]`, in any module of the program's crate, is a tool; a function without it is not. Its doc comment is the description the model reads when deciding whether to call it, so write it for that reader. `#[tool]` takes `public`, `anonymous`, `query`, `cron`, `stripe` and `callers(services)` or `callers(accounts)`.

What a program reaches past its arguments — who called, state, a database,
HTTP, other programs, wakes — is [the guest API](./guest-api.md).

## Public tools

A tool is internal unless marked `#[tool(public)]`: only its host calls it, for the program's own wakes, crons and Stripe events. A `public` tool is called by callers from outside, as the host authenticates them, and by other programs through `berm_lang::call`. `callers(services)` limits it to other programs, and `callers(accounts)` to callers from outside. `public` in `crab.toml` lists the tools the deploy may make public.

## Arguments

A tool's parameters are the properties of its arguments, a JSON object. A
parameter's doc comment is its description, and an `Option` parameter may be
omitted. A tool with no parameters takes any arguments and reads none.

Each parameter's type implements `Schema`: strings, numbers, `bool`, `Vec`,
`Option`, `BTreeMap<String, T>`, and any type with `#[derive(Schema)]`, in any
module or crate. The derive takes a struct with named fields — an object titled
by its name, its fields' doc comments their descriptions — or an enum of unit
variants, a string that is one of them in `snake_case`. A `Schema` type owns
its data, and the derive gives it serde's `Serialize` and `Deserialize`.

## Results and failure

A tool returns `berm_lang::Result<T>`. `T` is a `Schema` type, written as the
JSON result and declared in the manifest as `returns`; `()` writes nothing. An
absent `Option` field is left out rather than written as `null`.

`Err(berm_lang::Error)` reports failure through the ABI, its `message` the
caller's to read. It is not a trap: the host sees a tool that ran and failed. A
syscall's `CallError` and a `String` convert into it, so `?` reports either. An
enum marked `#[error_code]`, each variant with `#[msg("…")]`, converts into it
with that message. `berm_lang::deny(message)` fails the call as denied.

## Building

A program is a library crate with `berm-lang` as a dependency, built as a `cdylib`, with a `crab.toml` beside its `Cargo.toml`. It has no bin target and no `fn main`. `crab new <name>` scaffolds one.

```toml
# Cargo.toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
berm-lang = { git = "https://github.com/crabtalk/crab" }
```

```toml
# crab.toml
name = "my-program"
```

`crab.toml` declares the program: the service it deploys as, and what it reaches past its arguments. Its keys are in [the manifest](manifest.md#bermtoml).

`crab build [dir]` builds the image for `wasm32-unknown-unknown`, which `rustup target add wasm32-unknown-unknown` installs, and prints its path, `berm/<crate>.wasm` under cargo's target directory, the crate's name with `-` as `_`. `crab deploy` builds it the same way. The build appends the `.berm.program` section, from the package and `crab.toml`, and the `.berm.kits` section, from the kits the program links. A bare `cargo build` writes an image without them, which a host refuses.

Off a guest target the crate is an ordinary library, so tools can be unit tested
natively instead of cross-compiling to run anything at all. `berm_lang::test`
stands in for the host: it holds the argument blob and collects what a program
logged. `test::answer` arranges what another program answers, and `test::respond` what an HTTP request answers by method and URL, with `test::fetched` the requests sent as the program wrote them, `{secret:NAME}` unsubstituted. A call reaching a syscall that has no stand-in panics naming it,
rather than reading a plausible zero.

`berm_host::test::TestHost`, behind `berm-host`'s `test` feature, runs the built
image instead: the real engine and host, with fetches answered from what the
test arranged, a clock the test sets, and wakes the test fires. A call's result
and meter, the logs and state it committed, and the wakes it armed are read back
from it.

`berm-fixture` in this repository is the worked example — the smallest real
program, and what berm's own tests run against.

## Anonymous calls

A call from outside with no caller — no account, or an app's call with no user — reaches only a tool marked `anonymous`, and the host refuses it any other. An `anonymous` tool is `public`, and not `callers(services)`. `berm_lang::caller()` answers `None` in it for such a call.

## Queries

`#[tool(query)]` marks a tool read-only. While it runs, and in every call it makes, `berm.set`, `berm.emit` and `berm.call.after` are refused, so it leaves nothing for the host to commit. A host may serve it over `GET`.

## Crons

`#[tool(cron)]` marks a tool its host calls each time its cron expression fires, whatever the last call ended as, and not while its owner is held. A cron tool takes no parameters and is not `public`. Its expression is five fields, minute to day of week, read in UTC: `*/5 * * * *`. The deploy sets it from `[crons]` in `crab.toml`, or its owner does while it runs; until one is set it is not called.

## Stripe events

`#[tool(stripe)]` marks the tool its host calls with each Stripe event its daemon takes for the program, and not while its owner is held. The event object is the argument object, so the tool's parameters are the event's fields it reads. A program has at most one, and it is not `public`. A tool is `cron` or `stripe`, not both.

## Events

An `#[event]` struct anywhere in the program's crate is a typed log, and a
`Schema` type. `berm_lang::emit!(Noted { word })` emits it under the topic
`Noted`, its data the struct as JSON. The manifest lists each event under
`events` with its doc comment and the schema of its fields.

Logs are what a program's developer reads to debug it: the host keeps each one beside the call it was emitted in, whatever that call ended as, for 30 days. A log holds at most 4 KiB, its data and topics together, and is refused past that. A program's data goes in its database, read by other programs through its query tools.

## Calling another program

The IDL of the program deployed as `hn`, which its host serves, is saved as
`idls/hn.json`. `berm_lang::declare_program!(hn)` reads it from the crate's directory or the
nearest ancestor with an `idls/`, and generates:

- `hn::types`: every titled object and string enum in its tools' arguments and
  results.
- `hn::calls`: one function per tool, taking its arguments in the IDL's key
  order, an optional one as an `Option`, and returning its result (`()` for a
  tool with none).
- `hn::NAME`, which every call names.

A tool the IDL marks `deprecated` is generated with `#[deprecated]`.

## Changing an interface

A tool's callers were built against its IDL. A change that breaks them — see
[the manifest](manifest.md#compatibility) — keeps the old tool beside the new
one, marked `#[deprecated]`, until no caller uses it.

The IDL is a file in the crate; it changes only when it is saved again.
