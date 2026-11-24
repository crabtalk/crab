# SEO Rust API

```toml
[dependencies]
seo = { package = "crab-seo", git = "https://github.com/crabtalk/crab", tag = "v0.1.4", default-features = false }
webpage = { package = "crabkit-webpage", git = "https://github.com/crabtalk/crab", tag = "v0.1.4" }
```

```rust,ignore
use seo::report::Report;
use webpage::page::Page;

let page = Page::fetch("https://example.com")?;
let report = Report::try_from(&page)?;
```

`Report` retains the original summary fields and adds canonical/header evidence, descriptions, JSON-LD blocks, social properties, scoped robots directives, missing-alt image samples and HTML hreflang alternates. Evidence types are in the `canonical`, `structured`, `social`, `robots`, `hreflang` and `report` modules. Targets and JSON-LD contexts are not fetched.

`seo::access::Access::inspect` checks AI crawler access for a root URL. `Access::try_from(&page)` reuses an existing root `Page` and fetches the remaining access evidence. `Access::agent_links` contains the discovered `AgentLink` relations, and each `crawlers::Crawler` includes its `ContentSignals` as optional booleans, with `None` for an undeclared signal.

Linked services inherit the webpage library's HTTP permission. The standalone tools and site audit workflow are exported with the default `tools` feature.
