# crab

The [local host](https://crabtalk.ai/docs/guides/local), and the command-line client for it and CrabTalk Cloud.

```sh
crab dev -d
crab new example
cd example
crab build
crab up
crab ls
crab inspect example
crab idl example
crab logs example
crab usage example
crab rm example
```

`crab build` installs missing build tools automatically. It uses the official rustup installer for Rust and adds the `wasm32-unknown-unknown` target. Installed tools are available to the same invocation without restarting the shell. The local loop needs no account or login.

## crab.toml

Host calls are discovered from the linked Wasm imports. Set allowed outbound destinations under `[http]` with `fetch = ["api.example.com"]`. Credential hosts join `fetch`; list only other direct destinations under `[http] fetch`. Missing or empty lists add no hosts; linked libraries contribute their destinations.

`crab build` builds source and prints its image path. `crab up [dir]` builds and deploys your source, selecting workspace services with `-p`. `crab push [dir]` publishes images to Cloud without deploying them. `crab deploy owner/name --version 1.2.3` deploys a published image under the selected account. `crab pull owner/name:1.2.3` downloads it into the local host; local deployments use stored images and pull when needed.

```toml
# When the host calls each `#[tool(cron)]` tool: five fields, minute to day of
# week, in UTC.
[schedules]
tick = "*/5 * * * *"

# Settings each deployed copy holds its own value of, read through
# `berm_lang::var`.
[vars]
keywords = { default = "rust, wasm", description = "Words to watch" }

# What the service's database may hold: 1GiB when unset, from 64KiB to 10GiB.
[storage]
size = "2GiB"
# A SQLite database, relative to this file, a service on Cloud starts from on its first deploy.
seed = "seed.sqlite"

[image]
name = "alice/example"
deploy = true
logo = "https://example.com/logo.svg"
```

`logo` is optional and must be an absolute HTTPS URL. Builds, deployments and publication reject invalid values without fetching the image. The URL belongs to the service image and is retained by deployments; linked libraries do not contribute a logo.

Tools declare access, price, catalog and `budget(...)` on `#[tool]`. Access defaults to `"public"`; `"anonymous"`, `"accounts"`, `"services"` and `"internal"` select other callers. Price is in USD micros, at least 100 when set. The title is the first doc-comment line, at most 60 characters and without a final period; the remaining comment is the description. Every declaration, including price, is compiled into the image. `crab.toml` refuses `[tools]`.

The build embeds `[storage] size` as the image's default reservation. New deployments use it when no size is supplied; redeployments keep their existing reservation unless explicitly resized. The default is 1 GiB for shared storage and zero for a service using caller storage alone.

Declare caller storage with `[storage.caller]` and its migration directory. Service code uses `berm_lang::caller_storage` to read and write data scoped to the verified caller. On Cloud, the billed account's plan bounds the bytes held. With the local host, the unchecked caller (default `local`) and any supplied end user select the file. Removing a local deployment retains its caller files. Use `crab storage` to list your files and `crab callers NAME` to see caller metadata for a service you operate.

The build embeds `[schedules]` in the image. Every deploy applies those schedules; an omitted tool has no schedule. Build a new image to change them; published-image copies use their selected release's schedules. A write that would take the database past `size` fails, and a deploy is refused while the database holds more than its `size`. On a service's first deploy to Cloud, `crab up` uploads a copy of `seed` and the service's SQL and key-value data start from it. SQL seeds must hold to the image's migrations: those recorded as applied are the image's first, unchanged. Services without SQL accept a seed containing only exported key-value state. An existing service takes no seed, and the local host takes none.

`berm_lang::var("keywords")` answers the value the deployed copy holds, or the var's `default` while it holds none, from the call after one is set; a var `[vars]` does not declare is refused. A service declares at most 32, each name letters, digits and `_`, each value at most 4096 bytes. `crab var ls NAME` lists a service's vars with their values, `crab var set NAME VAR VALUE` sets one, and `crab var unset NAME VAR` returns it to its default. A deploy's request sets the values it names and keeps the rest; a deploy whose `[vars]` drops a var drops its value. Linked libraries do not contribute variable declarations; declare the settings your deployment needs itself.

## Developing

`crab dev` runs the local host on `127.0.0.1:7777`, its root `~/.crab/host`, in the foreground until Ctrl-C; `-d`/`--detach` starts it in the background, writing to `host.log` under its root. It is one per machine and knows nothing of directories: services reach it through `crab up` on the `default` context, which starts it in the background when nothing answers. `crab up [dir]` selects the service in that directory, or all Cargo workspace services beneath it with a `crab.toml`. The directory defaults to `.`; `-p <name>`, repeatable, selects services by name anywhere in the workspace instead. `crab start` and `crab stop` serve and stop services, named or the same set, keeping what they committed, and `crab ls` shows which run. `crab rm` with no name removes the same set. `crab dev status` prints its pid, root and start, and `crab dev stop` stops it.

`crab --context default status` probes the local host at `/meta/health`. On Cloud, `crab status` also reads the current credential through `GET /auth/status`: its type and expiry, and for keys, their account, permissions, daily budget and spend, and priced-service grants. This needs no management scope; the selected account stays separate from the credential’s account. Plan details appear when usage access permits them.

## Contexts

`crab` talks to the host its context names, the way `docker context` picks a daemon. `default` is the local host, `crab dev`, on `http://127.0.0.1:7777`; `--context` picks another for one command.

```sh
crab context add cloud https://api.crabtalk.ai --token "$KEY" --org acme
crab context use cloud
crab context ls
crab context use default
```

A service's manifest declares its secrets and their destinations. `crab secret ls NAME` lists names and hosts, never values. `crab secret fill NAME`, run by a person at a terminal, shows each credential's guide and fixed manifest hosts, then provisions or replaces the values they enter. On Cloud provisioning requires a signed-in owner. `crab secret request NAME` prints a link where an owner supplies unset credentials; `--credential ID` requests one again for rotation. `--copy KEY=VALUE` supplies provider setup values such as a callback URL. For direct provisioning, `printf %s "$VALUE" | crab secret put NAME SECRET` provisions or replaces a declared secret; `crab secret rm NAME SECRET` revokes one. Cloud CLI sessions open a browser approval request for revocation. API keys with `services:write` revoke directly. Outbound credentials need `hosts` in `[credentials]`; a hostless credential has no outbound destinations. Changing manifest destinations requires fresh provisioning. Saved values are never returned, including fields with `masked = false`.

Runtime secret resources belong to the account and have stable opaque IDs. A service using them declares `secret_resources = true` and its HTTP permissions. On Cloud, `crab secret resource request SERVICE "Research Bot" --host api.example.com` prints a secure provisioning link and the future secret ID. A signed-in owner of the secret’s account supplies the value and approves the grant there. The target can be a service in that account or a public service it can call in another account. Use `--id ID` to rotate an existing resource through the same flow. At a person's terminal, `crab secret resource create NAME` and `rotate ID` read the value from stdin. `resource ls` shows metadata, `resource grant ID SERVICE --host HOST` grants a service access, `resource revoke ID SERVICE` removes that grant, and `resource rm ID` deletes the secret and all grants. Cloud writes require a signed-in session; API keys can propose secure requests. Use `{secret:ID}` in outgoing headers or bodies. An authenticated call can use runtime secrets owned by its account. Scheduled calls require the service to bind the resource during an authenticated write call and hold a current grant for the destination. Use declared named secrets for credentials used on behalf of other callers. Grants never expand HTTP permissions, and secret values are never returned. Resource creation, rotation and grants do not redeploy the service.

A context with a token and `--org` or `--user` is Cloud. `crab up` sends built images to the selected host; `crab deploy owner/name` selects an image from Cloud. A write permission includes its resource’s read permission. Cloud deployments made with a CLI session wait for browser approval; `--yes` requires an API key with `deployments:write`. `crab push` requires publication permission for the image’s account. The local host requires no account.

`crab usage NAME` reports sampled database allocation in its Database column. WAL, logs and runtime caches are excluded; older hosts without database samples show `—`.

`crab deploys NAME` shows pending approval links and recorded attempts, including before the first deployment. Calls to an undeployed service awaiting approval return `E0046` to its owners. Pending uploads still count toward storage.

Contexts are kept in `~/.crab/contexts.toml`.

## Cloud account

`crab login` opens a browser approval page and prints its link and code. Approve the terminal while signed in to the intended account. The CLI saves that personal session as the `cloud` context. On a machine without a browser, open the printed link on another device. Cloud CLI sessions open a browser approval request for `crab rm`. API keys with `services:write` remove services directly. API keys reach only operations their permissions allow; identity, payment and security operations require a personal session.

`crab login --device` requests a dedicated agent key. It prints a link and a code and waits while a person signed in to Cloud reviews the connection name, account, permissions and daily budget. The default name is `crab (<context>)`; `--name` chooses the context. The key initially requests the standard read permissions and a $5 daily budget. Manage or revoke it under Settings → API keys. Existing personal sessions remain active; reconnect the agent and revoke its old session under Settings → Security to switch it to a key.

`crab plans` lists Cloud plans. On a local context it asks you to sign in with `crab login`.

`crab billing` shows purchased credit separately from compute credit, including compute credit's expiry, the recovery deadline and funds reserved for running or unsettled work. Compute credit pays runs, CPU, memory held and rows; priced tools and rotating IPs require purchased credit.

GitHub verification on CrabTalk issues one $1 trial grant, valid for 30 days. The deployment can configure the amount and terms. `crab plans` and verification expose its lifetime and recovery terms. Storage is included, the trial never refills each month, and plan changes never create another grant. After it ends, services pause and data is kept for 30 days; upgrade or download each service before the recovery deadline with `crab export OWNER/NAME` or Download in its Settings. The SQLite file contains SQL and key-value data, excluding workflow runs and checkpoints. Accounts returning from a paid plan also enter recovery when they have no execution funding.

`crab billing verify` verifies the account on the GitHub account it signs in with, which lets it deploy on Free while its execution entitlement lasts: the GitHub account is a year old or more, its own public repositories hold 10 stars or more, and it has verified no other account. When the check fails, it prints a link instead, `https://crabtalk.ai/verify/WDJB-MJHT`, whose page connects GitHub to the account and verifies it, or says why it cannot. `crab billing plan` and `credit` ask a person to pay: each prints a link, `https://crabtalk.ai/pay/WDJB-MJHT`, whose page shows the account, the price and `--for`, and takes the payment or a decline. The code also works typed at `https://crabtalk.ai/pay`, and lasts 30 minutes. A refusal from Cloud that names a code, as `Error: 402 [E0001]: …`, ends with `crab explain E0001`, which prints what it means and what to do about it; the same explanations are at `https://crabtalk.ai/docs/errors/introduction`. `crab request <code> --wait` follows a request: it returns once the request is done, declined or expired, and fails unless it was done. `crab request <code> --expire` ends one now, as if its time had run out; secrets already saved through it stay set.

```sh
crab login
crab login --device
crab status
crab usage                     # the account's services and its plan this period
crab earnings                  # what the account's priced tools earned, a month each, what is owed and the payouts
crab earnings connect          # opens Stripe's onboarding of the account's payout account, a US one
crab earnings dashboard        # opens the payout account's Stripe dashboard
crab billing                   # plan, credit, refill, card
crab billing verify
crab billing plan go --for "Go, to run the crawler past Free"
crab request WDJB-MJHT --wait
crab explain E0001             # what an error's code means and what to do about it
crab billing portal            # opens Stripe's portal in the browser
crab key create ci --days 90
crab app ls
crab org create acme
crab --org acme billing
```

A service's calls, cost, crons, wakes and sunset have their own commands. `--org` acts for an org in place of the context's account. Output is JSON with `--json`, and whenever stdout is not a terminal. A command that removes something asks first, and without a terminal refuses unless given `--yes`.

## Workflow runs

A deployed service may export workflows as well as tools. Inspect one before starting it:

```sh
crab workflow inspect owner/service.workflow
crab workflow run owner/service.workflow '{"question":"What changed?"}' --dry-run
crab workflow run owner/service.workflow '{"question":"What changed?"}' --yes
crab workflow status owner/service.workflow <run-id>
crab workflow continue owner/service.workflow <run-id> --yes
crab workflow cancel owner/service.workflow <run-id> --yes
```

Replace the address and input with a deployed workflow's contract. Use `service.workflow` on the local context. `inspect` (also `workflow dry-run`) resolves its graph, dependencies, call-count bounds and current quote without executing application tools. `run --dry-run` also validates the input. Input is JSON, read from stdin when omitted; empty input supplies `{}`.

`run` prints the graph and terms to stderr before confirmation, then returns the accepted run without waiting for completion. The fixed task price is charged only for a successful result. The owner's cumulative managed-spend cap and direct-call/control quote are separate; owner accounting is visible only to owners. Provider accounts charged independently remain outside the managed cap. A quote may change before admission, which revalidates it. `run`, `continue` and `cancel` use the existing confirmation prompt; scripts supply `--yes`. `--json`, also automatic when stdout is piped, prints one JSON inspection or run to stdout; progress and terms stay on stderr.

The run ID prints before admission. If the response is lost, read `status` first. `run --id <run-id>` may resend the same identity with identical input and accepted terms; a different ID creates a new run. `status` includes checkpoints, result, expiry and continuation eligibility. `run`, `status` and `continue` exit `2` for failed, indeterminate or cancelled outcomes, and `0` otherwise. An accepted `cancel` exits `0`.

`continue` retains the original input, image, prepared calls and successful checkpoints, uses the remaining owner cap and rechecks authorization and dependency terms. It never blindly retries an unknown external write; read its refusal and use `crab explain <code>` when provided. Expired or deleted checkpoints cannot continue. `cancel` stops new calls, but cannot recall a request already accepted by an external provider. On the local host, workflow inspection and admission reuse `call`'s unchecked `--caller`, `--issuer` and `--user` flags and default to `local`; admission requires a stable caller. Cloud uses the context credential. Local run controls operate on the local host's retained ledger.

## From GitHub Actions

The deploy is two lines:

```yaml
steps:
  - run: crab context add cloud https://api.crabtalk.ai --token "$KEY" --org acme
    env:
      KEY: ${{ secrets.CRABTALK_KEY }}
  - run: crab up --build --context cloud
```


`crab new` writes a service crate pinning `berm-lang` to the CLI release tag; it needs no running host.

`crab inspect .` builds the service in the current directory and reads its image; `crab inspect ./services/foo` selects another directory, and `-p` selects services in a workspace. Arguments starting with `.` or `/` are directories; other arguments are service addresses. `--json` prints the built manifest. Local inspection shows tool access, prices, budgets and catalog jobs, plus workflow spend and checkpoint defaults; it has no deployment runtime, unresolved secrets or Cloud quote.

`inspect` prints a service's README, a table of its tools with each one's access, title, catalog job, price, marks and description, and each tool's JSON Schema as the argument list it describes. On Cloud, `inspect owner/name` shows another account's public service: its README and public tools, without what only its owners see.

`inspect` shows the most one call of each tool may use, which `budget(...)` on `#[tool]` sets; `budget NAME TOOL` prints what its runs used and a suggested `budget`, without checking host ceilings. `call` calls one public tool and prints what the call used on stderr; `usage` prints a service's meters over the last 30 days, a day each. Agents reach tools over MCP. On the local host, `call` defaults to the unchecked caller `local`; `--anonymous` calls without a caller and reaches only anonymous tools. `--caller alice --issuer https://id.example --user 42` makes the call as that caller and user, unchecked, so a service reading `berm_lang::caller()` and `user()` runs as it would for them on Cloud. These identity flags are local-only; Cloud uses the credential.

Service code calls a running deployment with `berm_lang::call("owner/service", tool, args)` without a `calls` declaration. Cross-account targets must be public and accept service callers; priced calls charge the calling service's account. `crab idl owner/service` writes `idls/service.json`, retaining its owner for generated calls.

Before deployment or image publication, measure the built WebAssembly tool on the local host with the largest supported workloads, add headroom, and check every key against the destination's ceilings. Default ceilings are 100,000 µs CPU (100 ms), 1,000,000 rows read, 1,000,000 written and 128 MiB memory. Rebuild and retest under the chosen budget. Native tests and image builds do not establish that it fits; CLI call meters include nested calls. See [Sizing a budget](https://crabtalk.ai/docs/guides/budgets#size-a-budget-before-deployment).

`status({})` reads the current credential and access on Cloud without `keys:read`; on the local MCP endpoint it reports the unverified caller, or no caller.

`inspect(service)` on the local MCP endpoint returns the service definition, including its README and input schemas, and a `prices` map in USD micros (empty locally). Use `services` or `search`, then `inspect`, then `query` for tools marked query or `call` for other tools. Resource reads expose the README. Stopped services and services with no accessible tools or workflows are unavailable through inspection and resources.

Workflow tools use the same admission contract as HTTP and the CLI. `workflow_inspect({})` lists accessible workflows; supply `service` and `workflow` for the graph, schemas, task price and quote. `workflow_run` takes `{service, workflow, request: {id, quote, input}}`, where `id` is a client-generated UUID. Reuse the identical request after a transport failure. Poll `workflow_status` with `{service, workflow, id}`; `workflow_continue` and `workflow_cancel` take the same arguments. Responses retain the run identity and accepted terms. Continuation is explicit and cancellation cannot undo accepted effects. Local workflow operations default to the trusted operator `local`; the caller header overrides it.

Local MCP clients can send `x-berm-caller: {"id":"alice"}` on requests to `/mcp` to discover and call public tools as that caller. This identity is unverified; without the header, only anonymous tools are discoverable and callable.

A call that failed, trapped included, keeps its tool, arguments, caller and the digest of the image it ran, and its tape: every answer it had from outside the service, in order, up to 4 MiB a call and 64 MiB a service, counted in what the service stores. `crab --context cloud replay NAME ID` pulls that image and runs the call again in the process, each answer taken off the tape, so it fails as it failed in production without reaching a database, the network or another service; its logs print to stderr. The tape hashes outbound requests before secret substitution and includes responses as received. An upstream response can put a credential on a tape. A request that differs from the recorded one stops it, naming the syscall. `--on CONTEXT` runs the call on that context's host instead, against its state: its database, real fetches, its secrets.

## Benchmarking

```sh
crab bench services/web/seo check --cases cases.jsonl --runs 10
```

`bench DIRECTORY TOOL` builds and runs JSON Lines workloads on a temporary local host. Cases supply argument objects and HTTP responses, including headers, body files and optional response delays. Each case starts with a fresh migrated database; repetitions share its state. `--live-http` sends real requests instead of fixtures. The command uploads nothing.

The report separates the tool's own CPU, rows and peak memory from whole-chain cost, prices with the selected Cloud's rates, and suggests a budget with 1.5× headroom. Checks use runtime default ceilings. Incomplete runs and unexpected failures prevent a suggestion; an over-ceiling suggestion is reported without clipping. Exit `2` means the workload or budget did not pass. `--json` gives structured measurements, and `--offline` uses labeled SDK default rates. [Case format and limits](https://crabtalk.ai/docs/guides/bench).

## Publication and images



Deploy history retains the current image and two previous successful images; older records remain without their uploaded image. Existing deployments retain their own images, independently of the publisher’s releases.

`[image] public` sets a deployment’s visibility; `[image] deploy` lets others download the image and deploy their own services from it. Both default to `false` and may be enabled together. `crab push` builds and publishes images through Cloud without running an instance. It uses the same directory and `-p` selection as `crab up`. A deploy of another account’s image may set visibility and public-tool prices; these overrides are kept on redeploy and upgrade unless replaced.


`[image] name = "connect/tinyfish"` is the publication and pull address; the old `name` key is unsupported. `crab push` requires an image, builds local source and targets its account. The server requires account ownership; an API key must be bound to that account and grant `deployments:write`. The selected context account never replaces the publication address. Mixed-account publication groups are handled separately.

`crab up` resolves the image at the Cargo package version, including workspace-inherited versions, without invoking Cargo or installing Rust for a directly selected image. `--version` selects another release. A standalone manifest declares an exact suffix, such as `[image] name = "connect/tinyfish:1.2.3"`. A suffix on source publication must match Cargo's version. Workspace group discovery still uses Cargo.




## License

AGPL-3.0-only

A published image version identifies immutable bytes. Three versions remain downloadable; existing deployments retain their images. `[image] deploy = true` allows other accounts and anonymous callers to download the image. A deployment of another account’s image may set `--public` or `--private` and `--price tool=micros` (0 is free); omitted settings preserve that deployment’s overrides. Own-image deployments take settings from code. `--auto-upgrade` follows stable patches in the same major and minor version, subject to preflight review.
