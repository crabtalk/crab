# GitHub

Repositories, issues, comments, pull requests and releases through GitHub's REST API. An agent calls the deployed service through MCP; another service can link the same Rust API.

## Deploy

From `services/github`, run `crab build`, then `crab deploy`. Public reads work without a token. For private repositories or writes, set `GITHUB_TOKEN` through the approval page, granted only to `api.github.com`.

Use a fine-grained token restricted to the needed repositories: Metadata for repository reads, Issues for issues, Pull requests for PRs, and Contents for releases. Grant write permission only for operations you use. Keep the deployment private when the token is for your own agents.

| Tools | Operations |
| --- | --- |
| `repository` | Repository metadata |
| `issues`, `issue`, `create_issue`, `comment` | List/read/create issues; comment on issues or PRs |
| `pull_requests`, `pull_request`, `create_pull_request` | List/read/open PRs between existing branches |
| `releases`, `latest_release`, `create_release` | List/read/create releases |

Lists return `items` and `next_page`; pass that page with the same filters and size. Default size is 30, capped at 100. GitHub's issues endpoint includes PRs, identified by `pull_request`. Releases exclude tags without a release; `latest_release` excludes drafts and prereleases.

## Use in another service

```toml
[dependencies]
github = { package = "crab-github", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
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

The [auth library](../../kits/auth/README.md) uses this API and supplies OAuth credential setup, account linking and sessions. The application verifies OAuth state before passing the code. OAuth exchange is a Rust API, not an additional standalone tool.
