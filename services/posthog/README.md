# PostHog

Capture events, evaluate feature flags and query analytics in PostHog Cloud. Tools: `capture`, `capture_batch`, `flags`, `query`. US and EU Cloud are supported; self-hosted instances are not included in this version.

## Deploy

From this directory, run `crab build`, then `crab deploy`. Set `POSTHOG_REGION` to `us` (default) or `eu`.

- Capture and flags: set host-managed `POSTHOG_PROJECT_TOKEN`, granted to `us.i.posthog.com` or `eu.i.posthog.com` for your region.
- Analytics queries: set `POSTHOG_PROJECT_ID` to the numeric project ID and host-managed `POSTHOG_PERSONAL_API_KEY` with `query:read` and access to that project, granted to `us.posthog.com` or `eu.posthog.com`.

Credential groups are optional so capture-only and query-only deployments can omit the unused group. Set the credentials for the tools you call. The host substitutes them in outgoing requests; guest code never holds their values. This uses your own account's API key, not an OAuth installation flow.

Agents call the tools through MCP. Keep the deployment private when its analytics and ingestion permissions should only be used by your agents.

Events accept nested JSON properties, an optional ISO 8601 timestamp and UUID. Success means accepted, not immediately queryable. Calls make one attempt; use stable event UUIDs when resubmitting. `distinct_id` must have 1–200 characters.

Flag evaluation returns variants, metadata and `errors_while_computing_flags`. Missing flags are unknown, and partial results must not be treated as a complete evaluation. Evaluation does not capture `$feature_flag_called`; record an exposure event only when the flag is actually used.

Queries run blocking HogQL and preserve the provider's JSON response, including columns, rows and pagination metadata. Include a descriptive name and an appropriate SQL `LIMIT`; no automatic pagination, background polling or retries are performed.

## Use in another service

```toml
[dependencies]
posthog = { package = "crab-posthog", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
use posthog::{api::PostHog, types::{Event, Region}};

let posthog = PostHog::new(Region::Eu);
posthog.capture(&Event {
    event: "report_ready".into(),
    distinct_id: "user-42".into(),
    ..Event::default()
})?;
let rows = posthog.query(123, "select event, count() from events group by event limit 10", "Event totals")?;
```

Your deployment inherits credentials and HTTP permissions. Pass the region and project ID explicitly; standalone variables and tools are not inherited.

[Capture](https://posthog.com/docs/api/capture) · [Flags](https://posthog.com/docs/api/flags) · [Queries](https://posthog.com/docs/api/query)
