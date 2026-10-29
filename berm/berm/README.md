# berm

The runtime of programs. A program is one hash-pinned WebAssembly module; berm
compiles it once, holds it by the name it answers to, and instantiates it per
invocation under wasmtime — nothing survives the call.

```rust
let calls = Calls::by_name(Calls::DEFAULT_DEPTH);
let berm = Berm::new(&engine, calls, vec![]);
berm.deploy("example", &wasm)?;
let outcome = berm.call(None, "example", "echo", br#"{"query":"hi"}"#.to_vec()).await?;
```

## What a guest can reach

berm serves `berm.call`, which resolves a name against the set berm already
holds, and the reads of a call's own context: `berm.caller`, `berm.user`,
`berm.client`, `berm.now`, `berm.trace` and `berm.deny`.

Everything else is a `Syscall` the embedder registers, and that list is the
linker a program is instantiated with: a call to anything else traps because
nothing is registered for it, not because a check said no. The standard calls —
`berm.get`/`berm.set`, `berm.emit`, `berm.sql`, `berm.call.after`,
`berm.http.fetch`, `berm.http.proxy` — come from `berm::syscall`'s
constructors, which read and check the request
and hand the work to closures the embedder supplies. `berm.http.fetch` and
`berm.http.proxy` refuse a host the program's manifest does not declare under
the one called, and substitute `{secret:NAME}` in header values so the guest
never holds a secret. `berm.http.proxy` meters the bytes it sends and receives
as `proxied`.

## What an invocation runs under

A guest cannot decline how far a chain of programs may nest, the depth
`Calls` is built with, or how long one may run, `bound/watchdog.rs`.

`Manifest::try_from(bytes)` reads what an image claims to be without compiling or
running it.

```sh
cargo run --example measure    # prices a host call, an invocation, an allocation
```

## License

Apache-2.0
