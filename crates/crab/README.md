# crab

The [local host](https://berm.run/docs/berm/service/), and the command-line client for it and CrabTalk Cloud.

```sh
crab serve &
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

## berm.toml

`crab build` builds the package in the directory it is given, `.` by default, and prints the path of its finished image, `berm/<crate>.wasm` under cargo's target directory. `crab deploy` builds it the same way and deploys it as the `berm.toml` beside its `Cargo.toml` says. `crab new` writes one, beside a library crate built as a `cdylib`.

```toml
# The service this program deploys as.
name = "example"
# The registry repository Cloud pulls the image from, tagged with the crate's
# version on each deploy.
image = "ghcr.io/org/example"
# Tools this program may make public.
public = ["search"]

# When the host calls each `#[tool(cron)]` tool: five fields, minute to day of
# week, in UTC.
[crons]
tick = "*/5 * * * *"

# What the service's database may hold: 1GiB when unset, from 64KiB to 10GiB.
[database]
size = "2GiB"
# A SQLite database, relative to this file, a service on Cloud starts from on its first deploy.
seed = "seed.sqlite"
```

A deploy that makes a tool public is refused until `public` lists it. Without `[crons]`, each cron tool's expression is its owners' to set on the host; with it, a deploy sets each, and is refused while it would undo one its owners set since: `crab deploy --pull` writes theirs into `[crons]` first, and `--force` overrides them. A write that would take the database past `size` fails, and a deploy is refused while the database holds more than its `size`. On a service's first deploy to Cloud, `crab deploy` uploads a copy of `seed` and the service's database starts as it, held to the image's migrations: those it records applied are the image's first, unchanged. A service with a database takes no seed, and the local host takes none.

## Developing

`crab dev` runs the services of the workspace it is started in on the local host at `127.0.0.1:7777`, its root `~/.crab/dev`, and rebuilds and redeploys each as its files change, printing each deploy, each call and the call's logs, until Ctrl-C. In a service's directory it runs that service alone. A deploy goes over what it breaks, and skips the seed. `--detach` runs it in the background, writing to `~/.crab/dev/dev.log`, and returns once each service is deployed or failed; `crab dev status` prints what it serves and each service's last rebuild, and `crab dev stop` stops it.

## Contexts

`crab` talks to the host its context names, the way `docker context` picks a daemon. `default` is the local host, `crab serve`, on `http://127.0.0.1:7777`; `--context` picks another for one command.

```sh
crab context add cloud https://api.crabtalk.ai --token "$KEY" --org acme
crab context use cloud
crab context ls
crab context use default
```

On Cloud, a program's secrets are its own: `printf %s "$VALUE" | crab secret put NAME SECRET` sets one, `crab secret ls NAME` lists them by name, `crab secret rm NAME SECRET` revokes one, and `crab secret request NAME` asks a person to set the ones not set: it prints a link, `https://crab.talk/secrets/WDJB-MJHT`, where someone signed in who manages the program enters the values, one credential at a time, beside the guide its `berm.toml` declares, so none passes through the terminal. `--credential ID` asks for that credential alone, set or not; `--copy KEY=VALUE` gives a value the credentials have the person enter at the provider, such as a callback URL. `deploy` names each secret the manifest declares that is not set. The local host reads its secrets from `secrets.toml` under its root.

A context with a token and an `--org` or `--user` is Cloud, and its services are that account's. Against Cloud, `deploy` pushes the image to `image` and deploys that reference; against the local host it sends the image itself. Cloud pulls it anonymously, so the package must be public. A key needs `deploy` scope, and reaches services only; `--token-stdin` reads it from stdin.

Contexts are kept in `~/.crab/contexts.toml`.

## Cloud account

`crab login` signs in to Cloud in the browser, keeps the session as the `cloud` context and uses it. A session reaches everything below; a deploy key is refused by all of it but `usage`.

`crab login --email` signs in without a browser, with a code emailed to the address, and creates the account when the address has none. It sends the code and exits; `--code` finishes in a second run.

`crab billing plan`, `trial` and `credit` ask a person to pay: each prints a link, `https://crab.talk/pay/WDJB-MJHT`, whose page shows the account, the price and `--for`, and takes the payment or a decline. The code also works typed at `https://crab.talk/pay`, and lasts 30 minutes. `crab request <code> --wait` follows a payment or a secret request: it returns once the request is done, declined or expired, and fails unless it was done.

```sh
crab login                     # --github for GitHub
crab login --email you@example.com
crab login --email you@example.com --code 12345678
crab status
crab usage                     # the account's services and its plan this period
crab billing                   # plan, credit, refill, card
crab billing plan pro --for "Pro, to run the crawler past the free plan"
crab request WDJB-MJHT --wait
crab billing portal            # opens Stripe's portal in the browser
crab key create ci --days 90
crab app ls
crab org create acme
crab --org acme billing
```

A service's calls, cost, crons, wakes, visibility and sunset have their own commands. `--org` acts for an org in place of the context's account. Output is JSON with `--json`, and whenever stdout is not a terminal. A command that removes something asks first, and without a terminal refuses unless given `--yes`.

## From a workflow

`GITHUB_TOKEN` is the registry credential `deploy` pushes with, and the deploy is two lines:

```yaml
permissions:
  packages: write

steps:
  - run: crab context add cloud https://api.crabtalk.ai --token "$KEY" --org acme
    env:
      KEY: ${{ secrets.CRABTALK_KEY }}
  - run: crab deploy --context cloud
    env:
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

Locally, `GITHUB_TOKEN=$(gh auth token)` does the same.

`new` is the one subcommand that does not talk to the context's host. It writes
a program crate pinning the `berm-lang` that shipped with this binary.

`inspect` renders a tool's JSON Schema as the argument list it describes, since
a schema printed raw is something to read past rather than read.

`inspect` shows the most one call of each tool may use, and `budget NAME TOOL --cpu MICROS --rows N` sets it. `call` calls one public tool and prints what the call used on stderr; `usage` prints a program's meters over the last 30 days, a day each. Agents reach tools over MCP. On the local host, `--caller alice --issuer https://id.example --user 42` makes the call as that caller and user, unchecked, so a program reading `berm_lang::caller()` and `user()` runs as it would for them on Cloud.

A call that failed keeps its tool, arguments, caller and the digest of the image it ran, for as long as calls are kept. `crab --context cloud replay NAME ID` reads them and makes the call again on the local host, or on the context `--on` names, against that host's state: its database, real fetches, its secrets. It says when the program there runs another image.

## License

Apache-2.0
