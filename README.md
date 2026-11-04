# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crab.talk-blue)](https://crab.talk/docs)
[![Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white)](https://discord.gg/yGZDYnwbx6)
[![WeChat](https://img.shields.io/badge/WeChat-07C160?logo=wechat&logoColor=white)](https://cdn.crabtalk.ai/crabtalk/wechat.jpg)

Crab is the CLI and local host for CrabTalk. Build services, run them locally, and deploy them to CrabTalk Cloud.

## Install

macOS (Apple Silicon) and Linux:

```sh
curl -fsSL https://crab.talk/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://crab.talk/install.ps1 | iex
```

## Quick start

Writing services requires a Rust toolchain. From a new directory, create and run a service locally:

```sh
crab new example
cd example
crab --context default deploy
crab --context default call example echo '{"query":"hello"}' --caller alice
```

The local host starts automatically. To deploy the same service to CrabTalk Cloud:

```sh
crab login
crab deploy
```

## Explore

- [Documentation](https://crab.talk/docs) — guides for writing and deploying services.
- [CLI reference](crates/crab/README.md) — commands, contexts, and credentials.
- [Services](services/README.md) — Connect integrations and Utilities to call, deploy or reuse.
- [Libraries](crates/crabkit/README.md) — libraries to include in your services.
- [Berm](berm/berm/README.md) — the runtime that runs services.

## License

The runtime and CLI are [AGPL-3.0](LICENSE-AGPL).

SDKs, libraries, and services are [Apache-2.0](LICENSE-APACHE). Services you build are yours to license.
