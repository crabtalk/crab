# crabkit-supabase

Rust library for code running in a CrabTalk service. It has no standalone tools or deployable template; link it into your service, then expose the operations your users need.

Supabase over HTTPS through `berm.http.fetch`: PostgREST tables and Postgres functions, and Storage objects. Each request carries the project key from the secret `SUPABASE_KEY` in `apikey` and `Authorization: Bearer`, substituted by the host, so the service never holds it.

## Using it

```toml
[dependencies]
crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.4", features = ["supabase"] }
```

The library is also the crate `crabkit-supabase` on its own.

The library declares the Supabase credential, its setup guide and `SUPABASE_KEY`. Your service supplies only its project’s host in `crab.toml`:

```toml
[credentials.supabase]
hosts = ["abcd.supabase.co"]
```

Provision `SUPABASE_KEY` after deploying this manifest, using `crab secret fill <service>` in your terminal:

- the `anon` or publishable key reaches only what the project's Row Level Security policies allow;
- the `service_role` or secret key bypasses Row Level Security, and reads and writes every table and bucket.

## Tables

```rust,ignore
use crabkit::supabase::Supabase;

let supabase = Supabase::new("https://abcd.supabase.co");

// GET /rest/v1/todos?select=*&done=eq.false&limit=10&order=id.desc
let todos: Vec<Todo> = supabase
    .from("todos")
    .select("*")
    .eq("done", "false")
    .order("id", false)
    .limit(10)
    .get()?;

// POST, answering the inserted rows
let added: Vec<Todo> = supabase.from("todos").insert(&json!({"title": "walk"}))?;

// PATCH the filtered rows, answering them as updated
let done: Vec<Todo> = supabase.from("todos").eq("id", "7").update(&json!({"done": true}))?;

// DELETE the filtered rows, answering them
let gone: Vec<Todo> = supabase.from("todos").filter("id", "in", "(1,2)").delete()?;
```

Rows deserialize into any `T: DeserializeOwned`. `eq`, `neq`, `gt`, `gte`, `lt`, `lte`, `like`, `ilike` and `is` are PostgREST's operators of the same name; `filter(column, operator, value)` takes any other, e.g. `filter("tags", "cs", "{rust}")`. Values are percent-encoded and otherwise sent as written, so a value holding PostgREST's reserved characters inside `in.(…)` needs PostgREST's double quotes.

## Functions

```rust,ignore
// POST /rest/v1/rpc/add_them {"a":2,"b":3}
let sum: i64 = supabase.rpc("add_them", &json!({"a": 2, "b": 3}))?;
```

## Storage

```rust,ignore
let avatars = supabase.storage("avatars");
avatars.upload("users/ada.png", "image/png", &png, false)?; // true replaces an existing object
let png: Vec<u8> = avatars.download("users/ada.png")?;
avatars.delete("users/ada.png")?;
```

## Errors

A status outside 2xx fails as `Supabase answered <status>: <message> (<code>)`, read from PostgREST's `message` and `code` or Storage's `message` and `error`.

## License

Apache-2.0
