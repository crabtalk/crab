# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crab.talk-blue)](https://crab.talk/docs)
[![Discord](https://img.shields.io/badge/Discord-5865F2?logo=discord&logoColor=white)](https://discord.gg/yGZDYnwbx6)
[![WeChat](https://img.shields.io/badge/WeChat-07C160?logo=wechat&logoColor=white)](https://cdn.crabtalk.ai/crabtalk/wechat.jpg)

[English](../../README.md) | 简体中文

Crab 是 CrabTalk 的命令行工具和本地服务宿主，可用于构建服务、在本地运行，并部署到 CrabTalk Cloud。

## 安装

macOS（Apple Silicon）和 Linux：

```sh
curl -fsSL https://crab.talk/install.sh | sh
```

Windows（PowerShell）：

```powershell
irm https://crab.talk/install.ps1 | iex
```

## 快速开始

编写服务需要安装 Rust 工具链。在新目录中创建并在本地运行一个服务：

```sh
crab new example
cd example
crab --context default deploy
crab --context default call example echo '{"query":"hello"}' --caller alice
```

本地服务宿主会自动启动。将同一服务部署到 CrabTalk Cloud：

```sh
crab login
crab deploy
```

## 了解更多

- [文档](https://crab.talk/docs) — 编写和部署服务的指南。
- [CLI 参考](../../crates/crab/README.md) — 命令、上下文和凭证。
- [服务](../../services/README.md) — 可供调用、部署或复用的 Connect 集成和实用服务。
- [库](../../crates/crabkit/README.md) — 可在服务中使用的库。
- [Berm](../../berm/berm/README.md) — 运行服务的运行时。

## 许可证

运行时和 CLI 采用 [AGPL-3.0](../../LICENSE-AGPL) 许可证。

SDK、库和服务采用 [Apache-2.0](../../LICENSE-APACHE) 许可证。你开发的服务可自行选择许可证。
