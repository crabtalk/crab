---
title: berm
---

**[berm](./berm/overview.md)** is the OS for agent harnesses: it pins a WebAssembly module by hash, compiles it once, instantiates it per invocation, and hands it only the syscalls it was given. `crab serve` deploys programs and serves their tools over HTTP, and their `anonymous` tools over MCP to agents such as Claude Code, Codex and OpenCode.

## How to read this

Its chapters cover what the sandbox guarantees, how to write a guest against it, what a guest can reach, what an image declares about itself, how one travels between machines, and how the service serves it.

API documentation is not published; `cargo doc --open` builds it from the source.
