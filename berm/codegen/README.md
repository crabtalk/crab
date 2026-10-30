# berm-codegen

The proc macros of a service: `#[tool]`, `#[derive(Schema)]`, `#[event]`, `#[error_code]`, `declare_service!`, `syscalls!` and `hosts!`. Re-exported by
[`berm-lang`](https://crates.io/crates/berm-lang) — depend on that, not on this.

```rust
use berm_lang::{Result, tool};

/// Greets `name`.
#[tool]
pub fn greet(
    /// Who to greet.
    name: String,
) -> Result<String> {
    Ok(format!("hello, {name}"))
}
```

A function marked `#[tool]` is a tool; its doc comment is the description the model reads, and a tool without one is a compile error. Its parameters are its arguments and the `Ok` of its `Result` is its reply, each typed by `Schema`. `#[tool]` takes `query`, `cron`, `stripe` and `webhook`; who may call a tool is `access` in its `[tools]` table of `crab.toml`. `#[tool]`, `#[derive(Schema)]`, `#[event]` and `#[error_code]` go on items anywhere in the crate.

Each `#[tool]` expands to the function, its export `berm_tool_<name>`, and its record in the `.berm.abi` section, naming the crate it is compiled in. The service's own declarations are in its `crab.toml`, which `crab build` appends to the image as the `.berm.service` section, each tool's access, title and catalog job among them.

## License

Apache-2.0
