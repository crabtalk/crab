# crab

[![CI](https://github.com/crabtalk/crab/actions/workflows/ci.yml/badge.svg)](https://github.com/crabtalk/crab/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/crab-agent.svg)](https://crates.io/crates/crab-agent)
[![Docs](https://img.shields.io/badge/docs-crab.talk-blue)](https://crab.talk/docs)

The agent for the CrabTalk platform: write a service, run it locally, deploy it to CrabTalk Cloud, and call it.

```sh
cargo install crab-agent
crab login                     # CrabTalk Cloud; --email for a code by email
crab new example && cd example
crab dev -d                    # start the local host
crab --context default deploy  # build and run it there
crab deploy                    # or deploy it to Cloud
crab call example echo '{"query":"hello"}'
```

A service is a Rust library crate with a `crab.toml` beside its `Cargo.toml`. A function marked `#[tool]` is a tool, and its doc comment is what an agent reads when deciding to call it:

```rust
use berm_lang::{Result, tool};

/// Echo the query back.
#[tool]
pub fn echo(query: String) -> Result<String> {
    Ok(query)
}
```

Who may call it, and what it costs, is its table in `crab.toml`:

```toml
[tools.echo]
access = "public"
```

The docs are at [crab.talk/docs](https://crab.talk/docs/development/writing). The CLI, contexts and keys are in [crates/crab](crates/crab/README.md).

## Services

Open-source services deploy on their own or link into another service with `default-features = false`. The consuming deployment inherits credentials, HTTP permissions and migrations; it chooses its own exported tools. See [services](services/README.md).

- [Resend](services/resend/README.md): send emails and read delivery status.
- [GitHub](services/github/README.md): repositories, issues, pull requests and releases.
- [Upstash Redis](services/upstash/README.md): strings, expiry, counters, pipelines and transactions.
- [PostHog](services/posthog/README.md): events, feature flags and analytics queries.
- [OSV](services/osv/README.md): package vulnerabilities and advisory lookup.
- [Release watch](services/release-watch/README.md): retained release and vulnerability observations.

## Libraries

[crabkit](crates/crabkit/README.md) collects the kits crab provides, a feature each: `crabkit = { version = "0.1", features = ["auth"] }`. Linking a kit is using it.

- [auth](kits/auth/README.md): GitHub and Google sign-in for a product's users.
- [crawl](kits/crawl/README.md): an incremental crawler.
- [lake](kits/lake/README.md): feeds, RSS and Atom.
- [neon](kits/neon/README.md): Neon Postgres over HTTP.
- [stripe](kits/stripe/README.md): Stripe subscriptions.
- [supabase](kits/supabase/README.md): Supabase tables, functions and Storage.

## Building

Rust 2024 edition, POSIX only. `rustup target add wasm32-unknown-unknown`, then `cargo nextest run --workspace`. Services run on berm, the runtime in [berm/](berm/berm/README.md).

## License

The runtime and the CLI that runs it — `berm`, `berm-host`, `berm-oci`, `berm-build` and `crab-agent` — are licensed under the [GNU Affero General Public License, version 3](LICENSE-AGPL).

Everything else — `berm-lang`, `berm-codegen`, `berm-api`, `crabtalk-sdk`, `crabkit`, `crab-html` and the services and libraries — is licensed under the [Apache License, Version 2.0](LICENSE-APACHE). A service built with them is yours to license as you choose.
