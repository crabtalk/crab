---
name: crab
description: Build a product's services on crab — berm programs deployed with the crab CLI, linking crab's kits as libraries. Use before writing, changing or deploying a service on CrabTalk Cloud or the local host, before using a kit under `kits/`, and before changing a local crab checkout from a product's repository.
---

# Building on crab

crab is the platform a product's services run on: the `crab` CLI (`crates/crab`), the berm runtime (`berm/`), and the kits (`kits/`). A product is a separate repository whose services are its own.

## Services belong to the product

- A service is a library crate in the product's repository, built as a `cdylib` (`[lib] crate-type = ["cdylib", "rlib"]`, no bin target), with a `crab.toml` beside its `Cargo.toml`. Its tools are functions marked `#[berm_lang::tool(...)]`; what it reaches for is declared in `crab.toml`. `crab new` writes one; `crab build` builds its image and prints the path; `crab deploy` builds and deploys it. A bare `cargo build` writes an image a host refuses.
- Its tools, its database, its secrets and its public surface are the product's.
- `crab dev -d` starts the local host at `127.0.0.1:7777` in the background, one per machine. `crab --context default deploy` builds and deploys there the service whose directory it runs in, or at the workspace's root every service in it, or those named with `-p`, starting the host when it is not running; run it again after a change. `crab dev status` says whether the host runs; `crab dev stop` stops it. Other commands reach it with `--context default`. Its output is `~/.crab/host/host.log`.
- On Cloud each service reserves its `[database] size` of the account's plan storage, 1GiB when unset, whatever it holds. A deploy refused for storage is met by sizing each service to what it holds (`crab usage <service>` reports `stored`) before any change of plan.

## Kits are libraries

- `berm-lang` and `crabkit` come from crab's git repository, at one tag or branch for both; the kits are not on crates.io. `crab new` writes `berm-lang` at the tag of the CLI's release, and `crabkit` goes beside it with a feature per kit:

  ```toml
  [dependencies]
  berm-lang = { git = "https://github.com/crabtalk/crab", tag = "v0.1.1" }
  crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.1", features = ["auth"] }
  ```

- A kit is a crate with a `crab.toml` of its own. A service links it by turning on its `crabkit` feature, and calls it from its own tools as `crabkit::auth`. Linking a kit is using it: the build finds it among the crates compiled into the image, and the secrets, fetched hosts and migrations its `crab.toml` declares reach the service's manifest; its tables live in the service's database.
- A product never deploys a kit. When a product needs a kit's capability, it writes a service of its own that wraps the kit.
- Each kit's `README.md` lists what the service still declares itself.

## Signing in, paying and secrets

The person you act for is not at your machine: never open a browser or wait on a prompt for something only they can do.

- Sign in with `crab login --email <address>`: it emails a code and exits. Ask the person for the code from that email, then run `crab login --email <address> --code <code>`. A new address gets an account. Plain `crab login` opens a browser on your machine, which they cannot reach.
- To buy a plan, start the trial or buy credit, run `crab billing plan <slug>`, `crab billing trial` or `crab billing credit` with `--for` saying what it is for; the person reads it before paying. Send them the link it prints, with what it buys and its price, then run `crab request <code> --wait`. The code lasts 30 minutes; if it expires or they decline, ask before making another.
- Never ask the person to paste a secret's value into the chat. For local development on the local host, ask them to run `crab --context default secret fill <service>` in their own terminal, with `--copy KEY=VALUE` for the values its credentials have them enter at the provider: it shows each credential's guide and sets what they type on the local host, where test keys belong. It refuses to run where stdin is not a terminal, so never run it yourself. On Cloud, once the service is deployed, ask for every secret it has unset in one request: `crab secret request <service>` with `--for` saying what they are for and `--copy KEY=VALUE` for each value its credentials have the person enter at the provider, such as `--copy callback=<URL>`. The guides to getting them come from the `[credentials]` of its `crab.toml` and its kits'; a service's own secret that needs steps is declared as a credential with a `guide` naming the provider, the steps in its dashboard, and the URL of the page. `--credential <id>` asks again for one, as after rotating it. Send them the link it prints, then run `crab request <code> --wait`. They set them on that page, signed in; you see that they were set, never their values. A code sent to the wrong person, or asking for the wrong thing, is ended with `crab request <code> --expire` before you ask again; what was already saved stays set.

## Where the truth is

Read these before writing code. Paths are relative to the crab repository.

| Question | Source |
| --- | --- |
| The CLI, contexts, `crab.toml` | `crates/crab/README.md` |
| Tools, `#[tool(...)]` and its `public`, `anonymous`, `query`, `cron`, `stripe`, `callers(...)`; building with `crab build` | https://crab.talk/docs/build/writing, `berm/lang/README.md` |
| Caller, user, database, HTTP, calls between services | https://crab.talk/docs/build/guest-api |
| `crab.toml` keys, syscalls, secrets, credentials, kits | https://crab.talk/docs/build/crab-toml |
| `crab dev`, the local host, local secrets, replaying a failed call | https://crab.talk/docs/build/local |
| Deploying to Cloud, images and registries | https://crab.talk/docs/services/deploy |
| Secrets on Cloud and asking a person to set them | https://crab.talk/docs/services/secrets |
| A kit's API and setup | `kits/<kit>/README.md` |

## A local crab checkout

- A product builds against a local checkout by patching crab's git source in its root `Cargo.toml`, one path per crate it uses:

  ```toml
  [patch."https://github.com/crabtalk/crab"]
  berm-lang = { path = "../crab/berm/lang" }
  crabkit = { path = "../crab/crates/crabkit" }
  crabkit-auth = { path = "../crab/kits/auth" }
  ```

- What a service needs from berm or a kit and cannot get is a change to crab, made in crab. What only one product needs stays in that product's service.
