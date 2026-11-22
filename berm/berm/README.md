# berm

The runtime of services. A service is one hash-pinned WebAssembly module, validated and compiled at deployment. Executable code is cached separately from deployment metadata and reloaded on demand. Each invocation gets a fresh Wasmtime instance and store; no guest state crosses between calls.

```rust
let calls = Calls::by_name(Calls::DEFAULT_DEPTH);
let berm = Berm::new(&engine, calls, vec![]);
berm.deploy("example", &wasm)?;
let outcome = berm.call(None, "example", "echo", br#"{"query":"hi"}"#.to_vec()).await?;
```

`Config::code_cache` bounds cached code by accounted image bytes (default 64 MiB); `code_idle` expires entries after inactivity (default five minutes). Zero capacity disables retention between calls. Active calls pin their executable generation until completion. Scope memory pressure reclaims cached code before refusing work; active references remain counted. These byte counts are estimates, not process RSS.

`cache_dir` enables Wasmtime's disk compilation cache. Relative paths resolve against the current working directory; the cache directory is created when missing. `Berm::restore` restores a previously validated deployment without applying new-deployment admission. A recorded footprint avoids compilation; a missing footprint is measured once and the executable discarded. The next call obtains executable code under the current scope limit. Reloading does not run deployment migrations or change stored data. Concurrent cold calls share a load, and reload wait is subject to the chain's deadline. Loading runs on Tokio's blocking pool; calls require a Tokio runtime.

## What a guest can reach

The linked Wasm imports identify the host calls a service needs. A host refuses deployment when it cannot serve an imported call.

berm serves `berm.call`, which resolves a name against the set berm already holds, and the reads of a call's own context: `berm.caller`, `berm.user`, `berm.client`, `berm.client.ip`, `berm.workflow.run`, `berm.service`, `berm.deadline`, `berm.call.id`, `berm.now`, `berm.trace` and `berm.deny`.

`berm.client` returns the daily opaque key; `berm.client.ip` returns the outside IP. Both follow the call chain.

The embedder registers host capabilities as `Syscall`s. Known ABI calls with no registered implementation fail when invoked; the hosting layer refuses deploying images that import them. The standard calls — `berm.get`/`berm.set`/`berm.delete`, `berm.emit`, `berm.sql`, `berm.call.after`, `berm.http.fetch` — come from `berm::syscall`'s constructors, which read and check the request and hand the work to closures the embedder supplies. Outbound requests use hosts declared under `[http] fetch`. The host substitutes `{secret:NAME}` placeholders into the request.

## What an invocation runs under

A guest cannot decline how far a chain of services may nest, the depth `Calls` is built with, or how long one may run, `bound/watchdog.rs`.

The SQLite execution backend interrupts SQL statements at the invocation deadline, including statements that have not produced a result row. An interrupted writing invocation rolls back its transaction.

`Manifest::try_from(bytes)` reads what an image claims to be without compiling or running it.

```sh
cargo run --example measure    # prices a host call, an invocation, an allocation
```

Workflow control exports run through `Service::control` with their configured CPU/memory budget and the service buffer limit. Each invocation gets fresh guest state. Only argument/result ABI plumbing is permitted; application syscalls, including helper-mediated calls and clocks, trap before dispatch. Controls do not open application transaction scopes. Their meters are returned through the request's `Meters` and reported to `Calls::invoked`. `Calls::control_enter` supplies resource admission separately from application transactions. The host executes accepted workflow graphs using these exports.

`Callsite::budget` contains the effective invocation limits. `workflow::scope` carries opaque spend authorization and an absolute attempt deadline through nested calls. Invocation scopes complete bookkeeping for every known outcome before their application changes commit or roll back.

## License

AGPL-3.0-only
