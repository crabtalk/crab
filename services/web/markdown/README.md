# Webpage → Markdown

Read a public HTML page as Markdown, preserving headings, links, lists, tables and code blocks. Calls need no upstream API key and retain no page history.

## Call

With Crab signed in to Cloud, replace `owner/markdown` with an accessible running deployment:

```sh
crab call owner/markdown read '{"url":"https://example.com"}'
```

Returns the final URL, HTML page title, description, selected HTML region and Markdown. SVG titles do not supply the page title. You can also call it through [CrabTalk MCP](https://crab.talk/docs/guides/mcp).

The reader prefers the first main region, then the first article, then the body. It removes navigation, scripts, styles, footers, sidebars and forms. Relative links and image sources resolve against the page's base URL. HTTP(S), mail and telephone links are retained.

## Limits

Fetches server-returned UTF-8 HTML, without JavaScript execution, browser login or PDF parsing. Each request has a ten-second timeout; at most three redirects are followed. HTML over 512 KiB, extracted Markdown over 128 KiB, unsupported content types and unsuccessful HTTP responses fail explicitly. HTML complexity is bounded to 12,000 DOM nodes, 12,000 start tags, 12,000 total attributes, depth 128 and 128 attributes per element. A page requiring JavaScript may return incomplete content or no readable content.

[Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/web/markdown/LIBRARY.md)
