# berm-lang

Build a program for [berm](https://crab.talk/docs).

A program is one WebAssembly module, confined to its own memory, reaching the world only through the host calls it was given. This crate owns the ABI, so you never see a call number or a pointer pair.

```rust
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::string::String;
use berm_lang::{Result, Schema, tool};

/// Echo the query back.
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

A function marked `#[tool(...)]`, in any module of the program's crate, is a tool, and its doc comment is what the model reads when deciding to call it. Its parameters are its arguments — their types, their doc comments, and `Option` for the ones the model may omit — and the `Ok` of its `Result` is its reply. `#[tool]` takes `public`, `anonymous`, `query`, `cron`, `stripe` and `callers(services)` or `callers(accounts)`. `#[derive(Schema)]` gives a type its JSON Schema and its serde impls, in any module or crate.

**A tool is internal unless marked `#[tool(public)]`.** Only the host calls an internal tool, for the program's own wakes, crons and Stripe events. A `public` tool can be called by any account the host authenticates and any program allowed to reach it, with arguments the caller chooses. `berm_lang::caller()` says who called, as the host set it; a tool that limits its callers checks it.

Off the guest's target this is an ordinary library, which is why the
`cfg_attr` above is worth copying: tools then run under `cargo test` with no
wasm toolchain and no daemon. See `berm_lang::test`.

## Building

A program is a library crate with `[lib] crate-type = ["cdylib", "rlib"]` and a `crab.toml` beside its `Cargo.toml`; it has no bin target and no `fn main`. [Writing a Program](https://crab.talk/docs/build/writing#the-crate) has the layout.

```sh
rustup target add wasm32-unknown-unknown
crab build
```

`crab build` prints the path of the finished image, `berm/<crate>.wasm` under cargo's target directory. A bare `cargo build` writes an image without the sections the build appends, which a host refuses.

Leaving `[build] target` unset keeps the host build, and therefore `cargo test`, working without a flag.

The resulting `.wasm` is the whole artifact: one file, every platform.

## Reaching a runtime

Past its arguments, a program reaches who called, its own state, a database,
logs, HTTP, other programs, wakes and the clock, each through a syscall its host
serves. [The Guest API](https://crab.talk/docs/build/guest-api) covers each.

## License

Apache-2.0
