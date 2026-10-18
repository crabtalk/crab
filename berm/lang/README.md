# berm-lang

Build a program for [berm](https://berm.run).

A program is one WebAssembly module, confined to its own memory, reaching the world only through the host calls it was given. This crate owns the ABI, so you never see a call number or a pointer pair.

```rust
#![cfg_attr(target_arch = "wasm32", no_std, no_main)]

extern crate alloc;

use alloc::string::String;
use berm_lang::Schema;

#[berm_lang::program]
mod tools {
    use crate::Echo;
    use alloc::string::String;
    use berm_lang::Result;

    /// Echo the query back.
    #[public]
    pub fn echo(
        /// The text to echo back.
        query: String,
        /// Page number, zero-indexed.
        page: Option<u32>,
    ) -> Result<Echo> {
        Ok(Echo { query, page })
    }
}

/// What `echo` answers.
#[derive(Schema)]
pub struct Echo {
    pub query: String,
    pub page: Option<u32>,
}
```

Every `pub fn` is a tool and its doc comment is what the model reads when
deciding to call it. Its parameters are its arguments — their types, their doc
comments, and `Option` for the ones the model may omit — and the `Ok` of its
`Result` is its reply. `#[derive(Schema)]` gives a type its JSON Schema and its
serde impls, in any module or crate.

**A tool is internal unless marked `#[public]`.** Only the host calls an internal tool, for the program's own wakes, crons and Stripe events. A `#[public]` tool can be called by any account the host authenticates and any program allowed to reach it, with arguments the caller chooses. `berm_lang::caller()` says who called, as the host set it; a tool that limits its callers checks it.

Off the guest's target this is an ordinary library, which is why the
`cfg_attr` above is worth copying: tools then run under `cargo test` with no
wasm toolchain and no daemon. See `berm_lang::test`.

## Building

The tools live in the library, and a bin target that does nothing but
`extern crate` them is what builds the image.
[Writing a Program](https://berm.run/docs/berm/writing-a-program/#building)
has the layout.

```sh
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
```

Leaving `[build] target` unset keeps the host build, and therefore `cargo test`, working without a flag.

The resulting `.wasm` is the whole artifact: one file, every platform.

## Reaching a runtime

Past its arguments, a program reaches who called, its own state, a database,
logs, HTTP, other programs, wakes and the clock, each through a syscall its host
serves. [The Guest API](https://berm.run/docs/berm/guest-api/) covers each.

## License

Apache-2.0
