# berm-api

What a service says it is, and what the local host's control API speaks.

Here rather than in the service, so a client can read an image and talk to the
API without linking a compiler.

```rust
let manifest = berm_api::Manifest::try_from(wasm.as_slice())?;
for tool in &manifest.tools {
    println!("{}: {}", tool.name, tool.description);
}
```

`Manifest::try_from(&[u8])` reads the `.berm.abi`, `.berm.service` and `.berm.kits` sections without compiling or running the image. An image built against a different `ABI_VERSION`, or without the `.berm.service` section `crab build` appends, is refused here. `project::Project` reads a service's or a kit's `crab.toml`; its optional `image` is an `owner/service` publication address, parsed by `reference::Reference`. `Manifest.imports` comes from the linked Wasm imports; `Deps.http` holds the merged HTTP destinations configured under `[http]`.

`events::Source::try_from(&ToolSpec)` checks a durable event hook and extracts its filter and payload schemas. `Source::qualify` prefixes the external event name with its deployment address. `Source::page` decodes a source response and checks its resume framing. The cursor stays in the source's tool arguments and is excluded from subscription filters. Manifest loading rejects malformed sources and duplicate event names. These definitions do not implement MCP subscriptions or delivery.

Tool responses carry `host::DENIED` (`berm-denied: 1`) when the tool refused its caller. The `Output::Failed` body remains unchanged.

`Service`, `ToolSpec` and `Output` are the control API's own shapes.
`Output::Failed` is the service's own report and arrives with the same `200` a
result does: the call was fine and the tool said no.

## Workflow contracts

`workflow::graph` defines v1 entry records, typed call schemas, symbolic targets, dependencies, guards, and once/optional/each/branch/output shapes. `Entry::digest` hashes compact JSON with recursively sorted object keys; declaration and item order remain significant. `Source` pairs this graph identity with the complete service image digest. Unknown contract versions are refused.

`workflow::run` defines accepted bindings, attempts, prepared payloads, ordered item dispatches and complete checkpoints. Only skipped values supply optional absence; pending, failed and unknown outcomes remain distinct. An unknown call retains its prepared input and operation key. External targets declare their idempotency window in milliseconds; no window forbids redispatch after an unknown outcome. Error records retain response status, headers and the full body, including explanation and recovery fields. Timestamps and durations use epoch milliseconds and milliseconds respectively.

`workflow::Policy` configures the seven-day retention default and each-item ceiling (100 by default). Checkpoint capacity and control resource limits are explicit accepted fields. Workflow compilation emits these records; `berm-host` persists complete runs and executes accepted graphs. `Graph::check` verifies declaration order, bounds, case references, guard consistency and output reachability using the same validator as the compiler.

`workflow::config::Config` reads a workflow's ABI declaration: access, title, task price, owner spend cap, control budget, checkpoint capacity and retention. The defaults are public access, free task price, host control budgets, 100,000 USD micros ($0.10), 8 MiB checkpoints and seven days. Only a schedule is read from `[workflows.<name>]` in `crab.toml`; other keys and every `[tools]` table are refused. Manifest loading checks local call schemas, graph validity, control export signatures and schedule inputs without executing controls. `Service.workflows` exposes public entries and task prices; `Runtime.workflows` includes owner configuration. Invalid workflow declarations carry `workflow::config::Invalid`.

## License

Apache-2.0

`Deps::secret_resources` permits host-granted runtime secret IDs in HTTP placeholders. `project::Project` reads `secret_resources = true`; linked dependencies merge this capability and permission changes require upgrade review. Individual resources and grants are managed separately from image declarations. Fixed secret names cannot be opaque resource IDs.
