# crabkit

Every kit crab provides, re-exported from one crate under the kit's own name, each behind the feature of that name. No feature is on by default.

```toml
[dependencies]
crabkit = { version = "0.1", features = ["stripe"] }
```

| Kit | Crate | Does |
| --- | --- | --- |
| `auth` | [`crabkit-auth`](../../services/auth/README.md) | Sign-in for a product's users: GitHub and Google logins as sessions kept in its own database |
| `crawl` | [`crabkit-crawl`](../../services/crawl/README.md) | An incremental crawler: robots.txt, sitemaps, and the pages of the kinds a service names, recrawled when they change |
| `lake` | [`crabkit-lake`](../../services/lake/README.md) | Feeds a source service follows: items and threads fetched on its schedule into its database, and RSS and Atom reading |
| `neon` | [`crabkit-neon`](../../services/neon/README.md) | Neon Postgres over HTTP: parameterized statements and transactions |
| `stripe` | [`crabkit-stripe`](../../services/stripe/README.md) | Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan |
| `supabase` | [`crabkit-supabase`](../../services/supabase/README.md) | Supabase over HTTPS: PostgREST tables and functions, and Storage objects |

A service with the feature on links the kit and calls it from its own tools as `crabkit::stripe`. Linking a kit is using it: the build finds it among the crates compiled into the service's image, and what the kit's `crab.toml` declares reaches the service's manifest. The kit is the same crate as `crabkit-stripe` on its own, its manifest record and migrations named for it. Each kit's README covers what the service declares itself.

## License

Apache-2.0
