---
title: The Manifest
description: "What a program says it is: its ABI version, its tools, its README, and what it reaches for itself."
---

The manifest is JSON, carried in a `.berm.abi` section of the image rather than
behind an export. That placement is the point — reading what an image claims
must not mean running it. `Manifest::from_image` parses the section with nothing
compiled and nothing executed, which is what an embedder assembling a prompt, or
a service listing a registry, actually needs.

The section is a wasm custom section, which is what `#[link_section]` emits on
`wasm32`. It holds JSON records back to back, in the order the linker joined
them: the program's, and one `{"event": …}` per `#[event]`.
`Manifest::from_image` merges them into one manifest.

```rust,ignore
let manifest = Manifest::from_image(&bytes)?;
for tool in &manifest.tools {
    println!("{}: {}", tool.name, tool.description);
}
```

A `ToolSpec` carries the tool's name, the description a model reads, a JSON
Schema for its arguments, and optionally `returns`, a JSON Schema for its
result. Its flags are the tool's attributes: `public`, `anonymous`, `query`,
`cron`, `stripe` and `deprecated`, and `callers`, `services` or `accounts` when
a public tool takes only one kind. `events` lists the typed logs the program
emits, each with a name, a description and a JSON Schema of its `fields`, and
`database` the migrations of the database it keeps. The schemas are rendered at
compile time from the `Schema` impls of the types the tools and events name. A
tool's arguments are an object without a title; every other object schema
carries the struct's name as `title`, and a string enum its enum's name.
`version` is the crate's version when it was built.

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

`description`, `homepage` and `repository` are the crate's, from its
`Cargo.toml`, as `version` is. `readme` is the Markdown file
`#[program(readme = "README.md")]` names, read at compile time. Each is empty
when the crate sets none.

## Dependencies

```rust,ignore
#[berm_lang::program(
    programs = ["weather"],
    syscalls = { "berm.get", "berm.http.fetch" = ["api.example.com"] },
    secrets = ["example"],
    database = "migrations",
)]
```

is what an image says it will reach for once it runs: programs it calls by
name, syscalls it makes beyond the ABI's own, each beside what it reaches
through that call, and secrets it names as `{secret:NAME}` in a header or a body. The
manifest carries them as
`"deps": {"programs": [...], "syscalls": {"name": [...]}, "secrets": [...]}`.
`database` names a directory of migrations, read at compile time, and adds
`berm.sql` to the syscalls.

`berm.http.fetch` holds a program to them: a host it does not declare, or a
secret it does not declare, is refused before anything is sent.
`berm.http.proxy` does the same for the hosts declared under it, and sends
through the host's proxy. `berm.call`
reaches the program itself and the programs it declares, and refuses any other
name. A declared secret is substituted by the host with the value it granted the
program, and the guest never holds it. These are runtime
and resolved wherever the image lands: nothing here is fetched, installed, or
compiled in, which is what separates them from the crate's own
`[dependencies]`.

Declared by the author rather than found by reading the image, because a target
the program computes at the call is invisible to any scan — a list built that
way would be quietly short, and the runtime `Refused` would still be needed
underneath it.

A declared program not deployed refuses nothing: the deploy goes on, and the
program's runtime lists it as unresolved. A syscall the host does not serve
refuses the deploy. What a served syscall reaches through it is that syscall's
to grant.
Refusing would make deploy order significant, and a restart brings programs
back in whatever order the filesystem lists them — two that name each other
could then never come up at all.

## Kits

A kit is a library crate a program links, declaring at its root what it reaches for and the tables it keeps:

```rust,ignore
berm_lang::kit!(
    syscalls = { "berm.http.fetch" = ["api.example.com"] },
    secrets = ["example"],
    database = "migrations",
);
```

`kit!` takes `programs`, `syscalls`, `secrets` and `database`, and names the kit for its crate. A program links it with `#[berm_lang::program(uses = [example_kit])]`. Its manifest carries each kit as `"kits": [{"name", "version", "deps", "database"}]`, in `uses` order, and `Manifest::from_image` adds each kit's `deps` to the program's, so the runtime holds the program to both. A program that keeps no database of its own keeps one for a kit that does.

A kit's migrations run in the program's database before the program's own, each kit's in `uses` order. The host keeps the migrations it applied under the kit's name and the program's own under none, so a kit's migration and the program's may share a name. A kit no longer used leaves its tables and its applied migrations in place.

## Checks at load

`Berm::deploy` refuses an image before it can ever be offered to a model when:

- its manifest was built against a different `abi_version`;
- it exports no tools;
- its manifest declares a tool the image does not export, matched by the
  `berm_tool_` prefix;
- it declares more than one `#[stripe]` tool;
- it uses one kit twice;
- a `#[cron]` or `#[stripe]` tool is `#[public]`;
- an `#[anonymous]` tool is not `#[public]`, or is `#[callers(services)]`.

A broken image fails for whoever deployed it, not for whoever happens to use it
next.
