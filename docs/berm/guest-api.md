---
title: The Guest API
description: "What berm-lang gives a program past its arguments: who called, state, a database, logs, HTTP, other programs, wakes and the clock."
---

Each function here is a syscall. berm serves the call's own context — caller,
user, client, the clock, `trace` and `deny` — and `berm.call`. The rest are
served by the host the program is deployed to, and a call its host does not
serve traps.

A program lists the syscalls it makes past berm's own in its manifest, and a
deploy to a host that does not serve one is refused:

```rust,ignore
#[berm_lang::program(syscalls = { "berm.get", "berm.set", "berm.http.fetch" = ["api.example.com"] })]
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
  the identity provider that vouched for them and who they are to it. It is
  carried into every nested call.
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
- `set` is refused in a `#[query]` tool.

## A database

```rust,ignore
#[berm_lang::program(database = "migrations")]
```

- `database` names a directory of `*.sql` migrations, applied in name order at
  deploy. A deploy whose migrations do not begin with the ones already applied
  is refused.
- `berm_lang::sql(statement, params)` runs one statement, `params` a JSON array
  bound to its `?` placeholders, and answers
  `{"columns": [..], "rows": [[..]], "read": n, "written": n}`.
- Every statement of one invocation runs in one transaction, committed when it
  returns `Ok`. Transaction statements are refused, and so is a write under a
  `#[query]`.
- Tables named `_berm_*` are the runtime's.

```rust,ignore
let found = berm_lang::sql("SELECT title FROM items WHERE id = ?", "[42]")?;
```

## Logs

- `emit(topics, data)` emits a log under `topics`, which a reader filters by.
  `emit!` emits an [`#[event]`](./writing-a-program.md#events) struct under its
  name.
- A log is kept beside the call it was emitted in, whatever that call ended as.
- `emit` is refused in a `#[query]` tool.
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
- It reaches only the hosts the manifest lists under `berm.http.fetch`.
- `http::proxy` sends the same request through the host's proxy, to the hosts
  listed under `berm.http.proxy`.
- A header value may carry `{secret:NAME}` for a secret the manifest lists in
  `secrets`. The host substitutes it, and the program never holds the value.
- `CallError::Refused` means nothing was sent: a host or secret the manifest
  does not list, or a secret the host has not granted.

## Other programs

```rust,ignore
let top = berm_lang::call("hn", "top", r#"{"limit": 10}"#)?;
```

- `call(program, tool, args)` calls a `#[public]` tool of the program itself or
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
- A `#[query]` tool cannot arm one.

## The clock

`now()` answers the host's time, in milliseconds since the Unix epoch.

## HTML

`html::text(html)` turns HTML into text: tags are dropped, a block boundary
becomes a line break, a link becomes `[text](href)`, and entities are decoded.
