---
name: crab
description: Build, reuse, deploy and call CrabTalk services with the Crab CLI, including local development, libraries, templates, credentials and Cloud approvals.
---

# Building on crab

Crab is the CLI and local host for CrabTalk. This repository contains the CLI (`crates/crab`), Berm runtime (`berm/`), and deployable services and reusable libraries (`services/`).

## Choose how to use a capability

- Browse provider integrations in `services/connect/`, agent services in `services/agents/`, website tools in `services/web/`, and operational services in `services/ops/`. Library-only packages live under `services/kits/`. Check [services](../../services/README.md) and [crabkit](../../crates/crabkit/README.md) before writing an integration. Call an existing deployment, deploy a published template, or link its library when those fit the task. Write a product service for its own behavior.
- A service is a library crate in the product's repository, built as a `cdylib` (`[lib] crate-type = ["cdylib", "rlib"]`, no bin target), with a `crab.toml` beside its `Cargo.toml`. Its tools are functions marked `#[berm_lang::tool(...)]`; what it reaches for is declared in `crab.toml`. `crab new` writes one; `crab build` builds its image and prints the path; `crab deploy` builds and deploys it; `crab push` does the same and leaves it stopped; `crab stop` and `crab start` stop and serve it, keeping its data. A bare `cargo build` writes an image a host refuses.
- Its tools, its database, its secrets and its public surface are the product's.
- A setting whoever deploys a copy chooses, such as the keywords a monitor watches, is a var: declared under `[vars]` in `crab.toml` with a `default` and a `description`, read with `berm_lang::var("name")`. Each copy holds its own value, set with `crab var set <service> <var> <value>` and read from the next call. A secret is never a var.
- `crab dev -d` starts the local host at `127.0.0.1:7777` in the background, one per machine. `crab --context default deploy [dir]` builds and deploys there the service in `dir` (default `.`), or workspace services beneath it not marked `deploy = false`; `-p` selects names across the workspace instead, starting the host when it is not running; run it again after a change. `crab dev status` says whether the host runs; `crab dev stop` stops it. Other commands reach it with `--context default`. Its output is `~/.crab/host/host.log`.
- A service with standard-library dependencies enables `berm-lang = { workspace = true, features = ["std"] }` and omits `#![no_std]`. Default guest builds remain `no_std`.
- Each tool gets a budget in `crab.toml` before it goes to Cloud: `budget = { cpu = <µs>, read = <rows>, written = <rows>, memory = <MiB> }` on its `[tools.<tool>]`. Call it on the local host with the largest inputs it should take (`crab --context default call <service> <tool> '<arguments>' --caller alice`), read what the call prints on stderr, and set each key to about 1.5× the most it reached. Once it runs on Cloud, `crab budget <service> <tool>` prints what its runs used and the `budget = { … }` line that fits them. A call past its budget fails. Each call is billed its memory budget for as long as it runs, waiting included, so a tool without one pays for the host's default, 128 MiB. A tool whose need depends on its arguments calls `berm_lang::limit(0, 0, 0, <bytes>)` once, before doing the work, to lower that call's memory and its bill.
- On Cloud each service reserves its `[database] size`, 1GiB when unset, whatever it holds. An account's services reserve at most its plan's storage together, and a deploy past it is refused, so size each service to what it holds (`crab usage <service>` reports `stored`).

## Reusing services and libraries

- Keep `berm-lang`, `crabkit` and linked provider packages on the same Crab revision. `crab new` writes `berm-lang` at the tag of the CLI's release, and `crabkit` goes beside it with a feature per library:

  ```toml
  [dependencies]
  berm-lang = { git = "https://github.com/crabtalk/crab", tag = "v0.1.1" }
  crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.1", features = ["auth"] }
  ```

- A library with host requirements declares them in its own `crab.toml`. For libraries bundled by `crabkit`, enable the matching feature and call the API from your tools, such as `crabkit::auth`. Linking a library is using it: the build finds it among the crates compiled into the image, and the secrets, fetched hosts and migrations its `crab.toml` declares reach the service's manifest; its tables live in the service's database.
- A library-only library has no standalone deployment. Deployable services also expose Rust APIs: depend on them with `default-features = false` to disable their standalone tool exports. Linked dependencies contribute credentials, host permissions and migrations to the consuming service; their deployment name, visibility, tool access, prices, crons, vars and database sizing stay with their own deployment. A dependency with migrations needs a stable `id` in its `crab.toml`.
- Each library's `README.md` lists what the service still declares itself.
- For provider HTTP code, use `berm_lang::http::Client` for shared headers, non-2xx errors and JSON decoding. Keep provider-specific response envelopes and body-level failures in the provider crate. `crab-openapi` generates bindings from local OpenAPI 3.0/3.1 specs; read its README before using it.

## Deployment visibility and templates

- An optional `logo = "https://example.com/logo.svg"` in `crab.toml` names the service logo. It must be an absolute HTTPS URL; build and host validation do not fetch it. The logo stays with the image and template copies, and is not inherited from linked libraries.
- `public` exposes a Cloud deployment; `template` publishes its image for other accounts to deploy. Both default to `false`, independently of each tool's `access`.
- `crab publish [dir]` publishes or withdraws templates selected by directory or `-p`, including services marked `deploy = false`, without running instances. For example, `crab publish services/connect` selects that catalog without a catalog-level `crab.toml`. `crab deploy --from owner/name` creates a private copy without a local build, with its own credentials and state. Existing copies keep their version until upgraded.
- The `connect`, `agents`, `web` and `ops` catalog directories match the intended publisher orgs. Select the account explicitly, such as `crab --org web publish services/web`; the directory does not select an org.
- Images upload to the selected host by default. A context registry or `--registry` selects OCI publication instead; `image` is not a `crab.toml` key.

## Calling another account's service

- On Cloud, a service is addressed as `owner/name`, its page at `https://crab.talk/{owner}/{name}`. `crab inspect owner/name` prints another account's public service: its README and its public tools with their arguments. Read it before the first call.
- `crab call owner/name <tool> '<arguments as JSON>'` calls one of its public tools, reading the arguments from stdin when they are left out. It prints the result, and what the call used on stderr; a tool that ran and refused exits 2.

## Local callers and MCP

- `crab --context default call <service> <tool> '<arguments>' --caller alice` calls a public tool locally. Without a caller, only tools with `access = "anonymous"` can be called from outside.
- Local MCP uses Streamable HTTP at `http://127.0.0.1:7777/mcp`. Send `x-berm-caller: {"id":"alice"}` to discover and call public tools as that caller. Local identities are unverified; keep the endpoint on loopback.

## Signing in, paying and secrets

The person you act for is not at your machine: never open a browser or wait on a prompt for something only they can do.

- Sign in with `crab login --device`: it prints a link and a code, then waits until the person approves or declines, up to 30 minutes. Send them the link, or the code to enter at `crab.talk/device`; once they approve, you are signed in as them. Plain `crab login` opens a browser on your machine, which they cannot reach.
- A deploy on Hobby is refused until the account is verified: run `crab billing verify`. When it prints a link, send it to the person: its page connects GitHub to their account and verifies it. Verification issues one $1 grant valid for 30 days; unfunded hosting retains data for 30 days. The deployment can configure these terms; `crab billing` shows its available balance, expiry and recovery deadline. Storage is included; monthly resets and plan changes never replenish trial credit. Upgrade or export before recovery ends. Then run `crab request <code> --wait`; if it fails, the page said why, and Go is the way to deploy. To buy a plan or credit, run `crab billing plan <slug>` or `crab billing credit` with `--for` saying what it is for; the person reads it before paying. Send them the link it prints, with what it buys and its price, then run `crab request <code> --wait`. The code lasts 30 minutes; if it expires or they decline, ask before making another.
- On Cloud, `crab deploy` and `crab push` wait for the person to approve the deploy: send them the link it prints as its last line, with what it deploys. They review it on that page, signed in, and approve or decline; the command then prints each service and its page, or fails. The link lasts 15 minutes, and a newer deploy of the same service replaces it. Pass `--yes` only when the person asked you to deploy without their review. When supplying a deploy request's `context` through the API, use one short sentence describing the change, at most 160 characters. Put setup instructions in credential guides.
- An API key calls another account's priced service only once an owner of its account grants it, and spends at most its daily budget on tool prices, $5 unless they set another. A call refused for either prints a link: send it to the person with what the service is for and its price, then run `crab request <code> --wait` and call again once they approve. To ask before calling, run `crab key ask --service <owner>/<name>` or `crab key ask --budget <usd>`, with `--context` saying what it is for.
- A deploy that sets prices is refused until the account has a payout account that can receive payouts, a US one. Run `crab earnings connect` and send the person the link it prints; they finish Stripe's onboarding there, and the deploy goes through once Stripe enables payouts.
- Never ask the person to paste a secret's value into the chat. For local development on the local host, ask them to run `crab --context default secret fill <service>` in their own terminal, with `--copy KEY=VALUE` for the values its credentials have them enter at the provider: it shows each credential's guide and sets what they type on the local host, where test keys belong. It refuses to run where stdin is not a terminal, so never run it yourself. On Cloud, once the service is deployed, ask for every secret it has unset in one request: `crab secret request <service>` with `--for` saying what they are for and `--copy KEY=VALUE` for each value its credentials have the person enter at the provider, such as `--copy callback=<URL>`. The guides to getting them come from the `[credentials]` of its `crab.toml` and its libraries'; a service's own secret that needs steps is declared as a credential with a `guide` naming the provider, the steps in its dashboard, and the URL of the page, and `hosts` naming in full each host its secrets are sent to. The person sees the fixed manifest hosts when they provision the secrets. Destination changes require a manifest change, redeployment and fresh provisioning; API keys cannot set values or destinations. Saved values are never returned, including fields with `masked = false`. `--credential <id>` asks again for one, as after rotating it. Send them the link it prints, then run `crab request <code> --wait`. They set them on that page, signed in; you see that they were set, never their values. A code sent to the wrong person, or asking for the wrong thing, is ended with `crab request <code> --expire` before you ask again; what was already saved stays set.

## Where the truth is

- A refusal that names a code, as `Error: 402 [E0001]: …`, is explained by `crab explain E0001`: what it means, the corner cases and what to do. Run it before deciding anything or asking the person.

Read these before writing code. Paths are relative to the crab repository.

| Question | Source |
| --- | --- |
| The CLI, contexts, `crab.toml` | `crates/crab/README.md` |
| A whole working service to start from: its files and the commands that test, build, deploy and call it | https://crab.talk/docs/development/complete-service |
| `#[tool(...)]` and its `query`, `cron`, `stripe`, `webhook`; `#[derive(Schema)]`, `#[event]`, `emit!`, `#[error_code]`, `declare_service!`: what each accepts and generates | https://crab.talk/docs/development/macros |
| Every `berm_lang` function and type, with its signature | https://crab.talk/docs/development/api |
| Callers, data, HTTP, calls between services, crons and wakes, vars: each one's rules | the matching page under https://crab.talk/docs/development, as `/docs/development/http` |
| `crab.toml` keys, `[tools]` and its `access`, `title`, `price`, `catalog`, `[syscalls]`, secrets, credentials, libraries | https://crab.talk/docs/development/crab-toml |
| `crab dev`, the local host, local secrets, replaying a failed call | https://crab.talk/docs/guides/local |
| Deploying to Cloud, images and registries | https://crab.talk/docs/guides/deploy |
| Secrets on Cloud and asking a person to set them | https://crab.talk/docs/guides/secrets |
| Declaring and using secrets in code | https://crab.talk/docs/development/secrets |
| An error that names a code, as `[E0001]` | `crab explain E0001`, https://crab.talk/docs/errors/introduction |
| Writing a service README for crab.talk | `services/README-guide.md` |
| Linking service libraries | https://crab.talk/docs/development/reuse |
| A service or library's API and setup | `services/{connect,agents,web,ops}/<name>/README.md`, or `services/kits/<name>/README.md` for a library |
| Building a service from OpenAPI | https://crab.talk/docs/development/openapi; macro reference: `crates/openapi/README.md` |

## A local crab checkout

- A product builds against a local checkout by patching crab's git source in its root `Cargo.toml`, one path per crate it uses:

  ```toml
  [patch."https://github.com/crabtalk/crab"]
  berm-lang = { path = "../crab/berm/lang" }
  crabkit = { path = "../crab/crates/crabkit" }
  crabkit-auth = { path = "../crab/services/kits/auth" }
  ```

- Run Rust tests with `cargo nextest run`; the Crab workspace requires nextest 0.9.98 or newer, and its CI uses `cargo nextest run --workspace --release`. Tests belong in each crate's `tests/` directory.
- Workspace Wasm behavior tests read `berm_build::fixture::image(package)`. Register each image and its consuming tests in `.config/nextest.toml`; setup scripts build selected images before tests run. Only tests of the builder or CLI build behavior should compile inside a test.
- Update affected READMEs, examples, website guides and this skill alongside behavior or layout changes.
- What a service needs from berm or a library and cannot get is a change to crab, made in crab. What only one product needs stays in that product's service.
- To check a fix against a failed call's tape, run `crab replay <service> <id> --build` in the service crate. It builds locally, prints both digests, and reaches no live services. `--build` and `--on` exclude each other.
