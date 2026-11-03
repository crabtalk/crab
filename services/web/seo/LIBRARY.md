# SEO Rust API

```toml
[dependencies]
seo = { package = "crab-seo", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
webpage = { package = "crab-webpage", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
use seo::report::Report;
use webpage::page::Page;

let page = Page::fetch("https://example.com")?;
let report = Report::try_from(&page)?;
```

Linked services inherit the webpage library's HTTP permission. The standalone check tool is exported only with the default `tools` feature.
