# Release watch

Keep release and vulnerability evidence for a chosen dependency stack. Agents supply exact installed versions and explicit GitHub repository mappings, then read observations through MCP. The service links GitHub and OSV directly; neither needs a separate deployment.

## Deploy

From this directory, run `crab build`, then `crab deploy`. Keep the deployment private: its watchlist and history belong to the deployment and are shared by every authorized caller, not partitioned by account.

An optional host-managed `GITHUB_TOKEN` improves GitHub's anonymous rate limit and permits private repositories when authorized. Grant it to `api.github.com`. OSV needs no key.

Tools:

- `follow(watch)`: add or update a named package/version and repository mapping. At most 20 watches per deployment.
- `unfollow(name)`: stop checks; history remains.
- `watches()`: current mappings, last successful snapshots, attempt times and errors.
- `refresh(name)`: check immediately; provider failures appear in the returned status.
- `changes(after, limit?)`: retained observations, oldest first, with `next_cursor` and `has_more`. Start at zero; reads do not acknowledge or delete data.

```json
{
  "watch": {
    "name": "example",
    "package": {"ecosystem": "crates.io", "name": "example", "version": "1.0.0"},
    "owner": "example",
    "repo": "project"
  }
}
```

The internal `tick` cron runs every 15 minutes, checking up to five watches due for an hourly check. Failures also wait an hour before the next scheduled attempt. Manual refreshes make an immediate attempt and consume provider quota.

## Observations

The first successful check records a `baseline`, not an announcement of new releases. Later snapshots record `changed` only when evidence differs. Failed checks record `failed` while retaining the previous successful snapshot; recovery records `recovered` when the evidence is unchanged. Repeated identical failures do not add history. Changing the mapping or installed version starts a fresh baseline while preserving older observations with their original mapping.

A snapshot contains GitHub's first recent release page (up to 30 entries, including any visible drafts and prereleases), plus the complete OSV match set for the installed version. `releases_next_page` indicates older release pages were not fetched. Release history is an observation window, not a complete archive. OSV pagination failures leave prior data intact. Timestamps are Unix milliseconds; OSV/GitHub retain their own source timestamps.

The service does not compare package versions to repository tags, infer package-to-repository mappings, recommend upgrades, or claim a release fixes an advisory. Agents assess the retained source evidence. Removed OSV matches mean only that the query no longer returns them. Data and history remain until the deployment/database is removed; no automatic history cleanup is performed.

## Use in another service

```toml
[dependencies]
release-watch = { package = "crab-release-watch", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

Call `release_watch::follow`, `refresh`, `changes` or `refresh_due` from your tools. Requirements and namespaced SQLite migrations are inherited; standalone tools and schedules are not. Declare your own cron if you use `refresh_due`. All linked state belongs to your deployment. `observe(&Watch)` fetches evidence without storing it.

[GitHub releases](https://docs.github.com/en/rest/releases/releases) · [OSV queries](https://google.github.io/osv.dev/post-v1-query/)
