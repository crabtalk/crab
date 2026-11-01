# Release watch Rust integration

```toml
[dependencies]
release-watch = { package = "crab-release-watch", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

Call `release_watch::api::follow`, `refresh`, `changes` or `refresh_due` from your tools. Requirements and namespaced SQLite migrations are inherited; standalone tools and schedules are not. Declare your own cron if you use `refresh_due`. All linked state belongs to your deployment. `observe(&Watch)` fetches evidence without storing it.

[GitHub releases](https://docs.github.com/en/rest/releases/releases) · [OSV queries](https://google.github.io/osv.dev/post-v1-query/)
