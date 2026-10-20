# Neon

Neon's SQL over HTTP for berm programs: parameterized statements and transactions against a Neon Postgres database, each one HTTPS request through `berm.http.fetch`. The password stays a secret the host substitutes; the program never holds it.

## Using it

```toml
[dependencies]
crab-neon = "0.1"
```

The kit declares the secret `NEON_PASSWORD`. The endpoint's host differs per Neon project, so the program declares it under `berm.http.fetch`: `api.` followed by the connection string's host without its first label. For `ep-cool-river-123456.us-east-2.aws.neon.tech`, or its `-pooler` host, that is `api.us-east-2.aws.neon.tech`.

```rust,ignore
#[berm_lang::program(
    uses = [crab_neon],
    syscalls = { "berm.http.fetch" = ["api.us-east-2.aws.neon.tech"] },
)]
mod tools { /* ... */ }
```

Grant `NEON_PASSWORD` for that host. The connection string carries it as `{secret:NEON_PASSWORD}`, substituted unencoded, so the password must be valid in a URL as is.

## Queries

```rust,ignore
use crab_neon::{Neon, Statement};

let neon: Neon = "postgresql://app:{secret:NEON_PASSWORD}@ep-cool-river-123456.us-east-2.aws.neon.tech/app"
    .parse()?;

// Rows, each a tuple of its columns in order.
let items = neon.rows::<(i64, String)>("SELECT id, title FROM items WHERE done = $1", &(false,))?;

// Rows affected.
let updated = neon.execute("UPDATE items SET done = true WHERE id = $1", &(42,))?;

// Several statements in one transaction, in one request.
let answers = neon.transaction(&[
    Statement::new("INSERT INTO items (title) VALUES ($1)", &("write docs",))?,
    Statement::new("SELECT COUNT(*) FROM items", &[(); 0])?,
])?;
let counted = answers[1].rows::<(i64,)>()?;
```

Parameters are a tuple bound to `$1`, `$2`, … in order, or `&[(); 0]` for none. A column decodes by its type: `bool` as a boolean, `int2`, `int4`, `int8` and `oid` as integers, `float4` and `float8` as numbers (a string when not finite), `json` and `jsonb` as their value, `NULL` as `None`, and every other type as its Postgres text.

A failure Neon answers fails the call with `Neon answered <status>: <message> (<SQLSTATE>)`.
