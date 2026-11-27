# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crabtalk.ai-blue)](https://crabtalk.ai/docs)
[![Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white)](https://discord.gg/yGZDYnwbx6)
[![WeChat](https://img.shields.io/badge/WeChat-07C160?logo=wechat&logoColor=white)](https://cdn.crabtalk.ai/crabtalk/wechat.jpg)

Crab is the CLI and local host for CrabTalk. Build services, run them locally, and deploy them to CrabTalk Cloud.

## Install

macOS (Apple Silicon) and Linux:

```sh
curl -fsSL https://crabtalk.ai/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://crabtalk.ai/install.ps1 | iex
```

## Quick start

`crab build` installs missing build tools automatically. From a new directory, create and run a service locally without an account:

```sh
crab new example
cd example
crab --context default deploy
crab --context default call example echo '{"query":"hello"}'
```

The local host starts automatically. To deploy the same service to CrabTalk Cloud:

```sh
crab login
crab up
```

A repo can declare its published address in `crab.toml`, without a separate name:

```toml
[image]
name = "acme/weather"
deploy = true
```

`crab push` builds and publishes an image to Cloud. `crab up` builds and deploys your source. `crab deploy owner/name --version 1.2.3` deploys a published image; `crab pull owner/name:1.2.3` downloads it for the local host.

For services exporting workflows, `crab workflow inspect owner/service.workflow` resolves the graph and quote without execution; `crab workflow run` starts a retained run, and `status`, `continue` and `cancel` manage it. See [workflow commands](crates/crab/README.md#workflow-runs).

`crab export owner/service` downloads a consistent SQLite snapshot of the service’s shared SQL and key-value data as `service.sqlite`; `-o PATH` chooses a file. Local contexts support `crab export service`. Workflow runs and checkpoints are excluded. An export can seed a new Cloud deployment of the same service.

`crab storage` lists the SQLite files services hold for the signed-in caller. Use `crab storage download ID` or `crab storage delete ID` to manage a file. Service owners see caller metadata with `crab callers owner/service`.

## Explore

- [Documentation](https://crabtalk.ai/docs) — guides for writing and deploying services.
- [Benchmarking](https://crabtalk.ai/docs/guides/bench) — measure local WebAssembly workloads and estimate execution cost with `crab bench`.
- [ChatGPT plugin](plugins/README.md) — uploadable bundle for finding and calling deployed services.
- [CLI reference](crates/crab/README.md) — commands, contexts, and credentials.
- [Services](services/README.md) — agent and web services to call, deploy or reuse.
- [Libraries](crates/crabkit/README.md) — libraries to include in your services.
- [Berm](berm/berm/README.md) — the runtime that runs services.

## Development

Run `cargo nextest run`. One setup prepares all registered test images before tests run, including focused runs; Wasm behavior tests execute those images through Wasmtime. Add images to `berm/build/examples/prepare.rs` and read them with `berm_build::fixture::image(package)`. Cargo caches the separate package builds.

## License

The runtime and CLI are [AGPL-3.0](LICENSE-AGPL).

SDKs, libraries, and services are [Apache-2.0](LICENSE-APACHE). Services you build are yours to license.
