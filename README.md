# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crab.talk-blue)](https://crab.talk/docs)

The agent for the CrabTalk platform: write a service, deploy it, call it, and watch what it uses, on CrabTalk Cloud or the local host.

## Install

```sh
cargo install crab-agent
```

## Sign in

```sh
crab login                 # CrabTalk Cloud, in the browser
crab login --email you@example.com   # or with a code emailed to the address
crab context use default   # or the local host on http://127.0.0.1:7777
```

A context names the host `crab` talks to, the way `docker context` picks a daemon. See [crab](crates/crab/README.md) for contexts, keys, orgs and billing.

## Write a service

```sh
crab new example
```

A service is a Rust library crate built to one WebAssembly module, with a `crab.toml` beside its `Cargo.toml` that names it. A function marked `#[tool]` is a tool, and its doc comment is what a model reads when deciding to call it:

```rust
use alloc::string::String;
use berm_lang::{Result, tool};

/// Echo the query back.
#[tool(public)]
pub fn echo(
    /// The text to echo back.
    query: String,
) -> Result<String> {
    Ok(query)
}
```

A tool is internal unless marked `#[tool(public)]`: only the host calls an internal tool, for the service's own wakes, crons and Stripe events. A call with no caller reaches only a tool also marked `anonymous`. See [berm-lang](berm/lang/README.md) and [Writing a Program](https://crab.talk/docs/build/writing).

## Deploy and call it

```sh
crab deploy
crab call example echo '{"query":"hello"}'
crab logs example
crab usage example
```

`crab build` builds the crate's image and prints its path; `crab deploy` builds it the same way and deploys it as its `crab.toml` says. Against Cloud it pushes the image to the registry `image` names and deploys that reference. Agents reach a service's tools over MCP.

## Use ours

[crabkit](crates/crabkit/README.md) collects the kits crab provides, a feature each. A service depends on crabkit with the kit's feature, `crabkit = { version = "0.1", features = ["auth"] }`, and calls it from its own tools as `crabkit::auth`. Linking a kit is using it: what the kit's `crab.toml` declares reaches the service's manifest.

- [auth](kits/auth/README.md): Sign-in for a product's users. GitHub and Google logins as sessions kept in its own database.
- [crawl](kits/crawl/README.md): An incremental crawler. robots.txt, sitemaps, and the pages of the kinds a program names, recrawled when they change.
- [lake](kits/lake/README.md): Feeds a source service follows. Items and threads fetched on its schedule into its database, and RSS and Atom reading.
- [neon](kits/neon/README.md): Neon Postgres over HTTP. Parameterized statements and transactions.
- [stripe](kits/stripe/README.md): Stripe subscriptions for a product's users. Checkout, the customer portal, and each user's plan.
- [supabase](kits/supabase/README.md): Supabase over HTTPS. PostgREST tables and functions, and Storage objects.

## berm, the runtime

Each service runs on berm. A service is one WebAssembly module: berm pins it by hash, compiles it once, and instantiates it per invocation. It reaches the world only through the syscalls it was given, and that table can be read before it runs and is fixed once it does. Its tools are served over HTTP, and over MCP to agents such as Claude Code, Codex and OpenCode.

`crab serve` runs berm as a service of your own:

```sh
crab serve &
curl -X PUT --data-binary @example.wasm 'http://127.0.0.1:7777/programs/example?public=echo'
```

And `berm` embeds it in a host of your own:

```rust
let calls = Calls::by_name(call::DEFAULT_CALL_DEPTH);
let berm = Berm::new(&engine, calls, vec![], storage::Memory::new());
berm.deploy("example", &wasm)?;

// The outer result is the host's — a missing tool, a trap. The inner one is
// the program reporting failure, which is a result the model should see.
match berm.call(None, "example", "echo", br#"{"query":"hello"}"#.to_vec()).await?.result {
    Ok(result) => println!("{result}"),
    Err(failure) => eprintln!("{failure}"),
}
```

See [berm](berm/berm/README.md), [Running the Service](https://crab.talk/docs/build/local) and [Publishing a Program](https://crab.talk/docs/services/deploy).

## Building

```sh
cargo test --workspace
```

Rust 2024 edition, POSIX only. Building a service additionally needs the guest target:

```sh
rustup target add wasm32-unknown-unknown
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
