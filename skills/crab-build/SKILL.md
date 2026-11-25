---
name: crab-build
description: Write or change CrabTalk service code and test it locally with the Crab CLI. Covers tools, crab.toml, storage, workflow declarations, library reuse and tool budget sizing.
---

# Build a service

Read the relevant source before writing code; [Where the truth is](references/sources.md) maps each question to it. For a service that wraps an existing HTTP API, start with `crab-wrap`. Deploying to Cloud, secrets on Cloud and anything a person approves belong to `crab-ship`; a failed call belongs to `crab-debug`.

## Choose how to use a capability

Browse provider integrations in `services/connect/`, agent services in `services/agents/`, and website tools in `services/web/`. Library-only packages live under `services/kits/`. Check [services](https://github.com/crabtalk/crab/blob/main/services/README.md) and [crabkit](https://github.com/crabtalk/crab/blob/main/crates/crabkit/README.md) before writing an integration. Call an existing deployment, deploy a published image, or link its library when those fit the task. Write a product service for its own behavior. Its tools, its database, its secrets and its public surface are the product's.

## A service

- A service is a library crate in the product's repository, built as a `cdylib` (`[lib] crate-type = ["cdylib", "rlib"]`, no bin target), with a `crab.toml` beside its `Cargo.toml`. Its tools are functions marked `#[berm_lang::tool(...)]`; what it reaches for is declared in `crab.toml`.
- `crab new` writes one with `echo` and durable `remember`/`recall` examples, a 64 MiB storage reservation, and no generated tool-specific test; add tests for the tools you keep.
- `crab build` builds its image and prints the path. A bare `cargo build` writes an image a host refuses. `crab build` installs missing Rust build tools through the official rustup installer and adds the `wasm32-unknown-unknown` target; no manual toolchain setup is required.
- A service with standard-library dependencies enables `berm-lang = { workspace = true, features = ["std"] }` and omits `#![no_std]`. Default guest builds remain `no_std`.
- Host calls are discovered from the final Wasm imports. Configure direct fetch destinations outside your credentials under `[http]` with `fetch = ["api.example.com"]`; missing or empty lists add no hosts. Library destinations merge into the service.
- A setting whoever deploys a copy chooses, such as the keywords a monitor watches, is a var: declared under `[vars]` in `crab.toml` with a `default` and a `description`, read with `berm_lang::var("name")`. A secret is never a var.
- `http::Request::new(method, url, headers, body).rotating().send()` opts one request into hosted rotating egress. Declare the destination under `[http] fetch`. Local `crab dev` fetches directly and logs a warning.

## Tool declarations and titles

Declare metadata beside the function: `#[tool(query, catalog = "web.search", budget(cpu = 50_000, memory = 32))]`. Access defaults to `public`; use `access = "internal"` for internal calls, or `anonymous`, `accounts` or `services`. Price defaults to free; `price = 2_000` charges $0.002 per call from the built image. Catalog and idempotency default to absent; `idempotency = "24h"` declares a target-owned deduplication window. Repeat `example = "find recent Rust releases"` up to three times, at most 120 characters per task. The service page turns them into copyable agent prompts; `crab inspect` lists them.

The first doc-comment line is the title: at most 60 characters, no final period. Name the action verb first, such as "Send email" or "Count daily site views". Omit the service name and filler such as "This tool" or "Allows you to". After a blank comment line, describe inputs, side effects and limits.

Give every tool with `access` a catalog job: see [Catalog](https://crabtalk.ai/docs/guides/catalog). Internal cron and webhook handlers stay unclassified. Write the README by [the README guide](https://github.com/crabtalk/crab/blob/main/services/README-guide.md).

## Storage

- On Cloud a service reserves its `[storage] size`, 1GiB by default for shared storage. A service using caller storage alone reserves zero shared storage by default. The build records the reservation in the image. Size it to what the service holds and check the account's total against its plan.
- Declare `[storage.caller]` with its migrations directory to use `berm_lang::caller_storage`. Caller files are separate from shared storage and count against the billed account's plan. See [Caller storage](https://crabtalk.ai/docs/development/caller-storage).
- A tool reads its own pending storage writes during the call. Changes commit only on `Ok`; `Err`, a trap or an exceeded deadline discards them. Independent calls cannot read pending changes. HTTP actions and changes already committed by another service are outside the transaction. See [Transactions](https://crabtalk.ai/docs/development/data#transactions).

## Workflows

A `#[workflow]` impl compiles typed step, branch and output signatures into a graph. Its attributes declare access, task price, `max_cost`, `budget(...)`, `checkpoint_bytes`, `retain` and optional `example`. Defaults are public access, free calls, host control budgets, 100,000 USD micros ($0.10) cumulative owner cost, 8 MiB checkpoints and seven-day retention. Only `[schedules]` stays in `crab.toml`. `crab build` validates the image without executing builders; `crab inspect .` builds and reads its complete declarations.

## Calling other services

Service code calls running deployments with `berm_lang::call("owner/service", tool, args)`; no `calls` manifest field is used. Cross-account targets must be public and accept service callers; priced calls charge the calling service's account. `crab idl <service>` writes a deployed service's IDL, which `declare_service!` reads. `berm_lang::client()` reads the daily opaque client key and `berm_lang::client_ip()` the outside IPv4/IPv6 address, when supplied; both follow nested calls.

## Reusing services and libraries

- Keep `berm-lang`, `crabkit` and linked provider packages on the same Crab revision. `crab new` writes `berm-lang` at the tag of the CLI's release, and `crabkit` goes beside it with a feature per library:

  ```toml
  [dependencies]
  berm-lang = { git = "https://github.com/crabtalk/crab", tag = "v0.1.3" }
  crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.3", features = ["auth"] }
  ```

- A library with host requirements declares them in its own `crab.toml`. For libraries bundled by `crabkit`, enable the matching feature and call the API from your tools, such as `crabkit::auth`. Linking a library is using it: the secrets, fetched hosts and migrations its `crab.toml` declares reach the service's manifest; its tables live in the service's database.
- A library-only library has no standalone deployment. Deployable services also expose Rust APIs: depend on them with `default-features = false` to disable their standalone tool exports. Their deployment name, visibility, tool access, prices, crons, vars and database sizing stay with their own deployment. A dependency with migrations needs a stable `id` in its `crab.toml`.
- Each library's `README.md` lists what the service still declares itself.

## The local loop

- `crab dev -d` starts the local host at `127.0.0.1:7777` in the background on the default address. `crab dev status` says whether it runs; `crab dev stop` stops it. Its output is `~/.crab/host/host.log`. Use `crab --context default status` to check the connection. The local loop needs no account or login.
- `crab --context default up [dir]` starts the host if needed, then builds and deploys the service in `dir` (default `.`) or workspace services with a `crab.toml` beneath it. Repeat `-p <name>` to select services anywhere in the workspace. Run it again after a change.
- Use `crab` for local caller identities and `crab-ship` for local secret provisioning and service lifecycle commands.
- Run Rust tests with `cargo nextest run`. Tests belong in each crate's `tests/` directory.

## Tool budgets

Size each tool's `#[tool(budget(...))]` from its built WebAssembly execution before Cloud deployment or image publication. Native tests and image builds do not establish valid runtime budgets. Follow [Sizing tool budgets](references/budgets.md).

## A local crab checkout

- A product builds against a local checkout by patching crab's git source in its root `Cargo.toml`, one path per crate it uses:

  ```toml
  [patch."https://github.com/crabtalk/crab"]
  berm-lang = { path = "../crab/berm/lang" }
  crabkit = { path = "../crab/crates/crabkit" }
  crabkit-auth = { path = "../crab/services/kits/auth" }
  ```

- What a service needs from berm or a library and cannot get is a change to crab, made in crab. What only one product needs stays in that product's service.
- In the Crab workspace, Nextest 0.9.98 or newer is required and CI uses `cargo nextest run --release`. Workspace Wasm behavior tests read `berm_build::fixture::image(package)`; register images in `berm/build/examples/prepare.rs`. A Nextest setup script prepares the registered images even on focused runs. Only builder and CLI build tests should compile images inside a test. Update affected READMEs, examples, website guides and these skills alongside behavior or layout changes.
