# Webpage

Rust library for code running in a CrabTalk service. It has no standalone tools or deployable template; link it into your service, then expose the operations your users need.

Fetches a public HTML page within bounds, reads its metadata, and converts it to Markdown, preserving headings, links, lists, tables and code blocks.

## Using it

```toml
[dependencies]
crabkit = { git = "https://github.com/crabtalk/crab", tag = "v0.1.4", features = ["webpage"] }
```

The library is also the crate `crabkit-webpage` on its own. Its `crab.toml` declares `[http] fetch = ["*"]`, which reaches the service's manifest when the service links the library.

```rust,ignore
use crabkit::webpage::{page::Page, reading::Reading};

let page = Page::fetch("https://example.com")?;
let markdown = Reading::try_from(&page)?;
```

`Page` retains the final URL, resolved base URL, HTTP response and parsed HTML document with original link attributes. `Page::metas` returns matching meta elements, including empty declarations. `Reading` resolves or removes link and image URLs during Markdown conversion, leaving the page document unchanged.

## Limits

Fetches server-returned UTF-8 HTML, without JavaScript execution, browser login or PDF parsing. Each request has a ten-second timeout; at most three redirects are followed. HTML over 512 KiB, extracted Markdown over 128 KiB, unsupported content types and unsuccessful HTTP responses fail explicitly. `Page::fetch` bounds HTML complexity to 12,000 DOM nodes, 12,000 start tags, 12,000 total attributes, depth 128 and 128 attributes per element.

The reader prefers the first main region, then the first article, then the body. It removes navigation, scripts, styles, footers, sidebars and forms. Relative links and image sources resolve against the page's base URL. HTTP(S), mail and telephone links are retained.

## License

Apache-2.0
