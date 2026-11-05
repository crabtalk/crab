# crab

The [local host](https://crab.talk/docs/guides/local), and the command-line client for it and CrabTalk Cloud.

```sh
crab dev -d
crab new example
cd example
crab build
crab deploy
crab ls
crab inspect example
crab idl example
crab logs example
crab usage example
crab rm example
```

`crab build` installs missing build tools automatically. It uses the official rustup installer for Rust and adds the `wasm32-unknown-unknown` target. Installed tools are available to the same invocation without restarting the shell. The local loop needs no account or login.

## crab.toml

Host calls are discovered from the linked Wasm imports. Set allowed outbound destinations under `[http]` with `fetch = ["api.example.com"]` and `proxy = ["*.example.org"]`. Missing or empty lists allow none; linked libraries contribute their destinations.

`crab build` builds the package in the directory it is given, `.` by default, and prints the path of its finished image, `berm/<crate>.wasm` under cargo's target directory. `crab deploy` builds it the same way and deploys it as the `crab.toml` beside its `Cargo.toml` says. `crab new` writes one, beside a library crate built as a `cdylib`.

```toml
# The name it deploys as.
name = "example"
public = true
template = true
logo = "https://example.com/logo.svg"

# Who may call a tool, its display name, its price a call on Cloud in USD
# micros, and the catalog job it does. A tool without a table is internal.
[tools.search]
access = "public"
title = "Search the web"
price = 2_000
catalog = "web.search"

# When the host calls each `#[tool(cron)]` tool: five fields, minute to day of
# week, in UTC.
[crons]
tick = "*/5 * * * *"

# Settings each deployed copy holds its own value of, read through
# `berm_lang::var`.
[vars]
keywords = { default = "rust, wasm", description = "Words to watch" }

# What the service's database may hold: 1GiB when unset, from 64KiB to 10GiB.
[database]
size = "2GiB"
# A SQLite database, relative to this file, a service on Cloud starts from on its first deploy.
seed = "seed.sqlite"
```

`logo` is optional and must be an absolute HTTPS URL. Builds, deployments and publication reject invalid values without fetching the image. The URL belongs to the service image and is retained by template copies; linked libraries do not contribute a logo.

`access` is `"public"`; `"anonymous"`, public and callable with no caller; or restricted to `"accounts"` or `"services"`. `title` is at most 60 characters on one line, `price` at least 100, `catalog` starts with a platform listed in [`crates/sdk/catalog.toml`](../sdk/catalog.toml), and `budget = { cpu, read, written, memory }` sets what one call may use, each key at least 1, CPU in microseconds, rows read and written, `memory` in MiB, a key left out the host's default. A price or catalog on a tool without `access` is refused, as is a table for a tool the service lacks; the build writes `access`, `title`, `catalog` and `budget` into the image, and a deploy carries `price`.

The build embeds `[crons]` in the image. Every deploy applies those schedules; an omitted tool has no schedule. Change `crab.toml` and redeploy to change them, including for template copies. A write that would take the database past `size` fails, and a deploy is refused while the database holds more than its `size`. On a service's first deploy to Cloud, `crab deploy` uploads a copy of `seed` and the service's database starts as it, held to the image's migrations: those it records applied are the image's first, unchanged. A service with a database takes no seed, and the local host takes none.

`berm_lang::var("keywords")` answers the value the deployed copy holds, or the var's `default` while it holds none, from the call after one is set; a var `[vars]` does not declare is refused. A service declares at most 32, each name letters, digits and `_`, each value at most 4096 bytes. `crab var ls NAME` lists a service's vars with their values, `crab var set NAME VAR VALUE` sets one, and `crab var unset NAME VAR` returns it to its default. A deploy's request sets the values it names and keeps the rest; a deploy whose `[vars]` drops a var drops its value. Linked libraries do not contribute variable declarations; declare the settings your deployment needs itself.

## Developing

`crab dev` runs the local host on `127.0.0.1:7777`, its root `~/.crab/host`, in the foreground until Ctrl-C; `-d`/`--detach` starts it in the background, writing to `host.log` under its root. It is one per machine and knows nothing of directories: services reach it through `crab deploy` on the `default` context, which starts it in the background when nothing answers. `crab deploy [dir]` selects the service in that directory, or all Cargo workspace services beneath it whose `crab.toml` does not say `deploy = false`. The directory defaults to `.`; `-p <name>`, repeatable, selects services by name anywhere in the workspace instead. `crab push` deploys the same way and installs each service stopped; `crab start` and `crab stop` serve and stop services, named or the same set, keeping what they committed, and `crab ls` shows which run. `crab rm` with no name removes the same set. `crab dev status` prints its pid, root and start, and `crab dev stop` stops it.

`crab --context default status` probes the local host at `/meta/health`.

## Contexts

`crab` talks to the host its context names, the way `docker context` picks a daemon. `default` is the local host, `crab dev`, on `http://127.0.0.1:7777`; `--context` picks another for one command.

```sh
crab context add cloud https://api.crab.talk --token "$KEY" --org acme
crab context use cloud
crab context ls
crab context use default
```

A service's manifest declares its secrets and their destinations. `crab secret ls NAME` lists names and hosts, never values. `crab secret fill NAME`, run by a person at a terminal, shows each credential's guide and fixed manifest hosts, then provisions or replaces the values they enter. On Cloud it requires a signed-in owner; API keys cannot provision or revoke secrets. `crab secret request NAME` prints a link where an owner supplies unset credentials; `--credential ID` requests one again for rotation. `--copy KEY=VALUE` supplies provider setup values such as a callback URL. For direct provisioning, `printf %s "$VALUE" | crab secret put NAME SECRET` provisions or replaces a declared secret; `crab secret rm NAME SECRET` revokes one. Outbound credentials need `hosts` in `[credentials]`; a bare secret has no outbound destinations. Changing manifest destinations requires fresh provisioning. Saved values are never returned, including fields with `masked = false`.

A context with a token and an `--org` or `--user` is Cloud, and its services are that account's. `deploy` sends the image itself, to the local host and to Cloud alike, and Cloud retains it for deployments and optional template publication. A context registry or `--registry` selects an OCI registry instead. A write permission includes read access to the same resource. A key uses explicit resource permissions; direct deployment requires `deployments:write`, configuration changes require `services:write`; `--token-stdin` reads it from stdin. On Cloud, `deploy` and `push` print a link, `https://crab.talk/deploy/WDJB-MJHT`, as their last line and wait: someone signed in who owns the account sees what the deploy changes there and approves or declines it, and the command then reports each service and its page, or fails. The link lasts 15 minutes; other pending requests remain valid. `--yes` deploys without asking. The local host never asks.

Contexts are kept in `~/.crab/contexts.toml`.

## Cloud account

`crab login` signs in to Cloud in the browser, keeps the session as the `cloud` context and uses it. A session reaches everything below; a deploy key is refused by all of it but `usage`.

`crab login --device`, or `crab login` where no browser opens, prints a link and a code and waits: a person signed in to Cloud opens the link, or enters the code at `/device`, and approves, and the session is theirs.

`crab plans` lists Cloud plans. On a local context it asks you to sign in with `crab login`.

`crab billing` shows Purchased credit separately from Trial credit, including trial expiry, the recovery deadline and funds reserved for running or unsettled work. Trial credit runs services; priced tools require Purchased credit.

GitHub verification on CrabTalk issues one $1 trial grant, valid for 30 days. The deployment can configure the amount and terms. `crab plans` and verification expose its lifetime and recovery terms. Storage is included, the trial never refills each month, and plan changes never create another grant. After it ends, services pause and data is kept for 30 days; upgrade or export before the recovery deadline to avoid deletion. Accounts returning from a paid plan also enter recovery when they have no execution funding.

`crab billing verify` verifies the account on the GitHub account it signs in with, which lets it deploy on Hobby while its execution entitlement lasts: the GitHub account is a year old or more, its own public repositories hold 10 stars or more, and it has verified no other account. When the check fails, it prints a link instead, `https://crab.talk/verify/WDJB-MJHT`, whose page connects GitHub to the account and verifies it, or says why it cannot. `crab billing plan` and `credit` ask a person to pay: each prints a link, `https://crab.talk/pay/WDJB-MJHT`, whose page shows the account, the price and `--for`, and takes the payment or a decline. The code also works typed at `https://crab.talk/pay`, and lasts 30 minutes. A refusal from Cloud that names a code, as `Error: 402 [E0001]: …`, ends with `crab explain E0001`, which prints what it means and what to do about it; the same explanations are at `https://crab.talk/docs/errors/introduction`. `crab request <code> --wait` follows a request: it returns once the request is done, declined or expired, and fails unless it was done. `crab request <code> --expire` ends one now, as if its time had run out; secrets already saved through it stay set.

```sh
crab login                     # --github for GitHub
crab login --device
crab status
crab usage                     # the account's services and its plan this period
crab earnings                  # what the account's priced tools earned, a month each, what is owed and the payouts
crab earnings connect          # opens Stripe's onboarding of the account's payout account, a US one
crab earnings dashboard        # opens the payout account's Stripe dashboard
crab billing                   # plan, credit, refill, card
crab billing verify
crab billing plan pro --for "Pro, to run the crawler past Hobby"
crab request WDJB-MJHT --wait
crab explain E0001             # what an error's code means and what to do about it
crab billing portal            # opens Stripe's portal in the browser
crab key create ci --days 90
crab app ls
crab org create acme
crab --org acme billing
```

A service's calls, cost, crons, wakes, visibility and sunset have their own commands. `--org` acts for an org in place of the context's account. Output is JSON with `--json`, and whenever stdout is not a terminal. A command that removes something asks first, and without a terminal refuses unless given `--yes`.

## From a workflow

The deploy is two lines:

```yaml
steps:
  - run: crab context add cloud https://api.crab.talk --token "$KEY" --org acme
    env:
      KEY: ${{ secrets.CRABTALK_KEY }}
  - run: crab deploy --context cloud
```

When pushing to an OCI registry with `--registry` or a context registry, `GITHUB_TOKEN` supplies the registry credential. For GitHub Actions pushing to GHCR, give the job `packages: write` and set `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`. Locally, `GITHUB_TOKEN=$(gh auth token)` supplies the same credential.

`crab new` writes a service crate pinning `berm-lang` to the CLI release tag; it needs no running host.

`inspect` prints a service's README, a table of its tools with each one's access, title, catalog job, price, marks and description, and each tool's JSON Schema as the argument list it describes. On Cloud, `inspect owner/name` shows another account's public service: its README and public tools, without what only its owners see.

`inspect` shows the most one call of each tool may use, which `budget` on its `[tools.<tool>]` in `crab.toml` sets; `budget NAME TOOL` prints what its runs used and a suggested `budget`, without checking host ceilings. `call` calls one public tool and prints what the call used on stderr; `usage` prints a service's meters over the last 30 days, a day each. Agents reach tools over MCP. On the local host, `call` defaults to the unchecked caller `local`; `--anonymous` calls without a caller and reaches only anonymous tools. `--caller alice --issuer https://id.example --user 42` makes the call as that caller and user, unchecked, so a service reading `berm_lang::caller()` and `user()` runs as it would for them on Cloud. These identity flags are local-only; Cloud uses the credential.

Before deployment or template publication, measure the built WebAssembly tool on the local host with the largest supported workloads, add headroom, and check every key against the destination's ceilings. Default ceilings are 100,000 µs CPU (100 ms), 1,000,000 rows read, 1,000,000 written and 128 MiB memory. Rebuild and retest under the chosen budget. Native tests and image builds do not establish that it fits; CLI call meters include nested calls. See [Sizing a budget](https://crab.talk/docs/guides/budgets#size-a-budget-before-deployment).

Local MCP clients can send `x-berm-caller: {"id":"alice"}` on requests to `/mcp` to discover and call public tools as that caller. This identity is unverified; without the header, only anonymous tools are discoverable and callable.

A call that failed, trapped included, keeps its tool, arguments, caller and the digest of the image it ran, and its tape: every answer it had from outside the service, in order, up to 4 MiB a call and 64 MiB a service, counted in what the service stores. `crab --context cloud replay NAME ID` pulls that image and runs the call again in the process, each answer taken off the tape, so it fails as it failed in production without reaching a database, the network or another service; its logs print to stderr. A request that differs from the recorded one stops it, naming the syscall. `--on CONTEXT` runs the call on that context's host instead, against its state: its database, real fetches, its secrets.

## Benchmarking

```sh
crab bench services/web/markdown read --cases cases.jsonl --runs 10
```

`bench DIRECTORY TOOL` builds and runs JSON Lines workloads on a temporary local host. Cases supply argument objects and HTTP responses, including headers, body files and optional response delays. Each case starts with a fresh migrated database; repetitions share its state. `--live-http` sends real requests instead of fixtures. The command uploads nothing.

The report separates the tool's own CPU, rows and peak memory from whole-chain cost, prices with the selected Cloud's rates, and suggests a budget with 1.5× headroom. Checks use runtime default ceilings. Incomplete runs and unexpected failures prevent a suggestion; an over-ceiling suggestion is reported without clipping. Exit `2` means the workload or budget did not pass. `--json` gives structured measurements, and `--offline` uses labeled SDK default rates. [Case format and limits](https://crab.talk/docs/guides/bench).

## Publication and images

Template versions are immutable: changing an image requires a new package version. Each template retains three release images; its Versions tab selects exact images and lets publishers yank or unyank releases. New copies use the latest non-yanked stable release, or the latest non-yanked prerelease when no stable release is retained. Yanked releases cannot create new copies; existing copies keep working. `crab publish DIR --notes "Fix pagination"` attaches up to 2000 characters of release notes. Pruned images become unavailable for new deployments; version identities remain reserved.

`crab deploy --from connect/github --version 1.2.3` selects an exact retained release. Omit `--version` for the latest non-yanked stable release, falling back to prereleases when no stable release is retained. `--auto-upgrade` opts the copy into newer stable patches in the same major and minor version; it is off by default and appears in the approval page. The website’s Deployment card changes this setting and reports failures. Permission, setup, schedule, incompatible contract and migration changes require manual review. Stopped services stay stopped. Redeploying from history disables automatic updates; switching template sources requires a fresh opt-in. See [Automatic patch updates](https://crab.talk/docs/guides/deploy#automatic-patch-updates).

Deploy history retains the current image and two previous successful images; older records remain without their uploaded image. Existing template copies retain their own images, independently of the publisher’s releases.

`public` exposes your Cloud deployment; `template` publishes its image so others can deploy private copies. Both default to `false`. `crab publish` publishes or withdraws a template without running an instance, using the same approval flow as deployment. It uses the same directory and `-p` selection as deployment, but includes services marked `deploy = false`. For example, `crab publish services/connect` selects only that catalog; running `crab publish` inside it selects the same services. No catalog-level `crab.toml` is needed.

Images upload to the selected host by default. `crab context add local http://localhost:7777 --registry localhost:5001` selects an OCI registry for that context; `crab deploy --registry localhost:5001` overrides it for one command. Repositories use the service name, and deployments pin the OCI artifact digest.

`crab deploy --from owner/name` creates a private copy of a published template without building it locally. The copy has its own credentials and state.

An account owner can delete a template and all retained versions from the account page’s Templates actions menu. Running services and existing copies remain. Deletion requires a signed-in owner session; withdrawing with `template = false` retains the versions.

## License

AGPL-3.0-only
