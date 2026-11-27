# GitHub

Rust library for code running in a CrabTalk service. It has no standalone tools or deployable template; link it into your service, then expose the operations your users need.

GitHub repositories, issues, pull requests and releases through the REST API, and the OAuth code exchange that `crabkit-auth` signs users in with.

## Using it

Add the library to your service's crate:

```toml
[dependencies]
crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.4", features = ["github"] }
```

The library is also the crate `crabkit-github` on its own.

Call it from your own tools:

```rust,ignore
use berm_lang::{Result, tool};
use crabkit::github::{api, types::Release};

/// The latest release of a repository
#[tool]
pub fn latest(owner: String, repo: String) -> Result<Release> {
    api::latest_release(&owner, &repo)
}
```

The service declares nothing the library already declares: the library's `crab.toml` declares `github.com` and the optional `github_api` credential with `GITHUB_TOKEN` for `api.github.com`, and they reach the service's manifest when the service links the library.

- `api::repository`, `issues`, `issue`, `pull_requests`, `pull_request`, `releases` and `latest_release` read; `create_issue`, `comment`, `create_pull_request` and `create_release` write.
- Lists return `items` and `next_page`; pass the next page with the same filters and size. The default page size is 30, capped at 100. GitHub's issue lists can contain pull requests, identified by `pull_request`.
- `oauth::user(code, redirect)` exchanges an OAuth code and reads the signed-in GitHub user.
- Each operation makes one request. Errors keep GitHub's status, message and supplied retry and reset timing. Writes are not retried.

## Setting it up

Public repository reads need no credential. For private repositories, writes or GitHub's higher rate limit, set `GITHUB_TOKEN` to a fine-grained token scoped to the repositories you need: Metadata for repository reads, Issues for issues, Pull requests for pull requests and Contents for releases.

[REST API](https://docs.github.com/en/rest) · [Pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api) · [Rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
