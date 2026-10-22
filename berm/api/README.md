# berm-api

What a program says it is, and what the local host's control API speaks.

Here rather than in the service, so a client can read an image and talk to the
API without linking a compiler.

```rust
let manifest = berm_api::Manifest::from_image(&wasm)?;
for tool in &manifest.tools {
    println!("{}: {}", tool.name, tool.description);
}
```

`Manifest::from_image` reads the `.berm.abi`, `.berm.program` and `.berm.kits` sections without compiling or running the image. An image built against a different `ABI_VERSION`, or without the `.berm.program` section `crab build` appends, is refused here. `project::Project` reads a program's or a kit's `berm.toml`.

`Program`, `ToolSpec` and `Output` are the control API's own shapes.
`Output::Failed` is the program's own report and arrives with the same `200` a
result does: the call was fine and the tool said no.

## License

Apache-2.0
