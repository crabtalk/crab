# GitHub Rust integration

```toml
[dependencies]
github = { package = "crab-github", git = "https://github.com/crabtalk/crab", tag = "v0.1.4", default-features = false }
```

```rust,ignore
use github::{api, types::Page};

let release = api::latest_release("crabtalk", "crab")?;
let page = api::releases("crabtalk", "crab", &Page::default())?;
```

Disabling the default `tools` feature links the Rust API without exporting this service's tools. The consuming deployment inherits the HTTP host and optional credential; it owns execution and decides its public tool surface.

Each operation makes one request. Errors preserve GitHub's status, message and supplied retry/reset timing. Writes are not automatically retried. Respect rate limits and retain results between checks rather than polling per caller. Authentication is host-substituted; an unset optional token leaves off the Authorization header.

[REST API](https://docs.github.com/en/rest) · [Pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api) · [Rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)

## OAuth identities

`github::oauth::user(code, redirect)` exchanges a login code and reads the authorized user's profile. It uses `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` declared by the consuming app, granted to `github.com`. The GitHub library declares both GitHub HTTP hosts. It does not use `GITHUB_TOKEN` for this flow or return the exchanged access token.

The [auth library](https://github.com/crabtalk/crab/blob/main/services/kits/auth/README.md) uses this API and supplies OAuth credential setup, account linking and sessions. The application verifies OAuth state before passing the code. OAuth exchange is a Rust API, not an additional standalone tool.
