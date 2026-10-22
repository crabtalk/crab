# crabkit

Every kit crab provides, re-exported from one crate under the kit's own name, each behind the feature of that name. No feature is on by default.

```toml
[dependencies]
crabkit = { version = "0.1", features = ["stripe"] }
```

| Kit | Crate | Does |
| --- | --- | --- |
| `auth` | [`crabkit-auth`](https://crates.io/crates/crabkit-auth) | Sign-in for a product's users: GitHub and Google logins as sessions kept in its own database |
| `crawl` | [`crabkit-crawl`](https://crates.io/crates/crabkit-crawl) | An incremental crawler: robots.txt, sitemaps, and the pages of the kinds a program names, recrawled when they change |
| `lake` | [`crabkit-lake`](https://crates.io/crates/crabkit-lake) | Feeds a source service follows: items and threads fetched on its schedule into its database, and RSS and Atom reading |
| `neon` | [`crabkit-neon`](https://crates.io/crates/crabkit-neon) | Neon Postgres over HTTP: parameterized statements and transactions |
| `stripe` | [`crabkit-stripe`](https://crates.io/crates/crabkit-stripe) | Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan |
| `supabase` | [`crabkit-supabase`](https://crates.io/crates/crabkit-supabase) | Supabase over HTTPS: PostgREST tables and functions, and Storage objects |

A program with the feature on links the kit and calls it from its own tools as `crabkit::stripe`. Linking a kit is using it: the build finds it among the crates compiled into the program's image, and what the kit's `berm.toml` declares reaches the program's manifest. The kit is the same crate as `crabkit-stripe` on its own, its manifest record and migrations named for it. Each kit's README covers what the program declares itself.

## License

Apache-2.0
