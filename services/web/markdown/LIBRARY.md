# Webpage Rust API

```toml
[dependencies]
webpage = { package = "crab-webpage", git = "https://github.com/crabtalk/crab", branch = "dev", default-features = false }
```

```rust,ignore
use webpage::{page::Page, reading::Reading};

let page = Page::fetch("https://example.com")?;
let markdown = Reading::try_from(&page)?;
```

`Page` retains the final URL, resolved base URL, HTTP response and parsed HTML document. Linking the library inherits its HTTP host permissions without exporting the standalone read tool.
