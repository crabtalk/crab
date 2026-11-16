# berm-codegen

The proc macros of a service: `#[tool]`, `#[workflow]`, `#[derive(Schema)]`, `#[derive(Branch)]`, `#[event]`, `#[error_code]`, `declare_service!`, `syscalls!` and `hosts!`. Re-exported by [`berm-lang`](https://crates.io/crates/berm-lang) — depend on that, not on this.

```rust
use berm_lang::{Result, tool};

/// Greet by name
#[tool]
pub fn greet(
    /// Who to greet.
    name: String,
) -> Result<String> {
    Ok(format!("hello, {name}"))
}
```

A function marked `#[tool]` is a tool; the first line of its doc comment is its title and the remaining lines are its description, and a tool without one is a compile error. Its parameters are its arguments and the `Ok` of its `Result` is its reply, each typed by `Schema`. An `Option<T>` result has an `anyOf` schema accepting `T` or `null`; optional object fields may be omitted. `#[tool]` takes `query`, `cron`, `stripe`, `webhook`, `events = "name"`, `request`, `access`, `price`, `catalog`, `budget(...)`, `idempotency` and `example`. Caller access defaults to public; `access = "internal"` keeps a tool internal. An event source is read-only and cannot also be a cron, Stripe or webhook handler; its history contract is in [berm-lang](../lang/README.md#durable-event-sources). `#[tool]`, `#[derive(Schema)]`, `#[event]` and `#[error_code]` go on items anywhere in the crate.

Each `#[tool]` expands to the function, its export `berm_tool_<name>`, and its record in the `.berm.abi` section, naming the crate it is compiled in. Tool and workflow declarations are compiled into that ABI section. `crab build` appends service runtime settings from `crab.toml`.

`#[tool]` also emits a `<ToolName>Call` input descriptor with a local `workflow::Call` target. `declare_service!` adds external descriptors while retaining its calling functions and response types. `#[derive(Branch)]` emits case metadata and consuming `workflow::Projection<T>` implementations for distinct tuple payloads. See [Call descriptors and branches](../lang/README.md#call-descriptors-and-branches).

`#[workflow]` compiles a unit-struct impl into typed control adapters and a static graph record. Dependency names and type projections determine wiring and guards; method bodies are not inspected. Graph validation is shared with `berm_api::workflow::graph::Graph::check`. See [Workflow graphs](../lang/README.md#workflow-graphs). Native hosts execute these graphs with restricted controls through [`berm-host`](../host/README.md).

## License

Apache-2.0
