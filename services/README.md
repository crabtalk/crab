# Services

Each crate is a member of the root Cargo workspace. Deployable services build both a wasm image and a Rust library. Reusable libraries are available through [crabkit](../crates/crabkit/README.md).

## Catalogs

- [Connect](connect/README.md): third-party API integrations.
- [Utilities](utilities/README.md): services that perform a useful task, such as release and vulnerability monitoring.

Call an accessible running deployment or deploy a published template as a private copy. Catalog membership does not make a deployment public. Stateful utilities may require a private copy; each service documents its setup and state ownership.

Library-only packages live under [services/kits](kits/) and are listed in [crabkit](../crates/crabkit/README.md).

`crab build` operates on one service directory. `crab deploy [dir]` and `crab publish [dir]` also accept a catalog directory, selecting only its descendant services; the directory defaults to `.`. For example, run `crab publish services/connect` from the repository root or `crab publish` inside `services/connect`. No catalog-level `crab.toml` is needed. Use repeatable `-p <name>` to select services anywhere in the Cargo workspace instead. Agents call deployed tools through MCP.

To reuse a deployable service as a library, add the package as a dependency with `default-features = false`. This disables its standalone `tools` exports. Call the Rust functions from your own tools. `berm-build` inherits the dependency's credentials, host permissions and migrations, while its deployment name, tool access, cron settings, variables and database sizing stay with its own deployment. A dependency keeping migrations needs a stable migration ID.

All linked code executes inside the consuming service and uses its secrets and database. Calling a separately deployed service instead uses that deployment's execution and state.

## Building from source

Calling an existing deployment or deploying a published template needs no local build. To modify a service or deploy one without a published template, install [Crab](https://crab.talk/docs/guides/quickstart#1-install-the-cli) and a Rust toolchain, clone this repository, and select the service directory:

```sh
git clone https://github.com/crabtalk/crab
cd crab/services/connect/github
crab login
crab deploy
```

Choose the service directory from either catalog, such as `services/connect/github` or `services/utilities/release-watch`. `crab deploy` builds the image, adds the WebAssembly target through rustup if needed, and asks for approval on Cloud. Set the credentials and variables documented in the service's README. Generated integrations may need `curl` and network access to fetch their pinned provider specifications during the build.

For local development, use `crab --context default deploy`, set local credentials with `crab --context default secret fill github`, and pass `--caller alice` when calling a public tool. See [local development](https://crab.talk/docs/guides/local).

## Documentation

Follow the [service README guide](README-guide.md) for READMEs displayed on crab.talk. Rust integration examples for deployable services live in their `LIBRARY.md` files when provided.
