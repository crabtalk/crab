---
title: The Manifest
description: "What a program says it is: its ABI version, its tools, its README, and what it reaches for itself."
---

The manifest is JSON, read from three wasm custom sections of the image rather than from an export. `Manifest::from_image` parses them with nothing compiled and nothing executed.

`.berm.abi` holds what the compiler wrote, as `#[link_section]` emits on `wasm32`: JSON records back to back, in the order the linker joined them, one `{"tool": …, "abi_version": …, "crate": …}` per `#[tool]` and one `{"event": …}` per `#[event]`. `.berm.program` holds the program's own record, from its package and its `berm.toml`, and `.berm.kits` the kits it links; `crab build` appends both. `Manifest::from_image` merges them into one manifest, and refuses an image without `.berm.program`, which a bare `cargo build` writes, and an image holding a tool compiled in a crate other than the program's.

```rust,ignore
let manifest = Manifest::from_image(&bytes)?;
for tool in &manifest.tools {
    println!("{}: {}", tool.name, tool.description);
}
```

A `ToolSpec` carries the tool's name, the description a model reads, a JSON
Schema for its arguments, and optionally `returns`, a JSON Schema for its
result. Its flags are the options of its `#[tool(...)]`: `public`, `anonymous`, `query`, `cron` and `stripe`, `deprecated` from `#[deprecated]`, and `callers`, `services` or `accounts` when a public tool takes only one kind. `events` lists the typed logs the program emits, each with a name, a description and a JSON Schema of its `fields`, and `database` the migrations of the database it keeps. The schemas are rendered at compile time from the `Schema` impls of the types the tools and events name. A tool's arguments are an object without a title; every other object schema carries the struct's name as `title`, and a string enum its enum's name. `version` is the crate's version when it was built. `buffer` is the most bytes a call's arguments, and a tool's result, may hold: the host refuses arguments past it and fails a result past it.

## Compatibility

`Manifest::breaks(next)` lists every way `next` breaks a caller of this
manifest's public tools, its Stripe tool and its events: a tool or event
removed; a tool no longer public, no longer a query, no longer taking anonymous
calls, taking fewer kinds of caller, or no longer taking Stripe events; an
argument newly required; a result no longer declared; a result or event field no
longer always present; a changed type; an unsigned integer becoming signed; an
argument taking fewer strings or a result written with more. Arguments are
compared as the new program reads them, results and events as old callers read
them; added fields break nothing. The host refuses a redeploy that breaks a tool
or event a caller used in the last 30 days, unless the deploy is forced, and
reports the rest.

## Package

`name`, `description`, `homepage` and `repository` are the package's, from its `Cargo.toml`, as `version` is. `readme` is the Markdown file `readme` in `berm.toml` names, read when the image is built. Each is empty when the crate sets none.

## berm.toml

`berm.toml`, beside the program's `Cargo.toml`, declares the program:

```toml
name = "weather-desk"              # the service it deploys as; required
image = "ghcr.io/you/weather-desk" # the registry repository Cloud pulls from
public = ["forecast"]              # the tools the deploy may make public
readme = "README.md"
secrets = ["example"]
programs = ["weather"]
buffer = 16384                     # 16 KiB when absent

[syscalls]
"berm.get" = []
"berm.http.fetch" = ["api.example.com"]

[database]
size = "1GiB"
seed = "seed.db"
migrations = "migrations"

[crons]
tick = "*/5 * * * *"
```

`deploy = false` keeps the program out of a workspace's deploys: `crab dev` on the workspace leaves it out and says so, and `crab deploy` refuses it. Run in the program's own directory, `crab dev` still runs it. `[crons]` holds each cron tool's expression, five fields in UTC; a tool left out is not called until its owners set one on the host. `[database] size` is what the service's database may hold, and `seed` a SQLite database a service on Cloud with no database yet starts from. Paths are relative to `berm.toml`. An unknown key is refused.

## Dependencies

`programs`, `secrets`, `[syscalls]` and `[database] migrations` in `berm.toml` are what an image says it will reach for once it runs: programs it calls by name, syscalls it makes beyond the ABI's own, each beside what it reaches through that call, and secrets it names as `{secret:NAME}` in a header or a body. The manifest carries them as `"deps": {"programs": [...], "syscalls": {"name": [...]}, "secrets": [...]}`. `[database] migrations` names a directory of `*.sql` migrations, read when the image is built and applied in name order, and adds `berm.sql` to the syscalls.

`berm.http.fetch` holds a program to them: a host it does not declare, or a secret it does not declare, is refused before anything is sent. `berm.http.proxy` does the same for the hosts declared under it, and sends through the host's proxy. `berm.call` reaches the program itself and the programs it declares, and refuses any other name. A declared secret is substituted by the host with the value it granted the program, and the guest never holds it. These are runtime and resolved wherever the image lands: nothing here is fetched, installed, or compiled in, which is what separates them from the crate's own `[dependencies]`.

A declared host is a name, matched whole; `*.example.com`, which matches every subdomain of `example.com` and not `example.com` itself; or `*`, which matches every host. Matching ignores ASCII case. The embedder is handed each request with how its host was declared, by name or only through a pattern; the local host refuses a host reached only through a pattern when it is, or resolves only to, an address that is not public (loopback, private, link-local, shared, documentation, multicast).

### Credentials

Secrets a person gets together from one provider are declared as a credential instead of in `secrets`:

```toml
[credentials.github]
title = "GitHub OAuth app"
guide = "Create an OAuth app at https://github.com/settings/applications/new with the callback URL below."
copy = { callback = "Authorization callback URL" }
secrets = [
    { name = "GITHUB_CLIENT_ID", label = "Client ID", masked = false },
    { name = "GITHUB_CLIENT_SECRET", label = "Client secret" },
]
```

`guide` is Markdown, shown once for the credential. `copy` labels, by key, values the person enters at the provider; a secret request gives each value. Each secret has a `label`, its name when absent; `masked`, true when absent, keeps a set value from being shown again; and `pattern`, an optional regular expression a value that does not match is warned about, never refused. A secret is declared once, in `secrets` or in one credential. The manifest carries them as `"deps": {"credentials": {"github": {...}}}`, and their secrets among `"secrets"`.

A declared program not deployed refuses nothing: the deploy goes on, and the
program's runtime lists it as unresolved. A syscall the host does not serve
refuses the deploy. What a served syscall reaches through it is that syscall's
to grant.

## Kits

A kit is a library crate with a `berm.toml` beside its `Cargo.toml`, declaring what it reaches for and the tables it keeps:

```toml
id = "0123456789abcdef0123456789abcdef"
secrets = ["example"]
programs = ["weather"]

[syscalls]
"berm.http.fetch" = ["api.example.com"]

[database]
migrations = "migrations"
```

A kit's `berm.toml` declares only `id`, `formerly`, `secrets`, `[credentials]`, `programs`, `[syscalls]` and `[database] migrations`; any other key is refused. `id` is 32 lowercase hex digits, written once; a kit with `[database] migrations` declares one, and a program declares none. `formerly` lists the keys its applied migrations were kept under before `id`, and needs an `id`. A kit carries no tools: an image holding a tool compiled in a kit is refused.

The build finds a program's kits among the crates compiled into its image: each of its normal dependencies, direct or not, with a `berm.toml`. Linking a kit is using it. A program linking one kit at two versions is refused. Its manifest carries each kit as `"kits": [{"name", "id", "formerly", "version", "deps", "database"}]`, by name, and `Manifest::from_image` adds each kit's `deps` to the program's, so the runtime holds the program to both. A credential id declared twice, or one secret in two credentials, is refused. A program that keeps no database of its own keeps one for a kit that does.

A kit's migrations run in the program's database before the program's own, each kit's in name order. The host keeps the migrations it applied under the kit's `id` and the program's own under none: a kit's migration and the program's may share a name, and a kit whose package is renamed keeps its history. Before comparing, the host moves a kit's applied migrations kept under one of its `formerly` keys under its `id`, and refuses a database keeping them under two of its keys. A kit no longer linked leaves its tables and its applied migrations in place.

## Checks at load

`Berm::deploy` refuses an image before it can ever be offered to a model when:

- it has no `.berm.program` section;
- its manifest was built against a different `abi_version`;
- it exports no tools;
- a tool record was compiled in a crate other than the program's;
- its manifest declares a tool the image does not export, matched by the `berm_tool_` prefix;
- it declares more than one `stripe` tool;
- it names one kit twice;
- a `cron` or `stripe` tool is `public`;
- an `anonymous` tool is not `public`, or is `callers(services)`.

A broken image fails for whoever deployed it, not for whoever happens to use it
next.
