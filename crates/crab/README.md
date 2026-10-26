# crab

The [local host](https://crab.talk/docs/development/local), and the command-line client for it and CrabTalk Cloud.

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

## crab.toml

`crab build` builds the package in the directory it is given, `.` by default, and prints the path of its finished image, `berm/<crate>.wasm` under cargo's target directory. `crab deploy` builds it the same way and deploys it as the `crab.toml` beside its `Cargo.toml` says. `crab new` writes one, beside a library crate built as a `cdylib`.

```toml
# The service this program deploys as.
name = "example"
# The public registry repository the image is published to, tagged with the
# crate's version on each deploy. Without it, a deploy uploads the image to
# the host.
image = "ghcr.io/org/example"

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

Without `[crons]`, each cron tool's expression is its owners' to set on the host; with it, a deploy sets each, and is refused while it would undo one its owners set since: `crab deploy --pull` writes theirs into `[crons]` first, and `--force` overrides them. A write that would take the database past `size` fails, and a deploy is refused while the database holds more than its `size`. On a service's first deploy to Cloud, `crab deploy` uploads a copy of `seed` and the service's database starts as it, held to the image's migrations: those it records applied are the image's first, unchanged. A service with a database takes no seed, and the local host takes none.

## Developing

`crab dev` runs the local host on `127.0.0.1:7777`, its root `~/.crab/host`, in the foreground until Ctrl-C; `-d`/`--detach` starts it in the background, writing to `host.log` under its root. It is one per machine and knows nothing of directories: programs reach it through `crab deploy` on the `default` context, which starts it in the background when nothing answers. `crab deploy` picks services as cargo picks packages: in a service's directory, that service; at a workspace's root, every one whose `crab.toml` does not say `deploy = false`; `-p <name>`, repeatable, names them. `crab push` deploys the same way and installs each service stopped; `crab start` and `crab stop` serve and stop services, named or the same set, keeping what they committed, and `crab ls` shows which run. `crab rm` with no name removes the same set. `crab dev status` prints its pid, root and start, and `crab dev stop` stops it.

## Contexts

`crab` talks to the host its context names, the way `docker context` picks a daemon. `default` is the local host, `crab dev`, on `http://127.0.0.1:7777`; `--context` picks another for one command.

```sh
crab context add cloud https://api.crab.talk --token "$KEY" --org acme
crab context use cloud
crab context ls
crab context use default
```

A program's secrets are its own, on the local host as on Cloud: `printf %s "$VALUE" | crab secret put NAME SECRET` sets one, `crab secret ls NAME` lists them by name, `crab secret rm NAME SECRET` revokes one. `crab secret fill NAME`, run by a person at a terminal, asks for each credential the program declares, with its guide, and sets what they type on the host the context names; it refuses to run where stdin is not a terminal, so an agent cannot feed it values. On Cloud, `crab secret request NAME` asks a person to set the ones not set: it prints a link, `https://crab.talk/secrets/WDJB-MJHT`, where someone signed in who manages the program enters the values, one credential at a time, beside the guide its `crab.toml` declares, so none passes through the terminal. `--credential ID` asks for that credential alone, set or not; `--copy KEY=VALUE` gives a value the credentials have the person enter at the provider, such as a callback URL. `deploy` names each secret the manifest declares that is not set.

A context with a token and an `--org` or `--user` is Cloud, and its services are that account's. `deploy` sends the image itself, to the local host and to Cloud alike, and Cloud keeps it for that service alone. With `image`, or `--image`, it pushes the image there instead and Cloud deploys that reference: Cloud pulls it anonymously, so the package must be public. A key needs `deploy` scope, and reaches services only; `--token-stdin` reads it from stdin.

Contexts are kept in `~/.crab/contexts.toml`.

## Cloud account

`crab login` signs in to Cloud in the browser, keeps the session as the `cloud` context and uses it. A session reaches everything below; a deploy key is refused by all of it but `usage`.

`crab login --device`, or `crab login` where no browser opens, prints a link and a code and waits: a person signed in to Cloud opens the link, or enters the code at `/device`, and approves, and the session is theirs.

`crab billing plan`, `trial` and `credit` ask a person to pay: each prints a link, `https://crab.talk/pay/WDJB-MJHT`, whose page shows the account, the price and `--for`, and takes the payment or a decline. The code also works typed at `https://crab.talk/pay`, and lasts 30 minutes. `crab request <code> --wait` follows a payment or a secret request: it returns once the request is done, declined or expired, and fails unless it was done. `crab request <code> --expire` ends one now, as if its time had run out; secrets already saved through it stay set.

```sh
crab login                     # --github for GitHub
crab login --device
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

The deploy is two lines:

```yaml
steps:
  - run: crab context add cloud https://api.crab.talk --token "$KEY" --org acme
    env:
      KEY: ${{ secrets.CRABTALK_KEY }}
  - run: crab deploy --context cloud
```

A service with an `image` pushes it with `GITHUB_TOKEN` as the registry credential: give the job `packages: write` and the step `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`. Locally, `GITHUB_TOKEN=$(gh auth token)` does the same.

`new` is the one subcommand that does not talk to the context's host. It writes
a program crate pinning the `berm-lang` that shipped with this binary.

`inspect` prints a program's README and renders each tool's JSON Schema as the argument list it describes, since a schema printed raw is something to read past rather than read. On Cloud, `inspect owner/name` shows another account's public service: its README and public tools, without what only its owners see.

`inspect` shows the most one call of each tool may use, and `budget NAME TOOL --cpu MICROS --rows N` sets it. `call` calls one public tool and prints what the call used on stderr; `usage` prints a program's meters over the last 30 days, a day each. Agents reach tools over MCP. On the local host, `--caller alice --issuer https://id.example --user 42` makes the call as that caller and user, unchecked, so a program reading `berm_lang::caller()` and `user()` runs as it would for them on Cloud.

A call that failed, trapped included, keeps its tool, arguments, caller and the digest of the image it ran, and its tape: every answer it had from outside the program, in order, up to 4 MiB a call and 64 MiB a service, counted in what the service stores. `crab --context cloud replay NAME ID` pulls that image and runs the call again in the process, each answer taken off the tape, so it fails as it failed in production without reaching a database, the network or another service; its logs print to stderr. A request that differs from the recorded one stops it, naming the syscall. `--on CONTEXT` runs the call on that context's host instead, against its state: its database, real fetches, its secrets.

## License

Apache-2.0
