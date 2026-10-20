# crab-kits

Every kit crab provides, re-exported from one crate under the kit's own name.

| Kit | Crate | Does |
| --- | --- | --- |
| `auth` | [`crab-auth`](https://crates.io/crates/crab-auth) | Sign-in for a product's users: GitHub and Google logins as sessions kept in its own database |
| `lake` | [`crab-lake`](https://crates.io/crates/crab-lake) | Feeds a source service follows: items and threads fetched on its schedule into its database, and RSS and Atom reading |
| `neon` | [`crab-neon`](https://crates.io/crates/crab-neon) | Neon Postgres over HTTP: parameterized statements and transactions |
| `stripe` | [`crab-stripe`](https://crates.io/crates/crab-stripe) | Stripe subscriptions for a product's users: Checkout, the customer portal, and each user's plan |
| `supabase` | [`crab-supabase`](https://crates.io/crates/crab-supabase) | Supabase over HTTPS: PostgREST tables and functions, and Storage objects |

A program links a kit with `#[berm_lang::program(uses = [crab_kits::stripe])]` and calls it from its own tools. Each kit's README covers what the program declares itself.

## License

Apache-2.0
