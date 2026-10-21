---
name: crab
description: Build a product's services on crab — berm programs deployed with the crab CLI, linking crab's kits as libraries. Use before writing, changing or deploying a service on CrabTalk Cloud or bermd, before using a kit under `kits/`, and before changing a local crab checkout from a product's repository.
---

# Building on crab

crab is the platform a product's services run on: the `crab` CLI (`crates/crab`), the berm runtime (`berm/`), and the kits (`kits/`). A product is a separate repository whose services are its own.

## Services belong to the product

- A service is a crate in the product's repository, with a `berm.toml` beside its `Cargo.toml`. `crab new` writes one; `crab deploy` deploys it.
- Its tools, its database, its secrets and its public surface are the product's.
- On Cloud each service reserves its `[database] size` of the account's plan storage, 1GiB when unset, whatever it holds. A deploy refused for storage is met by sizing each service to what it holds (`crab usage <service>` reports `stored`) before any change of plan.

## Kits are libraries

- `berm-lang` and the kits come from crab's git repository, at one tag or branch for all of them; the kits are not on crates.io. `crab new` writes `berm-lang` at the tag of the CLI's release, and a kit goes beside it:

  ```toml
  [dependencies]
  berm-lang = { git = "https://github.com/crabtalk/crab", tag = "v0.1.0" }
  crab-auth = { git = "https://github.com/crabtalk/crab", tag = "v0.1.0" }
  ```

- A kit is linked with `#[berm_lang::program(uses = [crab_x])]` and called from the service's own tools. The kit's secrets, fetched hosts and migrations reach the service's manifest through `uses`; its tables live in the service's database.
- A product never deploys a kit. When a product needs a kit's capability, it writes a service of its own that wraps the kit.
- Each kit's `README.md` lists what the service still declares itself.

## Where the truth is

Read the source before writing code. Paths are relative to the crab repository.

| Question | Source |
| --- | --- |
| The CLI, contexts, `berm.toml` | `crates/crab/README.md` |
| Tools, `#[public]`, `#[anonymous]`, `#[query]`, `#[cron]` | `docs/berm/writing-a-program.md`, `berm/lang/README.md` |
| Caller, user, database, HTTP, calls between services | `docs/berm/guest-api.md` |
| Syscalls, secrets, kits and their migrations | `docs/berm/manifest.md` |
| Images and registries | `docs/berm/publishing.md` |
| bermd, HTTP and MCP serving | `docs/berm/service.md` |
| A kit's API and setup | `kits/<kit>/README.md` |

## A local crab checkout

- A product builds against a local checkout by patching crab's git source in its root `Cargo.toml`, one path per crate it uses:

  ```toml
  [patch."https://github.com/crabtalk/crab"]
  berm-lang = { path = "../crab/berm/lang" }
  crab-auth = { path = "../crab/kits/auth" }
  ```

- What a service needs from berm or a kit and cannot get is a change to crab, made in crab. What only one product needs stays in that product's service.
