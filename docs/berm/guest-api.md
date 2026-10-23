---
title: The Guest API
description: "What berm-lang gives a program past its arguments: who called, state, a database, logs, HTTP, other programs, wakes, the clock and randomness."
---

Each function here is a syscall. berm serves the call's own context — caller,
user, `vouch`, client, the clock, `trace` and `deny` — and `berm.call`. The rest are
served by the host the program is deployed to, and a call its host does not
serve traps.

A program lists the syscalls it makes past berm's own under `[syscalls]` in its `crab.toml`, and a deploy to a host that does not serve one is refused:

```toml
[syscalls]
"berm.get" = []
"berm.set" = []
"berm.http.fetch" = ["api.example.com"]
```

## Who called

```rust,ignore
match berm_lang::caller()? {
    Some(Caller::Outside(account)) => { /* someone the host authenticated */ }
    Some(Caller::Program(name)) => { /* another program, through berm_lang::call */ }
    None => { /* the host, for this program's own wake, cron or Stripe event */ }
}
```

- `caller()` is set by the host, never by the caller.
- `user()` answers the end user a call runs for, as `User { issuer, subject }`:
  the identity provider or program that vouched for them and who they are to
  it. It is carried into every nested call.
- `vouch(subject)` has the rest of this invocation, and every call it makes after, run for `subject`, with the program as the issuer, named as the programs it calls are told it is. The manifest declares `berm.vouch`, and it is refused in a chain that already runs for a user.
- `client()` answers an opaque key naming the outside client for one day, or
  `None` when the host supplied none. It is carried into every nested call.
- `deny(message)` makes an `Error` that fails the call as refusing its caller.

## State

```rust,ignore
let seen = berm_lang::get("last-run")?.unwrap_or_default();
berm_lang::set("last-run", &now.to_le_bytes())?;
let config: Config = berm_lang::get_json("config")?.unwrap_or_default();
```

- `get`, `set`, `get_json` and `set_json` read and write the program's own
  keys. Another program's keys cannot be named.
- `get` answers `None` for a key never written.
- Writes are committed when the call returns `Ok`.
- `set` is refused in a `query` tool.

## A database

```toml
# crab.toml
[database]
migrations = "migrations"
```

- `[database] migrations` names a directory of `*.sql` migrations, applied in name order at deploy. A deploy whose migrations do not begin with the ones already applied is refused.
- A kit the program links keeps its tables in the same database, its migrations applied first and checked the same way under the kit's name. See [Kits](./manifest.md#kits).
- `berm_lang::rows::<T>(statement, params)` runs one statement, `params` a tuple bound to its `?`s, and answers its rows, each a tuple of its columns.
- `berm_lang::sql(statement, params)` runs one statement, `params` a JSON array
  bound to its `?` placeholders, and answers
  `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
- Every statement of one invocation runs in one transaction, committed when it
  returns `Ok`. Transaction statements are refused, and so is a write under a
  `query`.
- Tables named `_berm_*` are the runtime's.

```rust,ignore
let found = berm_lang::sql("SELECT title FROM items WHERE id = ?", "[42]")?;
```

## Logs

- `emit(topics, data)` emits a log under `topics`, which a reader filters by.
  `emit!` emits an [`#[event]`](./writing-a-program.md#events) struct under its
  name.
- A log is kept beside the call it was emitted in, whatever that call ended as.
- `emit` is refused in a `query` tool.
- `trace(message)` writes a line to the host's own trace, not to the program's
  logs.

## HTTP

```rust,ignore
let page = berm_lang::http::fetch(
    "GET",
    "https://api.example.com/items",
    &[("Authorization", "Bearer {secret:example}")],
    b"",
)?;
```

- `http::fetch` sends one request and answers `Response { status, headers, body }`.
  A `4xx` or `5xx` from the far end is a `Response`.
- It reaches only the hosts the manifest declares under `berm.http.fetch`, by name or by `*.domain` or `*` ([manifest](manifest.md)).
- `http::proxy` sends the same request through the host's proxy, to the hosts
  listed under `berm.http.proxy`.
- `http::fetch_within` sends one request given up when nothing answered within its milliseconds, or at the call's deadline when that comes first; otherwise a request waits for the call's deadline.
- `http::fetch_all` sends a batch of `http::Request`s at once, each to a host declared under `berm.http.fetch` with its own timeout and the most bytes of its answer's body it takes, and answers each in its place, a refusal or failure answering its own.
- A header value or the body may carry `{secret:NAME}` for a secret the manifest lists in `secrets`. The host substitutes it, and the program never holds the value.
- `CallError::Refused` means nothing was sent: a host or secret the manifest
  does not list, or a secret the host has not granted.

## Other programs

```rust,ignore
let top = berm_lang::call("hn", "top", r#"{"limit": 10}"#)?;
```

- `call(program, tool, args)` calls a `public` tool of the program itself or
  of one the manifest lists in `programs`, and answers its result as JSON text.
- `CallError::Refused` means nothing ran: a name nothing answers to, or one the
  manifest does not list. `CallError::Failed` means the target ran and failed.
- The chain nests at most as deep as the host allows, 4 by default.
- [Calling another program](./writing-a-program.md#calling-another-program)
  covers the typed calls `declare_program!` generates.

## Wakes

```rust,ignore
berm_lang::after(300_000, "weather", "poll", "{}")?;
```

- `after(delay, program, tool, args)` has the tool run in `delay` milliseconds.
- A program holds one wake. Arming another drops the one pending.
- The woken call runs with no caller and carries nothing from the call that
  armed it; what it needs goes in state.
- A `query` tool cannot arm one.

## The clock and randomness

`now()` answers the host's time, in milliseconds since the Unix epoch.

`random(&mut buffer)` fills `buffer` from the host's secure random source. Neither needs declaring.
