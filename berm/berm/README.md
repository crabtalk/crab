# berm

The runtime of services. A service is one hash-pinned WebAssembly module; berm compiles it once, holds it by the name it answers to, and instantiates it per invocation under wasmtime — nothing survives the call.

```rust
let calls = Calls::by_name(Calls::DEFAULT_DEPTH);
let berm = Berm::new(&engine, calls, vec![]);
berm.deploy("example", &wasm)?;
let outcome = berm.call(None, "example", "echo", br#"{"query":"hi"}"#.to_vec()).await?;
```

## What a guest can reach

The linked Wasm imports identify the host calls a service needs. A host refuses deployment when it cannot serve an imported call.

berm serves `berm.call`, which resolves a name against the set berm already holds, and the reads of a call's own context: `berm.caller`, `berm.user`, `berm.client`, `berm.client.ip`, `berm.now`, `berm.trace` and `berm.deny`.

`berm.client` returns the daily opaque key; `berm.client.ip` returns the outside IP. Both follow the call chain.

The embedder registers host capabilities as `Syscall`s. Known ABI calls with no registered implementation fail when invoked; the hosting layer refuses deploying images that import them. The standard calls — `berm.get`/`berm.set`, `berm.emit`, `berm.sql`, `berm.call.after`, `berm.http.fetch`, `berm.http.proxy` — come from `berm::syscall`'s constructors, which read and check the request and hand the work to closures the embedder supplies. `berm.http.fetch` and `berm.http.proxy` refuse a host the service's manifest does not allow under `[http] fetch` or `[http] proxy`, and substitute `{secret:NAME}` in header values so the guest never holds a secret. `berm.http.proxy` meters the bytes it sends and receives as `proxied`.

## What an invocation runs under

A guest cannot decline how far a chain of services may nest, the depth `Calls` is built with, or how long one may run, `bound/watchdog.rs`.

`Manifest::try_from(bytes)` reads what an image claims to be without compiling or running it.

```sh
cargo run --example measure    # prices a host call, an invocation, an allocation
```

## License

AGPL-3.0-only
