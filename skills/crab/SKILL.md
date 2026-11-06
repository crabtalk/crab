---
name: crab
description: Build, reuse, deploy and call CrabTalk services with the Crab CLI, including local development, libraries, templates, credentials and Cloud approvals.
---

# Building on crab

Crab is the CLI and local host for CrabTalk. This repository contains the CLI (`crates/crab`), Berm runtime (`berm/`), and deployable services and reusable libraries (`services/`).

## Choose how to use a capability

- Browse provider integrations in `services/connect/`, agent services in `services/agents/`, website tools in `services/web/`, and operational services in `services/ops/`. Library-only packages live under `services/kits/`. Check [services](../../services/README.md) and [crabkit](../../crates/crabkit/README.md) before writing an integration. Call an existing deployment, deploy a published template, or link its library when those fit the task. Write a product service for its own behavior.
- A service is a library crate in the product's repository, built as a `cdylib` (`[lib] crate-type = ["cdylib", "rlib"]`, no bin target), with a `crab.toml` beside its `Cargo.toml`. Its tools are functions marked `#[berm_lang::tool(...)]`; what it reaches for is declared in `crab.toml`. `crab new` writes one with `echo` and durable `remember`/`recall` examples, a 64 MiB storage reservation, and no generated tool-specific test; add tests for the tools you keep. `crab build` builds its image and prints the path; `crab deploy` builds and deploys it; `crab push` does the same and leaves it stopped; `crab stop` and `crab start` stop and serve it, keeping its data. A bare `cargo build` writes an image a host refuses.
- Its tools, its database, its secrets and its public surface are the product's.
- Host calls are discovered from the final Wasm imports. Configure outbound destinations under `[http]` with `fetch = ["api.example.com"]` and `proxy = ["*.example.org"]`; missing or empty lists allow none. Library destinations merge into the service.
- A setting whoever deploys a copy chooses, such as the keywords a monitor watches, is a var: declared under `[vars]` in `crab.toml` with a `default` and a `description`, read with `berm_lang::var("name")`. Each copy holds its own value, set with `crab var set <service> <var> <value>` and read from the next call. A secret is never a var.
- `crab build` automatically installs missing Rust build tools through the official rustup installer, then adds the `wasm32-unknown-unknown` target. No manual toolchain setup is required. The local loop needs no account or login. `crab --context default status` checks the local host; `crab plans` requires a Cloud context selected by `crab login`.
- `crab dev -d` starts the local host at `127.0.0.1:7777` in the background, one per machine. `crab --context default deploy [dir]` builds and deploys there the service in `dir` (default `.`), or workspace services beneath it not marked `deploy = false`; `-p` selects names across the workspace instead, starting the host when it is not running; run it again after a change. `crab dev status` says whether the host runs; `crab dev stop` stops it. Other commands reach it with `--context default`. Its output is `~/.crab/host/host.log`.
- A service with standard-library dependencies enables `berm-lang = { workspace = true, features = ["std"] }` and omits `#![no_std]`. Default guest builds remain `no_std`.
- Size each tool's `crab.toml` budget from its built WebAssembly execution before Cloud deployment or template publication. Native tests and image builds do not establish valid runtime budgets. Follow [Sizing tool budgets](#sizing-tool-budgets).
- On Cloud each service reserves its `[database] size`, 1GiB when unset, whatever it holds. An account's services reserve at most its plan's storage together, and a deploy past it is refused, so size each service to what it holds (`crab usage <service>` reports `stored`).

## Sizing tool budgets

1. Define and enforce supported input bounds, including response sizes, batch counts, parser complexity and database size. Deploy locally with a provisional budget within the destination's ceilings; exercise representative and worst-case workloads repeatedly, including error paths. Use `crab bench <directory> <tool> --cases cases.jsonl --runs 10`, with representative and largest supported inputs and arranged HTTP responses; the [benchmark guide](https://crab.talk/docs/guides/bench) defines the JSON Lines format. `delay_ms` on a response accounts for memory held while waiting; `--live-http` executes real requests on every repetition.
2. Read the benchmark's own-tool CPU, rows and peak linear memory. CPU is in microseconds and memory in bytes; `crab.toml` memory is in MiB. Start with `ceil(1.5 × maximum)` per key, at least `1`. A budget-stopped run does not measure the work needed to finish. `crab call` and host-test meters include nested calls; `crab bench` separates own usage from chain cost. Exercise called tools separately too.
3. Check every value against the destination host. Default ceilings are `cpu = 100_000` (100 ms), `read = 1_000_000`, `written = 1_000_000`, `memory = 128`. `cpu = 500_000` is 500 ms and is refused by a default host. Omitted keys use defaults: the ceilings above except `written = 10_000`. Raising the local CPU ceiling does not change Cloud's.
4. Write the budget, rebuild, redeploy locally and repeat the workload before requesting Cloud approval or publishing. If it cannot fit with headroom, reduce resource use or supported input bounds; use batching or pagination when the operation supports them. Do not just lower the declaration.

`crab budget <service> <tool>` suggests budgets from Cloud's last 30 days. CPU and rows use successful chain totals: maxima below 100 runs, p99 from 100; failed runs are excluded. Memory uses the tool's own recorded peak. The suggestion does not check host ceilings or cover unobserved worst cases. Full workflow and units: [Budgets](https://crab.talk/docs/guides/budgets).

Memory is billed at the reserved budget for the call's full duration, waiting included. A tool can lower its budget once with `berm_lang::limit`; its memory argument is in bytes, and `0` leaves a dimension unchanged. See [Budgets in code](https://crab.talk/docs/development/budgets).

## Reusing services and libraries

- Keep `berm-lang`, `crabkit` and linked provider packages on the same Crab revision. `crab new` writes `berm-lang` at the tag of the CLI's release, and `crabkit` goes beside it with a feature per library:

  ```toml
  [dependencies]
  berm-lang = { git = "https://github.com/crabtalk/crab", tag = "v0.1.3" }
  crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.3", features = ["auth"] }
  ```

- A library with host requirements declares them in its own `crab.toml`. For libraries bundled by `crabkit`, enable the matching feature and call the API from your tools, such as `crabkit::auth`. Linking a library is using it: the build finds it among the crates compiled into the image, and the secrets, fetched hosts and migrations its `crab.toml` declares reach the service's manifest; its tables live in the service's database.
- A library-only library has no standalone deployment. Deployable services also expose Rust APIs: depend on them with `default-features = false` to disable their standalone tool exports. Linked dependencies contribute credentials, host permissions and migrations to the consuming service; their deployment name, visibility, tool access, prices, crons, vars and database sizing stay with their own deployment. A dependency with migrations needs a stable `id` in its `crab.toml`.
- Each library's `README.md` lists what the service still declares itself.
- For provider HTTP code, use `berm_lang::http::Client` for shared headers, non-2xx errors and JSON decoding. Keep provider-specific response envelopes and body-level failures in the provider crate. `crab-openapi` generates bindings from local OpenAPI 3.0/3.1 specs; read its README before using it.

## Deployment visibility and templates

- An optional `logo = "https://example.com/logo.svg"` in `crab.toml` names the service logo. It must be an absolute HTTPS URL; build and host validation do not fetch it. The logo stays with the image and template copies, and is not inherited from linked libraries.
- `public` exposes a Cloud deployment; `template` publishes its image for other accounts to deploy. Both default to `false`, independently of each tool's `access`.
- `crab publish [dir]` publishes or withdraws templates selected by directory or `-p`, including services marked `deploy = false`, without running instances. For example, `crab publish services/connect` selects that catalog without a catalog-level `crab.toml`. `crab deploy --from owner/name` creates a private copy without a local build, with its own credentials and state. Existing copies stay pinned unless their owner opts into automatic patch updates. `--version 1.2.3` selects an exact retained release; `--auto-upgrade` enables stable patches in the same major and minor version, subject to preflight and manual review for permission, setup, schedule, contract or migration changes. Redeploying from history disables automatic updates; changing template sources requires a fresh opt-in.
- `web/seo` and `web/markdown` are configured as public services without templates. Deploy them with `crab deploy`; `crab publish` alone does not start them.
- The `connect`, `agents`, `web` and `ops` catalog directories match the intended publisher orgs. Select the account explicitly, such as `crab --org web publish services/web`; the directory does not select an org.
- A signed-in account owner can delete a template and its retained images from the Template card’s actions menu or the account page’s Templates list, or through `DELETE /services/{owner}/{name}/template`. Running services and existing copies remain. Withdrawing with `template = false` retains the versions.
- Template version labels are immutable. New content requires a new package version; `crab publish --notes "…"` adds up to 2000 characters of release notes. Templates retain three release images; each deployed service retains its current image and two previous successful images. Small version identities remain after pruning or deletion. The Versions tab selects retained releases and lets publishers yank or unyank them. New copies use the latest non-yanked stable release, or the latest non-yanked prerelease when no stable release is retained. Yanking blocks new copies and upgrades; existing copies keep working.
- Images upload to the selected host by default. A context registry or `--registry` selects OCI publication instead; `image` is not a `crab.toml` key.

## Calling another account's service

- On Cloud, a service is addressed as `owner/name`, its page at `https://crab.talk/{owner}/{name}`. `crab inspect owner/name` prints another account's public service: its README and its public tools with their arguments. Read it before the first call.
- `crab call owner/name <tool> '<arguments as JSON>'` calls one of its public tools, reading the arguments from stdin when they are left out. It prints the result, and what the call used on stderr; a tool that ran and refused exits 2.

## Local callers and MCP

- `berm_lang::client()` reads the daily opaque client key. `berm_lang::client_ip()` reads the outside IPv4/IPv6 address, when supplied. Both follow nested calls.

- `crab --context default call <service> <tool> '<arguments>'` calls a public tool locally as the unchecked caller `local`. Use `--caller alice` to override it, or `--anonymous` to call without a caller; only tools with `access = "anonymous"` accept anonymous calls. These flags are local-only; Cloud uses the credential.
- Local MCP uses Streamable HTTP at `http://127.0.0.1:7777/mcp`. Send `x-berm-caller: {"id":"alice"}` to discover and call public tools as that caller. Local identities are unverified; keep the endpoint on loopback.

## Signing in, paying and secrets

The person you act for is not at your machine: never open a browser or wait on a prompt for something only they can do.

- Sign in with `crab login --device`: it prints a link and a code, then waits until the person approves or declines, up to 30 minutes. Send them the link, or the code to enter at `crab.talk/device`; they review the connection name, account, permissions and daily budget before approving a dedicated key. Manage it under Settings → API keys; it does not grant personal-session access. Plain `crab login` opens a browser on your machine, which they cannot reach.
- A deploy on Hobby is refused until the account is verified: run `crab billing verify`. When it prints a link, send it to the person: its page connects GitHub to their account and verifies it. Verification issues one $1 grant valid for 30 days; unfunded hosting retains data for 30 days. The deployment can configure these terms; `crab billing` shows its available balance, expiry and recovery deadline. Storage is included; monthly resets and plan changes never replenish trial credit. Upgrade or export before recovery ends. Then run `crab request <code> --wait`; if it fails, the page said why, and Go is the way to deploy. To buy a plan or credit, run `crab billing plan <slug>` or `crab billing credit` with `--for` saying what it is for; the person reads it before paying. Send them the link it prints, with what it buys and its price, then run `crab request <code> --wait`. The code lasts 30 minutes; if it expires or they decline, ask before making another.
- On Cloud, `crab deploy` and `crab push` wait for the person to approve the deploy: send them the link it prints as its last line, with what it deploys. They review it on that page, signed in, and approve or decline; the command then prints each service and its page, or fails. The link lasts 15 minutes, and a newer deploy of the same service replaces it. Personal CLI sessions always need browser approval, even without a terminal. Device logins use the dedicated key approved by the person. `--yes` requires an API key with `deployments:write`; use it only when the person asked you to deploy without their review. When supplying a deploy request's `context` through the API, use one short sentence describing the change, at most 160 characters. Put setup instructions in credential guides.
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
| `crab.toml` keys, `[tools]` and its `access`, `title`, `price`, `catalog`, `[http]`, secrets, credentials, libraries | https://crab.talk/docs/development/crab-toml |
| `crab bench`, JSON Lines workloads, HTTP fixtures and local cost estimates | https://crab.talk/docs/guides/bench |
| Budget units, measurement, headroom, host ceilings and E0013 | https://crab.talk/docs/guides/budgets |
| `crab dev`, the local host, local secrets, replaying a failed call | https://crab.talk/docs/guides/local |
| Deploying to Cloud, images and registries | https://crab.talk/docs/guides/deploy |
| Secrets on Cloud and asking a person to set them | https://crab.talk/docs/guides/secrets |
| Declaring and using secrets in code | https://crab.talk/docs/development/secrets |
| An error that names a code, as `[E0001]` | `crab explain E0001`, https://crab.talk/docs/errors/introduction |
| Writing a service README for crab.talk | `services/README-guide.md` |
| Linking service libraries | https://crab.talk/docs/development/reuse |
| A service or library's API and setup | `services/{connect,agents,web,ops}/<name>/README.md`, or `services/kits/<name>/README.md` for a library |
| Classifying tools for discovery and choosing catalog jobs | https://crab.talk/docs/guides/catalog |
| Building a service from OpenAPI | https://crab.talk/docs/development/openapi; macro reference: `crates/openapi/README.md` |

## A local crab checkout

- A product builds against a local checkout by patching crab's git source in its root `Cargo.toml`, one path per crate it uses:

  ```toml
  [patch."https://github.com/crabtalk/crab"]
  berm-lang = { path = "../crab/berm/lang" }
  crabkit = { path = "../crab/crates/crabkit" }
  crabkit-auth = { path = "../crab/services/kits/auth" }
  ```

- Run Rust tests with `cargo nextest run`; the Crab workspace requires nextest 0.9.98 or newer, and its CI uses `cargo nextest run --release`. Tests belong in each crate's `tests/` directory.
- Workspace Wasm behavior tests read `berm_build::fixture::image(package)`. Register images in `berm/build/examples/prepare.rs`; one Nextest setup builds the full test-image set before tests run, including focused runs. Test names need no configuration. Only tests of the builder or CLI build behavior should compile inside a test.
- Update affected READMEs, examples, website guides and this skill alongside behavior or layout changes.
- What a service needs from berm or a library and cannot get is a change to crab, made in crab. What only one product needs stays in that product's service.
- To check a fix against a failed call's tape, run `crab replay <service> <id> --build` in the service crate. It builds locally, prints both digests, and reaches no live services. `--build` and `--on` exclude each other.
