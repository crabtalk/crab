# crabkit

Reusable library modules for CrabTalk services, each behind a feature of the same name. No feature is on by default.

```toml
[dependencies]
crabkit = { version = "0.1", features = ["stripe"] }
```

| Library | Crate | Does |
| --- | --- | --- |
| `auth` | [`crabkit-auth`](../../services/kits/auth/README.md) | Sign-in for a product's users: GitHub and Google logins as sessions kept in its own database |
| `crawl` | [`crabkit-crawl`](../../services/kits/crawl/README.md) | An incremental crawler: robots.txt, sitemaps, and the pages of the kinds a service names, recrawled when they change |
| `lake` | [`crabkit-lake`](../../services/kits/lake/README.md) | Feeds a source service follows: items and threads fetched on its schedule into its database, and RSS and Atom reading |
| `neon` | [`crabkit-neon`](../../services/kits/neon/README.md) | Neon Postgres over HTTP: parameterized statements and transactions |
| `stripe` | [`crabkit-stripe`](../../services/kits/stripe/README.md) | Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan |
| `supabase` | [`crabkit-supabase`](../../services/kits/supabase/README.md) | Supabase over HTTPS: PostgREST tables and functions, and Storage objects |

A service with the feature on links the library and calls it from its own tools as `crabkit::stripe`. Linking a library is using it: the build finds it among the crates compiled into the service's image, and what the library's `crab.toml` declares reaches the service's manifest. The library is the same crate as `crabkit-stripe` on its own, its manifest record and migrations named for it. Each library's README covers what the service declares itself.

## License

Apache-2.0
