# Services

Each crate is a member of the root Cargo workspace. Deployable services build both a wasm image and a Rust library. Reusable libraries are available through [crabkit](../crates/crabkit/README.md).

## Catalogs

- [Connect](connect/README.md): third-party API integrations.
- [Agents](agents/README.md): shared agent memory, identities and conversations.
- [Web](web/README.md): SEO checks, webpage-to-Markdown extraction and website analytics.
- [Ops](ops/README.md): uptime and webhook inboxes.

The catalog directories are `connect`, `agents`, `web` and `ops`. Each service's `image` address selects its publication account; for example, `image = "connect/tinyfish"` publishes to `connect/tinyfish`. A context supplies the server and credentials, which must authorize publication there.

Call an accessible running deployment or deploy a published template as a private copy. Catalog membership does not make a deployment public. `web/seo` and `web/markdown` are configured as public services without templates. Each service documents its setup and state ownership.

Tools declare their discovery classification with `catalog` in `crab.toml`. See the [Service catalog guide](https://crabtalk.ai/docs/guides/catalog) for categories, platform prefixes and job names.

Library-only packages live under [services/kits](kits/) and are listed in [crabkit](../crates/crabkit/README.md).

`crab build` operates on one service directory. `crab deploy [dir]` and `crab push [dir]` also accept a catalog directory, selecting only its descendant services; the directory defaults to `.`. For example, run `crab push services/connect` from the repository root or `crab push` inside `services/connect`. No catalog-level `crab.toml` is needed. Use repeatable `-p <name>` to select services anywhere in the Cargo workspace instead. Agents call deployed tools through MCP.

To reuse a deployable service as a library, pin it to the same release tag as `berm-lang` and set `default-features = false`. This disables its standalone `tools` exports. Call the Rust functions from your own tools. `berm-build` inherits the dependency's credentials, host permissions and migrations, while its deployment name, logo, tool access, cron settings, variables and database sizing stay with its own deployment. A dependency keeping migrations needs a stable migration ID.

All linked code executes inside the consuming service and uses its secrets and database. Calling a separately deployed service instead uses that deployment's execution and state.

## Building from source

Calling an existing deployment or deploying a published template needs no local build. To modify a service or deploy one without a published template, install [Crab](https://crabtalk.ai/docs/guides/quickstart#1-install-the-cli), clone this repository, and select the service directory:

```sh
git clone --branch main --single-branch https://github.com/crabtalk/crab
cd crab/services/connect/github
crab login
crab deploy --build
```

Choose a service directory from a catalog, such as `services/connect/github` or `services/ops/uptime`. `crab deploy --build` builds the image, adds the WebAssembly target through rustup if needed, and asks for approval on Cloud. Set the credentials and variables documented in the service's README. Generated integrations may need `curl` and network access to fetch their pinned provider specifications during the build.

For local development, start the host with `crab dev -d`, use `crab --context default deploy --build` in the selected service directory, set local credentials with `crab --context default secret fill github`, and call tools as the default caller `local`. Use `--caller` to override it or `--anonymous` to call without a caller. See [local development](https://crabtalk.ai/docs/guides/local).

## Checking changes

From the repository root, `cargo nextest run --release` and `cargo clippy` use Cargo’s default members: runtime, CLI, SDK, generator fixtures, stateful services and handwritten integrations. Pure generated adapters and the large TikHub fixture are checked explicitly.

When changing or publishing a generated adapter, refetch and review its upstream specification, then run its native tests and build the service image. For example:

```sh
cargo nextest run --release -p crab-contextdev
crab build services/connect/contextdev
```

Before publishing or requesting deployment, use `crab bench <service-directory> <tool> --cases cases.jsonl --runs 10` ([case format](https://crabtalk.ai/docs/guides/bench)) to profile the built WebAssembly image with representative and largest supported workloads, including error paths. Add headroom, check every budget key against the destination host's ceilings, then rebuild and repeat the workload under the chosen budgets. Native tests and image builds do not establish valid runtime budgets; `crab budget` suggestions do not check ceilings. See [Sizing a budget](https://crabtalk.ai/docs/guides/budgets#size-a-budget-before-deployment).

`cargo nextest run --workspace --release` includes every package. Generator Wasm export and schema coverage lives in `crab-openapi` with a local fixture.

## Documentation

Follow the [service README guide](README-guide.md) for READMEs displayed on crabtalk.ai. Rust integration examples for deployable services live in their `LIBRARY.md` files when provided.
