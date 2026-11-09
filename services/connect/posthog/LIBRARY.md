# PostHog Rust integration

```toml
[dependencies]
posthog = { package = "crab-posthog", git = "https://github.com/crabtalk/crab", tag = "v0.1.4", default-features = false }
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
