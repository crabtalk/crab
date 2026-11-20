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
crab workflow run crabtalk/seo.audit '{"site":"https://example.com/docs/"}' --yes
```

`audit` reads `robots.txt` and the same-origin sitemap or sitemap index. Pass the site's root URL for the whole site or a path to narrow it. The workflow samples up to 100 listed URLs and reports sitemap entries blocked by robots rules before fetching them.

The report includes page metadata findings, duplicate titles and descriptions, same-origin canonicals returning a redirect or error, broken internal links, and reachable HTML pages missing from a complete sitemap. It checks up to eight distinct internal links per page and probes up to 100 additional targets across the audit. Discovery reads up to eight sitemap files of 256 KiB each and up to 1,000 distinct URLs. The report marks incomplete discovery, sampling and truncation.

A manual run compares findings with the previous audit for the starting account and site. Verified app-user histories are scoped to the user. It reports new findings first and reports fixed findings when the sample can be compared. Audits with incomplete evidence preserve the previous baseline.

The workflow charges 100 USD micros ($0.0001) per page step that reports an audit result, on a successful run. The quote is bounded by 100 pages. A sitemap entry skipped by robots rules runs no page step.

## Limits

The page checks inspect the returned UTF-8 HTML and response headers. `ai_access` reads `robots.txt` and `llms.txt` at the root page's origin without following their redirects, up to 512 KiB each, and checks the root path. The checks use static HTTP responses. Use `audit` to check same-origin canonical and internal link targets. Hreflang reciprocity, HTTP-header and sitemap hreflang annotations, and remote JSON-LD contexts are outside the page checks.

Accepts at most 12,000 DOM nodes, 12,000 start tags, 12,000 total attributes, depth 128 and 128 attributes per element. Fetches at most 512 KiB of HTML with up to three redirects and a ten-second timeout per request. Unsupported content types and unsuccessful HTTP responses fail explicitly. Reports include at most twenty h1 texts and twenty missing-alt image samples beside their full counts; metadata reports over 256 KiB fail explicitly. Image sample positions are one-based among all img elements; JSON-LD block positions are one-based among matching scripts. Other evidence lists are complete within the report limit.

This service is configured for public use without deployable private copies. Call an accessible running deployment, or use the source deployment instructions to build and deploy your own service.

[Source deployment](https://github.com/crabtalk/crab/blob/main/services/README.md#building-from-source) · [Rust API](https://github.com/crabtalk/crab/blob/main/services/web/seo/LIBRARY.md) · [Google metadata documentation](https://developers.google.com/search/docs/crawling-indexing/special-tags)
