# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crabtalk.ai-blue)](https://crabtalk.ai/docs)
[![Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white)](https://discord.gg/yGZDYnwbx6)
[![WeChat](https://img.shields.io/badge/WeChat-07C160?logo=wechat&logoColor=white)](https://cdn.crabtalk.ai/crabtalk/wechat.jpg)

Crab is the open-source CLI and runtime of [CrabTalk](https://crabtalk.ai). A service is a Rust crate whose `#[tool]` functions are its tools, compiled to WebAssembly with its own SQLite storage. Crab builds services, runs them on your machine with no account, and deploys them to CrabTalk Cloud, where each one is a remote MCP server at `https://{owner}.crabtalk.app/{name}`.

## Install

macOS (Apple Silicon) and Linux:

```sh
curl -fsSL https://crabtalk.ai/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://crabtalk.ai/install.ps1 | iex
```

For a coding agent, `npx skills add crabtalk/crab` installs the [skills](skills) that build, wrap, deploy and debug services.

## Quick start

Create a service and call it on the local host, which starts on first use:

```sh
crab new example
cd example
crab --context default up
crab --context default call example echo '{"query":"hello"}'
```

`crab build` installs missing build tools. To deploy the same service to CrabTalk Cloud:

```sh
crab login
crab up
```

## In this repository

- [crab](crates/crab/README.md): the CLI, its commands, contexts and `crab.toml`.
- [Berm](berm/berm/README.md): the runtime and host that run services, locally and on Cloud.
- [SDK](crates/sdk/README.md): the Rust client for the CrabTalk API.
- [Libraries](crates/crabkit/README.md): libraries to include in your services.
- [Services](services/README.md): agent and web services to call, deploy or reuse.
- [Skills](skills) and the [ChatGPT plugin](plugins/README.md): for agents that build or call services.
- [changelog.json](changelog.json): what each release changed.

## Learn more

- [Documentation](https://crabtalk.ai/docs): guides for writing, running and deploying services.
- [Self-hosting](https://crabtalk.ai/docs/guides/self-hosting): run services with crab on your own machines.
- [llms.txt](https://crabtalk.ai/llms.txt): CrabTalk for agents, with every docs page in Markdown.

## Development

Run `cargo nextest run`. One setup prepares all registered test images before tests run, including focused runs; Wasm behavior tests execute those images through Wasmtime. Add images to `berm/build/examples/prepare.rs` and read them with `berm_build::fixture::image(package)`. Cargo caches the separate package builds.

## License

The runtime and CLI are [AGPL-3.0](LICENSE-AGPL).

SDKs, libraries, and services are [Apache-2.0](LICENSE-APACHE). Services you build are yours to license.
