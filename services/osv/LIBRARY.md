# OSV Rust integration

```toml
[dependencies]
osv = { package = "crab-osv", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
use osv::{api, types::PackageVersion};

let package = PackageVersion {
    ecosystem: "crates.io".into(),
    name: "example".into(),
    version: "1.0.0".into(),
};
let page = api::query(&package, None)?;
let complete = api::query_all(&package)?;
```

`query_all` deduplicates IDs and reads up to ten pages. Repeated tokens, provider errors or the page cap fail the call rather than returning an incomplete match set. Use explicit page reads for larger results. Linked services inherit the HTTP permission without exporting the standalone tools.

[Query API](https://google.github.io/osv.dev/post-v1-query/) · [Batch API](https://google.github.io/osv.dev/post-v1-querybatch/) · [Advisories](https://google.github.io/osv.dev/get-v1-vulns/)
