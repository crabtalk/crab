# OSV

Query vulnerabilities for an installed package/version, query batches and retrieve full advisories. Uses the public OSV API; no credential is required.

## Deploy

From this directory, run `crab build`, then `crab deploy`. Agents call `query`, `query_batch` and `advisory` through MCP.

Supply the ecosystem's exact name, package name and installed version. This initial API supports package/version queries; it does not infer repositories or resolve lockfiles, commits or package URLs.

`query` returns full OSV records and an optional `next_page_token`. Pass it as `page_token` on the next call, including when a page contains no records. `query_batch` returns ordered ID/modification-time references with an independent token per input. Fetch details with `advisory`. The API preserves affected ranges, references and provider metadata as JSON.

No matches means OSV returned no known matching records; it is not proof that a package is secure. Calls are not automatically retried.

## Use in another service

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
