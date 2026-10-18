# berm

[![CI](https://github.com/crabtalk/berm/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/berm/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/berm.svg)](https://crates.io/crates/berm)
[![Docs](https://img.shields.io/badge/docs-berm.run-blue)](https://berm.run/docs/)

The OS for agent harnesses.

Programs are served over HTTP, and their `#[anonymous]` tools over MCP to agents
such as Claude Code, Codex and OpenCode. Each runs under a syscall table that is
readable before it runs and fixed once it does.

A program is one WebAssembly module. berm pins it by hash, compiles it once, and
instantiates it per invocation — arguments go in through syscalls, the result
comes back out through one, and nothing survives the call.

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

A program's public tools are reachable by name from anything deployed beside it that declares it. Beyond
that a program reaches the world only through the *syscalls* it was given, and
that table is the linker it is instantiated with — a call to anything else traps
because nothing is registered for it, not because a check said no. berm serves
calls between programs and the facts of a call; state, HTTP, a database and
wakes are syscalls the embedder registers, because where bytes persist and what
a request may reach are decisions about a host.

`Manifest::from_image(bytes)` reads what an image claims to be — its tools,
their schemas, when to reach for them, and what it will reach for itself —
without compiling or running it.

**A tool is internal unless marked `#[public]`.** Only the host calls an internal tool, for the program's own wakes, crons and Stripe events. A `#[public]` tool can be called by any account the host authenticates and any program allowed to reach it, with arguments the caller chooses; `berm_lang::caller()` says who called. A call with no caller reaches only a tool also marked `#[anonymous]`, and that tool reads no caller. A deploy that makes a tool public is refused until the deployer confirms it.

## Running programs

`bermd` is a long-running service that deploys programs and serves their tools.

```sh
bermd &
curl -X PUT --data-binary @example.wasm 'http://127.0.0.1:7777/programs/example?public=echo'
curl http://127.0.0.1:7777/programs
```

See [Running the Service](https://berm.run/docs/berm/service/) for the control
API and what a deployed program can reach.

## Moving programs

A program is one file, so it travels as one OCI layer with no tarball around it
— and because the layer is the image and nothing else, the digest a registry
addresses it by is sha256 of the file. See
[Publishing a Program](https://berm.run/docs/berm/publishing/).

## Documentation

- **[Guide and design notes](https://berm.run/docs/)** — how it
  works and why, with worked examples.
- **API reference** — `cargo doc --open`.

## Building

```sh
cargo test --workspace
```

Rust 2024 edition, POSIX only. Building a program additionally needs the guest
target:

```sh
rustup target add wasm32-unknown-unknown
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
