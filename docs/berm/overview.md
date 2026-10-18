---
title: Overview
---

A program is one WebAssembly module. berm pins it by hash, compiles it once, and
instantiates it per invocation: arguments go in through host calls, the result
comes back out through one, and nothing survives the call.

```rust,ignore
let calls = Calls::by_name(call::DEFAULT_CALL_DEPTH);
let berm = Berm::new(&engine, calls, vec![], storage::Memory::new());
berm.deploy("example", &wasm)?;

let outcome = berm.call(None, "example", "echo", br#"{"query":"hello"}"#.to_vec()).await?;
match outcome.result {
    Ok(result) => println!("{result}"),
    Err(failure) => eprintln!("{failure}"),
}
```

Two levels of result, because two things can go wrong and they are not the same
thing. The outer one is the host's: no such tool, a trap, a broken image. The
inner one, `Outcome::result`, is the program reporting failure through the ABI —
which is a result the model should see and react to, not an error the host
should swallow.

## Nothing survives the call

Every invocation gets a fresh instance, so guest memory is not carried between
calls. A program that wants something to outlive an invocation has to put it
somewhere a host is holding.

This is the opposite of a container, and it is why there is no lifecycle to
manage — nothing to start, stop, or restart. What is expensive is compilation,
not instantiation, and compilation is paid once per image.

## The linker is the boundary

A program reaches the world only through the *syscalls* it was given,
and that list is the linker it is instantiated with. A call to anything else
traps because nothing is registered for the number naming it — not because a
check ran and said no. There is no check to write, and none to forget.

berm serves `berm.call` itself, against the programs it holds, and the
reads of the call's own context: its caller, its user, its client, the time,
`berm.trace` and `berm.deny`. Everything else — state, logs, HTTP, a database, wakes — is a
`Syscall` the embedder passes to `Berm::new`, and that list is the whole of
what a guest can reach. berm's `syscall` module builds the standard ones, and
the embedder supplies what they act on.

## Where wasmtime stops and berm starts

wasmtime loads an image, generates native code for it, and calls it. It has no idea what a program is — no tools, no manifest, no arguments, no JSON.

berm is the layer that decides those things: that a tool is an export named `berm_tool_*`, that an image describes itself in a `.berm.abi` section, and that arguments arrive as a blob the guest pulls in. Every syscall is the same number, request and staged reply, carried by one import, `berm.syscall`.
