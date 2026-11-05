# Web

Services for reading, inspecting and measuring websites. The intended publisher org is `web`.

- [SEO](seo/README.md): basic page metadata and indexing findings, addressed as `web/seo` when published there.
- [Markdown](markdown/README.md): webpage content as Markdown, addressed as `web/markdown` when published there.
- [Analytics](analytics/README.md): website views, visitors and crawler reports; deploy your own private copy of `web/analytics` when its template is published.

Deploy SEO and Markdown as public services with `crab --org web deploy services/web -p seo -p markdown`. Publish the Analytics template with `crab --org web publish services/web/analytics`. Both commands use the selected authenticated Cloud context and ask for approval.
