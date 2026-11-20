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

Linked services inherit the webpage library's HTTP permission. The standalone tools and site audit workflow are exported with the default `tools` feature.
