# crab

The command-line client for [bermd](https://berm.run/docs/berm/service/) and CrabTalk Cloud.

```sh
crab new example
cd example
crab deploy
crab ls
crab inspect example
crab idl example
crab logs example
crab usage example
crab rm example
```

## berm.toml

`crab deploy` builds the package in the directory it is given, `.` by default, and deploys it as the `berm.toml` beside its `Cargo.toml` says. `crab new` writes one.

```toml
# The service this program deploys as.
name = "example"
# The registry repository Cloud pulls the image from, tagged with the crate's
# version on each deploy.
image = "ghcr.io/org/example"
# Tools this program may make public.
public = ["search"]

# When the host calls each `#[cron]` tool: five fields, minute to day of
# week, in UTC.
[crons]
tick = "*/5 * * * *"

# What the service's database may hold: 1GiB when unset, at most 10GiB.
[database]
size = "2GiB"
# A SQLite database, relative to this file, a service on Cloud starts from on its first deploy.
seed = "seed.sqlite"
```

A deploy that makes a tool public is refused until `public` lists it. Without `[crons]`, each cron tool's expression is its owners' to set on the host; with it, a deploy sets each, and is refused while it would undo one its owners set since: `crab deploy --pull` writes theirs into `[crons]` first, and `--force` overrides them. A write that would take the database past `size` fails, and a deploy is refused while the database holds more than its `size`. On a service's first deploy to Cloud, `crab deploy` uploads a copy of `seed` and the service's database starts as it, held to the image's migrations: those it records applied are the image's first, unchanged. A service with a database takes no seed, and a bermd takes none.

## Contexts

`crab` talks to the host its context names, the way `docker context` picks a daemon. `default` is the bermd on `http://127.0.0.1:7777`; `--context` picks another for one command.

```sh
crab context add cloud https://api.crabtalk.ai --token "$KEY" --org acme
crab context use cloud
crab context ls
crab context use default
```

On Cloud, a program's secrets are its own: `printf %s "$VALUE" | crab secret put NAME SECRET` sets one, `crab secret ls NAME` lists them by name, `crab secret rm NAME SECRET` revokes one, and `deploy` names each secret the manifest declares that is not set. A bermd reads its secrets from `secrets.toml` under its root.

A context with a token and an `--org` or `--user` is Cloud, and its services are that account's. Against Cloud, `deploy` pushes the image to `image` and deploys that reference; against bermd it sends the image itself. Cloud pulls it anonymously, so the package must be public. A key needs `deploy` scope, and reaches services only; `--token-stdin` reads it from stdin.

Contexts are kept in `~/.crab/contexts.toml`.

## Cloud account

`crab login` signs in to Cloud in the browser, keeps the session as the `cloud` context and uses it. A session reaches everything below; a deploy key is refused by all of it but `usage`.

```sh
crab login                     # --github for GitHub
crab status
crab usage                     # the account's services and its plan this period
crab billing                   # plan, credit, refill, card
crab billing plan pro          # plan, trial, credit and portal open Stripe; plan from a trial ends it at once
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

`inspect` shows the most one call of each tool may use, and `budget NAME TOOL --cpu MICROS --rows N` sets it. `call` calls one public tool and prints what the call used on stderr; `usage` prints a program's meters over the last 30 days, a day each. Agents reach tools over MCP. On bermd, `--caller alice --issuer https://id.example --user 42` makes the call as that caller and user, unchecked, so a program reading `berm_lang::caller()` and `user()` runs as it would for them on Cloud.

## License

Apache-2.0
