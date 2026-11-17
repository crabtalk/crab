# SEO check

Inspect one public page for technical SEO and social preview findings. Returns observed metadata and specific findings without a ranking score. Calls need no upstream API key and retain no history.

## Call

With Crab signed in to Cloud, replace `owner/seo` with an accessible running deployment:

```sh
crab call owner/seo check '{"url":"https://example.com"}'
```

Returns the final URL, title, descriptions, canonical declarations and targets, HTML language, h1 headings, image counts and missing-alt samples, scoped robots directives, JSON-LD blocks, social metadata, hreflang declarations and findings. You can also call it through [CrabTalk MCP](https://crabtalk.ai/docs/guides/mcp).

- Metadata checks cover missing or multiple HTML titles (excluding SVG titles), missing or multiple descriptions, missing language, missing or multiple h1 headings and images without alt attributes. Empty alt attributes remain valid for decorative images.
- Canonical checks read HTML links and HTTP `Link` headers, report invalid targets, declarations outside `<head>`, multiple declarations and conflicting targets. The `canonical` field uses the first declaration applicable to the final page URL; all declarations and raw Link headers remain available as evidence. Header targets resolve against the response URL; HTML targets use the document base URL.
- JSON-LD checks parse each `application/ld+json` script, list discovered `@type` values, including arrays and nested graphs, and report invalid JSON or root, `@type` and `@graph` shapes. These checks do not establish full JSON-LD validity or rich-result eligibility.
- Social checks report Open Graph's required title, type, image and URL, Twitter Card metadata, invalid page/image/player URLs and conflicting scalar properties. Twitter title, description and image checks allow Open Graph fallbacks; multiple Open Graph images are allowed. Social findings describe sharing metadata, not ranking factors.
- Robots evidence retains the HTML or header source and crawler scope. Recognized meta names are `robots`, `googlebot`, `googlebot-news` and `bingbot`; X-Robots-Tag crawler names are retained. Noindex findings name their scope; they do not assert that every crawler is blocked.
- Hreflang checks inspect HTML head alternates for supported language, script and region codes, absolute HTTP/HTTPS targets and conflicting URLs for one language. `x-default` is accepted. Identical repeated targets are not conflicts.

Informational findings identify things to review; an absent canonical or multiple h1 headings are not automatically errors. Title and description lengths are not pass/fail rules.

## Limits

Inspects the returned UTF-8 HTML and response headers. It does not render JavaScript, crawl the site, fetch robots.txt or linked resources, check rankings or verify indexing. Canonical and hreflang targets are not fetched, and hreflang reciprocity is not checked. HTTP-header and sitemap hreflang annotations are not inspected. JSON-LD contexts are not fetched.

Accepts at most 12,000 DOM nodes, 12,000 start tags, 12,000 total attributes, depth 128 and 128 attributes per element. Fetches at most 512 KiB of HTML with up to three redirects and a ten-second timeout per request. Unsupported content types and unsuccessful HTTP responses fail explicitly. Reports include at most twenty h1 texts and twenty missing-alt image samples beside their full counts; metadata reports over 256 KiB fail explicitly. Image sample positions are one-based among all img elements; JSON-LD block positions are one-based among matching scripts. Other evidence lists are complete within the report limit.

This service is configured for public use without deployable private copies. Call an accessible running deployment, or use the source deployment instructions to build and deploy your own service.

[Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/web/seo/LIBRARY.md) · [Google metadata documentation](https://developers.google.com/search/docs/crawling-indexing/special-tags)
