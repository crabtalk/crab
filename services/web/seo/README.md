# SEO

Inspect a public page for technical SEO, check whether AI crawlers can read a site, or audit a site and compare findings over time. These operations need no provider key.

## Call

With Crab signed in to Cloud, call CrabTalk's deployment, `crabtalk/seo`:

```sh
crab call crabtalk/seo check '{"url":"https://example.com"}'
```

Returns the final URL, title, descriptions, canonical declarations and targets, HTML language, h1 headings, image counts and missing-alt samples, scoped robots directives, JSON-LD blocks, social metadata, hreflang declarations and findings. You can also call it through [CrabTalk MCP](https://crabtalk.ai/docs/guides/mcp).

- Metadata checks cover missing or multiple HTML titles (excluding SVG titles), missing or multiple descriptions, missing language, missing or multiple h1 headings and images without alt attributes. Empty alt attributes remain valid for decorative images.
- Canonical checks read HTML links and HTTP `Link` headers, report invalid targets, declarations outside `<head>`, multiple declarations and conflicting targets. The `canonical` field uses the first declaration applicable to the final page URL; all declarations and raw Link headers remain available as evidence. Header targets resolve against the response URL; HTML targets use the document base URL.
- JSON-LD checks parse each `application/ld+json` script, list discovered `@type` values, including arrays and nested graphs, and report invalid JSON or root, `@type` and `@graph` shapes. These checks do not establish full JSON-LD validity or rich-result eligibility.
- Social checks report Open Graph's required title, type, image and URL, Twitter Card metadata, invalid page/image/player URLs and conflicting scalar properties. Twitter title, description and image checks allow Open Graph fallbacks; multiple Open Graph images are allowed. Social findings describe sharing metadata, not ranking factors.
- Robots evidence retains the HTML or header source and crawler scope. Recognized meta names are `robots`, `googlebot`, `googlebot-news` and `bingbot`; X-Robots-Tag crawler names are retained. Noindex findings name their scope; they do not assert that every crawler is blocked.
- Hreflang checks inspect HTML head alternates for supported language, script and region codes, absolute HTTP/HTTPS targets and conflicting URLs for one language. `x-default` is accepted. Identical repeated targets are not conflicts.

`ai_access` reads whether AI crawlers can reach and read a site:

```sh
crab call crabtalk/seo ai_access '{"site":"https://example.com/"}'
```

- Each of GPTBot, OAI-SearchBot, ChatGPT-User, ClaudeBot, Claude-SearchBot, Claude-User, PerplexityBot, Perplexity-User, Google-Extended, Applebot-Extended and CCBot is `allowed`, `blocked` or `unspecified` for the root page's path, with the `robots.txt` group that applied: the crawler's own, else `*`. Rules are read as RFC 9309 reads them: the longest matching pattern wins, `allow` wins a tie, and `*` and a final `$` match. `unspecified` means no group names the crawler and there is no `*` group, or `robots.txt` did not answer 2xx.
- `/llms.txt`: its status, the H1 it opens with, its Markdown link count, and which of its first ten links did not answer 2xx.
- Characters of visible text and script elements in the root page's HTML response; a page with under 200 characters that runs scripts is reported as needing JavaScript for its content.
- Whether the root page answers at its URL with `.md` appended (`index.html.md` for a path ending in `/`), and whether it answers `Accept: text/markdown` with Markdown.
- The root page's `check` report.

Informational findings identify things to review; an absent canonical or multiple h1 headings are not automatically errors. Title and description lengths are not pass/fail rules.

## Site audit

```sh
crab workflow run crabtalk/seo.audit '{"site":"https://example.com/docs/","pages":["https://example.com/docs/install"],"max":25}' --yes
```

Pass the site's root for the whole site or a path for one section. The audit reads robots.txt and same-origin sitemaps, then checks up to 1,000 listed URLs in URL order, resuming after the last URL it reached and wrapping at the end. It reuses ETag and Last-Modified validators and compares the SEO fields in HTML responses. It reports redirects, failed responses and sitemap entries blocked by robots rules. Discovery reads up to eight sitemap files of 4 MiB each. When discovery is incomplete, the report indicates it and preserves previously known sitemap rows.

Use the optional `pages` array to prioritize sitemap URLs you know changed. The audit fills the remaining slots with never-scanned pages, then changed pages with the most incoming internal links, then the oldest remaining scans. The optional `max` defaults to 1,000 and is capped at 1,000. A page beyond the limit stays pending for a later run.

The report opens with new and fixed findings. It also shows the full findings within the report limit and the counts of listed, checked, audited and pending pages. Cross-page checks cover duplicate titles and descriptions, canonical targets, broken internal links and reachable HTML pages missing from a complete sitemap. A cross-page fix requires current checks of the pages involved. Findings that rely on earlier scans are marked stale. The first completed audit establishes the baseline.

The audit keeps scans and the latest report for each site or section in your caller storage. Download or delete the data under Storage in your CrabTalk settings. Each caller can run one audit for a given site at a time.

A successful run charges $0.0001 per batch of up to 10 pending HTML pages, including a final partial batch. Checking for changes is free. The workflow quotes up to $0.01. Actual charges depend on how many pages reach the audit step within your `max`.

## Limits

The page checks inspect the returned UTF-8 HTML and response headers. `ai_access` reads `robots.txt` and `llms.txt` at the root page's origin without following their redirects, up to 512 KiB each, and checks the root path. The checks use static HTTP responses. Use `audit` to check same-origin canonical and internal link targets. Hreflang reciprocity, HTTP-header and sitemap hreflang annotations, and remote JSON-LD contexts are outside the page checks.

Accepts at most 12,000 DOM nodes, 12,000 start tags, 12,000 total attributes, depth 128 and 128 attributes per element. Fetches at most 512 KiB of HTML with up to three redirects and a ten-second timeout per request. Unsupported content types and unsuccessful HTTP responses fail explicitly. Reports include at most twenty h1 texts and twenty missing-alt image samples beside their full counts; metadata reports over 256 KiB fail explicitly. Image sample positions are one-based among all img elements; JSON-LD block positions are one-based among matching scripts. Other evidence lists are complete within the report limit.

This service is configured for public use without deployable private copies. Call an accessible running deployment, or use the source deployment instructions to build and deploy your own service.

[Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/web/seo/LIBRARY.md) · [Google metadata documentation](https://developers.google.com/search/docs/crawling-indexing/special-tags)
