---
name: crab
description: Build a product's services on crab — deployed with the crab CLI, linking crab's kits as libraries — and call other accounts' public services on CrabTalk Cloud. Use before writing, changing or deploying a service on CrabTalk Cloud or the local host, before using a kit under `kits/`, before calling a service someone else publishes, and before changing a local crab checkout from a product's repository.
---

# Building on crab

crab is the platform a product's services run on: the `crab` CLI (`crates/crab`), the berm runtime (`berm/`), and the kits (`kits/`). A product is a separate repository whose services are its own.

## Services belong to the product

- A service is a library crate in the product's repository, built as a `cdylib` (`[lib] crate-type = ["cdylib", "rlib"]`, no bin target), with a `crab.toml` beside its `Cargo.toml`. Its tools are functions marked `#[berm_lang::tool(...)]`; what it reaches for is declared in `crab.toml`. `crab new` writes one; `crab build` builds its image and prints the path; `crab deploy` builds and deploys it; `crab push` does the same and leaves it stopped; `crab stop` and `crab start` stop and serve it, keeping its data. A bare `cargo build` writes an image a host refuses.
- Its tools, its database, its secrets and its public surface are the product's.
- A setting whoever deploys a copy chooses, such as the keywords a monitor watches, is a var: declared under `[vars]` in `crab.toml` with a `default` and a `description`, read with `berm_lang::var("name")`. Each copy holds its own value, set with `crab var set <service> <var> <value>` and read from the next call. A secret is never a var.
- `crab dev -d` starts the local host at `127.0.0.1:7777` in the background, one per machine. `crab --context default deploy` builds and deploys there the service whose directory it runs in, or at the workspace's root every service in it, or those named with `-p`, starting the host when it is not running; run it again after a change. `crab dev status` says whether the host runs; `crab dev stop` stops it. Other commands reach it with `--context default`. Its output is `~/.crab/host/host.log`.
- Each tool gets a budget in `crab.toml` before it goes to Cloud: `budget = { cpu = <µs>, read = <rows>, written = <rows>, memory = <MiB> }` on its `[tools.<tool>]`. Call it on the local host with the largest inputs it should take (`crab --context default call <service> <tool> '<arguments>'`), read what the call prints on stderr, and set each key to about 1.5× the most it reached. Once it runs on Cloud, `crab budget <service> <tool>` prints what its runs used and the `budget = { … }` line that fits them. A call past its budget fails. Each call is billed its memory budget for as long as it runs, waiting included, so a tool without one pays for the host's default, 128 MiB. A tool whose need depends on its arguments calls `berm_lang::limit(0, 0, 0, <bytes>)` once, before doing the work, to lower that call's memory and its bill.
- On Cloud each service reserves its `[database] size`, 1GiB when unset, whatever it holds. An account's services reserve at most its plan's storage together, and a deploy past it is refused, so size each service to what it holds (`crab usage <service>` reports `stored`).

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

## Calling another account's service

- On Cloud, a service is addressed as `owner/name`, its page at `https://crab.talk/{owner}/{name}`. `crab inspect owner/name` prints another account's public service: its README and its public tools with their arguments. Read it before the first call.
- `crab call owner/name <tool> '<arguments as JSON>'` calls one of its public tools, reading the arguments from stdin when they are left out. It prints the result, and what the call used on stderr; a tool that ran and refused exits 2.

## Signing in, paying and secrets

The person you act for is not at your machine: never open a browser or wait on a prompt for something only they can do.

- Sign in with `crab login --device`: it prints a link and a code, then waits until the person approves or declines, up to 30 minutes. Send them the link, or the code to enter at `crab.talk/device`; once they approve, you are signed in as them. Plain `crab login` opens a browser on your machine, which they cannot reach.
- A deploy on Hobby is refused until the account is verified: run `crab billing verify`. When it prints a link, send it to the person: its page connects GitHub to their account and verifies it. Then run `crab request <code> --wait`; if it fails, the page said why, and Go is the way to deploy. To buy a plan or credit, run `crab billing plan <slug>` or `crab billing credit` with `--for` saying what it is for; the person reads it before paying. Send them the link it prints, with what it buys and its price, then run `crab request <code> --wait`. The code lasts 30 minutes; if it expires or they decline, ask before making another.
- On Cloud, `crab deploy` and `crab push` wait for the person to approve the deploy: send them the link it prints as its last line, with what it deploys. They review it on that page, signed in, and approve or decline; the command then prints each service and its page, or fails. The link lasts 15 minutes, and a newer deploy of the same service replaces it. Pass `--yes` only when the person asked you to deploy without their review.
- An API key calls another account's priced service only once an owner of its account grants it, and spends at most its daily budget on tool prices, $5 unless they set another. A call refused for either prints a link: send it to the person with what the service is for and its price, then run `crab request <code> --wait` and call again once they approve. To ask before calling, run `crab key ask --service <owner>/<name>` or `crab key ask --budget <usd>`, with `--context` saying what it is for.
- A deploy that sets prices is refused until the account has a payout account that can receive payouts, a US one. Run `crab earnings connect` and send the person the link it prints; they finish Stripe's onboarding there, and the deploy goes through once Stripe enables payouts.
- Never ask the person to paste a secret's value into the chat. For local development on the local host, ask them to run `crab --context default secret fill <service>` in their own terminal, with `--copy KEY=VALUE` for the values its credentials have them enter at the provider: it shows each credential's guide and sets what they type on the local host, where test keys belong. It refuses to run where stdin is not a terminal, so never run it yourself. On Cloud, once the service is deployed, ask for every secret it has unset in one request: `crab secret request <service>` with `--for` saying what they are for and `--copy KEY=VALUE` for each value its credentials have the person enter at the provider, such as `--copy callback=<URL>`. The guides to getting them come from the `[credentials]` of its `crab.toml` and its kits'; a service's own secret that needs steps is declared as a credential with a `guide` naming the provider, the steps in its dashboard, and the URL of the page, and `hosts` naming in full each host its secrets are sent to. The person confirms the hosts when they set the secrets, and a secret is sent to no other host, even one the manifest declares. `--credential <id>` asks again for one, as after rotating it. Send them the link it prints, then run `crab request <code> --wait`. They set them on that page, signed in; you see that they were set, never their values. A code sent to the wrong person, or asking for the wrong thing, is ended with `crab request <code> --expire` before you ask again; what was already saved stays set.

## Where the truth is

- A refusal that names a code, as `Error: 402 [E0001]: …`, is explained by `crab explain E0001`: what it means, the corner cases and what to do. Run it before deciding anything or asking the person.

Read these before writing code. Paths are relative to the crab repository.

| Question | Source |
| --- | --- |
| The CLI, contexts, `crab.toml` | `crates/crab/README.md` |
| A whole working service to start from: its files and the commands that test, build, deploy and call it | https://crab.talk/docs/development/complete-service |
| `#[tool(...)]` and its `query`, `cron`, `stripe`; `#[derive(Schema)]`, `#[event]`, `emit!`, `#[error_code]`, `declare_service!`: what each accepts and generates | https://crab.talk/docs/development/macros |
| Every `berm_lang` function and type, with its signature | https://crab.talk/docs/development/api |
| Callers, data, HTTP, calls between services, crons and wakes, vars: each one's rules | the matching page under https://crab.talk/docs/development, as `/docs/development/http` |
| `crab.toml` keys, `[tools]` and its `access`, `title`, `price`, `catalog`, `[syscalls]`, secrets, credentials, kits | https://crab.talk/docs/development/crab-toml |
| `crab dev`, the local host, local secrets, replaying a failed call | https://crab.talk/docs/guides/local |
| Deploying to Cloud, images and registries | https://crab.talk/docs/guides/deploy |
| Secrets on Cloud and asking a person to set them | https://crab.talk/docs/guides/secrets |
| Declaring and using secrets in code | https://crab.talk/docs/development/secrets |
| An error that names a code, as `[E0001]` | `crab explain E0001`, https://crab.talk/docs/errors/introduction |
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
