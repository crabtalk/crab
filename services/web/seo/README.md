# SEO check

Inspect one public page for basic technical SEO findings. Returns the observed metadata and specific findings without a ranking score. Calls need no upstream API key and retain no history.

## Call

With Crab signed in to Cloud, replace `owner/seo` with an accessible running deployment:

```sh
crab call owner/seo check '{"url":"https://example.com"}'
```

Returns the final URL, title, description, canonical URL, HTML language, h1 headings, image counts, robots directives and findings. You can also call it through [CrabTalk MCP](https://crab.talk/docs/guides/mcp).

Checks missing or multiple titles, missing descriptions, missing/invalid/multiple canonical links, missing language, missing or multiple h1 headings, images without alt attributes, and noindex directives. Informational findings identify things to review; an absent canonical or multiple h1 headings are not automatically errors. Empty alt attributes remain valid for decorative images.

## Limits

Inspects the returned UTF-8 HTML and X-Robots-Tag headers. It does not render JavaScript, crawl the site, fetch robots.txt, check rankings or verify that Google indexed the page. Robots directives are reported as observed; crawler-specific policies still need interpretation.

Fetches at most 512 KiB of HTML with up to three redirects and a ten-second timeout per request. Unsupported content types and unsuccessful HTTP responses fail explicitly. Reports include at most twenty h1 texts beside the total count; metadata reports over 256 KiB fail explicitly.

[Source deployment](https://github.com/crabtalk/crab/blob/dev/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/dev/services/web/seo/LIBRARY.md) · [Google metadata documentation](https://developers.google.com/search/docs/crawling-indexing/special-tags)
